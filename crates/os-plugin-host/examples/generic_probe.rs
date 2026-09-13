//! Installed API-2 Rust column lifecycle; independent of guest implementation.
use os_core::ensure;
use os_document::{Command, Document};
use os_plugin_api::{Permission, generic::Mode};
use os_plugin_host::{
    PluginHost,
    generic::{Invocation, Scope},
    worker::{JobFailure, JobOutcome},
};
use os_storage::{StorageBackend, ZipJsonStorage};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

const OWNER: &str = "org.example.columns";
fn invocation(doc: &Document, mode: Mode, width: f64) -> Invocation {
    let target = doc.model().extensions.keys().next().copied();
    Invocation {
        command_id: format!(
            "{OWNER}.{}",
            match mode {
                Mode::Create => "create",
                Mode::Edit => "edit",
                Mode::Delete => "delete",
                Mode::Migrate => "migrate",
            }
        ),
        inputs: if mode == Mode::Delete {
            BTreeMap::new()
        } else {
            BTreeMap::from([
                ("width".into(), json!(width)),
                ("depth".into(), json!(0.6)),
                ("height".into(), json!(3.0)),
            ])
        },
        scope: Scope {
            read: doc.model().levels.keys().copied().chain(target).collect(),
            write: if mode == Mode::Create {
                Default::default()
            } else {
                target.into_iter().collect()
            },
            create_count: usize::from(mode == Mode::Create),
        },
        timeout: Duration::from_secs(5),
    }
}
fn run(
    host: &PluginHost,
    doc: &mut Document,
    mode: Mode,
    width: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut job = host.start_generic_job(OWNER, doc, invocation(doc, mode, width), None)?;
    loop {
        if let Some(result) = host.poll_job(&mut job, doc, None, "Installed Rust plugin")? {
            ensure(
                matches!(result, JobOutcome::Committed),
                "expected command commit",
            )?;
            return Ok(());
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
fn drain(host: &PluginHost) -> Result<(), Box<dyn std::error::Error>> {
    let until = Instant::now() + Duration::from_secs(5);
    while host.active_jobs() > 0 {
        ensure(Instant::now() < until, "worker failed to drain")?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure(
        args.len() == 1,
        "usage: generic_probe <installed-plugin-directory>",
    )?;
    let directory = Path::new(&args[0]);
    let grants = [
        Permission::ModelRead,
        Permission::ModelWrite,
        Permission::UiTool,
    ]
    .into();
    let mut denied = PluginHost::default();
    ensure(
        denied
            .load_wasm_directory(directory, [Permission::ModelRead].into())
            .is_err(),
        "denied write unexpectedly activated",
    )?;
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory, grants)?;
    ensure(
        host.catalog(OWNER).is_some_and(|c| c.commands.len() == 3),
        "missing descriptors",
    )?;
    let mut doc = Document::new("Independent Rust column")?;
    let original = doc.model().clone();
    run(&host, &mut doc, Mode::Create, 0.4)?;
    let id = *doc
        .model()
        .extensions
        .keys()
        .next()
        .ok_or("missing column")?;
    run(&host, &mut doc, Mode::Edit, 0.8)?;
    let mut geometry_job = host.start_generic_geometry_job(
        OWNER,
        &doc,
        id,
        doc.model().levels.keys().copied().chain([id]).collect(),
        None,
        Duration::from_secs(5),
    )?;
    let geometry = loop {
        if let Some(result) = host.poll_job(&mut geometry_job, &mut doc, None, "Geometry")? {
            let JobOutcome::GenericGeometry(geometry) = result else {
                return Err("expected generic geometry".into());
            };
            break geometry;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    };
    let (geometry_id, _, mesh) = geometry.get(&doc, None)?;
    ensure(
        geometry_id == id && (mesh.signed_volume() - 1.44).abs() < 1e-9,
        "column geometry or identity mismatch",
    )?;
    ensure(
        doc.model().extensions[&id].payload["width"] == json!(0.8),
        "edit did not change width",
    )?;
    ensure(doc.undo(), "missing edit history")?;
    ensure(
        geometry.get(&doc, None).is_err(),
        "old geometry survived undo",
    )?;
    ensure(
        doc.model().extensions[&id].payload["width"] == json!(0.4),
        "undo changed identity or lost dimensions",
    )?;
    ensure(doc.redo(), "missing redo")?;
    run(&host, &mut doc, Mode::Delete, 0.0)?;
    ensure(doc.model().extensions.is_empty(), "delete failed")?;
    ensure(doc.undo(), "missing delete history")?;
    let before = doc.model().clone();
    let mut cancelled =
        host.start_generic_job(OWNER, &doc, invocation(&doc, Mode::Edit, 1.2), None)?;
    cancelled.cancel();
    ensure(
        matches!(
            host.poll_job(&mut cancelled, &mut doc, None, "Cancelled"),
            Err(JobFailure::Cancelled)
        ),
        "cancellation failed",
    )?;
    drain(&host)?;
    ensure(doc.model() == &before, "cancel changed model")?;
    let mut stale = host.start_generic_job(OWNER, &doc, invocation(&doc, Mode::Edit, 1.2), None)?;
    doc.execute(
        "Rename",
        vec![Command::RenameProject("New revision".into())],
    )?;
    ensure(
        matches!(
            host.poll_job(&mut stale, &mut doc, None, "Stale"),
            Err(JobFailure::StaleDocument)
        ),
        "stale result accepted",
    )?;
    drain(&host)?;
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("preserved.osb");
    host.unload(OWNER)?;
    ZipJsonStorage.save(&doc, &path)?;
    ensure(
        ZipJsonStorage.open(&path)?.model() == doc.model(),
        "missing-plugin preservation failed",
    )?;
    ensure(original.extensions.is_empty(), "original snapshot changed")?;
    println!(
        "PASS: installed Rust Wasm Describe/create/edit/delete, checked geometry and stale consumption, stable identity, undo/redo, denied grants, cancellation, stale revision, unload/save/reopen. Desktop activation not tested."
    );
    Ok(())
}
