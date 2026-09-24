#![cfg(feature = "wasm")]
use os_core::{Error, Id, Point2};
use os_document::{Command, Document};
use os_geometry::{GeometryKernel, PrismKernel};
use os_model::{Model, Wall, WallParams};
use os_plugin_api::{Manifest, Permission, Plugin, Request, Response, ResponseEnvelope};
use os_plugin_host::worker::{JobFailure, JobOutcome, PendingJob, ViewContext};
use os_plugin_host::{PluginHost, wasm::*};
use os_storage::{StorageBackend, ZipJsonStorage};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

fn finish(
    host: &PluginHost,
    job: &mut PendingJob,
    doc: &mut Document,
    view: Option<ViewContext>,
) -> std::result::Result<JobOutcome, JobFailure> {
    let limit = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(outcome) = host.poll_job(job, doc, view, "Worker edit")? {
            return Ok(outcome);
        }
        assert!(Instant::now() < limit, "worker never completed");
        std::thread::park_timeout(Duration::from_millis(1));
    }
}

fn finish_load(
    host: &mut PluginHost,
    load: &mut os_plugin_host::loading::PendingLoad,
) -> os_core::Result<String> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(id) = host.poll_wasm_load(load)? {
            return Ok(id);
        }
        assert!(Instant::now() < until, "load did not settle");
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
#[test]
fn background_load_is_explicit_host_bound_and_single_adoption() {
    let directory = install(&wall_manifest(), &response_module(&json(vec![])));
    let mut host = PluginHost::default();
    let mut load = host
        .start_wasm_load(directory.path().into(), grants(), Duration::from_secs(5))
        .unwrap();
    assert_eq!(host.manifests().count(), 0);
    assert!(host.load_pending());
    assert!(
        host.start_wasm_load(directory.path().into(), grants(), Duration::from_secs(5))
            .is_err()
    );
    assert!(PluginHost::default().poll_wasm_load(&mut load).is_err());
    assert_eq!(
        finish_load(&mut host, &mut load).unwrap(),
        "org.openstructure.walls"
    );
    assert!(host.worker_supported("org.openstructure.walls"));
    assert!(host.poll_wasm_load(&mut load).is_err());
    assert_eq!(host.manifests().count(), 1);
}
#[test]
fn background_load_rechecks_collisions_at_adoption() {
    let directory = install(&wall_manifest(), &response_module(&json(vec![])));
    let mut host = PluginHost::default();
    let mut load = host
        .start_wasm_load(directory.path().into(), grants(), Duration::from_secs(5))
        .unwrap();
    host.load_wasm_directory(directory.path(), grants())
        .unwrap();
    let activation = host.activation_id("org.openstructure.walls");
    assert!(finish_load(&mut host, &mut load).is_err());
    assert_eq!(host.activation_id("org.openstructure.walls"), activation);
    assert_eq!(host.manifests().count(), 1);
}
#[test]
fn cancelled_expired_and_denied_background_loads_never_activate() {
    let directory = install(&wall_manifest(), &response_module(&json(vec![])));
    let mut host = PluginHost::default();
    let mut load = host
        .start_wasm_load(directory.path().into(), grants(), Duration::from_secs(5))
        .unwrap();
    load.cancel();
    assert!(host.poll_wasm_load(&mut load).is_err());
    let until = Instant::now() + Duration::from_secs(5);
    while host.load_pending() {
        assert!(Instant::now() < until);
        std::thread::park_timeout(Duration::from_millis(1));
    }
    let mut load = host
        .start_wasm_load(directory.path().into(), grants(), Duration::from_nanos(1))
        .unwrap();
    assert!(host.poll_wasm_load(&mut load).is_err());
    while host.load_pending() {
        assert!(Instant::now() < until);
        std::thread::park_timeout(Duration::from_millis(1));
    }
    let mut load = host
        .start_wasm_load(
            directory.path().into(),
            BTreeSet::new(),
            Duration::from_secs(5),
        )
        .unwrap();
    assert!(matches!(
        finish_load(&mut host, &mut load),
        Err(Error::Permission(_))
    ));
    assert_eq!(host.manifests().count(), 0);
}
fn drained(host: &PluginHost) {
    let limit = Instant::now() + Duration::from_secs(5);
    while host.active_jobs() != 0 {
        assert!(Instant::now() < limit, "revoked worker did not drain");
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
fn command_host(document: &Document) -> (PluginHost, tempfile::TempDir) {
    let wall = Wall::new("org.openstructure.walls.wall", params(document.model()));
    let directory = install(
        &wall_manifest(),
        &response_module(&json(vec![Command::AddWall(wall)])),
    );
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), grants())
        .unwrap();
    (host, directory)
}
fn start(host: &PluginHost, document: &Document, view: Option<ViewContext>) -> PendingJob {
    host.start_job(
        "org.openstructure.walls",
        document,
        Request::CreateWall(params(document.model())),
        view,
        Duration::from_secs(5),
    )
    .unwrap()
}

#[test]
fn worker_commands_commit_exactly_once_and_preserve_history() {
    let mut doc = Document::new("Worker").unwrap();
    let (host, _directory) = command_host(&doc);
    let original = doc.model().clone();
    let mut job = start(&host, &doc, None);
    assert!(!job.id().0.is_nil());
    assert!(matches!(
        finish(&host, &mut job, &mut doc, None),
        Ok(JobOutcome::Committed)
    ));
    assert_eq!(doc.revision(), 1);
    assert_eq!(doc.model().walls.len(), 1);
    assert!(matches!(
        host.poll_job(&mut job, &mut doc, None, "Replay"),
        Err(JobFailure::AlreadySettled)
    ));
    assert!(doc.undo());
    assert_eq!(doc.model(), &original);
    assert!(doc.redo());
    drained(&host);
}

#[test]
fn legacy_model_guest_is_rejected_before_activation_or_model_dispatch() {
    let legacy = wall_manifest().replace("api_version = 9", "api_version = 4");
    // The normal install helper parses manifests through the current host and
    // therefore rejects API 4 before the directory loader sees the fixture.
    // Write the old guest package directly to exercise that load boundary.
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("plugin.toml"), &legacy).unwrap();
    std::fs::write(
        directory.path().join("probe.wasm"),
        response_module(&json(vec![])),
    )
    .unwrap();
    let mut host = PluginHost::default();
    let error = host
        .load_wasm_directory(directory.path(), grants())
        .unwrap_err()
        .to_string();
    assert!(error.contains("rebuild and reinstall"), "{error}");
    assert_eq!(host.manifests().count(), 0);
    assert_eq!(host.active_jobs(), 0);
}

#[test]
fn schema21_worker_member_edits_validate_whole_join_graph_and_remain_atomic() {
    use os_model::{WallAnchor, WallEndpoint, WallJoin, WallJoinParams};
    for detached in [false, true] {
        let mut doc = Document::new("Joined worker model").unwrap();
        let a = Wall::new("org.openstructure.walls.wall", params(doc.model()));
        let mut p = a.parameters.clone();
        p.start = a.parameters.end;
        p.end = Point2::new(5., 4.);
        let b = Wall::new("org.openstructure.walls.wall", p);
        let join = WallJoin::new(
            "core.wall_join",
            WallJoinParams::Corner {
                a: WallAnchor {
                    wall: a.id(),
                    endpoint: WallEndpoint::End,
                },
                b: WallAnchor {
                    wall: b.id(),
                    endpoint: WallEndpoint::Start,
                },
                owner: a.id(),
            },
        );
        doc.execute(
            "Connected",
            vec![
                Command::AddWall(a.clone()),
                Command::AddWall(b),
                Command::AddWallJoin(join),
            ],
        )
        .unwrap();
        let before = doc.model().clone();
        let revision = doc.revision();
        let mut parameters = a.parameters.clone();
        if detached {
            parameters.end.x += 1.;
        } else {
            parameters.start.x -= 1.;
        }
        let directory = install(
            &wall_manifest(),
            &response_module(&json(vec![Command::UpdateWall {
                id: a.id(),
                parameters: parameters.clone(),
            }])),
        );
        let mut host = PluginHost::default();
        host.load_wasm_directory(directory.path(), grants())
            .unwrap();
        let mut job = host
            .start_job(
                "org.openstructure.walls",
                &doc,
                Request::EditWall {
                    id: a.id(),
                    parameters,
                },
                None,
                Duration::from_secs(5),
            )
            .unwrap();
        let outcome = finish(&host, &mut job, &mut doc, None);
        if detached {
            assert!(outcome.is_err());
            assert_eq!(doc.model(), &before);
            assert_eq!(doc.revision(), revision);
        } else {
            assert!(matches!(outcome, Ok(JobOutcome::Committed)));
            assert_eq!(doc.model().wall_joins, before.wall_joins);
            assert!(doc.undo());
            assert_eq!(doc.model(), &before);
            assert!(doc.redo());
        }
        drained(&host);
    }
}

#[test]
fn worker_rejects_changed_reverted_and_reopened_documents() {
    for case in ["edit", "undo", "reopen"] {
        let mut doc = Document::new("Worker").unwrap();
        let (host, _directory) = command_host(&doc);
        let mut job = start(&host, &doc, None);
        if case == "reopen" {
            let session = doc.session_id();
            doc = Document::from_model(doc.model().clone()).unwrap();
            assert_ne!(doc.session_id(), session);
        } else {
            doc.execute("Rename", vec![Command::RenameProject("Changed".into())])
                .unwrap();
            if case == "undo" {
                assert!(doc.undo());
            }
        }
        let before = doc.model().clone();
        let revision = doc.revision();
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, None, "Stale"),
            Err(JobFailure::StaleDocument)
        ));
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), revision);
        drained(&host);
    }
}

#[test]
fn worker_view_switch_and_settings_changes_reject_old_replies() {
    let mut doc = Document::new("Views").unwrap();
    let (host, _directory) = command_host(&doc);
    let view = ViewContext {
        id: *doc.model().views.keys().next().unwrap(),
        settings_revision: 0,
    };
    for settings_revision in [1, u64::MAX] {
        assert!(
            host.start_job(
                "org.openstructure.walls",
                &doc,
                Request::CreateWall(params(doc.model())),
                Some(ViewContext {
                    settings_revision,
                    ..view
                }),
                Duration::from_secs(5),
            )
            .is_err()
        );
        assert_eq!(host.active_jobs(), 0);
    }
    for current in [
        None,
        Some(ViewContext {
            settings_revision: 1,
            ..view
        }),
    ] {
        let mut job = start(&host, &doc, Some(view));
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, current, "Stale view"),
            Err(JobFailure::StaleView)
        ));
        assert_eq!(doc.revision(), 0);
        drained(&host);
    }
    assert!(
        host.start_job(
            "org.openstructure.walls",
            &doc,
            Request::CreateWall(params(doc.model())),
            Some(ViewContext {
                id: Id::new(),
                settings_revision: 0
            }),
            Duration::from_secs(1)
        )
        .is_err()
    );
}

#[test]
fn cancelled_and_expired_workers_never_commit_and_eventually_drain() {
    let mut doc = Document::new("Cancellation").unwrap();
    let (host, _directory) = command_host(&doc);
    let mut job = start(&host, &doc, None);
    job.cancel();
    assert!(matches!(
        host.poll_job(&mut job, &mut doc, None, "Cancelled"),
        Err(JobFailure::Cancelled)
    ));
    drained(&host);
    let mut expired = host
        .start_job(
            "org.openstructure.walls",
            &doc,
            Request::CreateWall(params(doc.model())),
            None,
            Duration::from_nanos(1),
        )
        .unwrap();
    assert!(matches!(
        host.poll_job(&mut expired, &mut doc, None, "Expired"),
        Err(JobFailure::Deadline)
    ));
    drained(&host);
    assert!(doc.model().walls.is_empty());
    assert_eq!(doc.revision(), 0);
    assert!(!doc.can_undo());
    let mut fresh = start(&host, &doc, None);
    assert!(matches!(
        finish(&host, &mut fresh, &mut doc, None),
        Ok(JobOutcome::Committed)
    ));
    drained(&host);
}

#[test]
fn unload_and_reload_cannot_apply_a_previous_generation_reply() {
    let mut doc = Document::new("Reload").unwrap();
    let (mut host, directory) = command_host(&doc);
    let mut job = start(&host, &doc, None);
    host.unload("org.openstructure.walls").unwrap();
    assert_eq!(host.manifests().count(), 0);
    assert_eq!(
        host.registrations(os_plugin_api::RegistrationKind::Tool)
            .count(),
        0
    );
    host.load_wasm_directory(directory.path(), grants())
        .unwrap();
    assert!(matches!(
        host.poll_job(&mut job, &mut doc, None, "Late"),
        Err(JobFailure::PluginUnloaded)
    ));
    assert_eq!(doc.revision(), 0);
    drained(&host);
}

#[test]
fn geometry_reply_is_checked_again_when_consumed() {
    let directory = install(PROBE_MANIFEST, &wat::parse_str(PROBE).unwrap());
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), [Permission::ModelRead].into())
        .unwrap();
    let mut doc = Document::new("Geometry").unwrap();
    let mut job = host
        .start_job(
            "org.openstructure.probe",
            &doc,
            Request::GenerateWall { id: Id::new() },
            None,
            Duration::from_secs(5),
        )
        .unwrap();
    let JobOutcome::Geometry(reply) = finish(&host, &mut job, &mut doc, None).unwrap() else {
        panic!("not geometry")
    };
    assert_eq!(reply.solid(&doc, None).unwrap().height, 1.0);
    doc.execute("Changed", vec![Command::RenameProject("Different".into())])
        .unwrap();
    assert!(matches!(
        reply.solid(&doc, None),
        Err(JobFailure::StaleDocument)
    ));
    drained(&host);
}

#[test]
fn worker_authorization_precedes_dispatch_and_traps_are_recoverable() {
    let mut doc = Document::new("Denied").unwrap();
    let directory = install(
        &wall_manifest().replace("\"model.write\",", ""),
        &module("", "unreachable", "unreachable", "unreachable"),
    );
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), grants())
        .unwrap();
    assert!(matches!(
        host.start_job(
            "org.openstructure.walls",
            &doc,
            Request::CreateWall(params(doc.model())),
            None,
            Duration::from_secs(1)
        ),
        Err(Error::Permission(_))
    ));
    assert_eq!(host.active_jobs(), 0);
    host.unload("org.openstructure.walls").unwrap();
    let trap = install(
        &wall_manifest(),
        &module("", "i32.const 1", "i32.const 0", "unreachable"),
    );
    host.load_wasm_directory(trap.path(), grants()).unwrap();
    let mut job = start(&host, &doc, None);
    assert!(matches!(
        finish(&host, &mut job, &mut doc, None),
        Err(JobFailure::Rejected(_))
    ));
    assert_eq!(doc.revision(), 0);
    drained(&host);
}

#[test]
fn unload_requires_dependents_to_be_removed_first() {
    let first = install(PROBE_MANIFEST, &wat::parse_str(PROBE).unwrap());
    let second_manifest = PROBE_MANIFEST
        .replace("org.openstructure.probe", "org.openstructure.dependent")
        .replace(
            "dependencies = []",
            "dependencies = [{id='org.openstructure.probe', version='0.1.0'}]",
        );
    let second = install(&second_manifest, &wat::parse_str(PROBE).unwrap());
    let mut host = PluginHost::default();
    host.load_wasm_directory(first.path(), [Permission::ModelRead].into())
        .unwrap();
    host.load_wasm_directory(second.path(), [Permission::ModelRead].into())
        .unwrap();
    assert!(host.unload("org.openstructure.probe").is_err());
    assert_eq!(host.manifests().count(), 2);
    host.unload("org.openstructure.dependent").unwrap();
    host.unload("org.openstructure.probe").unwrap();
    assert_eq!(host.manifests().count(), 0);
}

#[test]
fn revoked_infinite_guest_drains_under_fuel_without_committing() {
    let directory = install(
        &wall_manifest(),
        &module(
            "",
            "i32.const 1",
            "i32.const 0",
            "(loop $forever br $forever) i64.const 1",
        ),
    );
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), grants())
        .unwrap();
    let mut doc = Document::new("Loop").unwrap();
    for cancel in [true, false] {
        let timeout = if cancel {
            Duration::from_secs(5)
        } else {
            Duration::from_nanos(1)
        };
        let mut job = host
            .start_job(
                "org.openstructure.walls",
                &doc,
                Request::CreateWall(params(doc.model())),
                None,
                timeout,
            )
            .unwrap();
        if cancel {
            job.cancel();
        }
        let result = host.poll_job(&mut job, &mut doc, None, "Revoked");
        if cancel {
            assert!(matches!(result, Err(JobFailure::Cancelled)));
        } else {
            assert!(matches!(result, Err(JobFailure::Deadline)));
        }
        drained(&host);
        assert_eq!(doc.revision(), 0);
        assert!(doc.model().walls.is_empty());
    }
}

const PROBE_MANIFEST: &str = include_str!("../../../fixtures/wasm-probe/plugin.toml");
const PROBE: &str = include_str!("../../../fixtures/wasm-probe/probe.wat");

fn module(extra: &str, version: &str, alloc: &str, invoke: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
      (memory (export "memory") 32)
      {extra}
      (func (export "os_abi_version") (result i32) {version})
      (func (export "os_alloc") (param i32) (result i32) {alloc})
      (func (export "os_invoke") (param i32 i32) (result i64) {invoke})
    )"#
    ))
    .unwrap()
}
fn echo() -> Vec<u8> {
    module(
        "",
        "i32.const 1",
        "i32.const 0",
        "local.get 1 i64.extend_i32_u i64.const 32 i64.shl local.get 0 i64.extend_i32_u i64.or",
    )
}
fn probe(bytes: &[u8]) -> WasmPlugin {
    WasmPlugin::new(Manifest::from_toml(PROBE_MANIFEST).unwrap(), bytes).unwrap()
}
fn response_module(text: &[u8]) -> Vec<u8> {
    let escaped: String = text.iter().map(|b| format!("\\{b:02x}")).collect();
    module(
        &format!("(data (i32.const 0) \"{escaped}\")"),
        "i32.const 1",
        "i32.const 1048576",
        &format!("i64.const {}", (text.len() as u64) << 32),
    )
}
fn install(manifest: &str, bytes: &[u8]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("plugin.toml"), manifest).unwrap();
    let parsed = Manifest::from_toml(manifest).unwrap();
    std::fs::write(
        directory
            .path()
            .join(parsed.entrypoint.strip_prefix("wasm:").unwrap()),
        bytes,
    )
    .unwrap();
    directory
}
fn wall_manifest() -> String {
    include_str!("../../../plugins/walls/plugin.toml")
        .replace("builtin:os-walls", "wasm:probe.wasm")
}
fn grants() -> BTreeSet<Permission> {
    [
        Permission::ModelRead,
        Permission::ModelWrite,
        Permission::UiTool,
        Permission::UiPanel,
    ]
    .into()
}
fn params(model: &Model) -> WallParams {
    WallParams {
        name: "External reply wall".into(),
        start: Point2::new(0.0, 0.0),
        end: Point2::new(5.0, 0.0),
        thickness: 0.2,
        height: 3.0,
        level: *model.levels.keys().next().unwrap(),
        material: None,
    }
}
fn json(commands: Vec<Command>) -> Vec<u8> {
    serde_json::to_vec(&ResponseEnvelope {
        api_version: os_plugin_api::API_VERSION,
        response: Response::Commands(commands),
    })
    .unwrap()
}

#[test]
fn byte_abi_round_trips_unicode_and_exact_message_limit() {
    let plugin = probe(&echo());
    for text in [
        "{\"name\":\"墙 · é\"}".to_owned(),
        "a".repeat(MAX_WASM_MESSAGE_BYTES),
    ] {
        assert_eq!(plugin.invoke_json(&text).unwrap(), text);
    }
    assert!(
        plugin
            .invoke_json(&"a".repeat(MAX_WASM_MESSAGE_BYTES + 1))
            .is_err()
    );
}

#[test]
fn external_fixture_loads_and_routes_geometry_without_host_recompilation() {
    let directory = install(PROBE_MANIFEST, &wat::parse_str(PROBE).unwrap());
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), [Permission::ModelRead].into())
        .unwrap();
    let Response::Solid(solid) = host
        .request(
            "org.openstructure.probe",
            &Model::new("Test"),
            Request::GenerateWall { id: Id::new() },
        )
        .unwrap()
    else {
        panic!("not geometry")
    };
    assert!((PrismKernel.tessellate(&solid).unwrap().signed_volume() - 1.0).abs() < 1e-9);
    assert!(
        host.load_wasm_directory(directory.path(), [Permission::ModelRead].into())
            .is_err()
    );
    assert_eq!(host.manifests().count(), 1);
}

#[test]
fn guest_state_is_fresh_for_every_invocation() {
    let bytes = module(
        "(global $counter (mut i32) (i32.const 0))",
        "i32.const 1",
        "i32.const 65536",
        "global.get $counter i32.const 1 i32.add global.set $counter i32.const 0 global.get $counter i32.const 48 i32.add i32.store8 i64.const 4294967296",
    );
    let plugin = probe(&bytes);
    assert_eq!(plugin.invoke_json("{}").unwrap(), "1");
    assert_eq!(plugin.invoke_json("{}").unwrap(), "1");
}

#[test]
fn fuel_covers_start_version_allocator_and_invocation_loops() {
    for (extra, version, alloc, invoke) in [
        (
            "(func $start (loop $forever br $forever)) (start $start)",
            "i32.const 1",
            "i32.const 0",
            "i64.const 1",
        ),
        (
            "",
            "(loop $forever br $forever) i32.const 1",
            "i32.const 0",
            "i64.const 1",
        ),
        (
            "",
            "i32.const 1",
            "(loop $forever br $forever) i32.const 0",
            "i64.const 1",
        ),
        (
            "",
            "i32.const 1",
            "i32.const 0",
            "(loop $forever br $forever) i64.const 1",
        ),
    ] {
        let error = probe(&module(extra, version, alloc, invoke))
            .invoke_json("{}")
            .unwrap_err();
        assert!(error.to_string().contains("fuel"), "{error}");
    }
}

#[test]
fn memory_table_and_recursion_limits_are_enforced() {
    for bytes in [
        module(
            "",
            "i32.const 1",
            "i32.const 0",
            "i32.const 257 memory.grow drop i64.const 1",
        ),
        module(
            "(table 1 funcref)",
            "i32.const 1",
            "i32.const 0",
            "ref.null func i32.const 5000 table.grow drop i64.const 1",
        ),
        module(
            "(func $recurse call $recurse)",
            "i32.const 1",
            "i32.const 0",
            "call $recurse i64.const 1",
        ),
        wat::parse_str(PROBE.replace("32 32", "257 257")).unwrap(),
        wat::parse_str(PROBE.replace("32 32", "32 32) (table 4097 funcref")).unwrap(),
    ] {
        assert!(probe(&bytes).invoke_json("{}").is_err());
    }
}

#[test]
fn version_traps_and_memory_ranges_fail_cleanly() {
    for (version, alloc, invoke) in [
        ("i32.const 2", "i32.const 0", "i64.const 4294967296"),
        ("i32.const 1", "i32.const -1", "i64.const 4294967296"),
        ("i32.const 1", "i32.const 2097151", "i64.const 4294967296"),
        ("i32.const 1", "i32.const 0", "unreachable"),
        ("i32.const 1", "i32.const 0", "i64.const 0"),
        ("i32.const 1", "i32.const 0", "i64.const -1"),
        ("i32.const 1", "i32.const 0", "i64.const 8589934591"),
    ] {
        assert!(
            probe(&module("", version, alloc, invoke))
                .invoke_json("{}")
                .is_err()
        );
    }
    assert!(probe(&response_module(&[0xff])).invoke_json("{}").is_err());
    assert!(
        probe(&module(
            "",
            "i32.const 1",
            "i32.const 0",
            &format!("i64.const {}", ((MAX_WASM_MESSAGE_BYTES + 1) as u64) << 32)
        ))
        .invoke_json("{}")
        .is_err()
    );
    assert_eq!(
        probe(&echo()).invoke_json("recovered").unwrap(),
        "recovered"
    );
}

#[test]
fn binary_validation_rejects_imports_signatures_and_excess_structure() {
    let manifest = Manifest::from_toml(PROBE_MANIFEST).unwrap();
    let globals = "(global i32 (i32.const 0))".repeat(1001);
    for bytes in [
        b"not wasm".to_vec(),
        PROBE.as_bytes().to_vec(),
        vec![0; MAX_ARTIFACT_BYTES + 1],
        wat::parse_str("(module)").unwrap(),
        wat::parse_str(PROBE.replace(
            "(memory",
            "(import \"wasi_snapshot_preview1\" \"fd_write\" (func)) (memory",
        ))
        .unwrap(),
        wat::parse_str(PROBE.replace(
            "(export \"os_alloc\") (param i32)",
            "(export \"os_alloc\") (param i64)",
        ))
        .unwrap(),
        module(&globals, "i32.const 1", "i32.const 0", "i64.const 1"),
    ] {
        assert!(WasmPlugin::new(manifest.clone(), &bytes).is_err());
    }
}

#[test]
fn independently_supplied_command_reply_commits_undoes_and_reopens() {
    let mut document = Document::new("Test").unwrap();
    let wall = Wall::new("org.openstructure.walls.wall", params(document.model()));
    let id = wall.id();
    let initial = document.model().clone();
    let directory = install(
        &wall_manifest(),
        &response_module(&json(vec![Command::AddWall(wall)])),
    );
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), grants())
        .unwrap();
    let request = Request::CreateWall(params(document.model()));
    host.execute(
        "org.openstructure.walls",
        &mut document,
        "External create",
        request,
    )
    .unwrap();
    let committed = document.model().clone();
    assert!(committed.walls.contains_key(&id));
    assert!(document.undo());
    assert_eq!(document.model(), &initial);
    assert!(document.redo());
    assert_eq!(document.model(), &committed);
    let path = directory.path().join("roundtrip.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &committed);
}

#[test]
fn denied_writes_fail_before_guest_trap_and_preserve_history() {
    let manifest = wall_manifest().replace("\"model.write\",", "");
    assert!(
        !Manifest::from_toml(&manifest)
            .unwrap()
            .permissions
            .contains(&Permission::ModelWrite)
    );
    let bytes = module("", "unreachable", "i32.const 0", "unreachable");
    let directory = install(&manifest, &bytes);
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), grants())
        .unwrap();
    let mut document = Document::new("Test").unwrap();
    let before = document.model().clone();
    let request = Request::CreateWall(params(&before));
    assert!(matches!(
        host.execute("org.openstructure.walls", &mut document, "Denied", request),
        Err(Error::Permission(_))
    ));
    assert_eq!(document.model(), &before);
    assert_eq!(document.revision(), 0);
    assert!(!document.can_undo());
}

#[test]
fn malformed_hostile_and_invalid_batches_never_commit() {
    let mut document = Document::new("Test").unwrap();
    document
        .execute("Rename", vec![Command::RenameProject("Renamed".into())])
        .unwrap();
    document.undo();
    document.drain_events();
    let before = document.model().clone();
    let revision = document.revision();
    let mut invalid = params(&before);
    invalid.height = -1.0;
    let valid_wall = Wall::new("org.openstructure.walls.wall", params(&before));
    for bytes in [
        response_module(b"{bad JSON"),
        response_module(br#"{"api_version":999,"response":{"result":"Commands","data":[]}}"#),
        response_module(&json(vec![Command::RenameProject("Attack".into())])),
        response_module(&json(vec![
            Command::AddWall(valid_wall.clone()),
            Command::AddWall(Wall::new("org.openstructure.walls.wall", invalid)),
        ])),
        response_module(&json(vec![])),
        response_module(&json(vec![Command::AddWall(valid_wall); 1001])),
        module(
            "",
            "i32.const 1",
            "i32.const 0",
            "(loop $forever br $forever) i64.const 1",
        ),
    ] {
        let directory = install(&wall_manifest(), &bytes);
        let mut host = PluginHost::default();
        host.load_wasm_directory(directory.path(), grants())
            .unwrap();
        assert!(
            host.execute(
                "org.openstructure.walls",
                &mut document,
                "Rejected",
                Request::CreateWall(params(&before))
            )
            .is_err()
        );
        assert_eq!(document.model(), &before);
        assert_eq!(document.revision(), revision);
        assert!(document.can_redo());
        assert!(!document.can_undo());
        assert!(document.drain_events().is_empty());
    }
}

#[test]
fn loading_errors_leave_no_registrations_or_plugins() {
    let bytes = wat::parse_str(PROBE).unwrap();
    for manifest in [
        PROBE_MANIFEST.replace("wasm:probe.wasm", "wasm:../probe.wasm"),
        PROBE_MANIFEST.replace("wasm:probe.wasm", "wasm:C:\\probe.wasm"),
        PROBE_MANIFEST.replace("api_version = 9", "api_version = 999"),
        PROBE_MANIFEST.replace(
            "dependencies = []",
            "dependencies = [{ id = 'org.example.missing', version = '1.0.0' }]",
        ),
        "x".repeat(MAX_MANIFEST_BYTES + 1),
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("plugin.toml"), manifest).unwrap();
        std::fs::write(directory.path().join("probe.wasm"), &bytes).unwrap();
        let mut host = PluginHost::default();
        assert!(
            host.load_wasm_directory(directory.path(), grants())
                .is_err()
        );
        assert_eq!(host.manifests().count(), 0);
    }
    let directory = install(PROBE_MANIFEST, &bytes);
    assert!(
        PluginHost::default()
            .load_wasm_directory(directory.path(), BTreeSet::new())
            .is_err()
    );
    assert!(
        PluginHost::default()
            .load(Box::new(probe(&bytes)), grants())
            .is_err()
    );
}

#[test]
fn truncated_and_mutated_binary_corpus_is_bounded_and_panic_free() {
    let bytes = wat::parse_str(PROBE).unwrap();
    for index in (0..bytes.len()).step_by(7) {
        let mut mutation = bytes.clone();
        mutation[index] ^= 0xff;
        for candidate in [&bytes[..index], mutation.as_slice()] {
            if let Ok(plugin) =
                WasmPlugin::new(Manifest::from_toml(PROBE_MANIFEST).unwrap(), candidate)
            {
                let _ = plugin.invoke_json("{}");
            }
        }
    }
}
