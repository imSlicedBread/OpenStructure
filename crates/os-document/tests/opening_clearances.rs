use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;

fn fixture(sweep: f64) -> (Document, Id, Id, [Id; 2]) {
    let mut doc = Document::new("Clearances").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            level,
            height: 3.,
            thickness: 0.2,
            material: None,
            path: if sweep == 0. {
                WallPath::Straight {
                    start: Point2::default(),
                    end: Point2::new(10., 0.),
                }
            } else {
                WallPath::CircularArc {
                    center: Point2::default(),
                    radius: 5.,
                    start_angle_rad: 0.2,
                    signed_sweep_rad: sweep,
                }
            },
        },
    );
    let host = wall.id();
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Door".into(),
            kind: OpeningKind::Door,
            width: 1.,
            height: 2.1,
            sill: 0.,
            family: Default::default(),
            window_operation: Default::default(),
            pane_position: Default::default(),
        },
    );
    let type_id = ty.id();
    let openings = [1., 7.].map(|offset| {
        Opening::new(
            "core.opening",
            OpeningParams {
                open_state: Default::default(),
                name: "Door".into(),
                host,
                offset,
                definition: OpeningDefinition::Typed { type_id },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        )
    });
    let ids = [openings[0].id(), openings[1].id()];
    doc.execute(
        "Fixture",
        vec![
            Command::AddWall(wall),
            Command::AddOpeningType(ty),
            Command::AddOpening(openings[0].clone()),
            Command::AddOpening(openings[1].clone()),
        ],
    )
    .unwrap();
    doc.execute(
        "Lock",
        vec![
            lock(ids[0], ClearanceEnd::Start, 1.),
            lock(ids[1], ClearanceEnd::End, 2.),
        ],
    )
    .unwrap();
    (doc, host, type_id, ids)
}
fn lock(id: Id, end: ClearanceEnd, distance: f64) -> Command {
    Command::SetOpeningClearance {
        id,
        clearance: Some(OpeningClearance { end, distance }),
    }
}
fn offsets(doc: &Document, ids: [Id; 2]) -> [f64; 2] {
    ids.map(|id| doc.model().openings[&id].parameters.offset)
}

#[test]
fn clearance_lock_analytic_resize_type_width_batch_and_history() {
    for sweep in [0., 2., -2.] {
        let (mut doc, host, ty, ids) = fixture(sweep);
        let before = doc.model().clone();
        let history = doc.history_stats();
        let mut wall = before.walls[&host].parameters.clone();
        match &mut wall.path {
            WallPath::Straight { end, .. } => end.x = 12.,
            WallPath::CircularArc {
                signed_sweep_rad, ..
            } => *signed_sweep_rad *= 1.2,
        }
        let mut parameters = before.opening_types[&ty].parameters.clone();
        parameters.width = 1.5;
        doc.execute(
            "Resize host and type",
            vec![
                Command::UpdateWall {
                    id: host,
                    parameters: wall,
                },
                Command::UpdateOpeningType { id: ty, parameters },
            ],
        )
        .unwrap();
        assert_eq!(offsets(&doc, ids), [1., 8.5]);
        assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
        for id in ids {
            let mut expected = before.openings[&id].clone();
            expected.parameters.offset = doc.model().openings[&id].parameters.offset;
            assert_eq!(doc.model().openings[&id], expected);
        }
        let after = doc.model().clone();
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(doc.redo());
        assert_eq!(doc.model(), &after);
        let commands = ids.map(|id| {
            let mut parameters = doc.model().openings[&id].parameters.clone();
            parameters.width_override = Some(2.);
            Command::UpdateOpening { id, parameters }
        });
        doc.execute("Batch widths", commands.to_vec()).unwrap();
        assert_eq!(offsets(&doc, ids), [1., 8.]);
    }
}

#[test]
fn clearance_lock_rejects_explicit_move_even_with_simultaneous_host_resize() {
    for resize in [false, true] {
        let (mut doc, host, _, ids) = fixture(0.);
        let before = doc.model().clone();
        let history = doc.history_stats();
        let mut parameters = before.openings[&ids[1]].parameters.clone();
        parameters.offset = 6.;
        let mut commands = vec![Command::UpdateOpening {
            id: ids[1],
            parameters,
        }];
        if resize {
            let mut wall = before.walls[&host].parameters.clone();
            *wall.path.straight_end_mut().unwrap() = Point2::new(12., 0.);
            commands.push(Command::UpdateWall {
                id: host,
                parameters: wall,
            });
        }
        let error = doc
            .execute("Conflicting move", commands)
            .unwrap_err()
            .to_string();
        assert!(error.contains("clearance lock"), "{error}");
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.history_stats(), history);
    }
}

#[test]
fn clearance_lock_collision_fit_and_rehost_abort_atomically() {
    let (mut doc, host, ty, ids) = fixture(0.);
    let mut other = doc.model().walls[&host].clone();
    other.header.id = Id::new();
    let other_host = other.id();
    doc.execute("Other host", vec![Command::AddWall(other)])
        .unwrap();
    let before = doc.model().clone();
    let history = doc.history_stats();
    let mut wide = before.opening_types[&ty].parameters.clone();
    wide.width = 4.;
    let mut rehost = before.openings[&ids[0]].parameters.clone();
    rehost.host = other_host;
    for commands in [
        vec![Command::UpdateOpeningType {
            id: ty,
            parameters: wide,
        }],
        vec![lock(ids[0], ClearanceEnd::Start, 0.)],
        vec![lock(ids[0], ClearanceEnd::End, 100.)],
        vec![
            Command::UpdateOpening {
                id: ids[0],
                parameters: rehost,
            },
            Command::SetOpeningClearance {
                id: ids[0],
                clearance: None,
            },
        ],
    ] {
        assert!(doc.execute("Invalid", commands).is_err());
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.history_stats(), history);
    }
}

#[test]
fn clearance_lock_edit_unlock_and_removal_preserve_geometry_and_ownership() {
    let (mut doc, _, _, ids) = fixture(-2.);
    doc.execute("Edit clearance", vec![lock(ids[1], ClearanceEnd::End, 1.5)])
        .unwrap();
    assert_eq!(offsets(&doc, ids), [1., 7.5]);
    let before = doc.model().clone();
    doc.execute(
        "Unlock",
        vec![Command::SetOpeningClearance {
            id: ids[1],
            clearance: None,
        }],
    )
    .unwrap();
    assert_eq!(doc.model().openings, before.openings);
    assert!(!doc.model().opening_clearances.contains_key(&ids[1]));
    doc.execute("Delete", vec![Command::RemoveOpening(ids[0])])
        .unwrap();
    assert!(!doc.model().opening_clearances.contains_key(&ids[0]));
    doc.undo();
    assert!(doc.model().opening_clearances.contains_key(&ids[0]));
}
