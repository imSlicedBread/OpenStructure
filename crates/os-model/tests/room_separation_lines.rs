use os_core::{Id, Point2};
use os_model::{MAX_ROOM_SEPARATION_LINES, Model, RoomSeparationLine, RoomSeparationLineParams};

fn line(model: &Model) -> RoomSeparationLine {
    RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level: *model.levels.keys().next().unwrap(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(2.0, 0.0),
        },
    )
}

#[test]
fn room_separation_level_coordinates_lattice_and_required_map() {
    let mut model = Model::new("Separators");
    let good = line(&model);
    model.room_separation_lines.insert(good.id(), good.clone());
    model.validate().unwrap();
    for case in 0..7 {
        let mut bad = good.parameters.clone();
        match case {
            0 => bad.level = Id::new(),
            1 => bad.start.x = f64::NAN,
            2 => bad.end.y = f64::INFINITY,
            3 => bad.start.x = -1_000_001.0,
            4 => bad.end = bad.start,
            5 => bad.end = Point2::new(0.49e-6, 0.0),
            _ => bad.end.y = 1_000_001.0,
        }
        assert!(bad.validate(&model).is_err(), "case {case}");
    }
    let value = serde_json::to_value(&model).unwrap();
    let mut missing = value.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    assert!(serde_json::from_value::<Model>(missing).is_err());
    let mut bad = serde_json::to_value(&good.parameters).unwrap();
    bad["view"] = serde_json::json!(Id::new());
    assert!(serde_json::from_value::<RoomSeparationLineParams>(bad).is_err());
    // No plan view is needed; removing the owning level is prohibited.
    model.views.clear();
    model.validate().unwrap();
    model.levels.clear();
    assert!(model.validate().is_err());
}

#[test]
fn room_separation_global_ids_header_and_entity_budget() {
    let model = Model::new("IDs");
    for id in [model.project.id(), *model.levels.keys().next().unwrap()] {
        let mut bad = model.clone();
        let mut duplicate = line(&model);
        duplicate.header.id = id;
        bad.room_separation_lines.insert(id, duplicate);
        assert!(bad.validate().is_err());
    }
    for case in 0..4 {
        let mut bad = model.clone();
        let mut entity = line(&model);
        let mut key = entity.id();
        match case {
            0 => key = Id::new(),
            1 => entity.header.type_id = "core.detail_line".into(),
            2 => entity.header.schema_version -= 1,
            _ => {
                // Reuse an existing entity UUID to exercise the global
                // identity check (Id::default() intentionally creates a UUID).
                key = *model.levels.keys().next().unwrap();
                entity.header.id = key;
            }
        }
        bad.room_separation_lines.insert(key, entity);
        assert!(bad.validate().is_err());
    }
    let mut many = model;
    for _ in 0..MAX_ROOM_SEPARATION_LINES {
        let entity = line(&many);
        many.room_separation_lines.insert(entity.id(), entity);
    }
    many.validate().unwrap();
    let extra = line(&many);
    many.room_separation_lines.insert(extra.id(), extra);
    assert!(many.validate().is_err());
}

#[test]
fn room_boundary_segments_include_same_level_walls_and_lines_in_uuid_order() {
    use os_model::{Wall, WallParams};

    let mut model = Model::new("Mixed room boundaries");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Physical edge".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    model.walls.insert(wall.id(), wall.clone());
    let line = line(&model);
    model.room_separation_lines.insert(line.id(), line.clone());

    let segments = model.room_boundary_segments(level).unwrap();
    assert_eq!(segments.len(), 2);
    assert!(segments.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(segments.iter().any(|(id, _, _)| *id == wall.id()));
    assert!(segments.iter().any(|(id, _, _)| *id == line.id()));
}
