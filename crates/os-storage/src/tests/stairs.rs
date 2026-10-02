use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{
    Level, LevelParams, Model, Stair, StairParams, StairRailing, StairRailingParams,
    StairRailingSide, StairRailingType, StairRailingTypeParams,
};
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

fn schema_59_without_railings() -> Value {
    let mut value = serde_json::to_value(Model::new("Schema 59")).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("railings");
    object.remove("railing_types");
    object.remove("ramps");
    object.remove("casework_types");
    object.remove("casework");
    object.insert("schema_version".into(), json!(59));
    fn set_headers(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(header) = object.get_mut("header") {
                    header["schema_version"] = json!(59);
                } else {
                    for value in object.values_mut() {
                        set_headers(value);
                    }
                }
            }
            Value::Array(values) => values.iter_mut().for_each(set_headers),
            _ => {}
        }
    }
    set_headers(&mut value);
    value
}

#[test]
fn schema_59_migration_adds_empty_railing_maps_and_advances_headers() {
    let old = schema_59_without_railings();
    let mut migrated = old.clone();
    migrate(&mut migrated, 59).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.railings.is_empty());
    assert!(model.railing_types.is_empty());
    assert_eq!(migrated["railings"], json!({}));
    assert_eq!(migrated["railing_types"], json!({}));
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        json!(SCHEMA_VERSION)
    );

    let mut ambiguous = old;
    ambiguous["railings"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 59).is_err());
    assert_eq!(ambiguous, before);
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

#[test]
fn native_stair_railing_round_trip_preserves_host_shared_type_and_identity() {
    let mut model = Model::new("Stair railing persistence");
    let lower = *model.levels.keys().next().unwrap();
    let building = model.levels[&lower].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.0,
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
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.5, 0.0),
            width: 1.2,
            riser_count: 15,
            structural_thickness: 0.2,
            material: None,
        },
    );
    let stair_id = stair.id();
    let ty = StairRailingType::new(
        "core.railing_type",
        StairRailingTypeParams {
            name: "Standard".into(),
            top_rail_height: 0.95,
            top_rail_width: 0.05,
            top_rail_depth: 0.05,
            post_width: 0.08,
            post_depth: 0.05,
            max_post_spacing: 0.6,
            material: None,
        },
    );
    let type_id = ty.id();
    let railing = StairRailing::new(
        "core.railing",
        StairRailingParams {
            name: "Left guardrail".into(),
            stair: stair_id,
            railing_type: type_id,
            side: StairRailingSide::Left,
        },
    );
    let id = railing.id();
    let mut document = Document::from_model(model).unwrap();
    document
        .execute(
            "Place stair railing",
            vec![
                Command::AddStair(stair),
                Command::AddRailingType(ty.clone()),
                Command::AddRailing(railing.clone()),
            ],
        )
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stair-railing.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().railings[&id], railing);
    assert_eq!(reopened.model().railing_types[&type_id], ty);
    assert_eq!(reopened.model(), document.model());
}
