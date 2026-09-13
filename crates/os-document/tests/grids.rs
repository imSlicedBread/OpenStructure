use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::{Grid, GridParams, Model, View, ViewParams};

fn grid(model: &Model, name: &str) -> Grid {
    Grid::new(
        "core.grid",
        GridParams {
            name: name.into(),
            building: *model.buildings.keys().next().unwrap(),
            start: Point2::new(1_000_000.0, 2_000_000.0),
            end: Point2::new(1_000_006.0, 2_000_008.0),
        },
    )
}

#[test]
fn grid_edits_history_noops_and_plan_invalidation() {
    let mut model = Model::new("Grids");
    let plan = View::new(
        "core.view",
        ViewParams::floor_plan("Plan", *model.levels.keys().next().unwrap()),
    );
    model.views.insert(plan.id(), plan.clone());
    let grid = grid(&model, "A");
    let mut doc = Document::from_model(model.clone()).unwrap();
    doc.execute("Grid", vec![Command::AddGrid(grid.clone())])
        .unwrap();
    assert!(doc.model().estimated_memory_bytes() > model.estimated_memory_bytes());
    let event = doc.drain_events().pop().unwrap();
    assert!(event.changed.contains(&grid.id()));
    assert!(event.invalidated.contains(&grid.id()));
    assert!(event.invalidated.contains(&plan.id()));
    let created = doc.model().clone();
    let mut edit = grid.parameters.clone();
    edit.name = "B".into();
    edit.end.x += 2.0;
    doc.execute(
        "Move and rename",
        vec![Command::UpdateGrid {
            id: grid.id(),
            parameters: edit,
        }],
    )
    .unwrap();
    assert_eq!(doc.model().grids[&grid.id()].header, grid.header);
    let edited = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &created);
    let revision = doc.revision();
    let history = doc.history_stats();
    doc.execute(
        "Noop",
        vec![Command::UpdateGrid {
            id: grid.id(),
            parameters: grid.parameters.clone(),
        }],
    )
    .unwrap();
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), history);
    assert!(doc.redo());
    assert_eq!(doc.model(), &edited);
    doc.execute("Remove", vec![Command::RemoveGrid(grid.id())])
        .unwrap();
    assert!(doc.model().grids.is_empty());
    assert!(
        doc.drain_events()
            .last()
            .unwrap()
            .invalidated
            .contains(&plan.id())
    );
    assert!(doc.undo());
    assert_eq!(doc.model(), &edited);
}

#[test]
fn invalid_grids_and_reference_breaks_reject_entire_batch() {
    let mut doc = Document::new("Grids").unwrap();
    let valid = grid(doc.model(), "A");
    for case in 0..10 {
        let mut bad = valid.clone();
        match case {
            0 => bad.parameters.name = " A ".into(),
            1 => bad.parameters.name = "\n".into(),
            2 => bad.parameters.name = "é".repeat(129),
            3 => bad.parameters.end = bad.parameters.start,
            4 => bad.parameters.start.x = f64::NAN,
            5 => {
                bad.parameters.start.x = -f64::MAX;
                bad.parameters.end.x = f64::MAX;
            }
            6 => bad.parameters.building = Id::new(),
            7 => bad.header.id = doc.model().project.id(),
            8 => bad.header.schema_version = 3,
            _ => bad.header.type_id = "core.level".into(),
        }
        let before = doc.model().clone();
        let stats = doc.history_stats();
        let revision = doc.revision();
        assert!(
            doc.execute(
                "Invalid",
                vec![
                    Command::RenameProject("Must rollback".into()),
                    Command::AddGrid(bad)
                ]
            )
            .is_err(),
            "case {case}"
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.history_stats(), stats);
        assert_eq!(doc.revision(), revision);
        assert!(doc.drain_events().is_empty());
    }
    doc.execute("A", vec![Command::AddGrid(valid.clone())])
        .unwrap();
    let duplicate = grid(doc.model(), "A");
    assert!(
        doc.execute("Duplicate name", vec![Command::AddGrid(duplicate)])
            .is_err()
    );
    let mut dependent = grid(doc.model(), "B");
    dependent
        .header
        .relationships
        .insert("aligned".into(), vec![valid.id()]);
    doc.execute("B", vec![Command::AddGrid(dependent.clone())])
        .unwrap();
    let before = doc.model().clone();
    assert!(
        doc.execute("Dangling", vec![Command::RemoveGrid(valid.id())])
            .is_err()
    );
    assert_eq!(doc.model(), &before);
    doc.execute(
        "Remove both",
        vec![
            Command::RemoveGrid(valid.id()),
            Command::RemoveGrid(dependent.id()),
        ],
    )
    .unwrap();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
}
