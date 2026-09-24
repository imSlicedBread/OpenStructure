use super::*;

#[test]
fn wall_type_dialog_cancel_save_and_history_at_supported_dpi_profiles() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.apply_wall();
        let wall_id = h.app.selected.expect("created wall selected");
        let original_wall = h.app.editor.document.model().walls[&wall_id].clone();
        let original_model = h.app.editor.document.model().clone();
        let original_revision = h.app.editor.document.revision();
        let original_history = h.app.editor.document.history_stats();
        h.frame(vec![]);
        h.reveal_property("Create type from this wall…");
        h.click("Create type from this wall…");
        assert!(
            h.visible_text_rect("Wall type and material layers")
                .is_some()
        );
        assert!(h.visible_text_rect("Save type / layers").is_some());
        h.click("Cancel");
        assert_eq!(h.app.editor.document.model(), &original_model);
        assert_eq!(h.app.editor.document.revision(), original_revision);
        assert_eq!(h.app.editor.document.history_stats(), original_history);

        h.reveal_property("Create type from this wall…");
        h.click("Create type from this wall…");
        h.click("Save type / layers");

        let model = h.app.editor.document.model();
        let assignment = model.wall_type_assignments[&wall_id];
        let wall_type = &model.wall_types[&assignment.type_id];
        assert_eq!(model.walls[&wall_id], original_wall);
        assert_eq!(wall_type.parameters.layers.len(), 1);
        assert!(
            (wall_type.parameters.layers[0].thickness - original_wall.parameters.thickness).abs()
                < 1e-9
        );
        assert!(h.app.editor.document.can_undo());

        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &original_model);
        h.click("Redo");
        assert_eq!(
            h.app.editor.document.model().wall_type_assignments[&wall_id].type_id,
            assignment.type_id
        );
    }
}

#[test]
fn stale_wall_type_dialog_cannot_commit_over_a_document_edit() {
    let mut h = Harness::new();
    h.app.apply_wall();
    let wall_id = h.app.selected.unwrap();
    h.frame(vec![]);
    h.reveal_property("Create type from this wall…");
    h.click("Create type from this wall…");
    let mut changed = h.app.editor.document.model().walls[&wall_id]
        .parameters
        .clone();
    changed.height += 0.25;
    h.app
        .editor
        .command(
            "External wall edit",
            Command::UpdateWall {
                id: wall_id,
                parameters: changed,
            },
        )
        .unwrap();
    let externally_edited = h.app.editor.document.model().clone();
    h.click("Save type / layers");
    assert_eq!(h.app.editor.document.model(), &externally_edited);
    assert!(
        h.visible_text_rect(
            "Invalid data: Document changed; cancel and reopen the wall type editor"
        )
        .is_some()
    );
    h.click("Cancel");
    assert_eq!(h.app.editor.document.model(), &externally_edited);
}
