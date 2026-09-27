mod common;

use os_document::Document;
use os_model::{Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-33-material-colors.json")).unwrap()
}

#[test]
fn material_color_schema33_frozen_migration_and_save_reopen() {
    let old = frozen();
    let mut value = old.clone();
    migrate(&mut value, 33).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    // Independently frozen expected bytes from SurfaceIdentity::color(), not the helper.
    assert_eq!(
        model
            .materials
            .values()
            .map(|m| m.parameters.color)
            .collect::<Vec<_>>(),
        vec![[158, 192, 146], [155, 140, 140]]
    );
    let mut reverse = value;
    reverse["schema_version"] = json!(33);
    for (key, data) in reverse.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = json!(33);
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = json!(33);
                }
                if key == "materials" {
                    entity["parameters"]
                        .as_object_mut()
                        .unwrap()
                        .remove("color");
                }
                if key == "schedules" {
                    assert_eq!(
                        entity["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("filters"),
                        Some(json!([]))
                    );
                }
                if key == "rooms" {
                    for field in ["floor_material", "wall_material", "ceiling_material"] {
                        assert_eq!(
                            entity["parameters"].as_object_mut().unwrap().remove(field),
                            Some(Value::Null)
                        );
                    }
                }
            }
        }
    }
    reverse.as_object_mut().unwrap().remove("stairs");
    reverse.as_object_mut().unwrap().remove("roofs");
    common::reverse_schema_38_migration(&mut reverse, &old);
    assert_eq!(
        reverse, old,
        "identity, assignments, density, finish strings and extensions preserved"
    );
    let doc = Document::from_model(model).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("colors.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}

#[test]
fn material_color_migration_rejects_ambiguous_and_malformed_data_atomically() {
    let old = frozen();
    let ids: Vec<_> = old["materials"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    for color in [
        Value::Null,
        json!([1, 2, 3]),
        json!([1, 2]),
        json!("red"),
        json!([256, 0, 0]),
    ] {
        let mut bad = old.clone();
        // Fail on the second material after the first would have been migrated.
        bad["materials"][&ids[1]]["parameters"]["color"] = color;
        let before = bad.clone();
        assert!(migrate(&mut bad, 33).is_err());
        assert_eq!(bad, before);
    }
    for case in 0..5 {
        let mut bad = old.clone();
        match case {
            0 => bad["materials"][&ids[1]]["header"]["id"] = json!("bad-id"),
            1 => bad["materials"][&ids[1]]["header"]["schema_version"] = json!(32),
            2 => bad["materials"][&ids[1]]["parameters"] = Value::Null,
            3 => bad["materials"][&ids[1]]["header"]["id"] = json!(ids[0]),
            _ => {
                bad.as_object_mut().unwrap().remove("materials");
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 33).is_err());
        assert_eq!(bad, before);
    }
    let mut current = old;
    migrate(&mut current, 33).unwrap();
    for color in [
        Value::Null,
        json!([1, 2]),
        json!([1, 2, 3, 4]),
        json!([-1, 0, 0]),
        json!([256, 0, 0]),
        json!([1.5, 0, 0]),
        json!("#ffffff"),
        json!({"r":1,"g":2,"b":3}),
    ] {
        let mut bad = current.clone();
        bad["materials"][&ids[0]]["parameters"]["color"] = color;
        let before = bad.clone();
        assert!(migrate(&mut bad, SCHEMA_VERSION).is_err());
        assert_eq!(bad, before);
    }
    current["materials"][&ids[0]]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("color");
    let before = current.clone();
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
    assert_eq!(current, before);
}
