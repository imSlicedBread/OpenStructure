mod common;

use os_document::Document;
use os_model::{Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

const FIELDS: [&str; 3] = ["floor_material", "wall_material", "ceiling_material"];
const ROOM: &str = "00000000-0000-4000-8000-000000000040";

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-34-room-materials.json")).unwrap()
}

#[test]
fn room_materials_frozen_schema34_preservation_and_save_reopen() {
    let old = frozen();
    let mut value = old.clone();
    migrate(&mut value, 34).unwrap();
    assert_eq!(SCHEMA_VERSION, 45);
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    let mut reverse = value.clone();
    reverse["schema_version"] = json!(34);
    for (key, data) in reverse.as_object_mut().unwrap() {
        if key == "project" {
            assert_eq!(data["header"]["schema_version"], SCHEMA_VERSION);
            data["header"]["schema_version"] = json!(34);
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    assert_eq!(entity["header"]["schema_version"], SCHEMA_VERSION);
                    entity["header"]["schema_version"] = json!(34);
                }
                if key == "rooms" {
                    for field in FIELDS {
                        assert_eq!(
                            entity["parameters"].as_object_mut().unwrap().remove(field),
                            Some(Value::Null)
                        );
                    }
                } else if key == "floors" {
                    entity["parameters"]
                        .as_object_mut()
                        .unwrap()
                        .remove("holes");
                } else if key == "schedules" {
                    assert_eq!(
                        entity["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("filters"),
                        Some(json!([]))
                    );
                }
            }
        }
    }
    reverse.as_object_mut().unwrap().remove("stairs");
    reverse.as_object_mut().unwrap().remove("roofs");
    common::reverse_schema_38_migration(&mut reverse, &old);
    assert_eq!(
        reverse, old,
        "only native versions and nullable refs may change"
    );
    let mut model: Model = serde_json::from_value(value).unwrap();
    let ids: Vec<_> = model.materials.keys().copied().collect();
    let room = &mut model.rooms.values_mut().next().unwrap().parameters;
    room.floor_material = Some(ids[0]);
    room.wall_material = Some(ids[1]);
    room.ceiling_material = Some(ids[0]);
    assert_eq!(room.floor_finish.as_deref(), Some("F-01"));
    assert_eq!(room.wall_finish, None);
    let doc = Document::from_model(model).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("room-materials.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}

#[test]
fn room_materials_migration_rejects_ambiguous_malformed_and_missing_fields_atomically() {
    for field in FIELDS {
        for existing in [
            Value::Null,
            json!("12345600-0000-4000-8000-000000000051"),
            json!(false),
        ] {
            let mut bad = frozen();
            bad["rooms"][ROOM]["parameters"][field] = existing;
            let before = bad.clone();
            assert!(migrate(&mut bad, 34).is_err());
            assert_eq!(bad, before);
        }
    }
    for case in 0..7 {
        let mut bad = frozen();
        match case {
            0 => bad["rooms"][ROOM]["parameters"] = Value::Null,
            1 => bad["rooms"][ROOM]["parameters"]["unknown"] = json!(1),
            2 => bad["rooms"][ROOM]["header"]["schema_version"] = json!(33),
            3 => bad["rooms"][ROOM]["header"]["id"] = json!("bad"),
            4 => {
                bad["materials"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"]["color"] = json!([256, 0, 0])
            }
            5 => {
                bad.as_object_mut().unwrap().remove("rooms");
            }
            _ => {
                bad["schedules"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["header"]["schema_version"] = json!(33)
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 34).is_err(), "case {case}");
        assert_eq!(bad, before);
    }
    let mut current = frozen();
    migrate(&mut current, 34).unwrap();
    for field in FIELDS {
        for case in 0..4 {
            let mut bad = current.clone();
            let params = bad["rooms"][ROOM]["parameters"].as_object_mut().unwrap();
            match case {
                0 => {
                    params.remove(field);
                }
                1 => {
                    params.insert(field.into(), json!("bad UUID"));
                }
                2 => {
                    params.insert(field.into(), json!("00000000-0000-4000-8000-000000000099"));
                }
                _ => {
                    params.insert(field.into(), json!([]));
                }
            }
            let before = bad.clone();
            assert!(migrate(&mut bad, 35).is_err());
            assert_eq!(bad, before);
        }
    }
}
