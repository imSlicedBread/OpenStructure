use os_core::Result;
use os_document::Document;
use os_plugin_api::{Manifest, Permission, Plugin, generic as wire};
use os_plugin_host::{
    PluginHost,
    generic::{Invocation, Scope},
};
use os_storage::{StorageBackend, ZipJsonStorage};
use std::{collections::BTreeMap, time::Duration};
struct Migrator {
    fault: &'static str,
    explicit: bool,
}
impl Plugin for Migrator {
    fn manifest(&self) -> Manifest {
        let mut m =
            Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml"))
                .unwrap();
        m.version = "2.0.0".into();
        m
    }
    fn invoke_json(&self, input: &str) -> Result<String> {
        let request: wire::Request = serde_json::from_str(input).unwrap();
        let reply = match request.operation {
            wire::Operation::Describe => {
                let mut catalog: wire::Catalog = serde_json::from_str(include_str!(
                    "../../../fixtures/generic-column/catalog.json"
                ))
                .unwrap();
                catalog.types[0].payload_schema_version = 2;
                catalog.commands[1].mode = if self.explicit {
                    wire::Mode::Migrate
                } else {
                    wire::Mode::Edit
                };
                catalog.commands[1].fields.clear();
                wire::Reply::Catalog(catalog)
            }
            wire::Operation::Invoke {
                snapshot,
                selection,
                ..
            } => {
                let mut edits = Vec::new();
                for mut element in snapshot
                    .elements
                    .into_iter()
                    .filter(|e| selection.contains(&e.id))
                {
                    let expected_schema_version = element.payload_schema_version;
                    element.payload_schema_version = 2;
                    let payload = element.payload.as_object_mut().unwrap();
                    payload.insert("width".into(), serde_json::json!(0.4));
                    payload.insert("depth".into(), serde_json::json!(0.6));
                    payload.insert("height".into(), serde_json::json!(3.0));
                    if self.fault == "rename" {
                        element.name = "Renamed".into();
                    }
                    if self.fault == "schema" {
                        element.payload_schema_version = 3;
                    }
                    edits.push(wire::Edit::Replace {
                        expected_schema_version,
                        element,
                    });
                }
                if self.fault == "partial" {
                    edits.pop();
                }
                wire::Reply::Edits(edits)
            }
            _ => panic!("unexpected operation"),
        };
        Ok(serde_json::to_string(&wire::Response {
            api_version: 2,
            request_id: request.request_id,
            context: request.context,
            result: reply,
        })
        .unwrap())
    }
}
fn fixture(fault: &'static str, explicit: bool) -> (PluginHost, Document, Invocation) {
    let mut host = PluginHost::default();
    host.load(
        Box::new(Migrator { fault, explicit }),
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ]
        .into(),
    )
    .unwrap();
    let mut model = os_model::Model::new("Migration");
    let entity: os_model::ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    model.plugin_requirements.insert(
        entity.owner.clone(),
        os_model::PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(entity.id, entity.clone());
    let mut second = entity;
    second.id = os_core::Id::new();
    model.extensions.insert(second.id, second);
    let ids = model.extensions.keys().copied().collect();
    let invocation = Invocation {
        command_id: "org.example.columns.edit".into(),
        inputs: BTreeMap::new(),
        scope: Scope {
            read: ids,
            write: model.extensions.keys().copied().collect(),
            create_count: 0,
        },
        timeout: Duration::from_secs(5),
    };
    (host, Document::from_model(model).unwrap(), invocation)
}
#[test]
fn explicit_migration_updates_payloads_and_requirement_in_one_undoable_commit() {
    let (host, mut doc, invocation) = fixture("", true);
    let before = doc.model().clone();
    host.execute_generic("org.example.columns", &mut doc, "Migrate", invocation)
        .unwrap();
    assert_eq!(doc.revision(), 1);
    assert_eq!(
        doc.model().plugin_requirements["org.example.columns"].version,
        "2.0.0"
    );
    for (id, entity) in &doc.model().extensions {
        assert_eq!(entity.payload_schema_version, 2);
        for (key, value) in before.extensions[id].payload.as_object().unwrap() {
            assert_eq!(entity.payload.get(key), Some(value));
        }
        assert!(!host.extension_needs_migration(doc.model(), *id));
    }
    let migrated = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &migrated);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("migrated.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &migrated);
}
#[test]
fn invalid_partial_and_unrequested_migrations_leave_original_unchanged() {
    for (fault, explicit) in [
        ("partial", true),
        ("rename", true),
        ("schema", true),
        ("", false),
    ] {
        let (host, mut doc, invocation) = fixture(fault, explicit);
        let before = doc.model().clone();
        assert!(
            host.execute_generic("org.example.columns", &mut doc, "Migrate", invocation)
                .is_err()
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
    }
}
#[test]
fn partial_owner_authorization_is_not_a_migration() {
    let (host, mut doc, mut invocation) = fixture("", true);
    invocation.scope.write.pop_first();
    let before = doc.model().clone();
    assert!(
        host.execute_generic("org.example.columns", &mut doc, "Migrate", invocation)
            .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.revision(), 0);
}
