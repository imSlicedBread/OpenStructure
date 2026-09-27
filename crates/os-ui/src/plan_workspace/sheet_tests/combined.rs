use super::*;

fn settle(app: &mut DesktopApp) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.plans.poll(&app.editor);
        #[cfg(feature = "external-plugins")]
        app.plans.providers.poll(&mut app.editor, app.plans.active);
        if app.sheet_page().is_ok() {
            return;
        }
        assert!(Instant::now() < deadline, "{:?}", app.sheet_page());
        std::thread::yield_now();
    }
}

fn fixture() -> (DesktopApp, Id, Id, Id) {
    let (mut app, section) = app_with_section();
    let plan = app
        .editor
        .create_floor_plan("Linked plan", app.active_level)
        .unwrap();
    app.focus_plan(Some(plan));
    app.create_sheet_from_active_view();
    let sheet = app.plans.active_sheet.unwrap();
    app.add_sheet_section(sheet, section);
    assert!(!app.status_error, "{}", app.status);
    settle(&mut app);
    (app, sheet, plan, section)
}

fn click(app: &mut DesktopApp, ctx: &egui::Context, size: egui::Vec2, scale: f32, label: &str) {
    let time = ctx.input(|i| i.time) + 1.0;
    let _ = ctx.run(raw_input(size, scale, time, vec![]), |ctx| {
        app.plan_workspace(ctx)
    });
    let output = ctx.run(raw_input(size, scale, time + 0.1, vec![]), |ctx| {
        app.plan_workspace(ctx)
    });
    let at = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == label => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .expect("visible action");
    assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains(at));
    for pressed in [true, false] {
        let _ = ctx.run(
            raw_input(
                size,
                scale,
                time + if pressed { 0.2 } else { 0.3 },
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            ),
            |ctx| app.plan_workspace(ctx),
        );
    }
}

#[test]
fn combined_sheet_scales_history_navigation_live_edit_reopen_and_pdf() {
    for (size, scale) in PROFILES {
        let (mut app, sheet, plan, section) = fixture();
        let before = app.editor.document.model().clone();
        let page = app.sheet_page().unwrap();
        let clips: Vec<_> = page.marks().iter().filter_map(|m| m.clip).collect();
        assert!(clips.iter().any(|c| c.max_mm.x <= 202.0));
        assert!(clips.iter().any(|c| c.min_mm.x >= 218.0));
        app.apply_sheet_viewport_layout(sheet, 1, 50.0, Point2::new(309.0, 126.0));
        assert!(!app.status_error, "{}", app.status);
        let edited = app.editor.document.model().clone();
        assert_eq!(
            edited.sheets[&sheet].parameters.viewports[0],
            before.sheets[&sheet].parameters.viewports[0]
        );
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &before);
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model(), &edited);
        settle(&mut app);
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let output = ctx.run(raw_input(size, scale, 1.0, vec![]), |ctx| {
            app.plan_workspace(ctx)
        });
        for label in ["Edit source plan", "Edit source section"] {
            assert!(
                output.shapes.iter().any(
                    |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == label)
                )
            );
        }
        let page = app.sheet_page().unwrap();
        app.plans.pdf_path = "combined-preview.pdf".into();
        app.prepare_sheet_pdf(&page);
        assert_eq!(
            app.plans.pending_pdf.as_ref().unwrap().bytes,
            page.to_pdf().unwrap()
        );
        // The source navigation action and picker preserve the persisted layout.
        click(&mut app, &ctx, size, scale, "Edit source section");
        assert_eq!(app.plans.active, Some(section));
        let _ = ctx.run(
            raw_input(size, scale, ctx.input(|i| i.time) + 0.1, vec![]),
            |ctx| app.sheet_pdf_confirmation(ctx),
        );
        assert!(app.plans.pending_pdf.is_none());
        app.open_sheet(sheet);
        assert_eq!(app.plans.active, Some(plan));
        settle(&mut app);
        app.prepare_sheet_pdf(&app.sheet_page().unwrap());
        let wall = *app.editor.document.model().walls.keys().next().unwrap();
        let mut parameters = app.editor.document.model().walls[&wall].parameters.clone();
        parameters.height = 2.0;
        parameters.end.x = 6.0;
        app.editor
            .command(
                "Change source wall",
                Command::UpdateWall {
                    id: wall,
                    parameters,
                },
            )
            .unwrap();
        assert!(app.sheet_page().is_err());
        let _ = ctx.run(
            raw_input(size, scale, ctx.input(|i| i.time) + 0.1, vec![]),
            |ctx| app.sheet_pdf_confirmation(ctx),
        );
        assert!(app.plans.pending_pdf.is_none());
        settle(&mut app);
        let live = app.sheet_page().unwrap();
        assert_ne!(live, page);
        for right in [false, true] {
            let marks = |page: &SheetPage| {
                page.marks()
                    .iter()
                    .filter(|m| m.clip.is_some_and(|c| (c.min_mm.x > 210.0) == right))
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_ne!(
                marks(&live),
                marks(&page),
                "both linked sources must reflect the wall edit"
            );
        }
        let mut parameters = app.editor.document.model().views[&section]
            .parameters
            .clone();
        parameters.section.as_mut().unwrap().end.x = 4.0;
        app.editor
            .command(
                "Change Section extents",
                Command::UpdateView {
                    id: section,
                    parameters,
                },
            )
            .unwrap();
        assert!(app.sheet_page().is_err());
        settle(&mut app);
        let new_extents = app.sheet_page().unwrap();
        assert_ne!(new_extents, live);
        let live = new_extents;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("combined.osb");
        let saved = app.editor.document.model().clone();
        app.editor.save(&path).unwrap();
        app.editor.open(&path).unwrap();
        app.plans.poll(&app.editor);
        app.open_sheet(sheet);
        settle(&mut app);
        assert_eq!(app.editor.document.model(), &saved);
        assert_eq!(app.sheet_page().unwrap(), live);
        assert_eq!(
            app.sheet_page().unwrap().to_pdf().unwrap(),
            live.to_pdf().unwrap()
        );
    }
}

#[test]
fn combined_add_and_both_source_actions_are_reachable() {
    for (size, scale) in PROFILES {
        let (mut app, sheet, plan, section) = fixture();
        app.editor.undo().unwrap();
        settle(&mut app);
        assert_eq!(
            app.editor.document.model().sheets[&sheet]
                .parameters
                .viewports
                .len(),
            1
        );
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        click(&mut app, &ctx, size, scale, "Add linked Section");
        assert!(!app.status_error, "{}", app.status);
        assert_eq!(
            app.editor.document.model().sheets[&sheet]
                .parameters
                .viewports
                .len(),
            2
        );
        settle(&mut app);
        click(&mut app, &ctx, size, scale, "Edit source plan");
        assert_eq!(app.plans.active, Some(plan));
        assert!(app.plans.active_sheet.is_none());
        app.open_sheet(sheet);
        settle(&mut app);
        click(&mut app, &ctx, size, scale, "Edit source section");
        assert_eq!(app.plans.active, Some(section));
        assert!(app.plans.active_sheet.is_none());
    }
}

#[cfg(feature = "external-plugins")]
#[test]
fn combined_provider_absence_blocks_native_only_fallback() {
    let (mut app, sheet, plan, _) = fixture();
    let mut model = app.editor.document.model().clone();
    let entity: os_model::ExtensionEntity = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/extension-envelope-v1.json"
    )))
    .unwrap();
    model.extensions.insert(entity.id, entity);
    model.plugin_requirements.insert(
        "org.example.columns".into(),
        os_model::PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    app.editor.document = os_document::Document::from_model(model).unwrap();
    app.plans.poll(&app.editor);
    app.open_sheet(sheet);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.plans.poll(&app.editor);
        if app.plans.ready() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(
        app.sheet_page()
            .unwrap_err()
            .to_string()
            .contains("providers changed")
    );
    app.plans.providers.poll(&mut app.editor, Some(plan));
    assert!(
        app.sheet_page()
            .unwrap_err()
            .to_string()
            .contains("provider graphics are incomplete")
    );
}

#[test]
fn combined_overlap_and_schedule_reject_atomically() {
    let (mut app, sheet, _, _) = fixture();
    let before = app.editor.document.model().clone();
    let history = app.editor.document.history_stats();
    app.apply_sheet_viewport_layout(sheet, 1, 100.0, Point2::new(110.0, 128.0));
    assert!(app.status_error && app.status.contains("overlap"));
    assert_eq!(app.editor.document.model(), &before);
    assert_eq!(app.editor.document.history_stats(), history);
    app.place_sheet_schedule(sheet, Some(Id::new()));
    assert!(app.status_error && app.status.contains("combined"));
    assert_eq!(app.editor.document.model(), &before);
    assert_eq!(app.editor.document.history_stats(), history);
    app.add_sheet_section(sheet, Id::new());
    assert!(app.status_error);
    assert_eq!(app.editor.document.model(), &before);
}
