use super::*;
use os_core::Point2;

fn fixture() -> (Document, Sheet, Id) {
    let mut model = Model::new("Sheets");
    let view = View::new(
        "core.view",
        ViewParams::floor_plan("Ground", *model.levels.keys().next().unwrap()),
    );
    let view_id = view.id();
    model.views.insert(view_id, view);
    let mut params = SheetParams::new("A101", "Ground sheet");
    params.viewports.push(SheetViewport {
        id: Id::new(),
        view: view_id,
        model_center_m: Point2::new(8.0, -3.0),
        paper_center_mm: Point2::new(210.0, 148.5),
        width_mm: 300.0,
        height_mm: 200.0,
        scale_denominator: 100.0,
        title_override: Some("Ground plan".into()),
    });
    (
        Document::from_model(model).unwrap(),
        Sheet::new("core.sheet", params),
        view_id,
    )
}

#[test]
fn sheets_atomic_commands_and_one_step_history_preserve_ids() {
    let (mut doc, sheet, _) = fixture();
    let original = doc.model().clone();
    let id = sheet.id();
    doc.execute("Add sheet", vec![Command::AddSheet(sheet.clone())])
        .unwrap();
    let added = doc.model().clone();
    assert_eq!(doc.history_stats().undo_entries, 1);
    assert!(doc.undo());
    assert_eq!(doc.model(), &original);
    assert!(doc.redo());
    assert_eq!(doc.model(), &added);
    let mut parameters = sheet.parameters.clone();
    parameters.number = "A102".into();
    parameters.name = "Revised".into();
    parameters.viewports[0].model_center_m = Point2::new(23.0, -19.0);
    parameters.viewports[0].scale_denominator = 50.0;
    parameters.viewports[0].title_override = None;
    parameters.viewports[0].width_mm = 200.0;
    parameters.viewports[0].paper_center_mm = Point2::new(120.0, 120.0);
    doc.execute("Edit sheet", vec![Command::UpdateSheet { id, parameters }])
        .unwrap();
    let edited = doc.model().clone();
    assert_eq!(edited.sheets[&id].header, sheet.header);
    assert_eq!(
        edited.sheets[&id].parameters.viewports[0].id,
        sheet.parameters.viewports[0].id
    );
    assert!(doc.undo());
    assert_eq!(doc.model(), &added);
    assert!(doc.redo());
    assert_eq!(doc.model(), &edited);
    doc.execute("Remove sheet", vec![Command::RemoveSheet(id)])
        .unwrap();
    assert!(doc.model().sheets.is_empty());
    assert!(doc.undo());
    assert_eq!(doc.model(), &edited);
    assert!(doc.redo());
    assert!(doc.model().sheets.is_empty());
}

#[test]
fn sheets_can_reference_a_plan_added_in_the_same_atomic_transaction() {
    let (mut doc, mut sheet, _) = fixture();
    let before = doc.model().clone();
    let level = *doc.model().levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("New source", level));
    sheet.parameters.viewports[0].view = view.id();
    doc.execute(
        "Create source and placement",
        vec![
            Command::AddSheet(sheet.clone()),
            Command::AddView(view.clone()),
        ],
    )
    .unwrap();
    assert_eq!(doc.model().sheets[&sheet.id()], sheet);
    assert_eq!(doc.model().views[&view.id()], view);
    assert_eq!(doc.history_stats().undo_entries, 1);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model().sheets[&sheet.id()], sheet);
}

#[test]
fn sheets_invalidation_reaches_extensions_referencing_placements() {
    let (mut doc, sheet, view) = fixture();
    let extension = ExtensionEntity {
        envelope_version: 1,
        id: Id::new(),
        owner: "org.example.sheet".into(),
        type_id: "org.example.sheet.item".into(),
        name: "Dependent".into(),
        payload_schema_version: 1,
        relationships: BTreeMap::new(),
        depends_on: BTreeSet::from([sheet.id(), sheet.parameters.viewports[0].id]),
        payload: Default::default(),
    };
    doc.execute(
        "Add dependent",
        vec![
            Command::AddSheet(sheet.clone()),
            Command::SetPluginRequirement {
                plugin_id: extension.owner.clone(),
                requirement: Some(PluginRequirement {
                    version: "1.0.0".into(),
                }),
            },
            Command::AddExtension(extension.clone()),
        ],
    )
    .unwrap();
    doc.drain_events();
    let mut parameters = doc.model().views[&view].parameters.clone();
    parameters.name = "Renamed source".into();
    doc.execute(
        "Source change",
        vec![Command::UpdateView {
            id: view,
            parameters,
        }],
    )
    .unwrap();
    for action in 0..3 {
        if action == 1 {
            assert!(doc.undo());
        }
        if action == 2 {
            assert!(doc.redo());
        }
        let event = doc.drain_events().pop().unwrap();
        assert!(event.invalidated.is_superset(&BTreeSet::from([
            view,
            sheet.id(),
            sheet.parameters.viewports[0].id,
            extension.id,
        ])));
    }
}

#[test]
fn sheets_rejected_transactions_preserve_model_revision_events_and_history() {
    let (mut doc, sheet, view) = fixture();
    doc.execute("Add", vec![Command::AddSheet(sheet.clone())])
        .unwrap();
    doc.drain_events();
    let original = doc.model().clone();
    let revision = doc.revision();
    let history = doc.history_stats();
    for case in 0..8 {
        let mut parameters = sheet.parameters.clone();
        let command = match case {
            0 => Command::AddSheet(sheet.clone()),
            1 => Command::RemoveSheet(Id::new()),
            2 => Command::UpdateSheet {
                id: Id::new(),
                parameters,
            },
            3 => {
                parameters.viewports[0].view = Id::new();
                Command::UpdateSheet {
                    id: sheet.id(),
                    parameters,
                }
            }
            4 => {
                parameters.viewports[0].width_mm = -1.0;
                Command::UpdateSheet {
                    id: sheet.id(),
                    parameters,
                }
            }
            5 => Command::AddSheet(Sheet::new("core.sheet", parameters)),
            6 => {
                parameters.viewports[0].id = view;
                Command::UpdateSheet {
                    id: sheet.id(),
                    parameters,
                }
            }
            _ => Command::RemoveView(view),
        };
        assert!(
            doc.execute(
                "Reject all",
                vec![Command::RenameProject("Must roll back".into()), command]
            )
            .is_err(),
            "case {case}"
        );
        assert_eq!(doc.model(), &original);
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.history_stats(), history);
        assert!(doc.drain_events().is_empty());
    }
    // Identical updates do not create history or events.
    doc.execute(
        "No-op",
        vec![Command::UpdateSheet {
            id: sheet.id(),
            parameters: sheet.parameters,
        }],
    )
    .unwrap();
    assert_eq!(doc.history_stats(), history);
    assert_eq!(doc.revision(), revision);
    assert!(doc.drain_events().is_empty());
}

#[test]
fn sheets_view_deletion_requires_prior_transactional_unplacement() {
    for remove_sheet in [false, true] {
        let (mut doc, sheet, view) = fixture();
        doc.execute("Add", vec![Command::AddSheet(sheet.clone())])
            .unwrap();
        let before = doc.model().clone();
        let unplace = if remove_sheet {
            Command::RemoveSheet(sheet.id())
        } else {
            let mut parameters = sheet.parameters.clone();
            parameters.viewports.clear();
            Command::UpdateSheet {
                id: sheet.id(),
                parameters,
            }
        };
        // Ordering is explicit: no temporary dangling placements at RemoveView.
        assert!(
            doc.execute(
                "Wrong order",
                vec![Command::RemoveView(view), unplace.clone()]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &before);
        doc.execute(
            "Unplace and remove",
            vec![unplace, Command::RemoveView(view)],
        )
        .unwrap();
        assert!(!doc.model().views.contains_key(&view));
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(doc.redo());
        assert!(!doc.model().views.contains_key(&view));
    }
}

#[test]
fn sheets_plan_reassignment_and_geometry_invalidate_before_and_after_placements() {
    let (mut doc, sheet, view) = fixture();
    let id = sheet.id();
    let viewport = sheet.parameters.viewports[0].id;
    let level = *doc.model().levels.keys().next().unwrap();
    let other = View::new("core.view", ViewParams::floor_plan("Other", level));
    let other_id = other.id();
    doc.execute(
        "Place",
        vec![Command::AddSheet(sheet.clone()), Command::AddView(other)],
    )
    .unwrap();
    let event = doc.drain_events().pop().unwrap();
    assert!(
        event
            .changed
            .is_superset(&BTreeSet::from([id, viewport, other_id]))
    );
    assert!(
        event
            .invalidated
            .is_superset(&BTreeSet::from([id, viewport, view]))
    );
    let mut parameters = sheet.parameters.clone();
    parameters.viewports[0].view = other_id;
    doc.execute("Reassign", vec![Command::UpdateSheet { id, parameters }])
        .unwrap();
    for action in 0..3 {
        if action == 1 {
            assert!(doc.undo());
        }
        if action == 2 {
            assert!(doc.redo());
        }
        let event = doc.drain_events().pop().unwrap();
        assert!(event.changed.contains(&viewport));
        assert!(
            event
                .invalidated
                .is_superset(&BTreeSet::from([id, viewport, view, other_id]))
        );
    }
    let mut params = doc.model().views[&other_id].parameters.clone();
    params.plan.as_mut().unwrap().basis.rotation = 0.7;
    doc.execute(
        "Rotate plan",
        vec![Command::UpdateView {
            id: other_id,
            parameters: params,
        }],
    )
    .unwrap();
    let event = doc.drain_events().pop().unwrap();
    assert_eq!(event.changed, BTreeSet::from([other_id]));
    assert!(
        event
            .invalidated
            .is_superset(&BTreeSet::from([id, viewport, other_id]))
    );
    assert_eq!(
        doc.model().sheets[&id].parameters.viewports[0].model_center_m,
        Point2::new(8.0, -3.0)
    );
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    doc.execute("Model geometry", vec![Command::AddWall(wall)])
        .unwrap();
    assert!(
        doc.drain_events()
            .pop()
            .unwrap()
            .invalidated
            .is_superset(&BTreeSet::from([id, viewport, other_id]))
    );
    doc.execute("Remove", vec![Command::RemoveSheet(id)])
        .unwrap();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.changed.is_superset(&BTreeSet::from([id, viewport])));
    assert!(
        event
            .invalidated
            .is_superset(&BTreeSet::from([id, viewport, other_id]))
    );
}
