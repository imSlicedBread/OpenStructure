use os_core::{Id, Result};
use os_document::Document;
use os_plugin_api::{Manifest, Permission, Plugin, generic as wire};
use os_plugin_host::{
    PluginHost,
    generic::{Invocation, Scope},
};
use os_storage::{StorageBackend, ZipJsonStorage};
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, time::Duration};

const OWNER: &str = "org.example.columns";
type Calls = Rc<RefCell<Vec<wire::Request>>>;
struct Column {
    calls: Calls,
    fault: Rc<RefCell<String>>,
    writable: bool,
}
fn manifest() -> Manifest {
    Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml")).unwrap()
}
fn catalog() -> wire::Catalog {
    serde_json::from_str(include_str!(
        "../../../fixtures/generic-column/catalog.json"
    ))
    .unwrap()
}

#[test]
fn read_access_to_native_wall_does_not_authorize_another_owner_to_edit_it() {
    let (host, calls, _) = host(true);
    let mut doc = Document::new("Native scope").unwrap();
    let wall = os_model::Wall::new(
        os_plugin_api::wall::TYPE,
        os_model::WallParams {
            name: "Wall".into(),
            start: os_core::Point2::new(0.0, 0.0),
            end: os_core::Point2::new(5.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level: *doc.model().levels.keys().next().unwrap(),
            material: None,
        },
    );
    let id = wall.id();
    doc.execute("Native", vec![os_document::Command::AddWall(wall)])
        .unwrap();
    let before = doc.model().clone();
    let count = calls.borrow().len();
    let invocation = Invocation {
        command_id: format!("{OWNER}.edit"),
        inputs: BTreeMap::from([("width".into(), json!(0.4))]),
        scope: Scope {
            read: [id].into(),
            write: [id].into(),
            create_count: 0,
        },
        timeout: Duration::from_secs(5),
    };
    assert!(
        host.execute_generic(OWNER, &mut doc, "Unauthorized", invocation)
            .is_err()
    );
    assert_eq!(calls.borrow().len(), count);
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.revision(), 1);
}

#[test]
fn geometry_is_scoped_read_only_validated_and_revision_bound() {
    let (host, calls, fault) = host(true);
    let mut doc = Document::new("Geometry scope").unwrap();
    let invocation = create(&doc);
    host.execute_generic(OWNER, &mut doc, "Create", invocation)
        .unwrap();
    let id = *doc.model().extensions.keys().next().unwrap();
    let scope = [id].into();
    let before = doc.model().clone();
    let revision = doc.revision();
    doc.drain_events();
    let geometry = host
        .generate_generic_geometry(OWNER, &doc, id, scope, Duration::from_secs(5))
        .unwrap();
    let (_, _, mesh) = geometry.get(&doc).unwrap();
    assert!((mesh.signed_volume() - 0.72).abs() < 1e-9);
    let wire::Operation::GenerateGeometry { snapshot, .. } =
        calls.borrow().last().unwrap().operation.clone()
    else {
        panic!("wrong request");
    };
    assert_eq!(snapshot.elements.len(), 1);
    assert!(
        snapshot.levels.is_empty(),
        "no implicit disclosure of referenced levels"
    );
    for bad in [
        "geometry edits",
        "geometry identity",
        "geometry dimensions",
        "request id",
        "revision",
        "session",
        "version",
        "malformed",
        "oversize",
    ] {
        *fault.borrow_mut() = bad.into();
        assert!(
            host.generate_generic_geometry(OWNER, &doc, id, [id].into(), Duration::from_secs(5))
                .is_err(),
            "{bad}"
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), revision);
        assert!(doc.drain_events().is_empty());
    }
    let count = calls.borrow().len();
    assert!(
        host.generate_generic_geometry(OWNER, &doc, id, Default::default(), Duration::from_secs(5))
            .is_err()
    );
    assert!(
        host.generate_generic_geometry(OWNER, &doc, id, [id].into(), Duration::ZERO)
            .is_err()
    );
    assert_eq!(calls.borrow().len(), count);
    let reopened = Document::from_model(doc.model().clone()).unwrap();
    assert!(geometry.get(&reopened).is_err());
    assert!(doc.undo());
    assert!(geometry.get(&doc).is_err());
    assert!(doc.redo());
    assert!(
        geometry.get(&doc).is_err(),
        "restoring the same model cannot revive old geometry"
    );
}
impl Plugin for Column {
    fn manifest(&self) -> Manifest {
        let mut m = manifest();
        m.capabilities.push(os_plugin_api::Capability::Geometry);
        if !self.writable {
            m.permissions.remove(&Permission::ModelWrite);
        }
        m
    }
    fn invoke_json(&self, input: &str) -> Result<String> {
        let request: wire::Request = serde_json::from_str(input).unwrap();
        self.calls.borrow_mut().push(request.clone());
        let fault = self.fault.borrow().clone();
        let mut result = match &request.operation {
            wire::Operation::GenerateGeometry {
                element_id,
                snapshot,
            } => {
                assert!(snapshot.elements.iter().any(|e| e.id == *element_id));
                if fault == "geometry edits" {
                    wire::Reply::Edits(vec![wire::Edit::Delete {
                        id: element_id.clone(),
                    }])
                } else {
                    wire::Reply::Geometry {
                        element_id: if fault == "geometry identity" {
                            Id::new().to_string()
                        } else {
                            element_id.clone()
                        },
                        recipe: os_plugin_api::geometry::Recipe::RectangularPrism(
                            os_plugin_api::geometry::RectangularPrism {
                                width: if fault == "geometry dimensions" {
                                    -1.0
                                } else {
                                    0.4
                                },
                                depth: 0.6,
                                height: 3.0,
                                translation: [0.0; 3],
                                rotation_z: 0.0,
                            },
                        ),
                    }
                }
            }
            wire::Operation::Describe => {
                let mut c = catalog();
                if fault == "catalog" {
                    c.commands[0].id = "org.other.tool".into();
                }
                wire::Reply::Catalog(c)
            }
            wire::Operation::Invoke {
                command_id,
                inputs,
                snapshot,
                selection,
                new_ids,
            } => {
                if fault == "slow" {
                    std::thread::sleep(Duration::from_millis(200));
                }
                if fault == "trap" {
                    return Err(os_core::Error::Invalid("test invocation failure".into()));
                }
                let edit = if command_id.ends_with(".create") {
                    let mut element = wire::Element {
                        id: new_ids[0].clone(),
                        owner: OWNER.into(),
                        type_id: "org.example.columns.rectangular".into(),
                        name: "Generic column".into(),
                        payload_schema_version: 1,
                        payload: json!({"width":inputs["width"],"depth":0.6,"height":3.0,"vendor":{"keep":true}}),
                        relationships: BTreeMap::new(),
                        depends_on: [snapshot.levels[0].id.clone()].into(),
                    };
                    match fault.as_str() {
                        "owner" => element.owner = "org.other.columns".into(),
                        "type" => element.type_id = "org.example.columns.other".into(),
                        "identity" => element.id = Id::new().to_string(),
                        "payload" => element.payload["width"] = json!(-1),
                        "reference" => {
                            element.depends_on.insert(Id::new().to_string());
                        }
                        "schema" => element.payload_schema_version = 99,
                        _ => {}
                    }
                    wire::Edit::Create(element)
                } else if command_id.ends_with(".edit") {
                    let target = if fault == "outside selection" {
                        snapshot
                            .elements
                            .iter()
                            .find(|e| !selection.contains(&e.id))
                            .unwrap()
                    } else {
                        snapshot
                            .elements
                            .iter()
                            .find(|e| e.id == selection[0])
                            .unwrap()
                    };
                    let mut element = target.clone();
                    element.payload["width"] = inputs["width"].clone();
                    wire::Edit::Replace {
                        expected_schema_version: element.payload_schema_version,
                        element,
                    }
                } else {
                    wire::Edit::Delete {
                        id: selection[0].clone(),
                    }
                };
                let mut edits = vec![edit.clone()];
                if fault == "mode" {
                    edits = vec![wire::Edit::Delete {
                        id: new_ids[0].clone(),
                    }];
                }
                if fault == "duplicate" {
                    edits.push(edit);
                }
                if fault == "too many" {
                    edits = vec![edits[0].clone(); 17];
                }
                wire::Reply::Edits(edits)
            }
        };
        if fault == "error" && matches!(request.operation, wire::Operation::Invoke { .. }) {
            result = wire::Reply::Error(wire::PluginError {
                code: wire::ErrorCode::InvalidInput,
                message: "Column constraint failed".into(),
                field: Some("width".into()),
            });
        }
        let mut response = wire::Response {
            api_version: wire::VERSION,
            request_id: request.request_id,
            context: request.context,
            result,
        };
        if !matches!(request.operation, wire::Operation::Describe) {
            match fault.as_str() {
                "request id" => response.request_id = Id::new().to_string(),
                "revision" => response.context.as_mut().unwrap().revision += 1,
                "session" => response.context.as_mut().unwrap().session = Id::new().to_string(),
                "version" => response.api_version = 1,
                "malformed" => return Ok("not json".into()),
                "oversize" => return Ok("x".repeat(wire::MAX_BYTES + 1)),
                _ => {}
            }
        }
        Ok(serde_json::to_string(&response).unwrap())
    }
}
fn host(writable: bool) -> (PluginHost, Calls, Rc<RefCell<String>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let fault = Rc::new(RefCell::new(String::new()));
    let mut host = PluginHost::default();
    host.load(
        Box::new(Column {
            calls: calls.clone(),
            fault: fault.clone(),
            writable,
        }),
        manifest().permissions,
    )
    .unwrap();
    (host, calls, fault)
}
fn create(doc: &Document) -> Invocation {
    Invocation {
        command_id: format!("{OWNER}.create"),
        inputs: BTreeMap::from([("width".into(), json!(0.4))]),
        scope: Scope {
            read: [*doc.model().levels.keys().next().unwrap()].into(),
            create_count: 1,
            ..Scope::default()
        },
        timeout: Duration::from_secs(5),
    }
}

#[test]
fn registered_create_edit_delete_history_and_missing_plugin_reopen() {
    let (mut host, calls, _) = host(true);
    let mut doc = Document::new("Generic workflow").unwrap();
    let original = doc.model().clone();
    host.execute_generic(
        OWNER,
        &mut doc,
        "Create column",
        create(&Document::from_model(original.clone()).unwrap()),
    )
    .unwrap();
    let created = doc.model().clone();
    assert_eq!(created.plugin_requirements[OWNER].version, "1.0.0");
    let id = *created.extensions.keys().next().unwrap();
    let mut invocation = create(&doc);
    invocation.command_id = format!("{OWNER}.edit");
    invocation.scope.read.insert(id);
    invocation.scope.write.insert(id);
    invocation.scope.create_count = 0;
    invocation.inputs.insert("width".into(), json!(0.8));
    host.execute_generic(OWNER, &mut doc, "Edit column", invocation.clone())
        .unwrap();
    assert_eq!(doc.model().extensions[&id].payload["width"], json!(0.8));
    assert_eq!(
        doc.model().extensions[&id].payload["vendor"],
        json!({"keep":true})
    );
    assert!(doc.undo());
    assert_eq!(doc.model(), &created);
    assert!(doc.redo());
    invocation.command_id = format!("{OWNER}.delete");
    invocation.inputs.clear();
    host.execute_generic(OWNER, &mut doc, "Delete column", invocation)
        .unwrap();
    assert!(doc.model().extensions.is_empty());
    assert!(doc.undo());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("generic.osb");
    host.unload(OWNER).unwrap();
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    assert_eq!(calls.borrow().len(), 4);
}

#[test]
fn every_rejected_reply_preserves_model_revision_history_and_events() {
    let (host, _, fault) = host(true);
    for case in [
        "owner",
        "type",
        "mode",
        "identity",
        "payload",
        "reference",
        "schema",
        "duplicate",
        "too many",
        "request id",
        "revision",
        "session",
        "version",
        "malformed",
        "oversize",
        "trap",
        "error",
    ] {
        *fault.borrow_mut() = case.into();
        let mut doc = Document::new("Unchanged").unwrap();
        let original = doc.model().clone();
        let invocation = create(&doc);
        assert!(
            host.execute_generic(OWNER, &mut doc, "Rejected", invocation)
                .is_err(),
            "accepted {case}"
        );
        assert_eq!(doc.model(), &original);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
        assert!(doc.drain_events().is_empty());
    }
}

#[test]
fn permissions_inputs_and_unregistered_commands_fail_before_invocation() {
    for case in ["permission", "input", "command", "selection", "deadline"] {
        let (host, calls, _) = host(case != "permission");
        let mut doc = Document::new("Protected").unwrap();
        let mut invocation = create(&doc);
        match case {
            "input" => {
                invocation.inputs.insert("width".into(), json!(-1));
            }
            "command" => invocation.command_id = "org.other.tool".into(),
            "selection" => {
                invocation.scope.write.insert(Id::new());
            }
            "deadline" => invocation.timeout = Duration::ZERO,
            _ => {}
        }
        assert!(
            host.execute_generic(OWNER, &mut doc, "Denied", invocation)
                .is_err()
        );
        assert_eq!(calls.borrow().len(), 1, "invoked after denied {case}");
        assert_eq!(doc.revision(), 0);
    }
}

#[test]
fn read_snapshot_excludes_other_objects_and_read_access_does_not_grant_write() {
    let (host, calls, fault) = host(true);
    let mut doc = Document::new("Scope").unwrap();
    let inv = create(&doc);
    host.execute_generic(OWNER, &mut doc, "First", inv).unwrap();
    let first = *doc.model().extensions.keys().next().unwrap();
    let inv = create(&doc);
    host.execute_generic(OWNER, &mut doc, "Second", inv)
        .unwrap();
    let wire::Operation::Invoke { snapshot, .. } = calls.borrow().last().unwrap().operation.clone()
    else {
        panic!()
    };
    assert!(snapshot.elements.is_empty());
    assert_eq!(snapshot.levels.len(), 1);
    let mut inv = create(&doc);
    inv.command_id = format!("{OWNER}.edit");
    inv.scope.read.extend(doc.model().extensions.keys());
    inv.scope.write.insert(first);
    inv.scope.create_count = 0;
    *fault.borrow_mut() = "outside selection".into();
    let original = doc.model().clone();
    let revision = doc.revision();
    assert!(
        host.execute_generic(OWNER, &mut doc, "Denied", inv)
            .is_err()
    );
    assert_eq!(doc.model(), &original);
    assert_eq!(doc.revision(), revision);
}

#[test]
fn invalid_catalog_activation_publishes_no_registrations() {
    let mut host = PluginHost::default();
    assert!(
        host.load(
            Box::new(Column {
                calls: Rc::new(RefCell::new(vec![])),
                fault: Rc::new(RefCell::new("catalog".into())),
                writable: true
            }),
            manifest().permissions
        )
        .is_err()
    );
    assert_eq!(host.manifests().count(), 0);
    assert!(host.catalog(OWNER).is_none());
}

#[test]
fn expired_results_and_incompatible_document_requirements_cannot_commit() {
    let (host, calls, fault) = host(true);
    let mut doc = Document::new("Deadline").unwrap();
    let original = doc.model().clone();
    *fault.borrow_mut() = "slow".into();
    let mut invocation = create(&doc);
    invocation.timeout = Duration::from_millis(100);
    assert!(
        host.execute_generic(OWNER, &mut doc, "Expired", invocation)
            .is_err()
    );
    assert_eq!(doc.model(), &original);
    assert_eq!(doc.revision(), 0);
    assert_eq!(
        calls.borrow().len(),
        2,
        "expired result must have reached the guest"
    );
    *fault.borrow_mut() = String::new();
    doc.execute(
        "Require other version",
        vec![os_document::Command::SetPluginRequirement {
            plugin_id: OWNER.into(),
            requirement: Some(os_model::PluginRequirement {
                version: "2.0.0".into(),
            }),
        }],
    )
    .unwrap();
    let count = calls.borrow().len();
    let invocation = create(&doc);
    assert!(
        host.execute_generic(OWNER, &mut doc, "Wrong version", invocation)
            .is_err()
    );
    assert_eq!(calls.borrow().len(), count);
    assert_eq!(doc.revision(), 1);
}

#[cfg(feature = "wasm")]
fn installed_column(doc: &Document, invoke_prefix: &str) -> tempfile::TempDir {
    // This deliberately small WAT fixture echoes the envelope nonce and uses a
    // host-reserved identity. It is not the separately built Rust SDK example.
    let placeholder = "00000000-0000-4000-8000-000000000000";
    let describe = serde_json::to_string(&wire::Response {
        api_version: 2,
        request_id: placeholder.into(),
        context: None,
        result: wire::Reply::Catalog(catalog()),
    })
    .unwrap();
    let element = wire::Element {
        id: placeholder.into(),
        owner: OWNER.into(),
        type_id: "org.example.columns.rectangular".into(),
        name: "External column".into(),
        payload_schema_version: 1,
        payload: json!({"width":0.4,"depth":0.6,"height":3.0}),
        relationships: BTreeMap::new(),
        depends_on: [doc.model().levels.keys().next().unwrap().to_string()].into(),
    };
    let edits = serde_json::to_string(&wire::Response {
        api_version: 2,
        request_id: placeholder.into(),
        context: Some(wire::Stamp {
            project: doc.model().project.id().to_string(),
            session: doc.session_id().to_string(),
            revision: doc.revision(),
        }),
        result: wire::Reply::Edits(vec![wire::Edit::Create(element)]),
    })
    .unwrap();
    let request_offset = describe.find(placeholder).unwrap();
    assert_eq!(edits.find(placeholder).unwrap(), request_offset);
    let entity_offset = edits.rfind(placeholder).unwrap();
    let escaped = |s: &str| s.bytes().map(|b| format!("\\{b:02x}")).collect::<String>();
    let needle = u64::from_le_bytes(*b"\"new_ids");
    let wat = format!(
        r#"(module
        (memory (export "memory") 2 256)
        (data (i32.const 0) "{describe}")
        (data (i32.const 8192) "{edits}")
        (func (export "os_abi_version") (result i32) i32.const 1)
        (func (export "os_alloc") (param i32) (result i32) i32.const 32768)
        (func (export "os_invoke") (param $ptr i32) (param $len i32) (result i64)
            (local $scan i32) (local $base i32) (local $size i32)
            local.get $len i32.const 200 i32.lt_u
            if
                i32.const 0 local.set $base i32.const {describe_len} local.set $size
            else
                {invoke_prefix}
                i32.const 8192 local.set $base i32.const {edits_len} local.set $size
                local.get $ptr local.set $scan
                block $found
                    loop $search
                        local.get $scan local.get $ptr local.get $len i32.add i32.ge_u if unreachable end
                        local.get $scan i64.load i64.const {needle} i64.eq br_if $found
                        local.get $scan i32.const 1 i32.add local.set $scan br $search
                    end
                end
                i32.const {entity_dest} local.get $scan i32.const 12 i32.add i32.const 36 memory.copy
            end
            local.get $base i32.const {request_offset} i32.add
            local.get $ptr i32.const {request_offset} i32.add i32.const 36 memory.copy
            local.get $size i64.extend_i32_u i64.const 32 i64.shl
            local.get $base i64.extend_i32_u i64.or)
        )"#,
        describe = escaped(&describe),
        edits = escaped(&edits),
        describe_len = describe.len(),
        edits_len = edits.len(),
        entity_dest = 8192 + entity_offset
    );
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("plugin.toml"),
        include_str!("../../../fixtures/generic-column/plugin.toml")
            .replace("builtin:generic-column-test", "wasm:column.wasm"),
    )
    .unwrap();
    std::fs::write(
        directory.path().join("column.wasm"),
        wat::parse_str(wat).unwrap(),
    )
    .unwrap();
    directory
}

#[cfg(feature = "wasm")]
#[test]
fn external_wasm_catalog_and_generic_creation_use_the_same_checked_route() {
    let mut doc = Document::new("External v2 transport").unwrap();
    let directory = installed_column(&doc, "");
    let mut host = PluginHost::default();
    host.load_wasm_directory(directory.path(), manifest().permissions)
        .unwrap();
    assert_eq!(host.catalog(OWNER).unwrap().commands.len(), 3);
    let invocation = create(&doc);
    host.execute_generic(OWNER, &mut doc, "External create", invocation)
        .unwrap();
    assert_eq!(doc.model().extensions.len(), 1);
    assert!(doc.undo());
    assert!(doc.redo());
    host.unload(OWNER).unwrap();
    let path = directory.path().join("preserved.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}

#[cfg(feature = "wasm")]
mod workers {
    use super::*;
    use os_document::Command;
    use os_plugin_host::worker::{JobFailure, JobOutcome, PendingJob, ViewContext};
    use std::time::Instant;

    fn loaded(doc: &Document, prefix: &str) -> (PluginHost, tempfile::TempDir) {
        let directory = installed_column(doc, prefix);
        let mut host = PluginHost::default();
        host.load_wasm_directory(directory.path(), manifest().permissions)
            .unwrap();
        (host, directory)
    }
    fn finish(
        host: &PluginHost,
        job: &mut PendingJob,
        doc: &mut Document,
    ) -> std::result::Result<JobOutcome, JobFailure> {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = host.poll_job(job, doc, None, "Generic worker")? {
                return Ok(result);
            }
            assert!(Instant::now() < until, "worker failed to settle");
            std::thread::park_timeout(Duration::from_millis(1));
        }
    }
    fn drained(host: &PluginHost) {
        let until = Instant::now() + Duration::from_secs(5);
        while host.active_jobs() != 0 {
            assert!(Instant::now() < until, "worker failed to drain");
            std::thread::park_timeout(Duration::from_millis(1));
        }
    }

    #[test]
    fn commit_is_single_consumption_undoable_and_durable_without_plugin() {
        let mut doc = Document::new("Worker column").unwrap();
        let original = doc.model().clone();
        let (mut host, directory) = loaded(&doc, "");
        let mut job = host
            .start_generic_job(OWNER, &doc, create(&doc), None)
            .unwrap();
        assert!(!job.id().0.is_nil());
        assert_eq!(doc.model(), &original);
        assert!(
            host.start_generic_job(OWNER, &doc, create(&doc), None)
                .is_err()
        );
        let invocation = create(&doc);
        assert!(
            host.execute_generic(OWNER, &mut doc, "Busy", invocation)
                .is_err()
        );
        assert!(matches!(
            finish(&host, &mut job, &mut doc),
            Ok(JobOutcome::Committed)
        ));
        assert_eq!(doc.revision(), 1);
        assert_eq!(doc.model().extensions.len(), 1);
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, None, "Replay"),
            Err(JobFailure::AlreadySettled)
        ));
        assert!(doc.undo());
        assert_eq!(doc.model(), &original);
        assert!(doc.redo());
        drained(&host);
        host.unload(OWNER).unwrap();
        let path = directory.path().join("worker.osb");
        ZipJsonStorage.save(&doc, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    }

    #[test]
    fn edits_undo_reopen_and_document_switch_revoke_generic_reply() {
        for case in ["edit", "undo", "reopen", "switch"] {
            let mut doc = Document::new("Stale column").unwrap();
            let (host, directory) = loaded(&doc, "");
            let mut job = host
                .start_generic_job(OWNER, &doc, create(&doc), None)
                .unwrap();
            match case {
                "reopen" => {
                    let path = directory.path().join("reopen.osb");
                    ZipJsonStorage.save(&doc, &path).unwrap();
                    doc = ZipJsonStorage.open(&path).unwrap();
                }
                "switch" => doc = Document::new("Other").unwrap(),
                _ => {
                    doc.execute("Rename", vec![Command::RenameProject("Changed".into())])
                        .unwrap();
                    if case == "undo" {
                        assert!(doc.undo());
                    }
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
    fn view_and_plugin_generation_changes_revoke_generic_reply() {
        let mut doc = Document::new("View column").unwrap();
        let (mut host, directory) = loaded(&doc, "");
        let view = ViewContext {
            id: *doc.model().views.keys().next().unwrap(),
            settings_revision: 0,
        };
        for settings_revision in [1, u64::MAX] {
            assert!(
                host.start_generic_job(
                    OWNER,
                    &doc,
                    create(&doc),
                    Some(ViewContext {
                        settings_revision,
                        ..view
                    }),
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
            let mut job = host
                .start_generic_job(OWNER, &doc, create(&doc), Some(view))
                .unwrap();
            assert!(matches!(
                host.poll_job(&mut job, &mut doc, current, "Stale view"),
                Err(JobFailure::StaleView)
            ));
            drained(&host);
        }
        assert!(
            host.start_generic_job(
                OWNER,
                &doc,
                create(&doc),
                Some(ViewContext {
                    id: Id::new(),
                    ..view
                })
            )
            .is_err()
        );
        let mut job = host
            .start_generic_job(OWNER, &doc, create(&doc), None)
            .unwrap();
        host.unload(OWNER).unwrap();
        host.load_wasm_directory(directory.path(), manifest().permissions)
            .unwrap();
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, None, "Reload"),
            Err(JobFailure::PluginUnloaded)
        ));
        drained(&host);
        assert_eq!(doc.revision(), 0);
        let mut fresh = host
            .start_generic_job(OWNER, &doc, create(&doc), None)
            .unwrap();
        assert!(matches!(
            finish(&host, &mut fresh, &mut doc),
            Ok(JobOutcome::Committed)
        ));
        drained(&host);
    }

    #[test]
    fn cancelled_dropped_trapping_and_fuel_exhausted_workers_drain_without_edits() {
        for prefix in [
            "",
            "unreachable",
            "(loop $forever br $forever)",
            "i32.const 8192 i32.const 120 i32.store8",
        ] {
            let mut doc = Document::new("Revoked column").unwrap();
            let (host, _directory) = loaded(&doc, prefix);
            let mut cancelled = host
                .start_generic_job(OWNER, &doc, create(&doc), None)
                .unwrap();
            cancelled.cancel();
            assert!(matches!(
                host.poll_job(&mut cancelled, &mut doc, None, "Cancelled"),
                Err(JobFailure::Cancelled)
            ));
            drained(&host);
            let dropped = host
                .start_generic_job(OWNER, &doc, create(&doc), None)
                .unwrap();
            drop(dropped);
            drained(&host);
            assert_eq!(doc.revision(), 0);
            assert!(doc.model().extensions.is_empty());
            assert!(!doc.can_undo());
            let mut fresh = host
                .start_generic_job(OWNER, &doc, create(&doc), None)
                .unwrap();
            let result = finish(&host, &mut fresh, &mut doc);
            if prefix.is_empty() {
                assert!(matches!(result, Ok(JobOutcome::Committed)));
            } else {
                assert!(matches!(result, Err(JobFailure::Rejected(_))));
                assert_eq!(doc.revision(), 0);
            }
            drained(&host);
        }
    }

    #[test]
    fn expired_generic_reply_cannot_commit_and_releases_its_slot() {
        let mut doc = Document::new("Expired column").unwrap();
        let (host, _directory) = loaded(&doc, "");
        let mut invocation = create(&doc);
        invocation.timeout = Duration::from_secs(1);
        let mut job = host
            .start_generic_job(OWNER, &doc, invocation, None)
            .unwrap();
        let until = Instant::now() + Duration::from_secs(1);
        while Instant::now() < until {
            std::thread::park_timeout(Duration::from_millis(1));
        }
        assert_eq!(host.active_jobs(), 1, "unread reply retains its slot");
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, None, "Expired"),
            Err(JobFailure::Deadline)
        ));
        drained(&host);
        assert_eq!(doc.revision(), 0);
        assert!(doc.model().extensions.is_empty());
        assert!(!doc.can_undo());
    }

    #[test]
    fn builtins_and_invalid_inputs_cannot_dispatch_generic_workers() {
        let doc = Document::new("Authorization").unwrap();
        let (builtin, calls, _) = host(true);
        assert!(
            builtin
                .start_generic_job(OWNER, &doc, create(&doc), None)
                .is_err()
        );
        assert_eq!(calls.borrow().len(), 1, "only Describe was invoked");
        let (host, _directory) = loaded(&doc, "");
        let mut invocation = create(&doc);
        invocation.scope.create_count = 0;
        assert!(
            host.start_generic_job(OWNER, &doc, invocation, None)
                .is_err()
        );
        assert_eq!(host.active_jobs(), 0);
        assert_eq!(doc.revision(), 0);
    }
}
