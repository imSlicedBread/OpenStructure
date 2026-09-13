//! Run an already-built independent Wall artifact through the native adapter.
use os_core::ensure;
use os_document::{Command, Document};
use os_plugin_api::{Permission, wall::OWNER};
use os_plugin_host::{
    PluginHost,
    generic::{Invocation, Scope},
    worker::JobOutcome,
};
use os_storage::{StorageBackend, ZipJsonStorage};
use serde_json::json;
use std::{collections::BTreeMap, path::Path, time::Duration};

fn invoke(
    host: &PluginHost,
    doc: &mut Document,
    mode: &str,
    height: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let level = *doc.model().levels.keys().next().unwrap();
    let target = doc.model().walls.keys().next().copied();
    let invocation = Invocation {
        command_id: format!("{OWNER}.{mode}"),
        inputs: if mode == "delete" {
            BTreeMap::new()
        } else {
            BTreeMap::from([
                ("start_x".into(), json!(2.0)),
                ("start_y".into(), json!(3.0)),
                ("end_x".into(), json!(5.0)),
                ("end_y".into(), json!(7.0)),
                ("thickness".into(), json!(0.2)),
                ("height".into(), json!(height)),
            ])
        },
        scope: Scope {
            read: [level].into_iter().chain(target).collect(),
            write: if mode == "create" {
                Default::default()
            } else {
                target.into_iter().collect()
            },
            create_count: usize::from(mode == "create"),
        },
        timeout: Duration::from_secs(5),
    };
    let mut job = host.start_generic_job(OWNER, doc, invocation, None)?;
    loop {
        if let Some(result) = host.poll_job(&mut job, doc, None, "Native wall plugin")? {
            ensure(
                matches!(result, JobOutcome::Committed),
                "expected wall commit",
            )?;
            return Ok(());
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure(
        args.len() == 1,
        "usage: native_wall_probe <installed-wall-directory>",
    )?;
    let mut host = PluginHost::default();
    host.load_wasm_directory(
        Path::new(&args[0]),
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ]
        .into(),
    )?;
    let mut doc = Document::new("Independent native wall")?;
    invoke(&host, &mut doc, "create", 3.0)?;
    ensure(
        doc.model().walls.len() == 1 && doc.model().extensions.is_empty(),
        "wall was not native",
    )?;
    let id = *doc.model().walls.keys().next().unwrap();
    let original = doc.model().walls[&id].clone();
    let mut geometry = host.start_generic_geometry_job(
        OWNER,
        &doc,
        id,
        doc.model().levels.keys().copied().chain([id]).collect(),
        None,
        Duration::from_secs(5),
    )?;
    loop {
        if let Some(result) = host.poll_job(&mut geometry, &mut doc, None, "Wall geometry")? {
            let JobOutcome::GenericGeometry(result) = result else {
                return Err("expected geometry".into());
            };
            let (_, _, mesh) = result.get(&doc, None)?;
            ensure(
                (mesh.signed_volume() - 3.0).abs() < 1e-8,
                "wall volume mismatch",
            )?;
            ensure(
                (mesh.vertices[0].x - 2.08).abs() < 1e-9
                    && (mesh.vertices[0].y - 2.94).abs() < 1e-9,
                "rotated wall placement mismatch",
            )?;
            break;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    invoke(&host, &mut doc, "edit", 4.0)?;
    ensure(
        doc.model().walls[&id].parameters.height == 4.0,
        "native edit failed",
    )?;
    ensure(doc.undo(), "missing undo")?;
    ensure(
        doc.model().walls[&id] == original,
        "undo did not restore native identity/data",
    )?;
    ensure(doc.redo(), "missing redo")?;
    let before = doc.model().clone();
    let revision = doc.revision();
    ensure(
        invoke(&host, &mut doc, "edit", -1.0).is_err(),
        "invalid dimensions accepted",
    )?;
    ensure(
        doc.model() == &before && doc.revision() == revision,
        "failed edit changed model",
    )?;
    invoke(&host, &mut doc, "delete", 0.0)?;
    ensure(doc.model().walls.is_empty(), "delete failed")?;
    ensure(doc.undo(), "delete undo failed")?;
    host.unload(OWNER)?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("wall.osb");
    ZipJsonStorage.save(&doc, &path)?;
    let mut reopened = ZipJsonStorage.open(&path)?;
    ensure(
        reopened.model() == doc.model(),
        "native wall preservation failed",
    )?;
    // Native document commands continue working when the external guest is absent.
    let mut parameters = reopened.model().walls[&id].parameters.clone();
    parameters.height = 5.0;
    reopened.execute(
        "Native edit without guest",
        vec![Command::UpdateWall { id, parameters }],
    )?;
    println!(
        "PASS: independent Rust Wall creates/edits/deletes native walls, checked rotated geometry, undo/redo, invalid-edit atomicity, unload/save/reopen and native editing without guest. Desktop tool integration remains pending."
    );
    Ok(())
}
