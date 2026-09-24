use os_core::{Id, Point2};
use os_model::*;

fn setup() -> (Model, RoomTag) {
    let mut model = Model::new("Room tags");
    let level = *model.levels.keys().next().unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Office".into(),
            level,
            seed: Point2::new(1., 1.),
            boundary_signature: vec![(Id::new(), true), (Id::new(), true), (Id::new(), false)],
        },
    );
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let tag = RoomTag::new(
        "core.room_tag",
        RoomTagParams {
            room: room.id(),
            view: view.id(),
            position: Point2::new(2., 2.),
        },
    );
    model.rooms.insert(room.id(), room);
    model.views.insert(view.id(), view);
    (model, tag)
}

#[test]
fn room_tag_uniqueness_is_per_room_and_view_and_labels_are_live() {
    let (mut model, tag) = setup();
    tag.parameters.validate_creation(&model).unwrap();
    model.room_tags.insert(tag.id(), tag.clone());
    model.validate().unwrap();
    let duplicate = RoomTag::new("core.room_tag", tag.parameters.clone());
    model.room_tags.insert(duplicate.id(), duplicate.clone());
    assert!(model.validate().is_err());
    model.room_tags.remove(&duplicate.id());
    let view = View::new(
        "core.view",
        ViewParams::floor_plan("Other", model.rooms[&tag.parameters.room].parameters.level),
    );
    let mut other = duplicate;
    other.parameters.view = view.id();
    model.views.insert(view.id(), view);
    model.room_tags.insert(other.id(), other);
    model.validate().unwrap();
    let saved = model.room_tags.clone();
    let room = model.rooms.get_mut(&tag.parameters.room).unwrap();
    room.parameters.number = "102".into();
    room.parameters.name = "Study".into();
    let live = tag.parameters.resolve(&model).unwrap();
    assert_eq!(
        (&*live.parameters.number, &*live.parameters.name),
        ("102", "Study")
    );
    assert_eq!(model.room_tags, saved);
    model.rooms.remove(&tag.parameters.room);
    model.validate().unwrap();
    assert_eq!(tag.parameters.resolve(&model).unwrap_err(), "Missing room");
    assert!(tag.parameters.validate_creation(&model).is_err());
}

#[test]
fn room_tag_creation_rejects_wrong_level_and_invalid_syntax_but_retains_orphans() {
    let (mut model, tag) = setup();
    for position in [
        Point2::new(f64::NAN, 0.),
        Point2::new(0., f64::INFINITY),
        Point2::new(1e6 + 1., 0.),
    ] {
        let mut bad = tag.parameters.clone();
        bad.position = position;
        assert!(bad.validate(&model).is_err());
    }
    let mut bad = tag.parameters.clone();
    bad.room = serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"").unwrap();
    assert!(bad.validate(&model).is_err());
    bad = tag.parameters.clone();
    bad.view = Id::new();
    assert!(bad.validate(&model).is_err());
    bad.view = *model
        .views
        .iter()
        .find(|(_, v)| v.parameters.kind != ViewKind::Plan)
        .unwrap()
        .0;
    assert!(bad.validate(&model).is_err());
    let mut syntax = serde_json::to_value(&tag).unwrap();
    syntax["parameters"]["label"] = "Frozen text".into();
    assert!(serde_json::from_value::<RoomTag>(syntax).is_err());
    let mut syntax = serde_json::to_value(&tag).unwrap();
    syntax["parameters"]["room"] = "bad UUID".into();
    assert!(serde_json::from_value::<RoomTag>(syntax).is_err());
    let level = model.levels.values().next().unwrap().clone();
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.,
            building: level.parameters.building,
        },
    );
    model
        .rooms
        .get_mut(&tag.parameters.room)
        .unwrap()
        .parameters
        .level = upper.id();
    model.levels.insert(upper.id(), upper);
    assert!(tag.parameters.validate_creation(&model).is_err());
    model.room_tags.insert(tag.id(), tag.clone());
    model.validate().unwrap();
    assert_eq!(
        tag.parameters.resolve(&model).unwrap_err(),
        "Room is on another level"
    );
    model.room_tags.get_mut(&tag.id()).unwrap().header.id = Id::new();
    assert!(model.validate().is_err());
}
