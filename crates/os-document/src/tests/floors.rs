use super::*;
use os_core::Point2;

fn fixture() -> (Document, Floor, Id) {
    let mut model = Model::new("Floors");
    let level = *model.levels.keys().next().unwrap();
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Concrete".into(),
            density_kg_m3: 2400.0,
        },
    );
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Slab".into(),
            level,
            material: Some(material.id()),
            boundary: vec![
                Point2::new(0., 0.),
                Point2::new(4., 0.),
                Point2::new(4., 1.),
                Point2::new(1., 1.),
                Point2::new(1., 4.),
                Point2::new(0., 4.),
            ],
            thickness: 0.2,
            top_offset: 0.0,
        },
    );
    model.materials.insert(material.id(), material);
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let view_id = view.id();
    model.views.insert(view_id, view);
    (Document::from_model(model).unwrap(), floor, view_id)
}

#[test]
fn floor_transactions_preserve_identity_and_invalidate_views_on_edit_delete_undo() {
    let (mut doc, floor, view) = fixture();
    let empty = doc.model().clone();
    let id = floor.id();
    doc.execute("Add floor", vec![Command::AddFloor(floor.clone())])
        .unwrap();
    assert!(doc.drain_events()[0].invalidated.contains(&view));
    assert_eq!(doc.history_stats().undo_entries, 1);
    assert!(doc.undo());
    assert_eq!(doc.model(), &empty);
    assert!(doc.redo());
    assert_eq!(doc.model().floors[&id], floor);
    doc.drain_events();
    let mut parameters = floor.parameters.clone();
    parameters.thickness = 0.35;
    parameters.top_offset = 0.1;
    doc.execute(
        "Edit floor",
        vec![Command::UpdateFloor {
            id,
            parameters: parameters.clone(),
        }],
    )
    .unwrap();
    assert_eq!(doc.model().floors[&id].header, floor.header);
    assert_eq!(doc.model().floors[&id].parameters, parameters);
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(event.invalidated.contains(&view));
    assert!(doc.undo());
    assert_eq!(doc.model().floors[&id], floor);
    assert!(doc.redo());
    doc.execute("Delete floor", vec![Command::RemoveFloor(id)])
        .unwrap();
    assert!(doc.model().floors.is_empty());
    assert!(
        doc.drain_events()
            .last()
            .unwrap()
            .invalidated
            .contains(&view)
    );
    assert!(doc.undo());
    assert_eq!(doc.model().floors[&id].parameters, parameters);
}

#[test]
fn invalid_edits_are_atomic_and_level_material_dependencies_reach_floor() {
    let (mut doc, floor, view) = fixture();
    let id = floor.id();
    doc.execute("Add", vec![Command::AddFloor(floor.clone())])
        .unwrap();
    doc.drain_events();
    let before = doc.model().clone();
    let revision = doc.revision();
    let entries = doc.history_stats().undo_entries;
    for case in 0..5 {
        let mut parameters = floor.parameters.clone();
        match case {
            0 => parameters.boundary.swap(1, 4),
            1 => parameters.level = Id::new(),
            2 => parameters.material = Some(Id::new()),
            3 => parameters.thickness = 0.0,
            _ => parameters.boundary[1] = parameters.boundary[0],
        }
        assert!(
            doc.execute("Invalid", vec![Command::UpdateFloor { id, parameters }])
                .is_err()
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.history_stats().undo_entries, entries);
        assert!(doc.drain_events().is_empty());
    }
    assert!(
        doc.execute(
            "Remove level",
            vec![Command::RemoveLevel(floor.parameters.level)]
        )
        .is_err()
    );
    let material = floor.parameters.material.unwrap();
    let affected = os_constraints::affected_entities(doc.model(), &BTreeSet::from([material]));
    assert!(affected.contains(&id));
    assert!(affected.contains(&view));
    let mut parameters = doc.model().levels[&floor.parameters.level]
        .parameters
        .clone();
    parameters.elevation = 3.0;
    doc.execute(
        "Raise level",
        vec![Command::UpdateLevel {
            id: floor.parameters.level,
            parameters,
        }],
    )
    .unwrap();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(event.invalidated.contains(&view));
    assert_eq!(doc.model().floors[&id], floor);
}
