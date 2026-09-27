use super::*;
use os_model::{ScheduleNumericOperator as N, ScheduleTextOperator as T};

fn text(field: ScheduleTextField, operator: T, value: &str) -> ScheduleFilter {
    ScheduleFilter::Text {
        field,
        operator,
        value: value.into(),
    }
}
fn number(field: ScheduleNumericField, operator: N, value: f64) -> ScheduleFilter {
    ScheduleFilter::Numeric {
        field,
        operator,
        value,
    }
}
fn fixture() -> (DesktopApp, Id, Id) {
    crate::opening_schedule::tests::fixture()
}

#[test]
fn opening_filters_and_category_case_and_unrounded_boundaries() {
    let (app, door, _) = fixture();
    let model = app.editor.document.model();
    let mut p = ScheduleParams::new("Filtered", ScheduleCategory::All);
    assert_eq!(defined_rows(model, Some(&p)).unwrap().len(), 2);
    p.filters = vec![
        text(ScheduleTextField::Name, T::Contains, "SCHEDULE"),
        number(ScheduleNumericField::Width, N::Less, 1.0),
    ];
    assert_eq!(defined_rows(model, Some(&p)).unwrap()[0].id, door);
    p.category = ScheduleCategory::Window;
    assert!(defined_rows(model, Some(&p)).unwrap().is_empty());
    p.category = ScheduleCategory::Door;
    p.filters
        .push(text(ScheduleTextField::Type, T::NotEquals, "D-900"));
    assert!(defined_rows(model, Some(&p)).unwrap().is_empty());
    for (operator, threshold, expected) in [
        (N::Equals, 0.9, 1),
        (N::NotEquals, 0.9, 0),
        (N::Less, 0.9, 0),
        (N::LessOrEqual, 0.9, 1),
        (N::Greater, 0.9, 0),
        (N::GreaterOrEqual, 0.9, 1),
        (N::Less, 0.9000001, 1),
        (N::Greater, 0.8999999, 1),
        (N::Equals, 0.9000001, 0),
    ] {
        p.filters = vec![number(ScheduleNumericField::Width, operator, threshold)];
        assert_eq!(defined_rows(model, Some(&p)).unwrap().len(), expected);
    }
    p.category = ScheduleCategory::All;
    p.filters = vec![text(ScheduleTextField::Type, T::StartsWith, "LEGACY")];
    assert_eq!(
        defined_rows(model, Some(&p)).unwrap()[0].kind,
        OpeningKind::Window
    );
}

#[test]
fn opening_filters_resolve_live_names_host_level_type_and_dimensions_with_history() {
    // Each change moves a row across a persisted rule; undo and redo recompute membership.
    for field in 0..7 {
        let (mut app, door, ty) = fixture();
        let host = app.editor.document.model().openings[&door].parameters.host;
        let level = app.editor.document.model().walls[&host].parameters.level;
        let model = app.editor.document.model();
        let (rule, command) = match field {
            0 => {
                let mut parameters = model.openings[&door].parameters.clone();
                parameters.name = "Entry".into();
                (
                    text(ScheduleTextField::Name, T::Equals, "ENTRY"),
                    Command::UpdateOpening {
                        id: door,
                        parameters,
                    },
                )
            }
            1 => {
                let mut parameters = model.opening_types[&ty].parameters.clone();
                parameters.name = "Entry type".into();
                (
                    text(ScheduleTextField::Type, T::Equals, "ENTRY TYPE"),
                    Command::UpdateOpeningType { id: ty, parameters },
                )
            }
            2 => {
                let mut parameters = model.walls[&host].parameters.clone();
                parameters.name = "Entry wall".into();
                (
                    text(ScheduleTextField::Host, T::Equals, "ENTRY WALL"),
                    Command::UpdateWall {
                        id: host,
                        parameters,
                    },
                )
            }
            3 => {
                let mut parameters = model.levels[&level].parameters.clone();
                parameters.name = "Entry level".into();
                (
                    text(ScheduleTextField::Level, T::Equals, "ENTRY LEVEL"),
                    Command::UpdateLevel {
                        id: level,
                        parameters,
                    },
                )
            }
            4 | 5 => {
                let mut parameters = model.openings[&door].parameters.clone();
                let dimension = if field == 4 {
                    parameters.width_override = Some(1.0000001);
                    ScheduleNumericField::Width
                } else {
                    parameters.height_override = Some(2.2000001);
                    ScheduleNumericField::Height
                };
                (
                    number(dimension, N::Greater, if field == 4 { 1.0 } else { 2.2 }),
                    Command::UpdateOpening {
                        id: door,
                        parameters,
                    },
                )
            }
            _ => {
                let window = model.openings.values().find(|o| o.id() != door).unwrap();
                let mut parameters = window.parameters.clone();
                if let os_model::OpeningDefinition::Legacy { sill, .. } = &mut parameters.definition
                {
                    *sill = 1.0000001;
                }
                (
                    number(ScheduleNumericField::Sill, N::Greater, 1.0),
                    Command::UpdateOpening {
                        id: window.id(),
                        parameters,
                    },
                )
            }
        };
        let category = if matches!(field, 4 | 5) {
            ScheduleCategory::Door
        } else {
            ScheduleCategory::All
        };
        let mut p = ScheduleParams::new("Live filter", category);
        p.filters.push(rule);
        let s = Schedule::new("core.schedule", p.clone());
        let id = s.id();
        app.editor
            .command("Filter", Command::AddSchedule(s))
            .unwrap();
        assert!(
            paper_table(app.editor.document.model(), id)
                .unwrap()
                .rows
                .is_empty()
        );
        app.editor.command("Change source", command).unwrap();
        let after = defined_rows(app.editor.document.model(), Some(&p)).unwrap();
        assert_eq!(
            after.len(),
            if matches!(field, 2 | 3) { 2 } else { 1 },
            "field {field}"
        );
        assert_eq!(
            paper_table(app.editor.document.model(), id)
                .unwrap()
                .rows
                .len(),
            after.len()
        );
        app.editor.undo().unwrap();
        assert!(
            defined_rows(app.editor.document.model(), Some(&p))
                .unwrap()
                .is_empty()
        );
        app.editor.redo().unwrap();
        assert_eq!(
            defined_rows(app.editor.document.model(), Some(&p)).unwrap(),
            after
        );
    }
    // Shared type dimensions also propagate to inherited rows.
    let (mut app, _, ty) = fixture();
    let mut p = ScheduleParams::new("Typed", ScheduleCategory::Door);
    p.filters = vec![
        number(ScheduleNumericField::Width, N::Greater, 1.0),
        number(ScheduleNumericField::Height, N::Greater, 2.2),
    ];
    assert!(
        defined_rows(app.editor.document.model(), Some(&p))
            .unwrap()
            .is_empty()
    );
    let mut parameters = app.editor.document.model().opening_types[&ty]
        .parameters
        .clone();
    parameters.width = 1.0000001;
    parameters.height = 2.2000001;
    app.editor
        .command(
            "Type dimensions",
            Command::UpdateOpeningType { id: ty, parameters },
        )
        .unwrap();
    assert_eq!(
        defined_rows(app.editor.document.model(), Some(&p))
            .unwrap()
            .len(),
        1
    );
    app.editor.undo().unwrap();
    assert!(
        defined_rows(app.editor.document.model(), Some(&p))
            .unwrap()
            .is_empty()
    );
    app.editor.redo().unwrap();
    assert_eq!(
        defined_rows(app.editor.document.model(), Some(&p))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn opening_filter_draft_add_edit_remove_save_cancel_stale_and_category_reset() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let (mut app, _, _) = fixture();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let original = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        click(&mut app, &ctx, size, scale, "New Door schedule");
        click(&mut app, &ctx, size, scale, "Add filter");
        click(&mut app, &ctx, size, scale, "Save definition");
        assert!(
            app.opening_schedule
                .definition_draft
                .as_ref()
                .unwrap()
                .error
                .is_some()
        );
        assert_eq!(app.editor.document.model(), &original);
        assert_eq!(app.editor.document.history_stats(), history);
        click(&mut app, &ctx, size, scale, "Text");
        click(&mut app, &ctx, size, scale, "Number");
        click(&mut app, &ctx, size, scale, "Greater or equal");
        click(&mut app, &ctx, size, scale, "Greater");
        let draft = app.opening_schedule.definition_draft.as_mut().unwrap();
        assert_eq!(
            draft.parameters.filters,
            vec![number(ScheduleNumericField::Width, N::Greater, 0.0)]
        );
        // Keep a precise threshold that rounds to the same displayed cell value.
        draft.parameters.filters[0] = number(ScheduleNumericField::Width, N::Greater, 0.8999999);
        click(&mut app, &ctx, size, scale, "Add filter");
        let draft = app.opening_schedule.definition_draft.as_mut().unwrap();
        draft.parameters.filters[1] = text(ScheduleTextField::Name, T::Contains, "DOOR");
        click(&mut app, &ctx, size, scale, "↑ 2");
        assert!(matches!(
            app.opening_schedule
                .definition_draft
                .as_ref()
                .unwrap()
                .parameters
                .filters[0],
            ScheduleFilter::Text { .. }
        ));
        click(&mut app, &ctx, size, scale, "Remove filter");
        assert_eq!(
            app.opening_schedule
                .definition_draft
                .as_ref()
                .unwrap()
                .parameters
                .filters
                .len(),
            1
        );
        click(&mut app, &ctx, size, scale, "Save definition");
        assert!(app.opening_schedule.definition_draft.is_none());
        let id = app.opening_schedule.selected.unwrap();
        let saved = app.editor.document.model().clone();
        assert_eq!(
            saved.schedules[&id].parameters.filters,
            vec![number(ScheduleNumericField::Width, N::Greater, 0.8999999)]
        );
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &original);
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model(), &saved);
        click(&mut app, &ctx, size, scale, "Configure schedule");
        click(&mut app, &ctx, size, scale, "RoomFinish");
        assert!(
            app.opening_schedule
                .definition_draft
                .as_ref()
                .unwrap()
                .parameters
                .filters
                .is_empty()
        );
        click(&mut app, &ctx, size, scale, "Cancel definition");
        assert_eq!(app.editor.document.model(), &saved);
        click(&mut app, &ctx, size, scale, "Configure schedule");
        app.editor
            .command("Stale", Command::RenameProject("Changed".into()))
            .unwrap();
        app.apply_definition();
        assert!(app.opening_schedule.definition_draft.is_none());
        assert_eq!(
            app.editor.document.model().schedules[&id],
            saved.schedules[&id]
        );
    }
}

#[test]
fn opening_filtered_rows_are_visible_in_schedule_window() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let (mut app, door, _) = fixture();
        app.opening_schedule.session = Some(app.editor.document.session_id());
        app.begin_definition(Some(ScheduleCategory::All));
        app.opening_schedule
            .definition_draft
            .as_mut()
            .unwrap()
            .parameters
            .filters = vec![text(ScheduleTextField::Name, T::Contains, "DOOR")];
        app.apply_definition();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut DesktopApp| {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .native_pixels_per_point = Some(scale);
            ctx.run(input, |ctx| app.opening_schedule_window(ctx))
        };
        frame(&mut app);
        let output = frame(&mut app);
        let has = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text == label))
        };
        assert!(has(&output, "Schedule door"));
        assert!(!has(&output, "Schedule window"));
        let mut parameters = app.editor.document.model().openings[&door]
            .parameters
            .clone();
        parameters.name = "Excluded".into();
        app.editor
            .command(
                "Rename",
                Command::UpdateOpening {
                    id: door,
                    parameters,
                },
            )
            .unwrap();
        let output = frame(&mut app);
        assert!(!has(&output, "Schedule door"));
        assert!(!has(&output, "Excluded"));
        app.editor.undo().unwrap();
        assert!(has(&frame(&mut app), "Schedule door"));
        app.editor.redo().unwrap();
        assert!(!has(&frame(&mut app), "Schedule door"));
    }
}
