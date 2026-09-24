use super::*;

fn profile_handles(h: &Harness) -> Vec<egui::Pos2> {
    h.output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) if circle.radius == 3. || circle.radius == 5. => {
                Some(circle.center)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn opening_family_profile_pointer_editor_apply_cancel_invalid_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.begin_new_opening_type(os_model::OpeningKind::Door);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Apply type");
        let id = *h
            .app
            .editor
            .document
            .model()
            .opening_types
            .keys()
            .next()
            .unwrap();
        let baseline = h.app.editor.document.model().clone();
        let stats = h.app.editor.document.history_stats();
        let handles = |h: &Harness| -> Vec<egui::Pos2> {
            h.output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Circle(c) if c.radius == 3. || c.radius == 5. => Some(c.center),
                    _ => None,
                })
                .collect()
        };
        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![]);
        let points = handles(&h);
        assert_eq!(points.len(), 4, "profile handles were not drawn");
        // Real pointer drag of upper-right vertex changes the authored elevation.
        h.drag(points[2], points[2] + egui::vec2(-18., 12.));
        assert_ne!(
            handles(&h)[2],
            points[2],
            "profile pointer drag did not move the vertex handle"
        );
        assert_ne!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .profile_for_test(),
            os_model::OpeningFamily::default().profile
        );
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Insert vertex");
        assert_eq!(handles(&h).len(), 5);
        h.click("Remove vertex");
        assert_eq!(handles(&h).len(), 4);
        h.click("Cancel type");
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.history_stats(), stats);
        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        let points = handles(&h);
        h.drag(points[2], points[1]);
        h.click("Apply type");
        assert!(
            h.app.opening_type_draft.is_some(),
            "invalid crossing/duplicate stays in editor"
        );
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.history_stats(), stats);
        h.click("Reset rectangle");
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Add frame");
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .frame_width_for_test(),
            0.05
        );
        let points = handles(&h);
        h.drag(points[2], points[2] + egui::vec2(-18., 12.));
        assert_ne!(
            handles(&h)[2],
            points[2],
            "second profile pointer drag did not move the vertex handle"
        );
        assert_ne!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .profile_for_test(),
            os_model::OpeningFamily::default().profile
        );
        h.click("Apply type");
        assert!(h.app.opening_type_draft.is_none());
        let applied = h.app.editor.document.model().clone();
        assert_ne!(
            applied.opening_types[&id].parameters.family,
            baseline.opening_types[&id].parameters.family
        );
        assert_eq!(
            applied.opening_types[&id].parameters.family.frame_width,
            0.05
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            stats.undo_entries + 1
        );
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Redo");
        assert_eq!(h.app.editor.document.model(), &applied);
        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        let points = handles(&h);
        h.drag(points[2], points[2] + egui::vec2(-8., 5.));
        h.app
            .editor
            .command("Concurrent", Command::RenameProject("Changed".into()))
            .unwrap();
        let changed = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.opening_type_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &changed);
    }
}

#[test]
fn authored_host_cut_profile_pointer_editor_preview_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.begin_new_opening_type(os_model::OpeningKind::Door);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Apply type");
        let id = *h
            .app
            .editor
            .document
            .model()
            .opening_types
            .keys()
            .next()
            .unwrap();
        let baseline = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();

        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![]);
        let points = profile_handles(&h);
        assert_eq!(points.len(), 4);
        h.drag(points[2], points[2] + egui::vec2(-18., 12.));
        assert_ne!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .profile_for_test(),
            os_model::OpeningFamily::default().profile
        );

        h.click("Host cut profile");
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(profile_handles(&h).len(), 4);
        h.click("Copy component to cut");
        h.frame(vec![]);
        h.frame(vec![]);
        let draft = h.app.opening_type_draft.as_ref().unwrap();
        assert_eq!(draft.profile_for_test(), draft.cut_profile_for_test());
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.history_stats(), history);

        h.click("Apply type");
        assert!(h.app.opening_type_draft.is_none());
        let applied = h.app.editor.document.model().clone();
        let family = &applied.opening_types[&id].parameters.family;
        assert_eq!(family.host_cut, os_model::OpeningHostCut::Profile);
        assert_ne!(
            family.cut_profile,
            os_model::OpeningFamily::default().cut_profile
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Redo");
        assert_eq!(h.app.editor.document.model(), &applied);
    }
}

#[test]
fn window_type_pane_position_edit_cancel_stale_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.apply_wall();
        let host = h.app.selected.expect("created wall is selected");
        let ty = os_model::OpeningType::new(
            "core.opening_type",
            os_model::OpeningTypeParams {
                family: Default::default(),
                name: "Window family".into(),
                kind: os_model::OpeningKind::Window,
                width: 1.0,
                height: 1.2,
                sill: 0.8,
                pane_position: os_model::WindowPanePosition::Center,
            },
        );
        let type_id = ty.id();
        let openings = [1.0, 3.0].map(|offset| {
            os_model::Opening::new(
                "core.opening",
                os_model::OpeningParams {
                    name: "Window instance".into(),
                    host,
                    offset,
                    definition: os_model::OpeningDefinition::Typed { type_id },
                    hinge: Default::default(),
                    swing: Default::default(),
                },
            )
        });
        h.app
            .editor
            .document
            .execute(
                "Add typed windows",
                std::iter::once(Command::AddOpeningType(ty))
                    .chain(openings.into_iter().map(Command::AddOpening))
                    .collect(),
            )
            .unwrap();
        h.app.editor.regenerate().unwrap();
        let baseline = h.app.editor.document.model().clone();
        let baseline_revision = h.app.editor.document.revision();
        let baseline_history = h.app.editor.document.history_stats();

        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(
            h.visible_text_rect("Pane position").is_some(),
            "draft={}; rendered={:?}",
            h.app.opening_type_draft.is_some(),
            h.output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        );
        h.click("Left face");
        h.click("Cancel type");
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.revision(), baseline_revision);
        assert_eq!(h.app.editor.document.history_stats(), baseline_history);

        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        h.click("Right face");
        h.click("Apply type");
        let updated = h.app.editor.document.model().clone();
        assert_eq!(
            updated.opening_types[&type_id].parameters.pane_position,
            os_model::WindowPanePosition::RightFace
        );
        assert_eq!(updated.openings.len(), 2);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            baseline_history.undo_entries + 1
        );
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Redo");
        assert_eq!(h.app.editor.document.model(), &updated);

        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        h.click("Center");
        h.app
            .editor
            .command(
                "Concurrent edit",
                Command::RenameProject("Concurrent".into()),
            )
            .unwrap();
        let concurrent = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.opening_type_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &concurrent);
    }
}
