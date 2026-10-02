use super::*;
use os_core::Point2;

#[test]
fn two_bay_lite_only_material_removal_is_atomic() {
    let (mut doc, type_id, _, _, _) = fixture();
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Lite".into(),
            density_kg_m3: 2500.,
            color: [100, 150, 190],
        },
    );
    let id = material.id();
    let mut p = doc.model().opening_types[&type_id].parameters.clone();
    p.family.side_lite = Some(os_model::SideLite {
        side: os_model::LiteSide::Start,
        width_fraction: 0.25,
        mullion_width: 0.05,
        material: Some(id),
    });
    doc.execute(
        "Lite",
        vec![
            Command::AddMaterial(material),
            Command::UpdateOpeningType {
                id: type_id,
                parameters: p,
            },
        ],
    )
    .unwrap();
    let before = doc.model().clone();
    let history = doc.history_stats();
    assert!(
        doc.execute("Remove lite material", vec![Command::RemoveMaterial(id)])
            .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
}

#[test]
fn opening_materials_references_are_validated_atomically_with_history() {
    let (mut doc, type_id, _, _, _) = fixture();
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Timber".into(),
            density_kg_m3: 500.,
            color: [180, 180, 180],
        },
    );
    let id = material.id();
    doc.execute("Material", vec![Command::AddMaterial(material)])
        .unwrap();
    let before = doc.model().clone();
    let history = doc.history_stats();
    let mut p = before.opening_types[&type_id].parameters.clone();
    p.family.panel_material = Some(id);
    p.family.frame_material = Some(Id::new());
    assert!(
        doc.execute(
            "Invalid",
            vec![Command::UpdateOpeningType {
                id: type_id,
                parameters: p.clone()
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    p.family.frame_material = Some(id);
    doc.execute(
        "Materials",
        vec![Command::UpdateOpeningType {
            id: type_id,
            parameters: p,
        }],
    )
    .unwrap();
    let after = doc.model().clone();
    let history_after = doc.history_stats();
    assert!(
        doc.execute(
            "Remove referenced material",
            vec![Command::RemoveMaterial(id)]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &after);
    assert_eq!(doc.history_stats(), history_after);
    assert_eq!(after.openings, before.openings);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
}

fn fixture() -> (Document, Id, Vec<Id>, Vec<Id>, Id) {
    let mut doc = Document::new("Shared openings").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
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
                path: os_model::WallPath::Straight {
                    start: Point2::new(0.0, y),
                    end: Point2::new(10.0, y),
                },
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
                    open_state: Default::default(),
                    width_override: None,
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
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
fn instance_dimensions_propagate_only_to_inherited_sizes_and_keep_atomic_history() {
    let (mut doc, ty, openings, walls, view) = fixture();
    let id = openings[0];
    let before = doc.model().clone();
    let history = doc.history_stats();
    let mut parameters = before.openings[&id].parameters.clone();
    parameters.width_override = Some(0.9);
    parameters.height_override = Some(2.1);
    doc.execute(
        "Pin dimensions",
        vec![Command::UpdateOpening { id, parameters }],
    )
    .unwrap();
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    let pinned = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    for dependent in [id, walls[0], view] {
        assert!(event.invalidated.contains(&dependent));
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert_eq!(
        doc.drain_events().pop().unwrap().invalidated,
        event.invalidated
    );
    assert!(doc.redo());
    assert_eq!(doc.model(), &pinned);
    assert_eq!(
        doc.drain_events().pop().unwrap().invalidated,
        event.invalidated
    );
    let mut parameters = pinned.opening_types[&ty].parameters.clone();
    parameters.width = 1.2;
    parameters.height = 2.3;
    doc.execute(
        "Change default sizes",
        vec![Command::UpdateOpeningType { id: ty, parameters }],
    )
    .unwrap();
    let changed = doc.model().clone();
    for opening in &openings {
        let resolved = changed
            .resolve_opening(&changed.openings[opening].parameters)
            .unwrap();
        assert_eq!(
            (resolved.width, resolved.height),
            if *opening == id {
                (0.9, 2.1)
            } else {
                (1.2, 2.3)
            }
        );
    }
    let event = doc.drain_events().pop().unwrap();
    for dependent in openings.iter().chain(walls.iter()).chain([&view, &ty]) {
        assert!(event.invalidated.contains(dependent));
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &pinned);
    assert!(doc.redo());
    assert_eq!(doc.model(), &changed);
    doc.drain_events();
    let history = doc.history_stats();
    let revision = doc.revision();
    for (width, height) in [
        (Some(2.5), None),
        (None, Some(3.0)),
        (Some(0.0005), None),
        (None, Some(f64::NAN)),
    ] {
        let mut p = changed.openings[&id].parameters.clone();
        p.width_override = width;
        p.height_override = height;
        assert!(
            doc.execute(
                "Invalid size",
                vec![Command::UpdateOpening { id, parameters: p }]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &changed);
        assert_eq!(doc.history_stats(), history);
        assert_eq!(doc.revision(), revision);
        assert!(doc.drain_events().is_empty());
    }
    let mut p = changed.opening_types[&ty].parameters.clone();
    p.width = 2.5; // inherited instances overlap
    assert!(
        doc.execute(
            "Invalid shared size",
            vec![Command::UpdateOpeningType {
                id: ty,
                parameters: p
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &changed);
    assert_eq!(doc.history_stats(), history);
    let mut p = changed.openings[&id].parameters.clone();
    p.width_override = Some(0.3);
    doc.execute(
        "Narrow pin",
        vec![Command::UpdateOpening { id, parameters: p }],
    )
    .unwrap();
    let before_frame = doc.model().clone();
    let mut p = before_frame.opening_types[&ty].parameters.clone();
    p.family.frame_width = 0.2; // default fits, pinned instance does not
    assert!(
        doc.execute(
            "Invalid frame",
            vec![Command::UpdateOpeningType {
                id: ty,
                parameters: p
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before_frame);
}

#[test]
fn sill_override_and_type_default_are_atomic_undoable_and_invalidate_dependents() {
    let (mut doc, ty, openings, walls, view) = fixture();
    let mut model = doc.model().clone();
    let parameters = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
    parameters.kind = OpeningKind::Window;
    parameters.height = 1.2;
    parameters.sill = 0.8;
    let schedule = os_model::Schedule::new(
        "core.schedule",
        os_model::ScheduleParams::new("Windows", os_model::ScheduleCategory::Window),
    );
    let schedule_id = schedule.id();
    model.schedules.insert(schedule_id, schedule);
    doc = Document::from_model(model).unwrap();
    doc.drain_events();
    let before = doc.model().clone();
    let history = doc.history_stats();
    let id = openings[0];
    let mut parameters = before.openings[&id].parameters.clone();
    parameters.sill_override = Some(0.8); // equal is still pinned
    doc.execute("Pin sill", vec![Command::UpdateOpening { id, parameters }])
        .unwrap();
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    let pinned = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    for dependent in [id, walls[0], view, schedule_id] {
        assert!(event.invalidated.contains(&dependent));
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert_eq!(
        doc.drain_events().pop().unwrap().invalidated,
        event.invalidated
    );
    assert!(doc.redo());
    assert_eq!(doc.model(), &pinned);
    assert_eq!(
        doc.drain_events().pop().unwrap().invalidated,
        event.invalidated
    );
    let mut parameters = pinned.opening_types[&ty].parameters.clone();
    parameters.sill = 1.0;
    doc.execute(
        "Default sill",
        vec![Command::UpdateOpeningType { id: ty, parameters }],
    )
    .unwrap();
    let changed = doc.model().clone();
    for opening in &openings {
        assert_eq!(
            changed
                .resolve_opening(&changed.openings[opening].parameters)
                .unwrap()
                .sill,
            if *opening == id { 0.8 } else { 1.0 }
        );
    }
    let event = doc.drain_events().pop().unwrap();
    for dependent in openings.iter().chain(walls.iter()).chain([&view, &ty]) {
        assert!(event.invalidated.contains(dependent));
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &pinned);
    assert!(doc.redo());
    assert_eq!(doc.model(), &changed);
    doc.drain_events();
    let history = doc.history_stats();
    let revision = doc.revision();
    for invalid in [f64::NAN, 0.0005, -1.0, 1.8] {
        let mut p = changed.openings[&id].parameters.clone();
        p.sill_override = Some(invalid);
        assert!(
            doc.execute(
                "Invalid sill",
                vec![Command::UpdateOpening { id, parameters: p }]
            )
            .is_err()
        );
        let mut p = changed.opening_types[&ty].parameters.clone();
        p.sill = invalid;
        assert!(
            doc.execute(
                "Invalid default",
                vec![Command::UpdateOpeningType {
                    id: ty,
                    parameters: p
                }]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &changed);
        assert_eq!(doc.history_stats(), history);
        assert_eq!(doc.revision(), revision);
        assert!(doc.drain_events().is_empty());
    }
    let mut p = changed.openings[&id].parameters.clone();
    p.sill_override = Some(0.1);
    p.offset = changed.openings[&openings[1]].parameters.offset;
    assert!(
        doc.execute(
            "Overlap",
            vec![Command::UpdateOpening { id, parameters: p }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &changed);
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
fn rehosting_opening_invalidates_old_and_new_hosts_through_history() {
    let (mut doc, _, openings, walls, _) = fixture();
    let before = doc.model().clone();
    let id = openings[0];
    let mut parameters = before.openings[&id].parameters.clone();
    parameters.host = walls[1];
    parameters.offset = 5.0;

    doc.execute(
        "Rehost opening",
        vec![Command::UpdateOpening { id, parameters }],
    )
    .unwrap();
    let committed = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.changed.contains(&id));
    assert!(
        event
            .invalidated
            .is_superset(&BTreeSet::from([walls[0], walls[1], id]))
    );

    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    let undo = doc.drain_events().pop().unwrap();
    assert!(
        undo.invalidated
            .is_superset(&BTreeSet::from([walls[0], walls[1], id]))
    );

    assert!(doc.redo());
    assert_eq!(doc.model(), &committed);
    let redo = doc.drain_events().pop().unwrap();
    assert!(
        redo.invalidated
            .is_superset(&BTreeSet::from([walls[0], walls[1], id]))
    );
}

#[test]
fn conversion_assignment_and_type_removal_are_atomic_and_undoable() {
    let (mut doc, old_type, openings, _, _) = fixture();
    let original = doc.model().clone();
    let new_type = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
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
