use os_document::{Command, Document};
use os_model::{ClearanceEnd, Model, OpeningClearance};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
mod common;

fn old_model() -> serde_json::Value {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-53-straight-walls.json")).unwrap();
    migrate(&mut value, 53).unwrap();
    common::remove_door_leaves(&mut value);
    for opening in value["openings"].as_object_mut().unwrap().values_mut() {
        opening["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("open_state");
    }
    value.as_object_mut().unwrap().remove("opening_clearances");
    value.as_object_mut().unwrap().remove("length_parameters");
    value
        .as_object_mut()
        .unwrap()
        .remove("opening_type_length_bindings");
    value["schema_version"] = 54.into();
    for (name, data) in value.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = 54.into();
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                entity["header"]["schema_version"] = 54.into();
            }
        }
    }
    value
}

#[test]
fn clearance_lock_schema54_migration_and_save_reopen() {
    let mut value = old_model();
    let openings = value["openings"].clone();
    migrate(&mut value, 54).unwrap();
    assert!(value["opening_clearances"].as_object().unwrap().is_empty());
    for (id, opening) in openings.as_object().unwrap() {
        let mut expected = opening["parameters"].clone();
        expected["open_state"] = "Default".into();
        assert_eq!(value["openings"][id]["parameters"], expected);
    }
    let model: Model = serde_json::from_value(value).unwrap();
    let mut doc = Document::from_model(model).unwrap();
    let opening = doc
        .model()
        .openings
        .values()
        .next()
        .expect("frozen fixture opening");
    let id = opening.id();
    let distance = opening.parameters.offset;
    doc.execute(
        "Lock",
        vec![Command::SetOpeningClearance {
            id,
            clearance: Some(OpeningClearance {
                end: ClearanceEnd::Start,
                distance,
            }),
        }],
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("locked.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let opened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(opened.model(), doc.model());
}

#[test]
fn clearance_lock_ambiguous_migration_and_invalid_current_reference_are_rejected() {
    let mut old = old_model();
    old["opening_clearances"] = serde_json::json!({});
    let before = old.clone();
    assert!(migrate(&mut old, 54).is_err());
    assert_eq!(old, before);
    let mut current = old_model();
    migrate(&mut current, 54).unwrap();
    current["opening_clearances"][os_core::Id::new().to_string()] =
        serde_json::json!({"end":"End", "distance":1.});
    let model: Model = serde_json::from_value(current).unwrap();
    assert!(model.validate().is_err());
}
