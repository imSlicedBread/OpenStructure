use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::{
    Model, Room, RoomParams, RoomSeparationLine, RoomSeparationLineParams, SCHEMA_VERSION, Wall,
    WallParams,
};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn schema_eighteen_with_wall_room() -> (Value, Id, Id, Vec<(Id, bool)>, Point2) {
    let mut model = Model::new("Legacy wall room");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Boundary".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(8.0, 0.0),
            height: 3.0,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    model.walls.insert(wall_id, wall);
    let signature = vec![(wall_id, true), (Id::new(), false), (Id::new(), true)];
    let seed = Point2::new(2.0, 2.0);
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Office".into(),
            level,
            seed,
            boundary_signature: signature.clone(),
        },
    );
    let room_id = room.id();
    model.rooms.insert(room_id, room);
    model.validate().unwrap();
    let mut value = serde_json::to_value(model).unwrap();
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
    value["schema_version"] = json!(18);
    value["project"]["header"]["schema_version"] = json!(18);
    for collection in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        for entity in value[collection].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(18);
        }
    }
    (value, room_id, level, signature, seed)
}

#[test]
fn schema_eighteen_migration_preserves_wall_room_and_rejects_ambiguity_atomically() {
    let (original, room_id, _, signature, seed) = schema_eighteen_with_wall_room();
    let mut migrated = original.clone();
    migrate(&mut migrated, 18).unwrap();
    let mut expected = original.clone();
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["room_separation_lines"] = json!({});
    expected["wall_joins"] = json!({});
    expected["wall_types"] = json!({});
    expected["wall_type_assignments"] = json!({});
    expected["columns"] = json!({});
    expected["plan_graphics_templates"] = json!({});
    expected["plan_graphics"] = json!({});
    expected["project"]["header"]["schema_version"] = json!(SCHEMA_VERSION);
    for collection in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        for entity in expected[collection].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    assert_eq!(migrated, expected);
    assert_eq!(migrated["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(migrated["room_separation_lines"], json!({}));
    let room_key = room_id.to_string();
    assert_eq!(
        migrated["rooms"][&room_key]["parameters"]["seed"],
        json!(seed)
    );
    assert_eq!(
        migrated["rooms"][&room_key]["parameters"]["boundary_signature"],
        json!(signature)
    );
    let model: Model = serde_json::from_value(migrated).unwrap();
    model.validate().unwrap();
    assert_eq!(model.rooms[&room_id].id(), room_id);

    let mut missing_map = expected.clone();
    missing_map
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    let before = missing_map.clone();
    assert!(migrate(&mut missing_map, SCHEMA_VERSION).is_err());
    assert_eq!(missing_map, before);

    for extra in [Value::Null, json!([]), json!({})] {
        let mut ambiguous = original.clone();
        ambiguous["room_separation_lines"] = extra;
        let before = ambiguous.clone();
        assert!(
            migrate(&mut ambiguous, 18)
                .unwrap_err()
                .to_string()
                .contains("ambiguous schema 18")
        );
        assert_eq!(ambiguous, before);
    }
    let mut bad_header = original.clone();
    bad_header["rooms"][&room_key]["header"]["schema_version"] = json!(19);
    let before = bad_header.clone();
    assert!(migrate(&mut bad_header, 18).is_err());
    assert_eq!(bad_header, before);
}

#[test]
fn migrated_room_and_new_separator_save_and_reopen_without_identity_loss() {
    let (mut value, room_id, level, signature, seed) = schema_eighteen_with_wall_room();
    migrate(&mut value, 18).unwrap();
    let mut document = Document::from_model(serde_json::from_value(value).unwrap()).unwrap();
    let line = RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level,
            start: Point2::new(0.0, 1.0),
            end: Point2::new(8.0, 1.0),
        },
    );
    let line_id = line.id();
    document
        .execute(
            "Add room separator",
            vec![Command::AddRoomSeparationLine(line)],
        )
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("room-separation.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), document.model());
    assert_eq!(reopened.model().schema_version, SCHEMA_VERSION);
    assert_eq!(reopened.model().rooms[&room_id].id(), room_id);
    assert_eq!(reopened.model().rooms[&room_id].parameters.seed, seed);
    assert_eq!(
        reopened.model().rooms[&room_id]
            .parameters
            .boundary_signature,
        signature
    );
    assert_eq!(
        reopened.model().room_separation_lines[&line_id].id(),
        line_id
    );
}
