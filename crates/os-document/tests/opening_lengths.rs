use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;

fn fixture() -> (Document, Id, [Id; 2], [Id; 2]) {
    let mut doc = Document::new("Shared lengths").unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            level: *doc.model().levels.keys().next().unwrap(),
            height: 4.,
            thickness: 0.2,
            material: None,
            path: WallPath::Straight {
                start: Point2::default(),
                end: Point2::new(12., 0.),
            },
        },
    );
    let driver = LengthParameter::new(
        "core.length_parameter",
        LengthParameterParams {
            name: "Shared size".into(),
            unit: LengthUnit::Metres,
            value: 1.,
        },
    );
    let pid = driver.id();
    let types = [OpeningKind::Door, OpeningKind::Window].map(|kind| {
        OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                name: format!("{kind:?}"),
                kind,
                width: 1.,
                height: 1.,
                sill: if kind == OpeningKind::Door { 0. } else { 1. },
                family: Default::default(),
                window_operation: Default::default(),
                pane_position: Default::default(),
            },
        )
    });
    let tids = types.each_ref().map(|t| t.id());
    let openings = [0, 1].map(|i| {
        Opening::new(
            "core.opening",
            OpeningParams {
                open_state: Default::default(),
                name: format!("Opening {i}"),
                host: wall.id(),
                offset: 1. + i as f64 * 7.,
                definition: OpeningDefinition::Typed { type_id: tids[i] },
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
    let ids = openings.each_ref().map(|o| o.id());
    let mut commands = vec![Command::AddWall(wall), Command::AddLengthParameter(driver)];
    for ty in types {
        let id = ty.id();
        let window = ty.parameters.kind == OpeningKind::Window;
        commands.push(Command::AddOpeningType(ty));
        commands.push(Command::SetOpeningTypeLengthBindings {
            id,
            bindings: OpeningTypeLengthBindings {
                width: Some(pid),
                height: Some(pid),
                sill: window.then_some(pid),
            },
        });
    }
    commands.extend(openings.into_iter().map(Command::AddOpening));
    doc.execute("Setup", commands).unwrap();
    doc.execute(
        "Lock window",
        vec![Command::SetOpeningClearance {
            id: ids[1],
            clearance: Some(OpeningClearance {
                end: ClearanceEnd::End,
                distance: 3.,
            }),
        }],
    )
    .unwrap();
    (doc, pid, tids, ids)
}
fn edit(doc: &Document, id: Id, value: f64) -> Command {
    let mut parameters = doc.model().length_parameters[&id].parameters.clone();
    parameters.value = value;
    Command::UpdateLengthParameter { id, parameters }
}

#[test]
fn shared_lengths_preview_pins_locks_invalidation_atomic_history_and_reset() {
    let (mut doc, pid, types, ids) = fixture();
    let mut pinned = doc.model().openings[&ids[0]].parameters.clone();
    pinned.width_override = Some(1.); // Equal-default pin remains authored.
    doc.execute(
        "Pin",
        vec![Command::UpdateOpening {
            id: ids[0],
            parameters: pinned,
        }],
    )
    .unwrap();
    doc.drain_events();
    let before = doc.model().clone();
    let history = doc.history_stats();
    let revision = doc.revision();
    let candidate = doc.preview_commands(vec![edit(&doc, pid, 1.2)]).unwrap();
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), history);
    assert!(doc.drain_events().is_empty());
    assert!((candidate.openings[&ids[1]].parameters.offset - 7.8).abs() < 1e-9);
    let window = candidate
        .resolve_opening(&candidate.openings[&ids[1]].parameters)
        .unwrap();
    assert_eq!((window.width, window.height, window.sill), (1.2, 1.2, 1.2));
    let door = candidate
        .resolve_opening(&candidate.openings[&ids[0]].parameters)
        .unwrap();
    assert_eq!((door.width, door.height, door.sill), (1., 1.2, 0.));
    doc.execute("Shared change", vec![edit(&doc, pid, 1.2)])
        .unwrap();
    assert_eq!(doc.model(), &candidate);
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    let event = doc.drain_events().pop().unwrap();
    for id in types.into_iter().chain(ids).chain([window.host]) {
        assert!(event.invalidated.contains(&id));
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &candidate);
    let history = doc.history_stats();
    for value in [0., -1., f64::NAN, 3., 20.] {
        assert!(
            doc.execute("Invalid", vec![edit(&doc, pid, value)])
                .is_err()
        );
        assert_eq!(doc.model(), &candidate);
        assert_eq!(doc.history_stats(), history);
    }
    doc.execute("Unchanged", vec![edit(&doc, pid, 1.2)])
        .unwrap();
    assert_eq!(doc.history_stats(), history);
    let mut reset = doc.model().openings[&ids[0]].parameters.clone();
    reset.width_override = None;
    doc.execute(
        "Reset",
        vec![Command::UpdateOpening {
            id: ids[0],
            parameters: reset,
        }],
    )
    .unwrap();
    assert_eq!(
        doc.model()
            .resolve_opening(&doc.model().openings[&ids[0]].parameters)
            .unwrap()
            .width,
        1.2
    );
}

#[test]
fn shared_lengths_rename_delete_unbind_and_unplaced_validation() {
    let (mut doc, pid, types, _) = fixture();
    let mut p = doc.model().length_parameters[&pid].parameters.clone();
    p.name = "Renamed".into();
    doc.execute(
        "Rename",
        vec![Command::UpdateLengthParameter {
            id: pid,
            parameters: p,
        }],
    )
    .unwrap();
    assert_eq!(doc.model().length_parameter_uses(pid).len(), 5);
    let before = doc.model().clone();
    let history = doc.history_stats();
    assert!(
        doc.execute(
            "Delete referenced",
            vec![Command::RemoveLengthParameter(pid)]
        )
        .unwrap_err()
        .to_string()
        .contains("5 dimension")
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    doc.execute(
        "Freeze and delete",
        types
            .into_iter()
            .map(|id| Command::SetOpeningTypeLengthBindings {
                id,
                bindings: Default::default(),
            })
            .chain([Command::RemoveLengthParameter(pid)])
            .collect(),
    )
    .unwrap();
    assert!(doc.model().length_parameters.is_empty());
    assert_eq!(
        doc.model()
            .resolve_opening_type(types[1])
            .unwrap()
            .parameters
            .sill,
        1.
    );
    let mut unplaced = before.clone();
    unplaced.openings.clear();
    unplaced.opening_clearances.clear();
    unplaced
        .element_lifecycles
        .retain(|id, _| unplaced.walls.contains_key(id));
    unplaced
        .length_parameters
        .get_mut(&pid)
        .unwrap()
        .parameters
        .value = 0.;
    assert!(unplaced.validate().is_err());
    let mut door_sill = before;
    door_sill
        .opening_type_length_bindings
        .get_mut(&types[0])
        .unwrap()
        .sill = Some(pid);
    assert!(door_sill.validate().is_err());
}
