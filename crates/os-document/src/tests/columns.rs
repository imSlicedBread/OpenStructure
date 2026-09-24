use super::*;
use os_core::Point2;

fn fixture() -> (Document, Column, Id, Id) {
    let mut model = Model::new("Columns");
    let level = *model.levels.keys().next().unwrap();
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Concrete".into(),
            density_kg_m3: 2400.0,
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    let column = Column::new(
        "core.column",
        ColumnParams {
            name: "C1".into(),
            level,
            center: Point2::new(2.0, 3.0),
            width: 0.4,
            depth: 0.6,
            height: 3.2,
            base_offset: 0.15,
            material: Some(material_id),
        },
    );
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let view_id = view.id();
    model.views.insert(view_id, view);
    (Document::from_model(model).unwrap(), column, level, view_id)
}

#[test]
fn column_transactions_keep_id_and_invalidate_on_edit_delete_and_undo() {
    let (mut doc, column, level, view) = fixture();
    let baseline = doc.model().clone();
    let id = column.id();
    doc.execute("Place column", vec![Command::AddColumn(column.clone())])
        .unwrap();
    assert_eq!(doc.history_stats().undo_entries, 1);
    assert!(doc.drain_events()[0].invalidated.contains(&view));
    assert!(doc.undo());
    assert_eq!(doc.model(), &baseline);
    assert!(doc.redo());
    assert_eq!(doc.model().columns[&id], column);

    doc.drain_events();
    let mut edited = column.parameters.clone();
    edited.center = Point2::new(4.0, -1.0);
    edited.width = 0.5;
    edited.base_offset = -0.1;
    doc.execute(
        "Edit column",
        vec![Command::UpdateColumn {
            id,
            parameters: edited.clone(),
        }],
    )
    .unwrap();
    assert_eq!(doc.model().columns[&id].header, column.header);
    assert_eq!(doc.model().columns[&id].parameters, edited);
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(event.invalidated.contains(&view));
    assert!(doc.undo());
    assert_eq!(doc.model().columns[&id], column);
    assert!(doc.redo());

    doc.execute("Delete column", vec![Command::RemoveColumn(id)])
        .unwrap();
    assert!(doc.model().columns.is_empty());
    assert!(
        doc.drain_events()
            .last()
            .unwrap()
            .invalidated
            .contains(&view)
    );
    assert!(doc.undo());
    assert_eq!(doc.model().columns[&id].parameters, edited);

    let old_elevation = doc.model().levels[&level].parameters.clone();
    doc.drain_events();
    doc.execute(
        "Raise level",
        vec![Command::UpdateLevel {
            id: level,
            parameters: LevelParams {
                elevation: 4.0,
                ..old_elevation
            },
        }],
    )
    .unwrap();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(event.invalidated.contains(&view));
    assert_eq!(doc.model().columns[&id].parameters, edited);
}

#[test]
fn invalid_column_edits_and_dependency_deletion_are_atomic() {
    let (mut doc, column, level, _) = fixture();
    let id = column.id();
    doc.execute("Place", vec![Command::AddColumn(column.clone())])
        .unwrap();
    doc.drain_events();
    let model = doc.model().clone();
    let revision = doc.revision();
    let history = doc.history_stats();
    for parameters in [
        ColumnParams {
            width: 0.0,
            ..column.parameters.clone()
        },
        ColumnParams {
            level: Id::new(),
            ..column.parameters.clone()
        },
        ColumnParams {
            material: Some(Id::new()),
            ..column.parameters.clone()
        },
        ColumnParams {
            center: Point2::new(f64::NAN, 0.0),
            ..column.parameters.clone()
        },
        ColumnParams {
            height: f64::INFINITY,
            ..column.parameters.clone()
        },
    ] {
        assert!(
            doc.execute(
                "Invalid column",
                vec![Command::UpdateColumn { id, parameters }]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &model);
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.history_stats(), history);
        assert!(doc.drain_events().is_empty());
    }
    assert!(
        doc.execute("Remove level", vec![Command::RemoveLevel(level)])
            .is_err()
    );
    let material = column.parameters.material.unwrap();
    assert!(
        doc.execute("Remove material", vec![Command::RemoveMaterial(material)])
            .is_err()
    );
    assert_eq!(doc.model(), &model);
}
