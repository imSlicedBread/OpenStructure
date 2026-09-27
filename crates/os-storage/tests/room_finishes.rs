mod common;

use os_document::Document;
use os_model::{Model, SCHEMA_VERSION, Schedule, ScheduleCategory, ScheduleParams};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-32-room-finishes.json")).unwrap()
}

#[test]
fn room_finishes_frozen_schema32_migration_preserves_intent_and_round_trips() {
    let old = frozen();
    let mut value = old.clone();
    migrate(&mut value, 32).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    let mut reverse = value.clone();
    reverse["schema_version"] = json!(32);
    for (key, value) in reverse.as_object_mut().unwrap() {
        if key == "project" {
            value["header"]["schema_version"] = json!(32);
        } else if key != "extensions"
            && let Some(map) = value.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = json!(32);
                }
                if key == "rooms" {
                    for field in [
                        "floor_finish",
                        "wall_finish",
                        "ceiling_finish",
                        "floor_material",
                        "wall_material",
                        "ceiling_material",
                    ] {
                        assert_eq!(
                            entity["parameters"].as_object_mut().unwrap().remove(field),
                            Some(Value::Null)
                        );
                    }
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
            }
        }
    }
    reverse.as_object_mut().unwrap().remove("stairs");
    reverse.as_object_mut().unwrap().remove("roofs");
    common::reverse_schema_38_migration(&mut reverse, &old);
    assert_eq!(reverse, old);
    let mut model: Model = serde_json::from_value(value).unwrap();
    let room = model.rooms.values_mut().next().unwrap();
    room.parameters.floor_finish = Some("F-01".into());
    room.parameters.wall_finish = Some("W-02".into());
    room.parameters.ceiling_finish = Some("C-03".into());
    let schedule = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Room Finishes", ScheduleCategory::RoomFinish),
    );
    model.schedules.insert(schedule.id(), schedule);
    let document = Document::from_model(model).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("finishes.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    assert_eq!(
        ZipJsonStorage.open(&path).unwrap().model(),
        document.model()
    );
}

#[test]
fn room_finishes_migration_rejects_ambiguous_invalid_and_missing_fields_atomically() {
    for case in 0..8 {
        let mut value = frozen();
        let room = value["rooms"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        match case {
            0..=2 => {
                room["parameters"][["floor_finish", "wall_finish", "ceiling_finish"][case]] =
                    Value::Null
            }
            3 => room["header"]["schema_version"] = json!(31),
            4 => room["parameters"] = json!(false),
            5 => room["parameters"]["unexpected"] = json!("x"),
            6 => {
                value.as_object_mut().unwrap().remove("rooms");
            }
            _ => {
                value["schedules"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"]["category"] = json!("RoomFinish")
            }
        }
        let original = value.clone();
        assert!(migrate(&mut value, 32).is_err(), "case {case}");
        assert_eq!(value, original);
    }
    let mut current = frozen();
    migrate(&mut current, 32).unwrap();
    for field in ["floor_finish", "wall_finish", "ceiling_finish"] {
        let mut bad = current.clone();
        bad["rooms"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()["parameters"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        let original = bad.clone();
        assert!(migrate(&mut bad, SCHEMA_VERSION).is_err());
        assert_eq!(bad, original);
    }
}
