#![cfg(feature = "wasm")]
use os_document::{Command, Document};
use os_model::{ExtensionEntity, Model, PluginRequirement};
use os_plugin_api::{
    Manifest, generic as wire,
    geometry::{Recipe, RectangularPrism},
};
use os_plugin_host::{
    PluginHost,
    worker::{JobFailure, JobOutcome, PendingJob, ViewContext},
};
use std::time::{Duration, Instant};
const OWNER: &str = "org.example.columns";
fn document() -> Document {
    let mut model = Model::new("Geometry worker");
    let mut entity: ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    entity.owner = OWNER.into();
    entity.type_id = "org.example.columns.rectangular".into();
    entity.depends_on.clear();
    entity.relationships.clear();
    entity.payload = serde_json::json!({"width":0.4,"depth":0.6,"height":3.0});
    model.plugin_requirements.insert(
        OWNER.into(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(entity.id, entity);
    Document::from_model(model).unwrap()
}
fn installed(doc: &Document, fault: &str) -> tempfile::TempDir {
    let placeholder = "00000000-0000-4000-8000-000000000000";
    let describe = serde_json::to_string(&wire::Response {
        api_version: 2,
        request_id: placeholder.into(),
        context: None,
        result: wire::Reply::Catalog(
            serde_json::from_str(include_str!(
                "../../../fixtures/generic-column/catalog.json"
            ))
            .unwrap(),
        ),
    })
    .unwrap();
    let id = *doc.model().extensions.keys().next().unwrap();
    let mut reply = wire::Response {
        api_version: 2,
        request_id: placeholder.into(),
        context: Some(wire::Stamp {
            project: doc.model().project.id().to_string(),
            session: doc.session_id().to_string(),
            revision: doc.revision(),
        }),
        result: wire::Reply::Geometry {
            element_id: id.to_string(),
            recipe: Recipe::RectangularPrism(RectangularPrism {
                width: 0.4,
                depth: 0.6,
                height: 3.0,
                translation: [0.0; 3],
                rotation_z: 0.0,
            }),
        },
    };
    if fault == "edits" {
        reply.result = wire::Reply::Edits(vec![wire::Edit::Delete { id: id.to_string() }]);
    }
    let output = if fault == "malformed" {
        "x".repeat(200)
    } else {
        serde_json::to_string(&reply).unwrap()
    };
    let offset = describe.find(placeholder).unwrap();
    let escape = |s: &str| s.bytes().map(|b| format!("\\{b:02x}")).collect::<String>();
    let prefix = match fault {
        "trap" => "unreachable",
        "loop" => "(loop $forever br $forever)",
        _ => "",
    };
    let wat = format!(
        r#"(module
        (memory (export "memory") 2 256)
        (data (i32.const 0) "{}") (data (i32.const 8192) "{}")
        (func (export "os_abi_version") (result i32) i32.const 1)
        (func (export "os_alloc") (param i32) (result i32) i32.const 32768)
        (func (export "os_invoke") (param $ptr i32) (param $len i32) (result i64)
            (local $base i32) (local $size i32)
            local.get $len i32.const 200 i32.lt_u if
                i32.const 0 local.set $base i32.const {} local.set $size
            else {prefix} i32.const 8192 local.set $base i32.const {} local.set $size end
            local.get $base i32.const {offset} i32.add local.get $ptr i32.const {offset} i32.add i32.const 36 memory.copy
            local.get $size i64.extend_i32_u i64.const 32 i64.shl local.get $base i64.extend_i32_u i64.or))"#,
        escape(&describe),
        escape(&output),
        describe.len(),
        output.len()
    );
    let dir = tempfile::tempdir().unwrap();
    let manifest = include_str!("../../../fixtures/generic-column/plugin.toml")
        .replace("builtin:generic-column-test", "wasm:geometry.wasm")
        .replace("[\"Modeling\"]", "[\"Modeling\", \"Geometry\"]");
    std::fs::write(dir.path().join("plugin.toml"), manifest).unwrap();
    std::fs::write(
        dir.path().join("geometry.wasm"),
        wat::parse_str(wat).unwrap(),
    )
    .unwrap();
    dir
}
fn load(host: &mut PluginHost, dir: &tempfile::TempDir) {
    let m =
        Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml")).unwrap();
    host.load_wasm_directory(dir.path(), m.permissions).unwrap();
}
fn start(host: &PluginHost, doc: &Document, view: Option<ViewContext>) -> PendingJob {
    let id = *doc.model().extensions.keys().next().unwrap();
    host.start_generic_geometry_job(OWNER, doc, id, [id].into(), view, Duration::from_secs(5))
        .unwrap()
}
fn finish(
    host: &PluginHost,
    doc: &mut Document,
    job: &mut PendingJob,
    view: Option<ViewContext>,
) -> Result<JobOutcome, JobFailure> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = host.poll_job(job, doc, view, "Geometry")? {
            return Ok(result);
        }
        assert!(Instant::now() < until);
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
fn drain(host: &PluginHost) {
    let until = Instant::now() + Duration::from_secs(5);
    while host.active_jobs() > 0 {
        assert!(Instant::now() < until);
        std::thread::park_timeout(Duration::from_millis(1));
    }
}

#[test]
fn geometry_worker_requires_the_persisted_view_revision_before_dispatch() {
    let mut model = document().model().clone();
    let view_id = *model.views.keys().next().unwrap();
    model
        .views
        .get_mut(&view_id)
        .unwrap()
        .parameters
        .settings_revision = 7;
    let mut doc = Document::from_model(model).unwrap();
    let before = doc.model().clone();
    let dir = installed(&doc, "");
    let mut host = PluginHost::default();
    load(&mut host, &dir);
    let id = *doc.model().extensions.keys().next().unwrap();
    for settings_revision in [0, 6, 8, u64::MAX] {
        let result = host.start_generic_geometry_job(
            OWNER,
            &doc,
            id,
            [id].into(),
            Some(ViewContext {
                id: view_id,
                settings_revision,
            }),
            Duration::from_secs(5),
        );
        assert!(
            result.is_err(),
            "accepted a view revision absent from the document"
        );
        assert_eq!(host.active_jobs(), 0);
    }
    let view = Some(ViewContext {
        id: view_id,
        settings_revision: 7,
    });
    let mut job = start(&host, &doc, view);
    let JobOutcome::GenericGeometry(result) = finish(&host, &mut doc, &mut job, view).unwrap()
    else {
        panic!("wrong outcome")
    };
    assert!(result.get(&doc, view).is_ok());
    assert!(matches!(
        result.get(
            &doc,
            Some(ViewContext {
                id: view_id,
                settings_revision: 8,
            })
        ),
        Err(JobFailure::StaleView)
    ));
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.revision(), 0);
    assert!(!doc.can_undo());
    drain(&host);
}

#[test]
fn geometry_worker_is_read_only_single_consumption_and_view_checked() {
    let mut doc = document();
    let before = doc.model().clone();
    let dir = installed(&doc, "");
    let mut host = PluginHost::default();
    load(&mut host, &dir);
    let view = ViewContext {
        id: *doc.model().views.keys().next().unwrap(),
        settings_revision: 0,
    };
    let mut job = start(&host, &doc, Some(view));
    let id = *doc.model().extensions.keys().next().unwrap();
    assert!(
        host.generate_generic_geometry(OWNER, &doc, id, [id].into(), Duration::from_secs(5))
            .is_err()
    );
    let JobOutcome::GenericGeometry(result) =
        finish(&host, &mut doc, &mut job, Some(view)).unwrap()
    else {
        panic!("wrong outcome");
    };
    assert!((result.get(&doc, Some(view)).unwrap().2.signed_volume() - 0.72).abs() < 1e-9);
    assert!(matches!(result.get(&doc, None), Err(JobFailure::StaleView)));
    assert!(matches!(
        host.poll_job(&mut job, &mut doc, Some(view), "Replay"),
        Err(JobFailure::AlreadySettled)
    ));
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.revision(), 0);
    assert!(!doc.can_undo());
    drain(&host);
    doc.execute("Rename", vec![Command::RenameProject("New".into())])
        .unwrap();
    assert!(matches!(
        result.get(&doc, Some(view)),
        Err(JobFailure::StaleDocument)
    ));
}

#[test]
fn stale_document_view_and_plugin_generations_reject_geometry_delivery() {
    for case in ["undo", "reopen", "view", "reload"] {
        let mut doc = document();
        let dir = installed(&doc, "");
        let mut host = PluginHost::default();
        load(&mut host, &dir);
        let mut job = start(&host, &doc, None);
        let mut view = None;
        match case {
            "undo" => {
                doc.execute("Rename", vec![Command::RenameProject("New".into())])
                    .unwrap();
                assert!(doc.undo());
            }
            "reopen" => doc = Document::from_model(doc.model().clone()).unwrap(),
            "view" => {
                view = Some(ViewContext {
                    id: *doc.model().views.keys().next().unwrap(),
                    settings_revision: 1,
                })
            }
            "reload" => {
                host.unload(OWNER).unwrap();
                load(&mut host, &dir);
            }
            _ => unreachable!(),
        }
        let result = host.poll_job(&mut job, &mut doc, view, "Stale");
        match case {
            "view" => assert!(matches!(result, Err(JobFailure::StaleView))),
            "reload" => assert!(matches!(result, Err(JobFailure::PluginUnloaded))),
            _ => assert!(matches!(result, Err(JobFailure::StaleDocument))),
        }
        drain(&host);
    }
}

#[test]
fn expired_and_dropped_geometry_tickets_release_capacity() {
    let mut doc = document();
    let dir = installed(&doc, "");
    let mut host = PluginHost::default();
    load(&mut host, &dir);
    let id = *doc.model().extensions.keys().next().unwrap();
    let mut job = host
        .start_generic_geometry_job(OWNER, &doc, id, [id].into(), None, Duration::from_secs(1))
        .unwrap();
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until {
        std::thread::park_timeout(Duration::from_millis(1));
    }
    assert_eq!(host.active_jobs(), 1);
    assert!(matches!(
        host.poll_job(&mut job, &mut doc, None, "Expired"),
        Err(JobFailure::Deadline)
    ));
    drain(&host);
    let dropped = start(&host, &doc, None);
    drop(dropped);
    drain(&host);
    assert_eq!(doc.revision(), 0);
    assert!(!doc.can_undo());
}

#[test]
fn cancelled_and_invalid_geometry_workers_never_mutate_and_drain() {
    for fault in ["", "edits", "malformed", "trap", "loop"] {
        let mut doc = document();
        let dir = installed(&doc, fault);
        let mut host = PluginHost::default();
        load(&mut host, &dir);
        let before = doc.model().clone();
        let mut job = start(&host, &doc, None);
        job.cancel();
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, None, "Cancel"),
            Err(JobFailure::Cancelled)
        ));
        drain(&host);
        let mut job = start(&host, &doc, None);
        let result = finish(&host, &mut doc, &mut job, None);
        if fault.is_empty() {
            assert!(matches!(result, Ok(JobOutcome::GenericGeometry(_))));
        } else {
            assert!(matches!(result, Err(JobFailure::Rejected(_))));
        }
        drain(&host);
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
    }
}
