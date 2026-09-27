use super::*;
use os_core::{Id, Point2};
use os_document::Command;
use os_model::{Ceiling, CeilingParams, Model, Room, RoomParams, View, ViewParams};
use serde_json::{Value, json};

fn schema_38_without_ceilings() -> Value {
    let mut model = Model::new("Schema 38");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Floor plan", level));
    model.views.insert(view.id(), view);
    let mut value = serde_json::to_value(model).unwrap();
    remove_phase_fields_from_legacy_fixture(&mut value);
    value.as_object_mut().unwrap().remove("ceilings");
    value["schema_version"] = json!(38);
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
        "stairs",
        "roofs",
        "sheets",
        "schedules",
        "plan_graphics_templates",
    ] {
        let entities: Vec<&mut Value> = if collection == "project" {
            vec![&mut value[collection]]
        } else {
            value[collection]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .collect()
        };
        for entity in entities {
            entity["header"]["schema_version"] = json!(38);
            if let Some(plan) = entity["parameters"]["plan"].as_object_mut() {
                plan.insert("schema_version".into(), json!(1));
                plan.remove("view_type");
                plan["visibility"]
                    .as_object_mut()
                    .unwrap()
                    .remove("ceilings");
            }
        }
    }
    value
}

#[test]
fn schema_38_adds_empty_ceilings_and_upgrades_plan_settings_atomically() {
    let old = schema_38_without_ceilings();
    let mut migrated = old.clone();
    migrate(&mut migrated, 38).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.ceilings.is_empty());
    assert_eq!(migrated["ceilings"], json!({}));
    let plan = model
        .views
        .values()
        .find_map(|view| view.parameters.plan)
        .unwrap();
    assert_eq!(plan.view_type, os_model::PlanViewType::FloorPlan);
    assert!(plan.visibility.ceilings);

    let mut ambiguous = old;
    ambiguous["ceilings"] = json!({"not-a-schema-38-ceiling": {}});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 38).is_err());
    assert_eq!(ambiguous, before);
}

fn schema_39_without_ceiling_room_source() -> Value {
    let mut model = Model::new("Schema 39 ceiling source");
    let level = *model.levels.keys().next().unwrap();
    let ceiling = Ceiling::new(
        "core.ceiling",
        CeilingParams {
            name: "Ceiling".into(),
            level,
            material: None,
            boundary_room: None,
            boundary: vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 3.0),
                Point2::new(0.0, 3.0),
            ],
            holes: Vec::new(),
            thickness: 0.12,
            elevation_offset: 2.55,
        },
    );
    model.ceilings.insert(ceiling.id(), ceiling);
    let mut value = serde_json::to_value(model).unwrap();
    remove_phase_fields_from_legacy_fixture(&mut value);
    value["schema_version"] = json!(39);
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
        "stairs",
        "roofs",
        "ceilings",
        "sheets",
        "schedules",
        "plan_graphics_templates",
    ] {
        let entities: Vec<&mut Value> = if collection == "project" {
            vec![&mut value[collection]]
        } else {
            value[collection]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .collect()
        };
        for entity in entities {
            entity["header"]["schema_version"] = json!(39);
            if collection == "ceilings" {
                entity["parameters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("boundary_room");
            }
        }
    }
    value
}

#[test]
fn schema_39_adds_nullable_live_room_source_atomically() {
    let old = schema_39_without_ceiling_room_source();
    let mut migrated = old.clone();
    migrate(&mut migrated, 39).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    let ceiling = model.ceilings.values().next().unwrap();
    assert_eq!(ceiling.parameters.boundary_room, None);
    assert_eq!(migrated["schema_version"], json!(SCHEMA_VERSION));
    let serialized_ceiling = migrated["ceilings"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    assert_eq!(
        serialized_ceiling["header"]["schema_version"],
        json!(SCHEMA_VERSION)
    );
    assert_eq!(
        serialized_ceiling["parameters"]["boundary_room"],
        Value::Null
    );

    let mut ambiguous = old;
    ambiguous["ceilings"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["parameters"]["boundary_room"] = json!(Id::new().to_string());
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 39).is_err());
    assert_eq!(ambiguous, before);
}

#[test]
fn native_ceiling_save_reopen_preserves_identity_room_source_and_geometry() {
    let mut model = Model::new("Ceiling persistence");
    let level = *model.levels.keys().next().unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Office".into(),
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            level,
            seed: Point2::new(1.0, 1.0),
            boundary_signature: vec![(Id::new(), true), (Id::new(), true), (Id::new(), false)],
        },
    );
    let room_id = room.id();
    model.rooms.insert(room_id, room);
    let ceiling = Ceiling::new(
        "core.ceiling",
        CeilingParams {
            name: "Perforated ceiling".into(),
            level,
            material: None,
            boundary_room: Some(room_id),
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
            thickness: 0.12,
            elevation_offset: 2.55,
        },
    );
    let id = ceiling.id();
    let mut document = Document::from_model(model).unwrap();
    document
        .execute("Place ceiling", vec![Command::AddCeiling(ceiling.clone())])
        .unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ceiling.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().ceilings[&id], ceiling);
    assert_eq!(reopened.model(), document.model());
    assert!(!reopened.can_undo());
}
