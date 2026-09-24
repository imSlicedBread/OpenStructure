use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{Manifest, StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use std::{fs, io::Write};
use zip::{ZipWriter, write::SimpleFileOptions};

const COLLECTIONS: &[&str] = &[
    "sites",
    "buildings",
    "levels",
    "walls",
    "openings",
    "opening_types",
    "rooms",
    "dimensions",
    "grids",
    "materials",
    "views",
    "floors",
];

#[test]
fn schema_eleven_migration_preserves_dimension_and_all_headers() {
    let original: Value = serde_json::from_str(include_str!(
        "../../../fixtures/schema-11-aligned-dimension.json"
    ))
    .unwrap();
    let mut value = original.clone();
    migrate(&mut value, 11).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    let mut expected = original.clone();
    set_version(&mut expected, SCHEMA_VERSION);
    expected["room_tags"] = json!({});
    expected["sheets"] = json!({});
    expected["schedules"] = json!({});
    expected["detail_lines"] = json!({});
    expected["room_separation_lines"] = json!({});
    expected["wall_joins"] = json!({});
    expected["wall_types"] = json!({});
    expected["wall_type_assignments"] = json!({});
    expected["columns"] = json!({});
    expected["plan_graphics_templates"] = json!({});
    expected["plan_graphics"] = json!({});
    assert_eq!(
        value, expected,
        "only schema versions and documented empty native maps may change; preserve every ID, header and prior dimension parameter"
    );
    let mut model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let prior = model.dimensions.values().next().unwrap().clone();
    assert_eq!(prior.parameters.resolve(&model).unwrap().length_metres, 4.0);
    let first = model.walls.values().next().unwrap().clone();
    let mut parameters = first.parameters.clone();
    parameters.end = Point2::new(0.0, 4.0);
    let second = Wall::new(&first.header.type_id, parameters);
    let second_id = second.id();
    model.walls.insert(second_id, second);
    let mut parameters = prior.parameters.clone();
    parameters.layout = DimensionLayout::Angular;
    parameters.second.wall = second_id;
    parameters
        .place_angular(&model, Point2::new(1.0, 1.0))
        .unwrap();
    let angular = Dimension::new("core.dimension", parameters);
    let id = angular.id();
    model.dimensions.insert(id, angular.clone());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("angular.osb");
    let storage = ZipJsonStorage;
    storage
        .save(&Document::from_model(model.clone()).unwrap(), &path)
        .unwrap();
    let reopened = storage.open(&path).unwrap();
    let loaded = reopened.model().clone();
    assert_eq!(loaded, model);
    assert_eq!(loaded.dimensions[&prior.id()], prior);
    assert_eq!(loaded.dimensions[&id], angular);
    assert!(
        (loaded.dimensions[&id]
            .parameters
            .resolve_angular(&loaded)
            .unwrap()
            .degrees()
            - 90.0)
            .abs()
            < 1e-10
    );
}

// Reconstruct schema 6 from a frozen earlier fixture, independently of migration.
fn schema_six() -> Value {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../fixtures/schema-3-pointer-walls.json"
    ))
    .unwrap();
    for collection in ["grids", "openings", "rooms"] {
        value[collection] = json!({});
    }
    set_version(&mut value, 6);
    value
}

fn set_version(value: &mut Value, version: u32) {
    value["schema_version"] = json!(version);
    value["project"]["header"]["schema_version"] = json!(version);
    for collection in COLLECTIONS {
        if let Some(entities) = value.get_mut(*collection).and_then(Value::as_object_mut) {
            for entity in entities.values_mut() {
                entity["header"]["schema_version"] = json!(version);
            }
        }
    }
}

#[test]
fn six_to_current_adds_native_maps_and_versions_atomically() {
    let original = schema_six();
    let mut value = original.clone();
    migrate(&mut value, 6).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    assert_eq!(value["dimensions"], json!({}));
    assert_eq!(value["opening_types"], json!({}));
    assert_eq!(value["floors"], json!({}));
    assert_eq!(value["detail_lines"], json!({}));
    assert_eq!(value["room_separation_lines"], json!({}));
    assert_eq!(value["columns"], json!({}));
    assert_eq!(value["plan_graphics_templates"], json!({}));
    assert_eq!(value["plan_graphics"], json!({}));
    serde_json::from_value::<Model>(value.clone())
        .unwrap()
        .validate()
        .unwrap();
    value.as_object_mut().unwrap().remove("dimensions");
    value.as_object_mut().unwrap().remove("opening_types");
    value.as_object_mut().unwrap().remove("floors");
    value.as_object_mut().unwrap().remove("room_tags");
    value.as_object_mut().unwrap().remove("sheets");
    value.as_object_mut().unwrap().remove("schedules");
    value.as_object_mut().unwrap().remove("detail_lines");
    value
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    value.as_object_mut().unwrap().remove("wall_joins");
    value.as_object_mut().unwrap().remove("wall_types");
    value
        .as_object_mut()
        .unwrap()
        .remove("wall_type_assignments");
    value.as_object_mut().unwrap().remove("columns");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    set_version(&mut value, 6);
    assert_eq!(value, original);
    for case in 0..5 {
        let mut bad = original.clone();
        match case {
            0 => bad["dimensions"] = json!({}),
            1 => bad["rooms"] = json!([]),
            2 => bad["project"]["header"]["schema_version"] = json!(5),
            3 => {
                bad.as_object_mut().unwrap().remove("rooms");
            }
            _ => bad["schema_version"] = json!(5),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 6).is_err());
        assert_eq!(bad, before);
    }
}

#[test]
fn schema_ten_dimensions_migrate_as_aligned_without_changing_identity() {
    let mut schema_ten: Value = serde_json::from_str(include_str!(
        "../../../fixtures/schema-10-aligned-dimension.json"
    ))
    .unwrap();
    let frozen_schema_ten = schema_ten.clone();
    let project_id = schema_ten["project"]["header"]["id"].clone();
    let wall_id = schema_ten["walls"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let view_id = schema_ten["views"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, view)| view["parameters"]["kind"] == "Plan")
        .unwrap()
        .0
        .clone();
    let dimension_id = schema_ten["dimensions"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();

    migrate(&mut schema_ten, 10).unwrap();
    assert_eq!(schema_ten["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(schema_ten["project"]["header"]["id"], project_id);
    assert_eq!(schema_ten["walls"][&wall_id]["header"]["id"], wall_id);
    assert_eq!(schema_ten["views"][&view_id]["header"]["id"], view_id);
    let migrated = &schema_ten["dimensions"][&dimension_id];
    assert_eq!(migrated["header"]["id"], dimension_id);
    assert_eq!(migrated["header"]["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(migrated["parameters"]["layout"], json!("Aligned"));
    assert_eq!(migrated["parameters"]["additional"], json!([]));
    assert_eq!(migrated["parameters"]["baseline_spacing_m"], json!(0.25));
    let parsed: Model = serde_json::from_value(schema_ten.clone()).unwrap();
    parsed.validate().unwrap();

    let mut ambiguous = frozen_schema_ten.clone();
    ambiguous["dimensions"][&dimension_id]["parameters"]["layout"] = json!("Chain");
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 10).is_err());
    assert_eq!(ambiguous, before);
}

#[test]
fn schema_six_archive_migrates_then_dimensions_and_orphans_roundtrip_with_identity() {
    let old = schema_six();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("schema-six.osb");
    let manifest = Manifest {
        format: "OpenStructure".into(),
        container_version: 2,
        schema_version: 6,
        project_id: serde_json::from_value(old["project"]["header"]["id"].clone()).unwrap(),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    let mut zip = ZipWriter::new(fs::File::create(&source).unwrap());
    zip.start_file("manifest.toml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
        .unwrap();
    zip.start_file("model.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(serde_json::to_string(&old).unwrap().as_bytes())
        .unwrap();
    zip.finish().unwrap();
    let original_bytes = fs::read(&source).unwrap();
    let migrated = ZipJsonStorage.open(&source).unwrap();
    let mut model = migrated.model().clone();
    let wall = model.walls.values().next().unwrap().clone();
    let view = View::new(
        "core.view",
        ViewParams::floor_plan("Dimension plan", wall.parameters.level),
    );
    let dimension = Dimension::new(
        "core.dimension",
        DimensionParams {
            layout: os_model::DimensionLayout::Aligned,
            additional: Vec::new(),
            baseline_spacing_m: 0.25,
            view: view.id(),
            first: DimensionReference {
                wall: wall.id(),
                endpoint: DimensionEndpoint::Start,
            },
            second: DimensionReference {
                wall: wall.id(),
                endpoint: DimensionEndpoint::End,
            },
            offset_m: -0.75,
            orphan_hint: Point2::new(1., -0.75),
        },
    );
    model.views.insert(view.id(), view);
    let mut doc = Document::from_model(model).unwrap();
    doc.execute("Dimension", vec![Command::AddDimension(dimension.clone())])
        .unwrap();
    let path = dir.path().join("dimension.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let mut reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), doc.model());
    assert_eq!(reopened.model().dimensions[&dimension.id()], dimension);
    reopened
        .execute("Remove wall", vec![Command::RemoveWall(wall.id())])
        .unwrap();
    assert_eq!(
        dimension.parameters.resolve(reopened.model()),
        Err(DimensionDiagnostic::MissingWall)
    );
    assert!(reopened.undo());
    assert!(dimension.parameters.resolve(reopened.model()).is_ok());
    assert!(reopened.redo());
    ZipJsonStorage.save(&reopened, &path).unwrap();
    let orphan = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(orphan.model(), reopened.model());
    assert_eq!(orphan.model().dimensions[&dimension.id()], dimension);
    assert_eq!(fs::read(source).unwrap(), original_bytes);
}
