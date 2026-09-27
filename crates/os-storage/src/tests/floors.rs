use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{Floor, FloorParams};
use serde_json::{Value, json};

// Frozen schema-8 input: the fixed building graph plus explicit schema-8 maps.
// No current model constructors/serialization or migration code builds this input.
fn schema_eight() -> Value {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/schema-3-pointer-walls.json"
    ))
    .unwrap();
    value["schema_version"] = json!(8);
    value["project"]["header"]["schema_version"] = json!(8);
    for map in ["grids", "rooms", "dimensions", "openings", "opening_types"] {
        value[map] = json!({});
    }
    for map in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "materials",
        "views",
    ] {
        for entity in value[map].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(8);
        }
    }
    value["plugin_requirements"] = json!({"org.example.fixture":{"version":"1.0.0"}});
    value["extensions"] = json!({"10000000-0000-0000-0000-000000000003": {
        "envelope_version":1,"id":"10000000-0000-0000-0000-000000000003",
        "owner":"org.example.fixture","type_id":"org.example.fixture.item","name":"Opaque",
        "payload_schema_version":42,"relationships":{},"depends_on":[],
        "payload":{"schema_version":8,"floors":{"opaque":[1,2,3]}}
    }});
    value
}

#[test]
fn frozen_eight_migration_adds_only_floors_and_native_versions() {
    let old = schema_eight();
    let mut migrated = old.clone();
    migrate(&mut migrated, 8).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.floors.is_empty());
    assert_eq!(migrated["extensions"], old["extensions"]);
    remove_phase_fields_from_legacy_fixture(&mut migrated);
    migrated.as_object_mut().unwrap().remove("opening_tags");
    migrated.as_object_mut().unwrap().remove("floors");
    migrated.as_object_mut().unwrap().remove("room_tags");
    migrated.as_object_mut().unwrap().remove("sheets");
    migrated.as_object_mut().unwrap().remove("schedules");
    migrated
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    migrated.as_object_mut().unwrap().remove("wall_joins");
    migrated.as_object_mut().unwrap().remove("wall_types");
    migrated
        .as_object_mut()
        .unwrap()
        .remove("wall_type_assignments");
    migrated.as_object_mut().unwrap().remove("columns");
    migrated
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    migrated.as_object_mut().unwrap().remove("plan_graphics");
    migrated["schema_version"] = json!(8);
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        SCHEMA_VERSION
    );
    migrated["project"]["header"]["schema_version"] = json!(8);
    for map in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "materials",
        "views",
        "grids",
        "rooms",
        "dimensions",
        "openings",
        "opening_types",
    ] {
        for entity in migrated[map].as_object_mut().unwrap().values_mut() {
            assert_eq!(entity["header"]["schema_version"], SCHEMA_VERSION);
            entity["header"]["schema_version"] = json!(8);
        }
    }
    migrated.as_object_mut().unwrap().remove("detail_lines");
    migrated
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    migrated.as_object_mut().unwrap().remove("plan_graphics");
    migrated.as_object_mut().unwrap().remove("stairs");
    migrated.as_object_mut().unwrap().remove("roofs");
    migrated.as_object_mut().unwrap().remove("ceilings");
    reverse_schema_38_plan_settings(&mut migrated);
    assert_eq!(migrated, old);
    for case in 0..3 {
        let mut bad = old.clone();
        match case {
            0 => bad["floors"] = json!({}),
            1 => bad["project"]["header"]["schema_version"] = json!(7),
            _ => {
                bad.as_object_mut().unwrap().remove("opening_types");
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 8).is_err());
        assert_eq!(bad, before);
    }
    let mut current = serde_json::to_value(model).unwrap();
    current.as_object_mut().unwrap().remove("floors");
    current.as_object_mut().unwrap().remove("columns");
    assert!(serde_json::from_value::<Model>(current).is_err());
}

#[test]
fn old_archive_migrates_then_floor_saves_reopens_without_identity_or_payload_loss() {
    let old = schema_eight();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("floor.osb");
    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    let manifest = Manifest {
        format: "OpenStructure".into(),
        container_version: 2,
        schema_version: 8,
        project_id: Id(old["project"]["header"]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    zip.start_file("manifest.toml", options).unwrap();
    zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
        .unwrap();
    zip.start_file("model.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&old).unwrap()).unwrap();
    zip.finish().unwrap();
    let mut doc = ZipJsonStorage.open(&path).unwrap();
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Slab".into(),
            level: *doc.model().levels.keys().next().unwrap(),
            material: None,
            boundary: vec![
                Point2::new(0., 0.),
                Point2::new(4., 0.),
                Point2::new(4., 4.),
                Point2::new(0., 4.),
            ],
            holes: vec![vec![
                Point2::new(1., 1.),
                Point2::new(2., 1.),
                Point2::new(2., 2.),
                Point2::new(1., 2.),
            ]],
            thickness: 0.2,
            top_offset: 0.1,
        },
    );
    let id = floor.id();
    doc.execute("Add floor", vec![Command::AddFloor(floor.clone())])
        .unwrap();
    ZipJsonStorage.save(&doc, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(doc.model(), reopened.model());
    assert_eq!(reopened.model().floors[&id], floor);
    assert_eq!(
        serde_json::to_value(&reopened.model().extensions).unwrap(),
        old["extensions"]
    );
    assert!(!reopened.can_undo());
}

#[test]
fn schema_35_floor_migration_adds_an_empty_opening_ring_list() {
    let mut model = Model::new("Schema 35 floor");
    let level = *model.levels.keys().next().unwrap();
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Existing slab".into(),
            level,
            material: None,
            boundary: vec![
                Point2::new(0., 0.),
                Point2::new(8., 0.),
                Point2::new(8., 8.),
                Point2::new(0., 8.),
            ],
            holes: Vec::new(),
            thickness: 0.2,
            top_offset: 0.0,
        },
    );
    let floor_id = floor.id();
    model.floors.insert(floor_id, floor);
    let mut old = serde_json::to_value(model).unwrap();
    remove_phase_fields_from_legacy_fixture(&mut old);
    old.as_object_mut().unwrap().remove("stairs");
    old.as_object_mut().unwrap().remove("roofs");
    old["schema_version"] = json!(35);
    for collection in [
        "project",
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
    ] {
        if collection == "project" {
            old[collection]["header"]["schema_version"] = json!(35);
        } else {
            for entity in old[collection].as_object_mut().unwrap().values_mut() {
                entity["header"]["schema_version"] = json!(35);
            }
        }
    }
    old["floors"][floor_id.to_string()]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("holes");

    migrate(&mut old, 35).unwrap();
    assert_eq!(
        old["floors"][floor_id.to_string()]["parameters"]["holes"],
        json!([])
    );
    let model: Model = serde_json::from_value(old).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.floors[&floor_id].parameters.holes.is_empty());
}
