use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{Model, Roof, RoofParams};
use serde_json::{Value, json};

fn schema_37_without_roofs() -> Value {
    let mut value = serde_json::to_value(Model::new("Schema 37")).unwrap();
    remove_phase_fields_from_legacy_fixture(&mut value);
    value.as_object_mut().unwrap().remove("roofs");
    value["schema_version"] = json!(37);
    value["project"]["header"]["schema_version"] = json!(37);
    for collection in [
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
        "stairs",
        "sheets",
        "schedules",
        "plan_graphics_templates",
    ] {
        for entity in value[collection].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(37);
        }
    }
    value
}

#[test]
fn schema_37_migration_adds_roofs_and_advances_native_headers_atomically() {
    let old = schema_37_without_roofs();
    let mut migrated = old.clone();
    migrate(&mut migrated, 37).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.roofs.is_empty());
    assert_eq!(migrated["roofs"], json!({}));
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        json!(SCHEMA_VERSION)
    );

    let mut ambiguous = old;
    ambiguous["roofs"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 37).is_err());
    assert_eq!(ambiguous, before);

    let mut current = serde_json::to_value(model).unwrap();
    current.as_object_mut().unwrap().remove("roofs");
    assert!(serde_json::from_value::<Model>(current).is_err());
}

#[test]
fn native_roof_save_reopen_retains_geometry_identity_and_parameters() {
    let model = Model::new("Roof persistence");
    let level = *model.levels.keys().next().unwrap();
    let roof = Roof::new(
        "core.roof",
        RoofParams {
            name: "Main roof".into(),
            level,
            material: None,
            boundary: vec![
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 0.0),
                Point2::new(8.0, 5.0),
                Point2::new(0.0, 5.0),
            ],
            holes: vec![vec![
                Point2::new(2.0, 2.0),
                Point2::new(3.0, 2.0),
                Point2::new(3.0, 3.0),
                Point2::new(2.0, 3.0),
            ]],
            thickness: 0.24,
            top_offset: 0.3,
            slope_start: Point2::new(0.0, 0.0),
            slope_end: Point2::new(0.0, 1.0),
            rise_per_run: -0.2,
        },
    );
    let id = roof.id();
    let mut document = Document::from_model(model).unwrap();
    document
        .execute("Place roof", vec![Command::AddRoof(roof.clone())])
        .unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("roof.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().roofs[&id], roof);
    assert_eq!(reopened.model(), document.model());
    assert!(!reopened.can_undo());
}
