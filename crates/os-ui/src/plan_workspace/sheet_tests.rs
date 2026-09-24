//! First persisted sheet, paper preview, and vector-PDF acceptance.
use super::*;
use std::time::{Duration, Instant};

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

#[test]
fn saved_schedule_sheet_live_preview_pdf_history_overflow_and_stale_export() {
    fn settle(app: &mut DesktopApp) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.plans.poll(&app.editor);
            if app.plans.ready() {
                break;
            }
            assert!(Instant::now() < deadline, "drawing did not settle");
            std::thread::yield_now();
        }
    }
    for (size, scale) in PROFILES {
        let (mut app, _) = app_with_plan();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, crate::default_wall(app.active_level));
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "D01".into(),
                host: wall.id(),
                offset: 1.0,
                definition: OpeningDefinition::Legacy {
                    kind: OpeningKind::Door,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.0,
                },
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let door = opening.id();
        let mut schedule = os_model::Schedule::new(
            "core.schedule",
            os_model::ScheduleParams::new("Door table", os_model::ScheduleCategory::Door),
        );
        schedule.parameters.columns = vec![
            os_model::ScheduleColumn::Name,
            os_model::ScheduleColumn::Width,
        ];
        let schedule_id = schedule.id();
        app.editor
            .document
            .execute(
                "Fixture",
                vec![
                    Command::AddWall(wall),
                    Command::AddOpening(opening),
                    Command::AddSchedule(schedule),
                ],
            )
            .unwrap();
        app.create_sheet_from_active_view();
        settle(&mut app);
        let sheet = app.plans.active_sheet.unwrap();
        let before = app.editor.document.model().clone();
        let revision = app.editor.document.revision();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run(raw_input(size, scale, 0.0, vec![]), |ctx| {
            app.plan_workspace(ctx)
        });
        let output = ctx.run(raw_input(size, scale, 1.0, vec![]), |ctx| {
            app.plan_workspace(ctx)
        });
        let pos = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Add saved schedule to sheet" => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .expect("visible Add saved schedule button");
        assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains(pos));
        app.place_sheet_schedule(sheet, Some(schedule_id));
        assert!(!app.status_error, "{}", app.status);
        assert_eq!(
            app.editor.document.revision(),
            revision + 1,
            "schedule placement should be one validated transaction"
        );
        assert_eq!(
            app.editor.document.model().sheets[&sheet]
                .parameters
                .schedule_placements
                .len(),
            1
        );
        let placed = app.editor.document.model().clone();
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &before);
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model(), &placed);
        settle(&mut app);
        let page = app.sheet_page().unwrap();
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert!(pdf.contains("<443031> Tj"));
        assert!(pdf.contains("<302E393030> Tj"));
        let output = ctx.run(raw_input(size, scale, 4.0, vec![]), |ctx| {
            app.plan_workspace(ctx)
        });
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "D01"))
        );
        app.plans.pdf_path = "schedule-validation.pdf".into();
        app.prepare_sheet_pdf(&page);
        assert!(app.plans.pending_pdf.is_some());
        let mut parameters = app.editor.document.model().openings[&door]
            .parameters
            .clone();
        parameters.name = "D02".into();
        app.editor
            .command(
                "Rename",
                Command::UpdateOpening {
                    id: door,
                    parameters: parameters.clone(),
                },
            )
            .unwrap();
        let _ = ctx.run(raw_input(size, scale, 5.0, vec![]), |ctx| {
            app.sheet_pdf_confirmation(ctx)
        });
        assert!(app.plans.pending_pdf.is_none());
        settle(&mut app);
        let pdf = String::from_utf8(app.sheet_page().unwrap().to_pdf().unwrap()).unwrap();
        assert!(pdf.contains("<443032> Tj"));
        assert!(!pdf.contains("<443031> Tj"));
        parameters.name = "W".repeat(200);
        app.editor
            .command(
                "Long name",
                Command::UpdateOpening {
                    id: door,
                    parameters,
                },
            )
            .unwrap();
        settle(&mut app);
        assert!(
            app.sheet_page()
                .unwrap_err()
                .to_string()
                .contains("overflow")
        );
        app.place_sheet_schedule(sheet, None);
        assert!(!app.status_error, "{}", app.status);
        settle(&mut app);
        let before_reject = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        app.place_sheet_schedule(sheet, Some(schedule_id));
        assert!(app.status_error);
        assert_eq!(app.editor.document.model(), &before_reject);
        assert_eq!(app.editor.document.history_stats(), history);
    }
}

fn app_with_plan() -> (DesktopApp, Id) {
    let mut app = DesktopApp::new().unwrap();
    let view = app
        .editor
        .create_floor_plan("Sheet source plan", app.active_level)
        .unwrap();
    app.plans.poll(&app.editor);
    app.focus_plan(Some(view));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.plans.poll(&app.editor);
        if app.plans.ready() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "source plan worker did not settle"
        );
        std::thread::yield_now();
    }
    (app, view)
}

fn app_with_section() -> (DesktopApp, Id) {
    let mut app = DesktopApp::new().unwrap();
    let level = app.active_level;
    let wall = os_model::Wall::new(
        os_walls::WALL_TYPE,
        os_model::WallParams {
            name: "Section sheet wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(8.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    app.editor
        .command("Add section sheet fixture", Command::AddWall(wall))
        .unwrap();
    let section = app
        .editor
        .create_section_view(
            "Sheet section",
            level,
            os_model::SectionViewSettings::new(
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 0.0),
                -0.5,
                4.0,
            ),
        )
        .unwrap();
    app.plans.poll(&app.editor);
    app.focus_plan(Some(section));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.plans.poll(&app.editor);
        if app.plans.ready() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "section worker did not settle: {:?}",
            app.plans.error
        );
        std::thread::yield_now();
    }
    (app, section)
}

fn raw_input(size: egui::Vec2, scale: f32, time: f64, events: Vec<egui::Event>) -> egui::RawInput {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        time: Some(time),
        events,
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(scale);
    input
}

#[test]
fn create_sheet_preview_scale_history_and_pdf_confirmation_round_trip() {
    for (size, scale) in PROFILES {
        let (mut app, view) = app_with_plan();
        let camera = PlanCamera {
            center: Point2::new(3.0, -2.0),
            ..PlanCamera::default()
        };
        app.plans.cameras.insert(view, camera);
        app.create_sheet_from_active_view();
        assert!(!app.status_error, "{}", app.status);
        let sheet_id = app.plans.active_sheet.unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.plans.poll(&app.editor);
            if app.plans.ready() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "sheet source plan did not refresh"
            );
            std::thread::yield_now();
        }
        let sheet = &app.editor.document.model().sheets[&sheet_id];
        assert_eq!(sheet.parameters.number, "A101");
        assert_eq!(sheet.parameters.viewports.len(), 1);
        assert_eq!(sheet.parameters.viewports[0].view, view);
        assert_eq!(sheet.parameters.viewports[0].model_center_m, camera.center);
        let viewport = &sheet.parameters.viewports[0];
        let map = PaperViewport {
            center_mm: viewport.paper_center_mm,
            width_mm: viewport.width_mm,
            height_mm: viewport.height_mm,
            model_center_m: viewport.model_center_m,
            scale_denominator: 100.0,
        };
        assert_eq!(
            map.model_to_paper(Point2::new(camera.center.x + 1.0, camera.center.y))
                .unwrap()
                .x
                - map.center_mm.x,
            10.0
        );

        app.sheet_page().unwrap();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let output = ctx.run(raw_input(size, scale, 1.0, vec![]), |ctx| {
            app.plan_workspace(ctx);
        });
        let visible_text: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Sheet source plan - Sheet"
            )),
            "sheet title missing from preview: {visible_text:?}"
        );
        assert!(visible_text.iter().any(|text| text == "Edit source plan"));
        assert!(visible_text.iter().any(|text| text == "Apply layout"));
        assert!(visible_text.iter().any(|text| text == "Export vector PDF"));
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Rect(rect) if rect.fill == egui::Color32::WHITE
        )));

        let original = app.editor.document.model().clone();
        let revision = app.editor.document.revision();
        app.apply_sheet_layout(sheet_id, 50.0, Point2::new(204.0, 120.0));
        assert!(!app.status_error, "{}", app.status);
        assert_eq!(app.editor.document.revision(), revision + 1);
        assert_eq!(
            app.editor.document.model().sheets[&sheet_id]
                .parameters
                .viewports[0]
                .paper_center_mm,
            Point2::new(204.0, 120.0)
        );
        let edited = app.editor.document.model().clone();
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &original);
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model(), &edited);
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("sheet.osb");
        app.editor.save(&project).unwrap();
        app.editor.open(&project).unwrap();
        assert_eq!(app.editor.document.model(), &edited);

        app.plans.poll(&app.editor);
        app.plans.active = Some(view);
        app.plans.active_sheet = Some(sheet_id);
        app.plans.poll(&app.editor);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.plans.ready() {
            assert!(Instant::now() < deadline, "reopened plan did not settle");
            app.plans.poll(&app.editor);
            std::thread::yield_now();
        }
        let reopened_page = app.sheet_page().unwrap();
        let pdf = reopened_page.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(
            String::from_utf8(pdf.clone())
                .unwrap()
                .contains("/MediaBox [0 0 1190.551181 841.889764]")
        );

        let path = directory.path().join("A101.pdf");
        app.plans.pdf_path = path.display().to_string();
        let was_dirty = app.editor.is_dirty();
        app.prepare_sheet_pdf(&reopened_page);
        let pending = app.plans.pending_pdf.as_ref().unwrap();
        assert_eq!(pending.path, path);
        assert_eq!(pending.bytes, pdf);
        assert_eq!(app.editor.is_dirty(), was_dirty);
        let ui_context = egui::Context::default();
        theme::apply(&ui_context);
        let confirmation = ui_context.run(raw_input(size, scale, 1.0, vec![]), |ctx| {
            app.show(ctx);
        });
        let export_at = confirmation.shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "Export PDF"
            {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
            } else {
                None
            }
        });
        let export_at = export_at.expect("PDF export requires a visible confirmation");
        let _ = ui_context.run(
            raw_input(
                size,
                scale,
                2.0,
                vec![
                    egui::Event::PointerMoved(export_at),
                    egui::Event::PointerButton {
                        pos: export_at,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            ),
            |ctx| app.show(ctx),
        );
        let _ = ui_context.run(
            raw_input(
                size,
                scale,
                2.0 + 1.0 / 60.0,
                vec![egui::Event::PointerButton {
                    pos: export_at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            ),
            |ctx| app.show(ctx),
        );
        assert_eq!(std::fs::read(path).unwrap(), pdf);
        assert_eq!(app.editor.is_dirty(), was_dirty);
    }
}

#[test]
fn section_sheets_export_linked_cut_lines_and_reopen_in_source_view_space() {
    let (mut app, section) = app_with_section();
    app.plans.cameras.insert(
        section,
        PlanCamera {
            center: Point2::new(100.0, 100.0),
            pixels_per_metre: 30.0,
        },
    );
    app.create_sheet_from_active_view();
    assert!(!app.status_error, "{}", app.status);
    let sheet_id = app.plans.active_sheet.unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.plans.poll(&app.editor);
        if app.plans.ready() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "sheet section drawing did not settle"
        );
        std::thread::yield_now();
    }

    let viewport = &app.editor.document.model().sheets[&sheet_id]
        .parameters
        .viewports[0];
    assert_eq!(viewport.view, section);
    assert_eq!(viewport.model_center_m, Point2::new(4.0, 1.75));
    let page = app.sheet_page().unwrap();
    let clipped_paths = |page: &SheetPage| {
        page.marks()
            .iter()
            .filter(|mark| {
                mark.clip.is_some()
                    && matches!(&mark.kind, PaperMarkKind::Path { closed: false, .. })
            })
            .count()
    };
    assert!(
        clipped_paths(&page) >= 4,
        "section cut contours missing from sheet"
    );
    let pdf = page.to_pdf().unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"));

    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("section-sheet.osb");
    app.editor.save(&project).unwrap();
    app.editor.open(&project).unwrap();
    app.plans.poll(&app.editor);
    app.plans.active = Some(section);
    app.plans.active_sheet = Some(sheet_id);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.plans.ready() {
        assert!(Instant::now() < deadline, "reopened section did not settle");
        app.plans.poll(&app.editor);
        std::thread::yield_now();
    }
    let reopened_page = app.sheet_page().unwrap();
    assert_eq!(clipped_paths(&reopened_page), clipped_paths(&page));
    assert!(reopened_page.to_pdf().unwrap().starts_with(b"%PDF-1.4"));

    let mut multiple = app.editor.document.model().sheets[&sheet_id]
        .parameters
        .clone();
    let mut duplicate = multiple.viewports[0].clone();
    duplicate.id = Id::new();
    multiple.viewports.push(duplicate);
    app.editor
        .command(
            "Add unsupported second viewport",
            Command::UpdateSheet {
                id: sheet_id,
                parameters: multiple,
            },
        )
        .unwrap();
    assert!(
        app.sheet_page()
            .unwrap_err()
            .to_string()
            .contains("multiple viewports are not rendered yet")
    );
}

#[test]
fn pdf_writer_never_clobbers_without_explicit_replace_flag() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("drawing.pdf");
    write_sheet_pdf(&path, b"first", false).unwrap();
    assert!(write_sheet_pdf(&path, b"second", false).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"first");
    write_sheet_pdf(&path, b"second", true).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"second");
}
