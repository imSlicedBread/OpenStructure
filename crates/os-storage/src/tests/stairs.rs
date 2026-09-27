use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{Level, LevelParams, Model, Stair, StairParams};
use serde_json::{Value, json};

fn schema_36_without_stairs() -> Value {
    let mut value = serde_json::to_value(Model::new("Schema 36")).unwrap();
    remove_phase_fields_from_legacy_fixture(&mut value);
    value.as_object_mut().unwrap().remove("stairs");
    value.as_object_mut().unwrap().remove("roofs");
    value["schema_version"] = json!(36);
    let collections = [
        "sites",
        "buildings",
        "levels",
        "walls",
        "wall_joins",
        "wall_types",
        "openings",
        "opening_types",
        "opening_tags",
        "rooms",
        "room_tags",
        "detail_lines",
        "room_separation_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "columns",
        "sheets",
        "schedules",
        "plan_graphics_templates",
    ];
    value["project"]["header"]["schema_version"] = json!(36);
    for collection in collections {
        for entity in value[collection].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(36);
        }
    }
    value
}

#[test]
fn schema_36_migration_adds_the_stair_map_and_advances_native_headers() {
    let old = schema_36_without_stairs();
    let mut migrated = old.clone();
    migrate(&mut migrated, 36).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.stairs.is_empty());
    assert_eq!(migrated["stairs"], json!({}));
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        json!(SCHEMA_VERSION)
    );

    let mut ambiguous = old;
    ambiguous["stairs"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 36).is_err());
    assert_eq!(ambiguous, before);

    let mut current = serde_json::to_value(model).unwrap();
    current.as_object_mut().unwrap().remove("stairs");
    assert!(serde_json::from_value::<Model>(current).is_err());
}

#[test]
fn native_stair_save_reopen_retains_levels_identity_and_parameters() {
    let mut model = Model::new("Stair persistence");
    let lower = *model.levels.keys().next().unwrap();
    let building = model.levels[&lower].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.2,
            building,
        },
    );
    let upper_id = upper.id();
    model.levels.insert(upper_id, upper);
    let stair = Stair::new(
        "core.stair",
        StairParams {
            name: "Main stair".into(),
            lower_level: lower,
            upper_level: upper_id,
            start: Point2::new(1.0, 2.0),
            end: Point2::new(4.6, 2.0),
            width: 0.9,
            riser_count: 18,
            structural_thickness: 0.16,
            material: None,
        },
    );
    let id = stair.id();
    let mut document = Document::from_model(model).unwrap();
    document
        .execute("Place stair", vec![Command::AddStair(stair.clone())])
        .unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stair.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().stairs[&id], stair);
    assert_eq!(reopened.model(), document.model());
    assert!(!reopened.can_undo());
}
