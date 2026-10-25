#[path = "../../os-model/tests/support/drafting.rs"]
mod support;
use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
#[test]
fn drafting_preview_edit_delete_history_and_view_deletion_are_atomic() {
    for section in [false, true] {
        let (model, view) = support::fixture(section);
        let mut doc = Document::from_model(model).unwrap();
        let note = TextNote::new("core.text_note", TextNoteParams::new(view));
        let id = note.id();
        let line = DetailLine::new(
            "core.detail_line",
            DetailLineParams {
                view,
                start: Point2::default(),
                end: Point2::new(1., 2.),
            },
        );
        let before = doc.model().clone();
        let history = doc.history_stats();
        let commands = vec![Command::AddTextNote(note), Command::AddDetailLine(line)];
        let preview = doc.preview_commands(commands.clone()).unwrap();
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.history_stats(), history);
        doc.execute("Drafting", commands).unwrap();
        assert_eq!(doc.model(), &preview);
        assert_eq!(doc.model().walls, before.walls);
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(doc.redo());
        let mut p = doc.model().text_notes[&id].parameters.clone();
        p.text = "Revised\nNote".into();
        p.position = Point2::new(2., 3.);
        doc.execute("Edit", vec![Command::UpdateTextNote { id, parameters: p }])
            .unwrap();
        let edited = doc.model().clone();
        assert!(
            doc.execute("Delete view", vec![Command::RemoveView(view)])
                .is_err()
        );
        assert_eq!(doc.model(), &edited);
        doc.execute("Delete", vec![Command::RemoveTextNote(id)])
            .unwrap();
        assert!(!doc.model().text_notes.contains_key(&id));
        assert!(doc.undo());
        assert_eq!(doc.model(), &edited);
    }
}
