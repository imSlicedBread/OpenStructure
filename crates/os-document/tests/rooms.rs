use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::{Level, LevelParams, Room, RoomParams, View, ViewParams, Wall, WallParams};

fn setup() -> (Document, Room, Wall) {
    let doc = Document::new("Rooms").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Partition".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(5., 0.),
                end: Point2::new(5., 8.),
            },
            height: 3.,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let room = Room::new(
        "core.room",
        RoomParams {
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            number: "101".into(),
            name: "Office".into(),
            level,
            seed: Point2::new(2., 4.),
            boundary_signature: vec![(wall.id(), true), (Id::new(), true), (Id::new(), false)],
        },
    );
    (doc, room, wall)
}

#[test]
fn room_material_update_remove_rejection_and_history_are_atomic() {
    use os_model::{Material, MaterialParams};
    for slot in 0..3 {
        let (mut doc, room, _) = setup();
        let material = Material::new(
            "core.material",
            MaterialParams {
                name: "Oak".into(),
                density_kg_m3: 650.,
                color: [160, 100, 60],
            },
        );
        let material_id = material.id();
        doc.execute(
            "Setup",
            vec![
                Command::AddMaterial(material),
                Command::AddRoom(room.clone()),
            ],
        )
        .unwrap();
        let before = doc.model().clone();
        let mut parameters = room.parameters.clone();
        parameters.floor_finish = Some("F-01".into());
        parameters.wall_finish = Some("W-02".into());
        parameters.ceiling_finish = Some("C-03".into());
        parameters.name = "Study".into();
        *[
            &mut parameters.floor_material,
            &mut parameters.wall_material,
            &mut parameters.ceiling_material,
        ][slot] = Some(material_id);
        doc.execute(
            "Assign",
            vec![Command::UpdateRoom {
                id: room.id(),
                parameters: parameters.clone(),
            }],
        )
        .unwrap();
        let assigned = doc.model().clone();
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(doc.redo());
        assert_eq!(doc.model(), &assigned);
        doc.drain_events();
        let revision = doc.revision();
        let history = doc.history_stats();
        let mut invalid = parameters.clone();
        invalid.floor_material = Some(Id::new());
        for command in [
            Command::RemoveMaterial(material_id),
            Command::UpdateRoom {
                id: room.id(),
                parameters: invalid,
            },
        ] {
            assert!(
                doc.execute(
                    "Invalid batch",
                    vec![Command::RenameProject("Rejected".into()), command]
                )
                .is_err()
            );
            assert_eq!(doc.model(), &assigned);
            assert_eq!(doc.revision(), revision);
            assert_eq!(doc.history_stats(), history);
            assert!(doc.drain_events().is_empty());
        }
        parameters.floor_material = None;
        parameters.wall_material = None;
        parameters.ceiling_material = None;
        // Final whole-model state is authoritative, even if removal precedes clearing the reference.
        doc.execute(
            "Unassign and remove",
            vec![
                Command::RemoveMaterial(material_id),
                Command::UpdateRoom {
                    id: room.id(),
                    parameters,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            doc.model().rooms[&room.id()]
                .parameters
                .floor_finish
                .as_deref(),
            Some("F-01")
        );
        assert!(doc.undo());
        assert_eq!(doc.model(), &assigned);
        assert!(doc.redo());
        assert!(!doc.model().materials.contains_key(&material_id));
    }
}

#[test]
fn room_commands_are_atomic_and_preserve_identity_through_history() {
    let (mut doc, room, _) = setup();
    doc.execute("Place", vec![Command::AddRoom(room.clone())])
        .unwrap();
    let original = doc.model().clone();
    let mut changed = room.parameters.clone();
    changed.name = "Study".into();
    changed.number = "102".into();
    changed.floor_finish = Some("F-01".into());
    changed.wall_finish = Some("W-02".into());
    changed.ceiling_finish = Some("C-03".into());
    doc.execute(
        "Edit",
        vec![Command::UpdateRoom {
            id: room.id(),
            parameters: changed,
        }],
    )
    .unwrap();
    assert_eq!(doc.model().rooms[&room.id()].header, room.header);
    let edited = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &original);
    assert!(doc.redo());
    assert_eq!(doc.model(), &edited);
    doc.drain_events();
    let revision = doc.revision();
    let stats = doc.history_stats();
    let mut invalid = room.parameters.clone();
    invalid.floor_finish = Some("invalid\nfinish".into());
    assert!(
        doc.execute(
            "Invalid batch",
            vec![
                Command::RenameProject("Bad".into()),
                Command::UpdateRoom {
                    id: room.id(),
                    parameters: invalid
                }
            ]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &edited);
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), stats);
    assert!(doc.drain_events().is_empty());
    assert!(
        doc.execute("Duplicate", vec![Command::AddRoom(room.clone())])
            .is_err()
    );
    assert!(
        doc.execute(
            "Remove level",
            vec![Command::RemoveLevel(room.parameters.level)]
        )
        .is_err()
    );
    doc.execute("Delete room", vec![Command::RemoveRoom(room.id())])
        .unwrap();
    assert!(!doc.model().rooms.contains_key(&room.id()));
    assert!(doc.undo());
    assert_eq!(doc.model(), &edited);
    assert!(doc.redo());
    assert!(doc.model().rooms.is_empty());
}

#[test]
fn moved_deleted_and_releveled_walls_invalidate_rooms_without_rewriting_intent() {
    let (mut doc, room, wall) = setup();
    let level = room.parameters.level;
    let other = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.,
            building: doc.model().levels[&level].parameters.building,
        },
    );
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    doc.execute(
        "Setup",
        vec![
            Command::AddRoom(room.clone()),
            Command::AddWall(wall.clone()),
            Command::AddLevel(other.clone()),
            Command::AddView(view.clone()),
        ],
    )
    .unwrap();
    doc.drain_events();
    let mut moved = wall.parameters.clone();
    moved.path.straight_start_mut().unwrap().x = 1.;
    moved.path.straight_end_mut().unwrap().x = 1.;
    doc.execute(
        "Cross seed",
        vec![Command::UpdateWall {
            id: wall.id(),
            parameters: moved,
        }],
    )
    .unwrap();
    assert_eq!(doc.model().rooms[&room.id()], room);
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&room.id()));
    assert!(event.invalidated.contains(&view.id()));
    doc.execute("Delete partition", vec![Command::RemoveWall(wall.id())])
        .unwrap();
    assert_eq!(doc.model().rooms[&room.id()], room);
    assert!(doc.drain_events()[0].invalidated.contains(&room.id()));
    assert!(doc.undo());
    assert!(doc.drain_events()[0].invalidated.contains(&room.id()));
    let mut upper = wall.parameters.clone();
    upper.level = other.id();
    doc.execute(
        "Change level",
        vec![Command::UpdateWall {
            id: wall.id(),
            parameters: upper,
        }],
    )
    .unwrap();
    assert!(doc.drain_events()[0].invalidated.contains(&room.id()));
    assert!(
        doc.model()
            .room_boundary_segments(level)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        doc.model()
            .room_boundary_segments(other.id())
            .unwrap()
            .len(),
        1
    );
    assert!(doc.model().room_boundary_segments(Id::new()).is_err());
}
