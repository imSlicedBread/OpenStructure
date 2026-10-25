#[path = "../../os-model/tests/support/callouts.rs"]
mod support;
use os_document::{Command, Document};
use os_model::*;
use support::*;

#[test]
fn callout_enlarged_plan_preview_atomic_commit_and_one_step_history() {
    let (mut model, parent, _, marker) = fixture();
    model.plan_callouts.remove(&marker);
    let p = model
        .views
        .get_mut(&parent)
        .unwrap()
        .parameters
        .plan
        .as_mut()
        .unwrap();
    p.basis.rotation = 0.63;
    p.range.top = 4.;
    p.visibility.windows = false;
    let mut doc = Document::from_model(model).unwrap();
    let before = doc.model().clone();
    let history = doc.history_stats();
    let (marker, target, commands) = doc
        .enlarged_plan_commands(parent, "Enlarged".into(), crop(), 25.)
        .unwrap();
    let preview = doc.preview_commands(commands.clone()).unwrap();
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    assert_eq!(preview.plan_callouts[&marker].parameters.target, target);
    let mut expected = before.views[&parent].parameters.clone();
    expected.name = "Enlarged".into();
    expected.settings_revision = 0;
    expected.plan.as_mut().unwrap().crop = Some(crop());
    expected.plan.as_mut().unwrap().scale_denominator = 25.;
    assert_eq!(preview.views[&target].parameters, expected);
    assert_eq!(preview.walls, before.walls);
    assert_eq!(preview.dimensions, before.dimensions);
    assert_eq!(preview.detail_lines, before.detail_lines);
    doc.execute("Create callout", commands).unwrap();
    let after = doc.model().clone();
    assert_eq!(after, preview);
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
    let (_, _, mut invalid) = doc
        .enlarged_plan_commands(parent, "Failed".into(), crop(), 25.)
        .unwrap();
    invalid.push(Command::RemoveView(parent));
    let history = doc.history_stats();
    assert!(doc.execute("Invalid compound", invalid).is_err());
    assert_eq!(doc.model(), &after);
    assert_eq!(doc.history_stats(), history);
    assert!(
        doc.enlarged_plan_commands(parent, "Invalid scale".into(), crop(), 0.)
            .is_err()
    );
}

#[test]
fn callout_marker_crop_reference_transfer_and_deletion_are_undoable_and_deliberate() {
    let (mut model, parent, target, marker) = fixture();
    let (source, vp) = sheet(&mut model, target, "A201", "7");
    let dest = Sheet::new("core.sheet", SheetParams::new("A301", "Destination"));
    let dest_id = dest.id();
    model.sheets.insert(dest_id, dest);
    let mut doc = Document::from_model(model).unwrap();
    let original = doc.model().clone();
    let mut p = original.plan_callouts[&marker].parameters.clone();
    p.boundary.min.x -= 1.;
    p.boundary.max.x -= 1.;
    p.destination = Some(vp);
    doc.execute(
        "Move marker and choose reference",
        vec![Command::UpdatePlanCallout {
            id: marker,
            parameters: p.clone(),
        }],
    )
    .unwrap();
    assert_eq!(
        doc.model().views,
        original.views,
        "marker movement must not change target crop"
    );
    let before_transfer = doc.model().clone();
    let mut from = before_transfer.sheets[&source].parameters.clone();
    let mut to = before_transfer.sheets[&dest_id].parameters.clone();
    to.viewports.push(from.viewports.remove(0));
    doc.execute(
        "Transfer",
        vec![
            Command::UpdateSheet {
                id: source,
                parameters: from,
            },
            Command::UpdateSheet {
                id: dest_id,
                parameters: to,
            },
        ],
    )
    .unwrap();
    assert_eq!(doc.model().callout_reference(&p).label(), "7 / A301");
    let transferred = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before_transfer);
    assert!(doc.redo());
    assert_eq!(doc.model(), &transferred);
    for command in [
        Command::RemoveView(parent),
        Command::RemoveView(target),
        Command::RemoveSheet(dest_id),
    ] {
        let history = doc.history_stats();
        assert!(doc.execute("Dangling reference", vec![command]).is_err());
        assert_eq!(doc.model(), &transferred);
        assert_eq!(doc.history_stats(), history);
    }
    p.destination = None;
    doc.execute(
        "Explicitly clear and delete destination",
        vec![
            Command::UpdatePlanCallout {
                id: marker,
                parameters: p.clone(),
            },
            Command::RemoveSheet(dest_id),
        ],
    )
    .unwrap();
    assert_eq!(
        doc.model().callout_reference(&p),
        CalloutReference::Unplaced
    );
    doc.execute(
        "Remove marker only",
        vec![Command::RemovePlanCallout(marker)],
    )
    .unwrap();
    assert!(doc.model().views.contains_key(&target));
    assert!(doc.undo());
    assert!(doc.model().plan_callouts.contains_key(&marker));
}
