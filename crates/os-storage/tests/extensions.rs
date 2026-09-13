use os_core::Id;
use os_document::{Command, Document};
use os_model::{ExtensionEntity, Model, PluginRequirement};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::json;
use std::collections::BTreeMap;

fn document() -> Document {
    let mut model = Model::new("Preserved plugin data");
    let mut entity: ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    entity
        .depends_on
        .insert(*model.levels.keys().next().unwrap());
    model.plugin_requirements.insert(
        entity.owner.clone(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(entity.id, entity);
    Document::from_model_and_files(
        model,
        BTreeMap::from([("assets/opaque.bin".into(), vec![0, 255, 17])]),
    )
    .unwrap()
}

#[test]
fn missing_plugin_round_trip_survives_core_edits_and_history() {
    let mut doc = document();
    let original = doc.model().clone();
    doc.execute("Rename", vec![Command::RenameProject("Renamed".into())])
        .unwrap();
    assert!(doc.undo());
    assert_eq!(doc.model(), &original);
    assert!(doc.redo());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preserved.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(doc.model(), reopened.model());
    assert_eq!(
        reopened.auxiliary_files()["assets/opaque.bin"],
        [0, 255, 17]
    );
    assert_eq!(original.extensions, reopened.model().extensions);
    assert_eq!(
        original.plugin_requirements,
        reopened.model().plugin_requirements
    );
}

#[test]
fn unknown_payload_numbers_keep_arbitrary_precision_across_storage() {
    let mut doc = document();
    let mut entity = doc.model().extensions.values().next().unwrap().clone();
    let payload = r#"{"integer":184467440737095516170,"decimal":0.123456789012345678901234567890,"huge":1e400}"#;
    entity.payload = serde_json::from_str(payload).unwrap();
    let expected = entity.payload.clone();
    let id = entity.id;
    doc.execute(
        "Opaque values",
        vec![Command::ReplaceExtension {
            expected_schema_version: 1,
            entity,
        }],
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("precise.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().extensions[&id].payload, expected);
    assert_eq!(expected["integer"].to_string(), "184467440737095516170");
    assert_eq!(
        expected["decimal"].to_string(),
        "0.123456789012345678901234567890"
    );
    assert!(expected["huge"].as_f64().is_none());
}

#[test]
fn migration_proposals_are_atomic_undoable_and_keep_identity_and_unknown_payload() {
    let mut doc = document();
    let original = doc.model().clone();
    let mut e = original.extensions.values().next().unwrap().clone();
    let id = e.id;
    e.payload_schema_version = 2;
    let width = e
        .payload
        .as_object_mut()
        .unwrap()
        .remove("width_mm")
        .unwrap();
    e.payload["width_metres"] = json!(width.as_f64().unwrap() / 1000.0);
    let commands = vec![
        Command::ReplaceExtension {
            expected_schema_version: 1,
            entity: e.clone(),
        },
        Command::SetPluginRequirement {
            plugin_id: e.owner.clone(),
            requirement: Some(PluginRequirement {
                version: "2.0.0".into(),
            }),
        },
    ];
    let mut bad = commands.clone();
    bad.push(Command::RemoveWall(Id::new()));
    assert!(doc.execute("Rejected migration", bad).is_err());
    assert_eq!(doc.model(), &original);
    assert_eq!(doc.revision(), 0);
    assert!(!doc.can_undo());
    assert!(doc.drain_events().is_empty());
    doc.execute("Migrate column payload", commands).unwrap();
    let migrated = doc.model().clone();
    assert_eq!(
        migrated.extensions[&id].payload["unknown_vendor_data"],
        original.extensions[&id].payload["unknown_vendor_data"]
    );
    assert_eq!(migrated.extensions[&id].payload["width_metres"], json!(0.4));
    assert!(
        doc.execute(
            "Stale migration",
            vec![Command::ReplaceExtension {
                expected_schema_version: 1,
                entity: e
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &migrated);
    assert!(doc.undo());
    assert_eq!(doc.model(), &original);
    assert!(doc.redo());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("migrated.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &migrated);
}

#[test]
fn deletion_and_dependency_removal_require_a_valid_final_graph() {
    let mut doc = document();
    let e = doc.model().extensions.values().next().unwrap().clone();
    let level = *e.depends_on.first().unwrap();
    assert!(
        doc.execute("Delete host", vec![Command::RemoveLevel(level)])
            .is_err()
    );
    assert!(
        doc.execute(
            "Remove requirement",
            vec![Command::SetPluginRequirement {
                plugin_id: e.owner.clone(),
                requirement: None
            }]
        )
        .is_err()
    );
    let mut changed = e.clone();
    changed.owner = "org.other.columns".into();
    assert!(
        doc.execute(
            "Change owner",
            vec![Command::ReplaceExtension {
                expected_schema_version: 1,
                entity: changed
            }]
        )
        .is_err()
    );
    doc.execute(
        "Explicit removal",
        vec![
            Command::RemoveExtension(e.id),
            Command::SetPluginRequirement {
                plugin_id: e.owner,
                requirement: None,
            },
            Command::RemoveLevel(level),
        ],
    )
    .unwrap();
    assert!(doc.model().extensions.is_empty());
    assert!(doc.undo());
    assert!(doc.model().extensions.contains_key(&e.id));
}

#[test]
fn extension_reference_invalidation_is_transitive_and_undo_aware() {
    let mut doc = document();
    let first = doc.model().extensions.values().next().unwrap().clone();
    let mut second = first.clone();
    second.id = Id::new();
    second.depends_on = [first.id].into();
    let second_id = second.id;
    doc.execute("Dependent", vec![Command::AddExtension(second)])
        .unwrap();
    doc.drain_events();
    let level = *first.depends_on.first().unwrap();
    let mut parameters = doc.model().levels[&level].parameters.clone();
    parameters.elevation = 4.0;
    doc.execute(
        "Raise",
        vec![Command::UpdateLevel {
            id: level,
            parameters,
        }],
    )
    .unwrap();
    for event in doc.drain_events() {
        assert_eq!(event.invalidated, [first.id, second_id].into());
    }
    doc.undo();
    assert_eq!(
        doc.drain_events()[0].invalidated,
        [first.id, second_id].into()
    );
}

#[test]
fn native_migration_failure_preserves_input_and_rejects_ambiguous_old_data() {
    for case in ["bad header", "ambiguous extension", "unknown root"] {
        let mut value: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/schema-0-model.json")).unwrap();
        match case {
            "bad header" => value["project"]["header"] = json!(null),
            "ambiguous extension" => value["extensions"] = json!({}),
            "unknown root" => value["unrecognized_root"] = json!({"preserve": true}),
            _ => unreachable!(),
        }
        let original = value.clone();
        assert!(migrate(&mut value, 0).is_err());
        assert_eq!(value, original);
    }
}

#[test]
fn malformed_extension_mutations_are_panic_free_and_never_partially_migrate() {
    let source = document();
    let original = serde_json::to_value(source.model()).unwrap();
    let id = source.model().extensions.keys().next().unwrap().to_string();
    for field in [
        "id",
        "owner",
        "type_id",
        "envelope_version",
        "payload_schema_version",
        "relationships",
        "depends_on",
        "payload",
    ] {
        for mutation in [
            json!(null),
            json!(-1),
            json!("unexpected"),
            json!([]),
            json!({}),
        ] {
            let mut candidate = original.clone();
            candidate["extensions"][&id][field] = mutation;
            let before = candidate.clone();
            if migrate(&mut candidate, os_model::SCHEMA_VERSION).is_err() {
                assert_eq!(candidate, before);
            } else {
                serde_json::from_value::<Model>(candidate)
                    .unwrap()
                    .validate()
                    .unwrap();
            }
        }
    }
}
