use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::{
    ExtensionEntity, Model, Opening, OpeningDefinition, OpeningKind, OpeningParams,
    PluginRequirement, Room, RoomParams, SCHEMA_VERSION, Wall, WallParams,
};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const COLLECTIONS: &[&str] = &[
    "sites",
    "buildings",
    "levels",
    "walls",
    "openings",
    "opening_types",
    "grids",
    "materials",
    "views",
];

fn existing_model() -> Model {
    let mut model = Model::new("Existing walls and openings");
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Boundary".into(),
            start: Point2::new(0., 0.),
            end: Point2::new(10., 0.),
            height: 3.,
            thickness: 0.2,
            level: *model.levels.keys().next().unwrap(),
            material: None,
        },
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            hinge: Default::default(),
            swing: Default::default(),
            name: "Door".into(),
            host: wall.id(),
            offset: 2.,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.,
            },
        },
    );
    model.walls.insert(wall.id(), wall);
    model.openings.insert(opening.id(), opening);
    let mut extension: ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    extension.payload = serde_json::from_str(r#"{"integer":184467440737095516170,"decimal":0.123456789012345678901234567890,"huge":1e400}"#).unwrap();
    model.plugin_requirements.insert(
        extension.owner.clone(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(extension.id, extension);
    model.validate().unwrap();
    model
}
fn schema_five() -> Value {
    let mut value = serde_json::to_value(existing_model()).unwrap();
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
    value.as_object_mut().unwrap().remove("rooms");
    value.as_object_mut().unwrap().remove("dimensions");
    value.as_object_mut().unwrap().remove("opening_types");
    value.as_object_mut().unwrap().remove("floors");
    opening_data_to_schema_seven(&mut value);
    set_version(&mut value, 5);
    value
}

fn opening_data_to_schema_seven(value: &mut Value) {
    for opening in value["openings"].as_object_mut().unwrap().values_mut() {
        let parameters = opening["parameters"].as_object_mut().unwrap();
        // Schema 7 predates per-door hinge/swing orientation.
        parameters.remove("hinge");
        parameters.remove("swing");
        let definition = parameters.remove("definition").unwrap();
        let legacy = definition.get("Legacy").unwrap();
        for key in ["kind", "width", "height", "sill"] {
            parameters.insert(key.into(), legacy[key].clone());
        }
    }
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
fn five_to_current_adds_native_maps_and_updates_native_versions() {
    let original = schema_five();
    let mut value = original.clone();
    migrate(&mut value, 5).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    assert_eq!(value["rooms"], json!({}));
    assert_eq!(value["floors"], json!({}));
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
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
    value.as_object_mut().unwrap().remove("rooms");
    value.as_object_mut().unwrap().remove("dimensions");
    value.as_object_mut().unwrap().remove("opening_types");
    value.as_object_mut().unwrap().remove("floors");
    opening_data_to_schema_seven(&mut value);
    set_version(&mut value, 5);
    assert_eq!(value, original); // Includes native identities, opening data and exact opaque numbers.
}

#[test]
fn ambiguous_missing_and_corrupt_migrations_are_atomic() {
    for case in 0..6 {
        let mut value = schema_five();
        match case {
            0 => value["rooms"] = json!({}),
            1 => value["walls"] = json!([]),
            2 => value["project"]["header"]["schema_version"] = json!(6),
            3 => {
                value.as_object_mut().unwrap().remove("openings");
            }
            4 => value["schema_version"] = json!(4),
            _ => value["project"]["header"]["id"] = json!("not-a-uuid"),
        }
        let before = value.clone();
        assert!(migrate(&mut value, 5).is_err());
        assert_eq!(value, before);
    }
    let mut current = serde_json::to_value(existing_model()).unwrap();
    current.as_object_mut().unwrap().remove("rooms");
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
}

#[test]
fn saved_room_signature_and_stale_wall_ids_survive_reopen_and_history() {
    let model = existing_model();
    let wall = *model.walls.keys().next().unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Office".into(),
            level: *model.levels.keys().next().unwrap(),
            seed: Point2::new(2., 2.),
            boundary_signature: vec![(wall, true), (Id::new(), true), (Id::new(), false)],
        },
    );
    let mut doc = Document::from_model_and_files(
        model,
        BTreeMap::from([("assets/opaque.bin".into(), vec![0, 255, 7])]),
    )
    .unwrap();
    doc.execute("Room", vec![Command::AddRoom(room.clone())])
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("room.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let mut reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), doc.model());
    assert_eq!(
        reopened.auxiliary_files()["assets/opaque.bin"],
        doc.auxiliary_files()["assets/opaque.bin"]
    );
    let opening = *reopened.model().openings.keys().next().unwrap();
    reopened
        .execute(
            "Remove boundary",
            vec![Command::RemoveOpening(opening), Command::RemoveWall(wall)],
        )
        .unwrap();
    assert_eq!(reopened.model().rooms[&room.id()], room);
    assert!(reopened.undo());
    assert_eq!(reopened.model(), doc.model());
    assert!(reopened.redo());
    ZipJsonStorage.save(&reopened, &path).unwrap();
    let stale = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(stale.model().rooms[&room.id()], room);
    assert!(stale.model().walls.is_empty());
}
