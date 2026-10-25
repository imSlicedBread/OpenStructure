use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
#[test]
fn drafting_create_settings_history_and_owned_annotation_sheet_deletion_guards() {
    let mut doc = Document::new("Drafting").unwrap();
    let view = View::new("core.view", ViewParams::drafting("Detail"));
    let id = view.id();
    let before = doc.model().clone();
    let preview = doc
        .preview_commands(vec![Command::AddView(view.clone())])
        .unwrap();
    assert_eq!(doc.model(), &before);
    doc.execute("Create", vec![Command::AddView(view)]).unwrap();
    assert_eq!(doc.model(), &preview);
    doc.undo();
    assert_eq!(doc.model(), &before);
    doc.redo();
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view: id,
            start: Point2::default(),
            end: Point2::new(1., 0.),
        },
    );
    let lid = line.id();
    let note = TextNote::new("core.text_note", TextNoteParams::new(id));
    let nid = note.id();
    doc.execute(
        "Annotations",
        vec![Command::AddDetailLine(line), Command::AddTextNote(note)],
    )
    .unwrap();
    let stable = doc.model().clone();
    let history = doc.history_stats();
    assert!(
        doc.execute("Remove", vec![Command::RemoveView(id)])
            .is_err()
    );
    assert_eq!(doc.model(), &stable);
    assert_eq!(doc.history_stats(), history);
    doc.execute(
        "Remove annotations",
        vec![Command::RemoveDetailLine(lid), Command::RemoveTextNote(nid)],
    )
    .unwrap();
    let mut parameters = doc.model().views[&id].parameters.clone();
    parameters.name = "Renamed".into();
    parameters.drafting.as_mut().unwrap().scale_denominator = 25.;
    doc.execute("Settings", vec![Command::UpdateView { id, parameters }])
        .unwrap();
    assert_eq!(doc.model().views[&id].parameters.settings_revision, 1);
    let mut sheet = Sheet::new("core.sheet", SheetParams::new("D1", "Detail"));
    sheet.parameters.viewports.push(SheetViewport {
        id: os_core::Id::new(),
        view: id,
        model_center_m: Point2::default(),
        paper_center_mm: Point2::new(150., 100.),
        width_mm: 100.,
        height_mm: 100.,
        scale_denominator: 25.,
        detail_number: "1".into(),
        title_override: None,
    });
    let sid = sheet.id();
    doc.execute("Sheet", vec![Command::AddSheet(sheet)])
        .unwrap();
    assert!(
        doc.execute("Remove", vec![Command::RemoveView(id)])
            .is_err()
    );
    doc.execute(
        "Explicit remove",
        vec![Command::RemoveSheet(sid), Command::RemoveView(id)],
    )
    .unwrap();
    assert!(!doc.model().views.contains_key(&id));
    doc.undo();
    assert!(doc.model().views.contains_key(&id));
}
