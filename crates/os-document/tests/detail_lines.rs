use os_core::Point2;
use os_document::{Command, Document};
use os_model::{DetailLine, DetailLineParams, View, ViewParams};

#[test]
fn add_edit_remove_are_atomic_history_steps_and_plan_deletion_requires_line_removal() {
    let mut doc = Document::new("Details").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let view_id = view.id();
    doc.execute("Plan", vec![Command::AddView(view)]).unwrap();
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view: view_id,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(2.0, 0.0),
        },
    );
    let id = line.id();
    let before = doc.model().clone();
    doc.execute("Draw", vec![Command::AddDetailLine(line)])
        .unwrap();
    let added = doc.model().clone();
    assert_eq!(doc.revision(), 2);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &added);
    let mut edited = added.detail_lines[&id].parameters.clone();
    edited.end = Point2::new(3.0, 1.0);
    doc.execute(
        "Edit",
        vec![Command::UpdateDetailLine {
            id,
            parameters: edited.clone(),
        }],
    )
    .unwrap();
    assert_eq!(doc.model().detail_lines[&id].parameters, edited);
    assert_eq!(doc.model().detail_lines[&id].id(), id);
    let changed = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &added);
    assert!(doc.redo());
    assert_eq!(doc.model(), &changed);
    let revision = doc.revision();
    assert!(
        doc.execute("Delete plan", vec![Command::RemoveView(view_id)])
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.model(), &changed);
    doc.execute("Erase", vec![Command::RemoveDetailLine(id)])
        .unwrap();
    assert!(!doc.model().detail_lines.contains_key(&id));
    assert!(doc.undo());
    assert_eq!(doc.model(), &changed);
    assert!(doc.redo());
    assert!(!doc.model().detail_lines.contains_key(&id));
    doc.execute("Delete plan", vec![Command::RemoveView(view_id)])
        .unwrap();
}
