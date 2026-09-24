use super::*;

fn click(app: &mut DesktopApp, ctx: &egui::Context, size: egui::Vec2, scale: f32, label: &str) {
    let frame = |app: &mut DesktopApp, events| {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(scale);
        ctx.run(input, |ctx| app.opening_schedule_window(ctx))
    };
    frame(app, vec![]);
    let output = frame(app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == label => Some(t.pos + egui::vec2(4., 4.)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {label}"));
    for pressed in [true, false] {
        frame(
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn saved_schedule_create_configure_reopen_and_navigate_profiles() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let (mut app, door, _) = super::super::tests::fixture();
        let plan = app
            .editor
            .create_floor_plan("Plan", app.active_level)
            .unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let original = app.editor.document.model().clone();
        click(&mut app, &ctx, size, scale, "New Door schedule");
        assert_eq!(app.editor.document.model(), &original);
        click(&mut app, &ctx, size, scale, "Save definition");
        let id = app.opening_schedule.selected.unwrap();
        assert_eq!(
            app.editor.document.model().schedules[&id]
                .parameters
                .category,
            ScheduleCategory::Door
        );
        click(&mut app, &ctx, size, scale, "Configure schedule");
        click(&mut app, &ctx, size, scale, "All");
        let draft = app.opening_schedule.definition_draft.as_mut().unwrap();
        draft.parameters.name = "Doors and windows".into();
        draft.parameters.columns = vec![ScheduleColumn::Name, ScheduleColumn::Width];
        draft.parameters.sort = ScheduleSort::Width;
        let before = app.editor.document.model().clone();
        click(&mut app, &ctx, size, scale, "Save definition");
        let configured = app.editor.document.model().clone();
        assert_eq!(
            configured.schedules[&id].parameters.category,
            ScheduleCategory::All
        );
        assert!(app.editor.document.undo());
        assert_eq!(app.editor.document.model(), &before);
        assert!(app.editor.document.redo());
        assert_eq!(app.editor.document.model(), &configured);
        click(&mut app, &ctx, size, scale, "New Window schedule");
        click(&mut app, &ctx, size, scale, "Save definition");
        assert_eq!(app.editor.document.model().schedules.len(), 2);
        // Reopen from the explicit View-workspace picker.
        click(&mut app, &ctx, size, scale, "Window schedule 1");
        click(&mut app, &ctx, size, scale, "Doors and windows");
        assert_eq!(app.opening_schedule.selected, Some(id));
        click(&mut app, &ctx, size, scale, "Schedule door");
        assert_eq!(app.selected, Some(door));
        assert_eq!(app.plans.active, Some(plan));
    }
}

#[test]
fn definition_validation_stale_and_escape_guards() {
    let (mut app, _, _) = super::super::tests::fixture();
    app.begin_definition(Some(ScheduleCategory::Door));
    app.opening_schedule
        .definition_draft
        .as_mut()
        .unwrap()
        .parameters
        .columns
        .clear();
    app.apply_definition();
    assert!(app.editor.document.model().schedules.is_empty());
    assert!(
        app.opening_schedule
            .definition_draft
            .as_ref()
            .unwrap()
            .error
            .is_some()
    );
    app.editor
        .document
        .execute("Change", vec![Command::RenameProject("Changed".into())])
        .unwrap();
    app.apply_definition();
    assert!(app.opening_schedule.definition_draft.is_none());
    assert!(app.editor.document.model().schedules.is_empty());
    app.begin_definition(Some(ScheduleCategory::Window));
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| app.opening_schedule_window(ctx),
    );
    assert!(app.opening_schedule.definition_draft.is_none());
    assert!(app.editor.document.model().schedules.is_empty());
}

#[test]
fn saved_definition_derives_category_columns_sort_and_live_edits() {
    let (mut app, door, ty) = super::super::tests::fixture();
    let mut p = ScheduleParams::new("All", ScheduleCategory::All);
    p.columns = vec![ScheduleColumn::Width, ScheduleColumn::Name];
    p.sort = ScheduleSort::Width;
    let s = Schedule::new("core.schedule", p.clone());
    app.editor
        .command("Add schedule", Command::AddSchedule(s))
        .unwrap();
    let before = defined_rows(app.editor.document.model(), Some(&p)).unwrap();
    assert_eq!(before[0].id, door);
    assert_eq!(
        p.columns
            .iter()
            .map(|c| before[0].cell(*c))
            .collect::<Vec<_>>(),
        vec!["0.900", "Schedule door"]
    );
    let mut parameters = app.editor.document.model().opening_types[&ty]
        .parameters
        .clone();
    parameters.width = 1.4;
    app.editor
        .command(
            "Wider door",
            Command::UpdateOpeningType { id: ty, parameters },
        )
        .unwrap();
    let edited = defined_rows(app.editor.document.model(), Some(&p)).unwrap();
    assert_eq!(edited[1].id, door);
    app.editor.document.undo();
    assert_eq!(
        defined_rows(app.editor.document.model(), Some(&p)).unwrap(),
        before
    );
    app.editor.document.redo();
    assert_eq!(
        defined_rows(app.editor.document.model(), Some(&p)).unwrap(),
        edited
    );
    let mut parameters = app.editor.document.model().openings[&door]
        .parameters
        .clone();
    parameters.name = "Renamed live".into();
    app.editor
        .command(
            "Rename",
            Command::UpdateOpening {
                id: door,
                parameters,
            },
        )
        .unwrap();
    assert_eq!(
        defined_rows(app.editor.document.model(), Some(&p)).unwrap()[1].name,
        "Renamed live"
    );
    app.editor.document.undo();
    assert_eq!(
        defined_rows(app.editor.document.model(), Some(&p)).unwrap(),
        edited
    );
    for (category, expected) in [
        (ScheduleCategory::Door, OpeningKind::Door),
        (ScheduleCategory::Window, OpeningKind::Window),
    ] {
        p.category = category;
        for sort in ScheduleSort::ALL {
            p.sort = sort;
            let r = defined_rows(app.editor.document.model(), Some(&p)).unwrap();
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].kind, expected);
        }
    }
}
