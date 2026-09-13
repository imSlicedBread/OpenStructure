//! Developer-only buffer ABI spike. No imports, WASI or desktop activation.
use crate::PluginHost;
use os_core::{Error, Result, ensure};
use os_plugin_api::{Manifest, Permission, Plugin};
use std::{collections::BTreeSet, fs::File, io::Read, path::Path};
use wasmi::{
    Config, EnforcedLimits, Engine, ExternType, Linker, Module, Store, StoreLimitsBuilder,
};

pub const ABI_VERSION: i32 = 1;
pub const MAX_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_WASM_MESSAGE_BYTES: usize = 1024 * 1024;
pub const MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;
pub const CALL_FUEL: u64 = 1_000_000;

/// Validated module bytes; guest state is deliberately not retained between calls.
#[derive(Clone)]
pub struct WasmPlugin {
    manifest: Manifest,
    engine: Engine,
    module: Module,
}

fn fault(error: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("Wasm plugin: {error}"))
}

fn artifact_name(manifest: &Manifest) -> Result<&str> {
    let name = manifest.entrypoint.strip_prefix("wasm:").unwrap_or("");
    ensure(
        name.ends_with(".wasm")
            && name.len() > 5
            && name.len() <= 128
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            && !name.starts_with('.')
            && !name.contains(".."),
        "Wasm entrypoint must be wasm:<adjacent-file>.wasm",
    )?;
    Ok(name)
}

impl WasmPlugin {
    /// Compile and validate without running guest code. The buffer ABI version
    /// value is checked under fuel on each invocation, before request bytes copy.
    pub fn new(manifest: Manifest, bytes: &[u8]) -> Result<Self> {
        manifest.validate()?;
        artifact_name(&manifest)?;
        ensure(
            bytes.len() <= MAX_ARTIFACT_BYTES,
            "Wasm artifact exceeds 4 MiB",
        )?;
        ensure(
            bytes.starts_with(b"\0asm\x01\0\0\0"),
            "expected a Wasm binary module",
        )?;
        let mut config = Config::default();
        config
            .consume_fuel(true)
            .enforced_limits(EnforcedLimits::strict())
            .ignore_custom_sections(true)
            .wasm_multi_memory(false)
            .wasm_memory64(false)
            .wasm_custom_page_sizes(false)
            .set_max_recursion_depth(128)
            .set_max_stack_height(64 * 1024)
            .set_max_cached_stacks(0);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes).map_err(fault)?;
        ensure(
            module.imports().next().is_none(),
            "Wasm imports are forbidden (including WASI)",
        )?;
        ensure(
            matches!(module.get_export("memory"), Some(ExternType::Memory(_))),
            "Wasm memory export missing",
        )?;
        use wasmi::ValType::{I32, I64};
        for (name, params, results) in [
            ("os_abi_version", &[][..], &[I32][..]),
            ("os_alloc", &[I32][..], &[I32][..]),
            ("os_invoke", &[I32, I32][..], &[I64][..]),
        ] {
            ensure(
                matches!(module.get_export(name), Some(ExternType::Func(f)) if f.params() == params && f.results() == results),
                format!("Wasm export {name} has a missing or incompatible signature"),
            )?;
        }
        Ok(Self {
            manifest,
            engine,
            module,
        })
    }
}

impl Plugin for WasmPlugin {
    fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }

    fn invoke_json(&self, request: &str) -> Result<String> {
        ensure(
            request.len() <= MAX_WASM_MESSAGE_BYTES,
            "Wasm request exceeds 1 MiB",
        )?;
        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_MEMORY_BYTES)
            .memories(1)
            .instances(1)
            .tables(1)
            .table_elements(4096)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|limits| limits);
        store.set_fuel(CALL_FUEL).map_err(fault)?;
        let linker = Linker::new(&self.engine);
        // Start functions run only after both the limiter and fuel are installed.
        let instance = linker
            .instantiate_and_start(&mut store, &self.module)
            .map_err(fault)?;
        let version = instance
            .get_typed_func::<(), i32>(&store, "os_abi_version")
            .map_err(fault)?;
        ensure(
            version.call(&mut store, ()).map_err(fault)? == ABI_VERSION,
            "Wasm buffer ABI version mismatch",
        )?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| fault("memory export missing"))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&store, "os_alloc")
            .map_err(fault)?;
        let ptr = alloc
            .call(&mut store, request.len() as i32)
            .map_err(fault)? as u32;
        memory
            .write(&mut store, ptr as usize, request.as_bytes())
            .map_err(fault)?;
        let invoke = instance
            .get_typed_func::<(i32, i32), i64>(&store, "os_invoke")
            .map_err(fault)?;
        let packed = invoke
            .call(&mut store, (ptr as i32, request.len() as i32))
            .map_err(fault)? as u64;
        let output_ptr = packed as u32 as usize;
        let length = (packed >> 32) as usize;
        ensure(
            length > 0 && length <= MAX_WASM_MESSAGE_BYTES,
            "invalid Wasm response length (limit 1 MiB)",
        )?;
        // Check the full range before allocating/copying; no guest pointer is dereferenced.
        let end = output_ptr
            .checked_add(length)
            .ok_or_else(|| fault("response range overflow"))?;
        let bytes = memory
            .data(&store)
            .get(output_ptr..end)
            .ok_or_else(|| fault("response outside guest memory"))?;
        let text = std::str::from_utf8(bytes).map_err(fault)?;
        Ok(text.to_owned())
    }
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(fault)?;
    ensure(
        file.metadata().map_err(fault)?.is_file(),
        "plugin input is not a regular file",
    )?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(fault)?;
    ensure(bytes.len() <= limit, "plugin input exceeds size limit")?;
    Ok(bytes)
}

pub(super) fn read_directory(
    directory: &Path,
    grants: &BTreeSet<Permission>,
) -> Result<WasmPlugin> {
    let root = directory.canonicalize().map_err(fault)?;
    ensure(root.is_dir(), "plugin directory missing")?;
    let manifest_path = root.join("plugin.toml").canonicalize().map_err(fault)?;
    ensure(
        manifest_path.starts_with(&root),
        "plugin manifest escapes directory",
    )?;
    let bytes = read_bounded(&manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest = Manifest::from_toml(std::str::from_utf8(&bytes).map_err(fault)?)?;
    if !manifest.permissions.is_subset(grants) {
        return Err(Error::Permission(
            "plugin requested permissions not granted by host".into(),
        ));
    }
    let path = root
        .join(artifact_name(&manifest)?)
        .canonicalize()
        .map_err(fault)?;
    ensure(
        path.starts_with(&root),
        "Wasm artifact escapes plugin directory",
    )?;
    let bytes = read_bounded(&path, MAX_ARTIFACT_BYTES)?;
    WasmPlugin::new(manifest, &bytes)
}

impl PluginHost {
    /// Explicit, synchronous developer installation. Never call from a UI frame
    /// or automatically from document dependencies. Installation directories must
    /// not be concurrently writable by an adversary (not a race-proof FS sandbox).
    pub fn load_wasm_directory(
        &mut self,
        directory: &Path,
        grants: BTreeSet<Permission>,
    ) -> Result<()> {
        let plugin = read_directory(directory, &grants)?;
        let id = plugin.manifest.id.clone();
        let runtime = std::sync::Arc::new(super::worker::Runtime::new(plugin.clone()));
        self.load_checked(Box::new(plugin), grants)?;
        self.loaded.get_mut(&id).expect("just registered").worker = Some(runtime);
        Ok(())
    }
}
