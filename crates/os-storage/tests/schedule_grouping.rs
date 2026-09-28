use os_document::Document;
use os_model::{Model, SCHEMA_VERSION, ScheduleCategory, ScheduleGroupField, ScheduleParams};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
mod common;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-46-schedule-grouping.json")).unwrap()
}

#[test]
fn frozen_46_grouping_defaults_preserve_every_other_value_and_reopen() {
    let original = frozen();
    let mut migrated = original.clone();
    migrate(&mut migrated, 46).unwrap();
    let mut expected = original;
    common::apply_wall_paths(&mut expected);
    common::apply_opening_visibility(&mut expected);
    expected["schema_version"] = SCHEMA_VERSION.into();
    for (key, data) in expected.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
                if key == "schedules" {
                    entity["parameters"]["group_by"] = json!([]);
                    entity["parameters"]["phase"] = json!({"mode": "legacy_unphased"});
                }
            }
        }
    }
    assert_eq!(migrated, expected);
    let mut model: Model = serde_json::from_value(migrated).unwrap();
    assert!(
        model
            .schedules
            .values()
            .all(|s| s.parameters.group_by.is_empty())
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grouped.osb");
    for keys in [
        vec![],
        vec![ScheduleGroupField::Type],
        vec![ScheduleGroupField::Level, ScheduleGroupField::Width],
    ] {
        for schedule in model.schedules.values_mut() {
            schedule.parameters.group_by = keys.clone();
        }
        let document = Document::from_model(model.clone()).unwrap();
        ZipJsonStorage.save(&document, &path).unwrap();
        assert_eq!(
            ZipJsonStorage.open(&path).unwrap().model(),
            document.model()
        );
    }
}

#[test]
fn grouping_migration_rejects_ambiguity_bad_headers_and_missing_current_field_atomically() {
    let original = frozen();
    let id = original["schedules"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    for case in 0..6 {
        let mut bad = original.clone();
        match case {
            0 => bad["schedules"][id]["parameters"]["group_by"] = json!([]),
            1 => bad["schedules"][id]["parameters"] = Value::Null,
            2 => bad["schedules"][id]["header"]["schema_version"] = 45.into(),
            3 => bad["project"]["header"]["schema_version"] = 45.into(),
            4 => bad["schedules"] = json!([]),
            _ => {
                bad.as_object_mut().unwrap().remove("phases");
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 46).is_err());
        assert_eq!(bad, before);
    }
    let mut current = original.clone();
    migrate(&mut current, 46).unwrap();
    current["schedules"][id]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("group_by");
    let before = current.clone();
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
    assert_eq!(current, before);
    let mut params =
        serde_json::to_value(ScheduleParams::new("Rooms", ScheduleCategory::RoomFinish)).unwrap();
    params["group_by"] = json!(["kind"]);
    assert!(
        serde_json::from_value::<ScheduleParams>(params)
            .unwrap()
            .validate()
            .is_err()
    );
}
