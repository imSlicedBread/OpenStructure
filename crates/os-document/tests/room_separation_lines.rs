use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::{Room, RoomParams, RoomSeparationLine, RoomSeparationLineParams, View, ViewParams};

#[test]
fn room_separation_transactions_history_invalidation_and_failed_batch() {
    let mut doc = Document::new("Separators").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let line = RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(5.0, 0.0),
        },
    );
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "1".into(),
            name: "Room".into(),
            level,
            seed: Point2::new(1.0, 1.0),
            boundary_signature: vec![(line.id(), true), (Id::new(), true), (Id::new(), false)],
        },
    );
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    doc.execute(
        "Setup",
        vec![
            Command::AddRoom(room.clone()),
            Command::AddView(view.clone()),
        ],
    )
    .unwrap();
    let before = doc.model().clone();
    doc.drain_events();
    doc.execute(
        "Separator",
        vec![Command::AddRoomSeparationLine(line.clone())],
    )
    .unwrap();
    let added = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    for id in [line.id(), room.id(), view.id()] {
        assert!(event.invalidated.contains(&id));
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &added);
    let mut moved = line.parameters.clone();
    moved.start.y = 2.0;
    moved.end.y = 2.0;
    doc.execute(
        "Move",
        vec![Command::UpdateRoomSeparationLine {
            id: line.id(),
            parameters: moved,
        }],
    )
    .unwrap();
    assert_eq!(
        doc.model().room_separation_lines[&line.id()].header,
        line.header
    );
    assert_eq!(doc.model().rooms[&room.id()], room);
    let edited = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &added);
    assert!(doc.redo());
    assert_eq!(doc.model(), &edited);
    doc.drain_events();
    let revision = doc.revision();
    let stats = doc.history_stats();
    let mut invalid = line.parameters.clone();
    invalid.level = Id::new();
    assert!(
        doc.execute(
            "Bad batch",
            vec![
                Command::RemoveRoomSeparationLine(line.id()),
                Command::AddRoomSeparationLine(RoomSeparationLine::new(
                    "core.room_separation_line",
                    invalid
                ))
            ]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &edited);
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), stats);
    assert!(doc.drain_events().is_empty());
    assert!(
        doc.execute(
            "Duplicate",
            vec![Command::AddRoomSeparationLine(line.clone())]
        )
        .is_err()
    );
    assert!(
        doc.execute(
            "Missing",
            vec![Command::RemoveRoomSeparationLine(Id::new())]
        )
        .is_err()
    );
    doc.execute("Delete", vec![Command::RemoveRoomSeparationLine(line.id())])
        .unwrap();
    assert_eq!(doc.model().rooms[&room.id()], room);
    assert!(doc.drain_events()[0].invalidated.contains(&room.id()));
    assert!(doc.undo());
    assert_eq!(doc.model(), &edited);
    assert!(doc.redo());
    assert!(doc.model().room_separation_lines.is_empty());
}
