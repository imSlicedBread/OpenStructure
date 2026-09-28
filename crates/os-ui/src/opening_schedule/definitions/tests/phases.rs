use super::*;

#[test]
fn new_opening_schedules_pin_latest_but_room_finish_stays_unphased() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let (mut app, _, _) = super::super::super::tests::fixture();
        let latest = app.editor.document.model().latest_phase();
        app.begin_definition(Some(ScheduleCategory::Door));
        assert_eq!(
            app.opening_schedule
                .definition_draft
                .as_ref()
                .unwrap()
                .parameters
                .phase,
            SchedulePhase::PhaseAware {
                target: latest,
                filter: os_model::PhaseFilter::ShowAll,
            }
        );

        // The phase controls are part of the saved-definition draft, not a
        // separate immediate mutation, and are visible at both supported DPI
        // profiles.
        app.opening_schedule.open = true;
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(scale);
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.schedule_definition_controls(ui));
        });
        for label in ["Schedule phase", "Phase aware", "Pinned: New Construction"] {
            assert!(output.shapes.iter().any(
                |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains(label))
            ), "missing phase control {label} at scale {scale}");
        }

        // Canceling a new definition leaves the document and history untouched.
        let before = app.editor.document.model().clone();
        let history_before = app.editor.document.history_stats();
        app.opening_schedule.definition_draft = None;
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history_before);

        app.begin_definition(Some(ScheduleCategory::RoomFinish));
        assert_eq!(
            app.opening_schedule
                .definition_draft
                .as_ref()
                .unwrap()
                .parameters
                .phase,
            SchedulePhase::LegacyUnphased
        );
    }
}

#[test]
fn phase_definition_save_cancel_stale_revision_and_history_are_atomic() {
    use os_model::PhaseFilter;

    let (mut app, _, _) = super::super::super::tests::fixture();
    let original = app.editor.document.model().clone();
    app.begin_definition(Some(ScheduleCategory::All));
    app.apply_definition();
    let schedule_id = app.opening_schedule.selected.unwrap();
    let initial = app.editor.document.model().clone();
    let revision = app.editor.document.revision();

    // Editing is draft-only until Save; discarding the draft cannot alter the
    // saved schedule or consume an undo step.
    app.begin_definition(None);
    app.opening_schedule
        .definition_draft
        .as_mut()
        .unwrap()
        .parameters
        .phase = SchedulePhase::PhaseAware {
        target: None,
        filter: PhaseFilter::ShowNew,
    };
    assert_eq!(app.editor.document.model(), &initial);
    app.opening_schedule.definition_draft = None;
    assert_eq!(app.editor.document.model(), &initial);

    app.begin_definition(None);
    app.opening_schedule
        .definition_draft
        .as_mut()
        .unwrap()
        .parameters
        .phase = SchedulePhase::PhaseAware {
        target: None,
        filter: PhaseFilter::ShowNew,
    };
    app.apply_definition();
    assert_eq!(app.editor.document.revision(), revision + 1);
    assert_eq!(
        app.editor.document.model().schedules[&schedule_id]
            .parameters
            .phase,
        SchedulePhase::PhaseAware {
            target: None,
            filter: PhaseFilter::ShowNew,
        }
    );
    app.editor.undo().unwrap();
    assert_eq!(app.editor.document.model(), &initial);
    app.editor.redo().unwrap();
    assert_eq!(
        app.editor.document.model().schedules[&schedule_id]
            .parameters
            .phase,
        SchedulePhase::PhaseAware {
            target: None,
            filter: PhaseFilter::ShowNew,
        }
    );

    // A draft captured before another document edit is discarded on Save.
    app.begin_definition(None);
    app.opening_schedule
        .definition_draft
        .as_mut()
        .unwrap()
        .parameters
        .phase = SchedulePhase::LegacyUnphased;
    let mut changed = app.editor.document.model().schedules[&schedule_id]
        .parameters
        .clone();
    changed.name.push_str(" changed elsewhere");
    let before_external = app.editor.document.model().clone();
    app.editor
        .command(
            "Change schedule elsewhere",
            os_document::Command::UpdateSchedule {
                id: schedule_id,
                parameters: changed,
            },
        )
        .unwrap();
    let after_external = app.editor.document.model().clone();
    app.apply_definition();
    assert_eq!(app.editor.document.model(), &after_external);
    assert!(app.status.contains("document changed"));

    app.editor.undo().unwrap();
    assert_eq!(app.editor.document.model(), &before_external);
    app.editor.undo().unwrap();
    assert_eq!(app.editor.document.model(), &initial);
    app.editor.undo().unwrap();
    assert_eq!(app.editor.document.model(), &original);
}
