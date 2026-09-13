//! Explicit background installation. No document or mutable host crosses threads.
use crate::{PluginHost, wasm::WasmPlugin};
use os_core::{Error, Result, ensure};
use os_plugin_api::{Permission, Plugin};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

struct Permit(Arc<AtomicBool>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
struct Prepared {
    plugin: WasmPlugin,
    catalog: Option<os_plugin_api::generic::Catalog>,
}
pub struct PendingLoad {
    host: Arc<AtomicBool>,
    receiver: Option<Receiver<Result<Prepared>>>,
    permit: Option<Arc<Permit>>,
    deadline: Instant,
    grants: BTreeSet<Permission>,
}
impl PendingLoad {
    /// Revokes delivery; preparation keeps its capacity permit until it exits.
    pub fn cancel(&mut self) {
        self.receiver = None;
        self.permit = None;
    }
}
impl PluginHost {
    pub fn load_pending(&self) -> bool {
        self.load_busy.load(Ordering::Acquire)
    }
    /// One preparation (including unread result) per host. The deadline revokes
    /// adoption, not OS file IO or interpreter compilation already in progress.
    pub fn start_wasm_load(
        &self,
        directory: PathBuf,
        grants: BTreeSet<Permission>,
        timeout: Duration,
    ) -> Result<PendingLoad> {
        ensure(
            !timeout.is_zero() && timeout <= Duration::from_secs(30),
            "load deadline must be positive and at most 30 seconds",
        )?;
        ensure(
            self.load_busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "plugin preparation already pending",
        )?;
        let permit = Arc::new(Permit(self.load_busy.clone()));
        let running = permit.clone();
        let (send, receiver) = mpsc::sync_channel(1);
        let reviewed = grants.clone();
        let deadline = Instant::now() + timeout;
        std::thread::Builder::new()
            .name("plugin-load".into())
            .spawn(move || {
                let _running = running;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let plugin = crate::wasm::read_directory(&directory, &reviewed)?;
                    ensure(Instant::now() < deadline, "plugin load deadline expired")?;
                    let manifest = plugin.manifest();
                    let catalog = if manifest.api_version == os_plugin_api::generic::VERSION {
                        Some(crate::generic::describe(&plugin, &manifest)?)
                    } else {
                        None
                    };
                    Ok(Prepared { plugin, catalog })
                }))
                .unwrap_or_else(|_| Err(Error::Invalid("plugin preparation panicked".into())));
                let _ = send.send(result);
            })
            .map_err(|e| Error::Invalid(e.to_string()))?;
        Ok(PendingLoad {
            host: self.load_busy.clone(),
            receiver: Some(receiver),
            permit: Some(permit),
            deadline,
            grants,
        })
    }
    /// Poll and adopt exactly once, revalidating current dependencies/collisions.
    /// This never invokes or compiles guest code on the caller thread.
    pub fn poll_wasm_load(&mut self, load: &mut PendingLoad) -> Result<Option<String>> {
        ensure(
            Arc::ptr_eq(&self.load_busy, &load.host),
            "load belongs to another host",
        )?;
        if Instant::now() >= load.deadline {
            load.cancel();
            return Err(Error::Invalid("plugin load deadline expired".into()));
        }
        let receiver = load
            .receiver
            .as_ref()
            .ok_or_else(|| Error::Invalid("plugin load cancelled or already settled".into()))?;
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err(Error::Invalid("plugin preparation disconnected".into()))
            }
        };
        load.cancel();
        let prepared = result?;
        let manifest = prepared.plugin.manifest();
        self.validate_load(&manifest, &load.grants)?;
        ensure(
            Instant::now() < load.deadline,
            "plugin load deadline expired",
        )?;
        let id = manifest.id.clone();
        let runtime = Arc::new(crate::worker::Runtime::new(prepared.plugin.clone()));
        self.install_validated(Box::new(prepared.plugin), manifest, prepared.catalog);
        self.loaded.get_mut(&id).expect("installed plugin").worker = Some(runtime);
        Ok(Some(id))
    }
}
