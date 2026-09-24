use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

// Representative schema 15 input constructed from a frozen schema 13 model,
// independently of the migration under test and of Model::new defaults.
fn representative_fifteen() -> Value {
    let mut value: Value =
        serde_json::from_str(include_str!("../src/tests/schema-13-sheets.json")).unwrap();
    value["schema_version"] = json!(15);
    value["project"]["header"]["schema_version"] = json!(15);
    value["sheets"] = json!({});
    for map in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "floors",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "dimensions",
        "grids",
        "materials",
        "views",
        "sheets",
    ] {
        for entity in value[map].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(15);
        }
    }
    value
}

#[test]
fn migration_preserves_ids_metadata_and_opaque_data_then_multiple_definitions_roundtrip() {
    let original = representative_fifteen();
    let mut migrated = original.clone();
    migrate(&mut migrated, 15).unwrap();
    let mut expected = original;
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["schedules"] = json!({});
    expected["detail_lines"] = json!({});
    expected["room_separation_lines"] = json!({});
    expected["wall_joins"] = json!({});
    expected["wall_types"] = json!({});
    expected["wall_type_assignments"] = json!({});
    expected["columns"] = json!({});
    expected["plan_graphics_templates"] = json!({});
    expected["plan_graphics"] = json!({});
    expected["project"]["header"]["schema_version"] = json!(SCHEMA_VERSION);
    for map in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "floors",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "dimensions",
        "grids",
        "materials",
        "views",
        "sheets",
    ] {
        for entity in expected[map].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    assert_eq!(
        migrated, expected,
        "all prior values including opaque version-like keys are preserved"
    );
    assert!(!migrated["extensions"].as_object().unwrap().is_empty());
    let mut doc = Document::from_model(serde_json::from_value(migrated).unwrap()).unwrap();
    let doors = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Door list", ScheduleCategory::Door),
    );
    let mut windows = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Window sizes", ScheduleCategory::Window),
    );
    windows.parameters.columns = vec![ScheduleColumn::Width, ScheduleColumn::Name];
    windows.parameters.sort = ScheduleSort::Width;
    doc.execute(
        "Schedules",
        vec![Command::AddSchedule(doors), Command::AddSchedule(windows)],
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schedules.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), doc.model());
    assert_eq!(reopened.model().schedules.len(), 2);
}

#[test]
fn migration_rejects_ambiguous_and_malformed_input_without_adoption() {
    for case in 0..8 {
        let mut value = representative_fifteen();
        match case {
            0 => value["schedules"] = json!({}),
            1 => value["schedules"] = Value::Null,
            2 => value["project"]["header"]["schema_version"] = json!(14),
            3 => {
                value.as_object_mut().unwrap().remove("walls");
            }
            4 => value["sheets"] = json!([]),
            5 => value["project"]["header"]["type_id"] = json!("core.schedule"),
            6 => value["project"]["header"]["id"] = json!("00000000-0000-0000-0000-000000000000"),
            _ => value["extra"] = json!(true),
        }
        let before = value.clone();
        assert!(migrate(&mut value, 15).is_err(), "case {case}");
        assert_eq!(value, before);
    }
    let mut current = serde_json::to_value(Model::new("Current")).unwrap();
    current.as_object_mut().unwrap().remove("schedules");
    let before = current.clone();
    assert!(migrate(&mut current, 16).is_err());
    assert_eq!(current, before);
}
