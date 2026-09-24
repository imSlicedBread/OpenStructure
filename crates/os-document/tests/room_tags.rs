use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;

fn setup() -> (Document, RoomTag) {
    let mut model = Model::new("Tags");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
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
    let tag = RoomTag::new(
        "core.room_tag",
        RoomTagParams {
            view: view.id(),
            room: room.id(),
            position: Point2::new(2., 2.),
        },
    );
    model.views.insert(view.id(), view);
    model.rooms.insert(room.id(), room);
    (Document::from_model(model).unwrap(), tag)
}

fn assert_atomic_rejection(doc: &mut Document, commands: Vec<Command>) {
    doc.drain_events();
    let model = doc.model().clone();
    let revision = doc.revision();
    let history = doc.history_stats();
    assert!(doc.execute("Rejected batch", commands).is_err());
    assert_eq!(doc.model(), &model);
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), history);
    assert!(doc.drain_events().is_empty());
}

#[test]
fn room_tag_commands_are_atomic_invalidate_view_and_preserve_identity_in_history() {
    let (mut doc, tag) = setup();
    let mut invalid = tag.clone();
    invalid.parameters.room = Id::new();
    assert_atomic_rejection(
        &mut doc,
        vec![
            Command::RenameProject("Rejected".into()),
            Command::AddRoomTag(invalid),
        ],
    );
    let before = doc.model().clone();
    doc.execute("Place", vec![Command::AddRoomTag(tag.clone())])
        .unwrap();
    assert_eq!(doc.history_stats().undo_entries, 1);
    let placed = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&tag.id()));
    assert!(event.invalidated.contains(&tag.parameters.view));
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &placed);
    for duplicate in [
        tag.clone(),
        RoomTag::new("core.room_tag", tag.parameters.clone()),
    ] {
        assert_atomic_rejection(
            &mut doc,
            vec![
                Command::RenameProject("Rejected".into()),
                Command::AddRoomTag(duplicate),
            ],
        );
    }
    let mut parameters = tag.parameters.clone();
    parameters.position.x = f64::INFINITY;
    assert_atomic_rejection(
        &mut doc,
        vec![
            Command::RenameProject("Rejected".into()),
            Command::UpdateRoomTag {
                id: tag.id(),
                parameters,
            },
        ],
    );
    assert_atomic_rejection(
        &mut doc,
        vec![
            Command::RemoveRoomTag(tag.id()),
            Command::RemoveRoomTag(tag.id()),
        ],
    );
    let mut parameters = tag.parameters.clone();
    parameters.position = Point2::new(3., 4.);
    doc.execute(
        "Move",
        vec![Command::UpdateRoomTag {
            id: tag.id(),
            parameters,
        }],
    )
    .unwrap();
    let moved = doc.model().clone();
    assert_eq!(moved.room_tags[&tag.id()].header, tag.header);
    assert_eq!(doc.history_stats().undo_entries, 2);
    assert!(
        doc.drain_events()[0]
            .invalidated
            .contains(&tag.parameters.view)
    );
    assert!(doc.undo());
    assert_eq!(doc.model(), &placed);
    assert!(doc.redo());
    assert_eq!(doc.model(), &moved);
    doc.drain_events();
    doc.execute("Delete", vec![Command::RemoveRoomTag(tag.id())])
        .unwrap();
    assert!(doc.model().room_tags.is_empty());
    assert!(
        doc.drain_events()[0]
            .invalidated
            .contains(&tag.parameters.view)
    );
    assert!(doc.undo());
    assert_eq!(doc.model(), &moved);
    assert!(doc.redo());
    assert!(doc.model().room_tags.is_empty());
}

#[test]
fn room_tag_room_edits_invalidate_and_room_deletion_retains_orphan_view_deletion_is_batched() {
    let (mut doc, tag) = setup();
    doc.execute("Place", vec![Command::AddRoomTag(tag.clone())])
        .unwrap();
    let mut parameters = doc.model().rooms[&tag.parameters.room].parameters.clone();
    parameters.number = "102".into();
    parameters.name = "Study".into();
    doc.drain_events();
    doc.execute(
        "Rename room",
        vec![Command::UpdateRoom {
            id: tag.parameters.room,
            parameters,
        }],
    )
    .unwrap();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&tag.id()));
    assert!(event.invalidated.contains(&tag.parameters.view));
    assert_eq!(doc.model().room_tags[&tag.id()], tag);
    let before = doc.model().clone();
    doc.execute(
        "Delete room",
        vec![Command::RemoveRoom(tag.parameters.room)],
    )
    .unwrap();
    assert_eq!(doc.model().room_tags[&tag.id()], tag);
    assert_eq!(
        tag.parameters.resolve(doc.model()).unwrap_err(),
        "Missing room"
    );
    assert!(doc.drain_events()[0].invalidated.contains(&tag.id()));
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert!(tag.parameters.resolve(doc.model()).is_err());
    assert_atomic_rejection(&mut doc, vec![Command::RemoveView(tag.parameters.view)]);
    let orphan = doc.model().clone();
    doc.execute(
        "Delete view with tags",
        vec![
            Command::RemoveRoomTag(tag.id()),
            Command::RemoveView(tag.parameters.view),
        ],
    )
    .unwrap();
    assert!(!doc.model().views.contains_key(&tag.parameters.view));
    assert!(doc.model().room_tags.is_empty());
    assert!(doc.undo());
    assert_eq!(doc.model(), &orphan);
    assert!(doc.redo());
    assert!(!doc.model().views.contains_key(&tag.parameters.view));
}
