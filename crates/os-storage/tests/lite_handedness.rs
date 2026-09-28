mod common;
use os_document::{Command, Document};
use os_model::{
    LiteSide::{End, Start},
    Model, SCHEMA_VERSION,
};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-50-lite-handedness.json")).unwrap()
}

#[test]
fn lite_side_frozen_50_migration_only_adds_null_and_advances_all_headers() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 50).unwrap();
    let mut expected = original;
    common::apply_wall_paths(&mut expected);
    expected["schema_version"] = SCHEMA_VERSION.into();
    for (key, data) in expected.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
                if key == "openings" {
                    entity["parameters"]["lite_side_override"] = Value::Null;
                } else if key == "opening_types" {
                    entity["parameters"]["window_operation"] = "Fixed".into();
                }
            }
        }
    }
    assert_eq!(value, expected);
    let mut model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let ids: Vec<_> = model
        .openings
        .values()
        .filter(|o| o.parameters.type_id().is_some())
        .map(|o| o.id())
        .collect();
    assert!(!ids.is_empty());
    let dir = tempfile::tempdir().unwrap();
    for id in ids {
        let ty = model.openings[&id].parameters.type_id().unwrap();
        // This frozen fixture contains one typed Window (RightFace, sill 0.8 m).
        // Keep it a Window through type edits; valid Door coverage is in os-model.
        for default in [Start, End] {
            model
                .opening_types
                .get_mut(&ty)
                .unwrap()
                .parameters
                .family
                .side_lite
                .as_mut()
                .unwrap()
                .side = default;
            for pin in [None, Some(Start), Some(End)] {
                model
                    .openings
                    .get_mut(&id)
                    .unwrap()
                    .parameters
                    .lite_side_override = pin;
                let doc = Document::from_model(model.clone()).unwrap();
                let path = dir.path().join("lite.osb");
                ZipJsonStorage.save(&doc, &path).unwrap();
                let mut reopened = ZipJsonStorage.open(&path).unwrap();
                assert_eq!(reopened.model(), doc.model());
                let mut parameters = model.opening_types[&ty].parameters.clone();
                let changed = if default == Start { End } else { Start };
                parameters.family.side_lite.as_mut().unwrap().side = changed;
                reopened
                    .execute(
                        "Type side",
                        vec![Command::UpdateOpeningType { id: ty, parameters }],
                    )
                    .unwrap();
                assert_eq!(
                    reopened
                        .model()
                        .resolve_opening(&reopened.model().openings[&id].parameters)
                        .unwrap()
                        .family
                        .side_lite
                        .unwrap()
                        .side,
                    pin.unwrap_or(changed)
                );
                let updated = reopened.model().clone();
                assert!(reopened.undo());
                assert_eq!(reopened.model(), doc.model());
                assert!(reopened.redo());
                assert_eq!(reopened.model(), &updated);
            }
        }
    }
}

#[test]
fn lite_side_migration_and_current_wire_reject_malformed_data_atomically() {
    for case in 0..8 {
        let mut value = frozen();
        let id = value["openings"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        match case {
            0 => value["openings"][&id]["parameters"]["lite_side_override"] = Value::Null,
            1 => value["openings"][&id]["parameters"]["lite_side_override"] = json!("Start"),
            2 => value["openings"][&id]["parameters"] = json!([]),
            3 => value["openings"][&id]["header"]["schema_version"] = 49.into(),
            4 => value["project"]["header"]["schema_version"] = 51.into(),
            5 => value["walls"] = json!([]),
            6 => {
                value.as_object_mut().unwrap().remove("phases");
            }
            _ => value["schema_version"] = 49.into(),
        }
        let before = value.clone();
        assert!(migrate(&mut value, 50).is_err(), "case {case}");
        assert_eq!(value, before);
    }
    let mut current = frozen();
    migrate(&mut current, 50).unwrap();
    let id = current["openings"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    for bad in [None, Some(json!("Left")), Some(json!(0)), Some(json!({}))] {
        let mut value = current.clone();
        let params = value["openings"][&id]["parameters"]
            .as_object_mut()
            .unwrap();
        if let Some(bad) = bad {
            params.insert("lite_side_override".into(), bad);
        } else {
            params.remove("lite_side_override");
        }
        let before = value.clone();
        assert!(migrate(&mut value, SCHEMA_VERSION).is_err());
        assert_eq!(value, before);
    }
}
