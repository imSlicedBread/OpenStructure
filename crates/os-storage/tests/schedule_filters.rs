mod common;

use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

const SCHEDULE: &str = "00000000-0000-4000-8000-000000000071";
fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-41-schedule-filters.json")).unwrap()
}

#[test]
fn schema_41_schedule_filters_frozen_migration_and_persisted_rules_roundtrip() {
    let original = frozen();
    let mut migrated = original.clone();
    migrate(&mut migrated, 41).unwrap();
    assert_eq!(migrated["schema_version"], SCHEMA_VERSION);
    assert_eq!(
        migrated["schedules"][SCHEDULE]["parameters"]["filters"],
        json!([])
    );
    let mut reversed = migrated.clone();
    reversed["schema_version"] = 41.into();
    for (key, value) in reversed.as_object_mut().unwrap() {
        if key == "project" {
            assert_eq!(value["header"]["schema_version"], SCHEMA_VERSION);
            value["header"]["schema_version"] = 41.into();
        } else if key != "extensions"
            && let Some(map) = value.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                assert_eq!(entity["header"]["schema_version"], SCHEMA_VERSION);
                entity["header"]["schema_version"] = 41.into();
            }
        }
    }
    reversed["schedules"][SCHEDULE]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("filters");
    common::remove_phase_fields(&mut reversed);
    assert_eq!(
        reversed, original,
        "only filters and native headers change; opaque metadata is preserved"
    );
    let mut doc = Document::from_model(serde_json::from_value(migrated).unwrap()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schedule-filters.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
    ] {
        let mut params = ScheduleParams::new(format!("Filtered {category:?}"), category);
        params.filters = vec![
            ScheduleFilter::Text {
                field: ScheduleTextField::Host,
                operator: ScheduleTextOperator::StartsWith,
                value: "WALL".into(),
            },
            ScheduleFilter::Numeric {
                field: ScheduleNumericField::Width,
                operator: ScheduleNumericOperator::Greater,
                value: 0.9000001,
            },
        ];
        doc.execute(
            "Add filtered definition",
            vec![Command::AddSchedule(Schedule::new("core.schedule", params))],
        )
        .unwrap();
    }
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}

#[test]
fn schema_41_schedule_filters_ambiguity_and_failures_are_atomic() {
    for case in 0..10 {
        let mut bad = frozen();
        match case {
            0 => {
                bad.as_object_mut().unwrap().remove("schedules");
            }
            1 => bad["schedules"] = Value::Null,
            2 => bad["schedules"] = json!([]),
            3 => bad["schedules"][SCHEDULE]["parameters"]["filters"] = json!([]),
            4 => bad["schedules"][SCHEDULE]["parameters"]["filters"] = Value::Null,
            5 => bad["schedules"][SCHEDULE]["parameters"] = json!([]),
            6 => bad["schedules"][SCHEDULE]["header"]["schema_version"] = 40.into(),
            7 => bad["schedules"][SCHEDULE]["parameters"]["name"] = "".into(),
            8 => {
                bad.as_object_mut()
                    .unwrap()
                    .remove("plan_graphics_templates");
            }
            _ => bad["project"]["header"]["schema_version"] = 40.into(),
        }
        let before = bad.clone();
        let error = migrate(&mut bad, 41).unwrap_err();
        if matches!(case, 3 | 4) {
            assert!(
                error
                    .to_string()
                    .contains("ambiguous schema 41 schedule filters")
            );
        }
        assert_eq!(bad, before, "case {case}");
    }
    let mut current = frozen();
    migrate(&mut current, 41).unwrap();
    current["schedules"][SCHEDULE]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("filters");
    let before = current.clone();
    assert!(migrate(&mut current, 42).is_err());
    assert_eq!(current, before);
}
