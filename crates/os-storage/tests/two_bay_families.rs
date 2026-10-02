use os_model::{Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
mod common;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-49-opening-families.json")).unwrap()
}

#[test]
fn two_bay_frozen_49_only_adds_default_and_advances_versions() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 49).unwrap();
    let mut expected = original;
    common::apply_wall_paths(&mut expected);
    common::apply_lite_override(&mut expected);
    expected["schema_version"] = SCHEMA_VERSION.into();
    for (key, data) in expected.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
            }
        }
    }
    for ty in expected["opening_types"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        ty["parameters"]["window_operation"] = "Fixed".into();
        ty["parameters"]["family"]["version"] = 6.into();
        ty["parameters"]["family"]["side_lite"] = Value::Null;
    }
    assert_eq!(value, expected);
    let model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let document = os_document::Document::from_model(model.clone()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("families.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);
}

#[test]
fn two_bay_malformed_migration_is_strict_and_atomic() {
    for case in 0..10 {
        let mut value = frozen();
        let ty = value["opening_types"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        let family = &mut value["opening_types"][&ty]["parameters"]["family"];
        match case {
            0 => family["side_lite"] = Value::Null,
            1 => family["side_lite"] = json!({}),
            2 => family["version"] = 5.into(),
            3 => family["version"] = 3.into(),
            4 => *family = Value::Null,
            5 => {
                family.as_object_mut().unwrap().remove("panel_material");
            }
            6 => family["surprise"] = true.into(),
            7 => value["project"]["header"]["schema_version"] = 48.into(),
            8 => {
                value.as_object_mut().unwrap().remove("walls");
            }
            _ => value["schema_version"] = 48.into(),
        }
        let before = value.clone();
        assert!(migrate(&mut value, 49).is_err(), "case {case}");
        assert_eq!(value, before);
    }
    let mut current = frozen();
    migrate(&mut current, 49).unwrap();
    let ty = current["opening_types"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    current["opening_types"][&ty]["parameters"]["family"]
        .as_object_mut()
        .unwrap()
        .remove("side_lite");
    let before = current.clone();
    assert!(migrate(&mut current, 50).is_err());
    assert_eq!(current, before);
}
