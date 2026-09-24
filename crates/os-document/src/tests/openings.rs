use super::*;
use os_core::Point2;

fn fixture() -> (Document, Id, Vec<Id>, Vec<Id>, Id) {
    let mut doc = Document::new("Shared openings").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            family: Default::default(),
            name: "Door 900".into(),
            pane_position: Default::default(),
            kind: OpeningKind::Door,
            width: 0.9,
            height: 2.1,
            sill: 0.0,
        },
    );
    let type_id = ty.id();
    let mut commands = vec![Command::AddOpeningType(ty)];
    let mut walls = Vec::new();
    let mut openings = Vec::new();
    for y in [0.0, 4.0] {
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Host".into(),
                start: Point2::new(0.0, y),
                end: Point2::new(10.0, y),
                thickness: 0.2,
                height: 3.0,
                level,
                material: None,
            },
        );
        let host = wall.id();
        walls.push(host);
        commands.push(Command::AddWall(wall));
        for offset in [1.0, 3.0] {
            let opening = Opening::new(
                "core.opening",
                OpeningParams {
                    hinge: Default::default(),
                    swing: Default::default(),
                    name: "Door".into(),
                    host,
                    offset,
                    definition: OpeningDefinition::Typed { type_id },
                },
            );
            openings.push(opening.id());
            commands.push(Command::AddOpening(opening));
        }
    }
    let view = View::new(
        "core.view",
        ViewParams {
            name: "Plan".into(),
            kind: ViewKind::Plan,
            level: Some(level),
            settings_revision: 0,
            plan: Some(PlanSettings::default()),
            section: None,
        },
    );
    let view_id = view.id();
    commands.push(Command::AddView(view));
    doc.execute("Place shared doors", commands).unwrap();
    doc.drain_events();
    (doc, type_id, openings, walls, view_id)
}

#[test]
fn shared_type_edit_renames_and_resizes_all_instances_in_one_undo_step() {
    let (mut doc, ty, openings, walls, view) = fixture();
    let before = doc.model().clone();
    let revision = doc.revision();
    let mut p = before.opening_types[&ty].parameters.clone();
    p.name = "Door 1200".into();
    p.width = 1.2;
    p.height = 2.2;
    doc.execute(
        "Edit type",
        vec![Command::UpdateOpeningType {
            id: ty,
            parameters: p,
        }],
    )
    .unwrap();
    assert_eq!(doc.revision(), revision + 1);
    assert_eq!(doc.model().openings, before.openings); // no cached dimensions rewritten
    for id in &openings {
        let resolved = doc
            .model()
            .resolve_opening(&doc.model().openings[id].parameters)
            .unwrap();
        assert_eq!((resolved.width, resolved.height), (1.2, 2.2));
        assert_eq!(resolved.type_name.as_deref(), Some("Door 1200"));
        assert_eq!(resolved.type_id, Some(ty));
    }
    let after = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    assert_eq!(event.changed, BTreeSet::from([ty]));
    for id in openings.iter().chain(walls.iter()).chain([&view, &ty]) {
        assert!(
            event.invalidated.contains(id),
            "missing invalidation for {id}"
        );
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    let undo_event = doc.drain_events().pop().unwrap();
    assert_eq!(undo_event.invalidated, event.invalidated);
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
    assert_eq!(
        doc.drain_events().pop().unwrap().invalidated,
        event.invalidated
    );
}

#[test]
fn invalid_type_edits_roll_back_model_history_revision_and_events() {
    let (mut doc, ty, _, _, _) = fixture();
    // Keep redo state as well as undo state available during failed edits.
    doc.execute("Rename", vec![Command::RenameProject("Other".into())])
        .unwrap();
    doc.undo();
    doc.drain_events();
    let before = doc.model().clone();
    let revision = doc.revision();
    let history = doc.history_stats();
    for case in 0..5 {
        let mut p = before.opening_types[&ty].parameters.clone();
        match case {
            0 => p.width = 2.0,  // touches the next opening
            1 => p.width = 10.0, // host end clearance
            2 => p.height = 3.0, // head clearance
            3 => p.kind = OpeningKind::Window,
            _ => p.name.clear(),
        }
        assert!(
            doc.execute(
                "Invalid type",
                vec![
                    Command::RenameProject("Should roll back".into()),
                    Command::UpdateOpeningType {
                        id: ty,
                        parameters: p
                    }
                ]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.history_stats(), history);
        assert!(doc.drain_events().is_empty());
        assert!(doc.can_redo());
    }
    assert!(
        doc.execute("Delete in use", vec![Command::RemoveOpeningType(ty)])
            .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    assert!(doc.drain_events().is_empty());
}

#[test]
fn conversion_assignment_and_type_removal_are_atomic_and_undoable() {
    let (mut doc, old_type, openings, _, _) = fixture();
    let original = doc.model().clone();
    let new_type = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            family: Default::default(),
            name: "New type".into(),
            ..original.opening_types[&old_type].parameters.clone()
        },
    );
    let new_id = new_type.id();
    let mut commands = vec![
        Command::AddOpeningType(new_type),
        Command::RemoveOpeningType(old_type),
    ];
    for id in &openings {
        let mut p = original.openings[id].parameters.clone();
        p.definition = OpeningDefinition::Typed { type_id: new_id };
        commands.push(Command::UpdateOpening {
            id: *id,
            parameters: p,
        });
    }
    doc.execute("Assign replacement type", commands).unwrap();
    assert!(!doc.model().opening_types.contains_key(&old_type));
    for id in &openings {
        assert_eq!(doc.model().openings[id].parameters.type_id(), Some(new_id));
    }
    doc.undo();
    assert_eq!(doc.model(), &original);
    let id = openings[0];
    let mut p = doc.model().openings[&id].parameters.clone();
    p.definition = OpeningDefinition::Legacy {
        kind: OpeningKind::Door,
        width: 0.9,
        height: 2.1,
        sill: 0.0,
    };
    doc.execute(
        "Legacy size",
        vec![Command::UpdateOpening { id, parameters: p }],
    )
    .unwrap();
    let legacy = doc.model().clone();
    let ty = OpeningType::new(
        "core.opening_type",
        original.opening_types[&old_type].parameters.clone(),
    );
    let mut p = legacy.openings[&id].parameters.clone();
    p.definition = OpeningDefinition::Typed { type_id: ty.id() };
    doc.execute(
        "Create type from opening",
        vec![
            Command::AddOpeningType(ty),
            Command::UpdateOpening { id, parameters: p },
        ],
    )
    .unwrap();
    doc.undo();
    assert_eq!(doc.model(), &legacy);
}
