use super::super::sheet_tests::{app_with_section, raw_input};
use super::*;
use std::time::{Duration, Instant};

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    scale: f32,
    time: f64,
}
impl Harness {
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.1;
        self.ctx
            .run(raw_input(self.size, self.scale, self.time, events), |ctx| {
                self.app.plan_workspace(ctx)
            })
    }
    fn click(&mut self, label: &str) {
        self.frame(vec![]);
        let output = self.frame(vec![]);
        let at = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing visible control {label}"));
        assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains(at));
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerMoved(at), button(at, pressed)]);
        }
    }
    fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            let parameters = &self.app.plans.sheet_draft.as_ref().unwrap().parameters;
            let page = self.app.sheet_page_for(parameters);
            if page.is_ok() {
                return;
            }
            assert!(Instant::now() < deadline, "{page:?}");
            std::thread::yield_now();
        }
    }
    fn point(&self, point: Point2) -> egui::Pos2 {
        let canvas = self
            .app
            .plans
            .sheet_draft
            .as_ref()
            .unwrap()
            .canvas_rect
            .unwrap();
        let (paper, scale) = sheet_fit_rect(canvas, 420.0, 297.0).unwrap();
        paper_to_screen(point, paper, scale)
    }
    fn drag(&mut self, a: Point2, b: Point2) {
        let start = self.point(a);
        let end = self.point(b);
        self.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
        self.frame(vec![egui::Event::PointerMoved(end)]);
        self.frame(vec![egui::Event::PointerMoved(end)]);
        self.frame(vec![button(end, false)]);
    }
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn fixture(size: egui::Vec2, scale: f32) -> (Harness, Id, [Id; 4], Id, Id) {
    let (mut app, section) = app_with_section();
    let wall = *app.editor.document.model().walls.keys().next().unwrap();
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "D01".into(),
            host: wall,
            offset: 1.0,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            },
            open_state: Default::default(),
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let door = opening.id();
    let mut params = os_model::ScheduleParams::new("Doors", os_model::ScheduleCategory::Door);
    params.columns = vec![
        os_model::ScheduleColumn::Name,
        os_model::ScheduleColumn::Width,
    ];
    let schedule = os_model::Schedule::new("core.schedule", params);
    let schedule_id = schedule.id();
    app.editor
        .document
        .execute(
            "Fixtures",
            vec![Command::AddOpening(opening), Command::AddSchedule(schedule)],
        )
        .unwrap();
    let plan = app
        .editor
        .create_floor_plan("Floor plan", app.active_level)
        .unwrap();
    let left = app
        .editor
        .create_wall_elevation("Left elevation", wall, os_model::ElevationSide::Left)
        .unwrap();
    let right = app
        .editor
        .create_wall_elevation("Right elevation", wall, os_model::ElevationSide::Right)
        .unwrap();
    app.focus_plan(Some(plan));
    app.create_sheet_from_active_view();
    let sheet = app.plans.active_sheet.unwrap();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    (
        Harness {
            app,
            ctx,
            size,
            scale,
            time: 0.0,
        },
        sheet,
        [plan, left, right, section],
        schedule_id,
        door,
    )
}

fn arrange(h: &mut Harness, views: [Id; 4], schedule: Id) {
    h.frame(vec![]);
    for view in &views[1..] {
        let before = h
            .app
            .plans
            .sheet_draft
            .as_ref()
            .unwrap()
            .parameters
            .viewports
            .clone();
        h.app.plans.sheet_draft.as_mut().unwrap().source = Some(*view);
        h.click("Add linked view");
        let after = &h
            .app
            .plans
            .sheet_draft
            .as_ref()
            .unwrap()
            .parameters
            .viewports;
        assert_eq!(
            &after[..before.len()],
            &before,
            "adding must not rearrange existing placements"
        );
    }
    let draft = h.app.plans.sheet_draft.as_mut().unwrap();
    for (i, v) in draft.parameters.viewports.iter_mut().enumerate() {
        v.paper_center_mm = Point2::new(
            if i % 2 == 0 { 109.0 } else { 310.0 },
            if i < 2 { 53.0 } else { 133.0 },
        );
        v.width_mm = 182.0;
        v.height_mm = 64.0;
        v.scale_denominator = if i == 1 { 50.0 } else { 100.0 };
        v.model_center_m = if i == 0 {
            Point2::new(4.0, 0.0)
        } else {
            Point2::new(4.0, 1.5)
        };
        v.title_override = Some(format!("Detail {}", i + 1));
    }
    draft.schedule = Some(schedule);
    h.click("Add saved schedule to sheet");
    h.settle();
}

#[test]
fn coordinated_sheet_four_sources_schedule_draft_pointer_history_refresh_and_pdf_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let (mut h, sheet, views, schedule, door) = fixture(size, scale);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        arrange(&mut h, views, schedule);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(
            h.app.plans.active,
            Some(views[0]),
            "adding sources must not navigate"
        );
        h.drag(Point2::new(109.0, 53.0), Point2::new(112.0, 55.0));
        let moved = h
            .app
            .plans
            .sheet_draft
            .as_ref()
            .unwrap()
            .parameters
            .viewports[0]
            .clone();
        assert!(
            (moved.paper_center_mm.x - 112.0).abs() < 0.1,
            "{:?}",
            moved.paper_center_mm
        );
        h.drag(Point2::new(203.0, 87.0), Point2::new(207.0, 90.0));
        let resized = &h
            .app
            .plans
            .sheet_draft
            .as_ref()
            .unwrap()
            .parameters
            .viewports[0];
        assert!(
            (resized.width_mm - 186.0).abs() < 0.1,
            "{}",
            resized.width_mm
        );
        assert_eq!(h.app.editor.document.model(), &before);
        let draft_page = h
            .app
            .sheet_page_for(&h.app.plans.sheet_draft.as_ref().unwrap().parameters)
            .unwrap();
        let exact = h.app.plans.sheet_draft.as_ref().unwrap().parameters.clone();
        h.click("Apply layout");
        assert!(!h.app.status_error, "{}", h.app.status);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_eq!(
            h.app.editor.document.model().sheets[&sheet].parameters,
            exact
        );
        let committed = h.app.editor.document.model().clone();
        h.app.editor.undo().unwrap();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.editor.redo().unwrap();
        assert_eq!(h.app.editor.document.model(), &committed);
        h.settle();
        let page = h.app.sheet_page().unwrap();
        assert_eq!(page, draft_page);
        h.app.plans.pdf_path = "coordinated.pdf".into();
        h.app.prepare_sheet_pdf(&page);
        assert_eq!(
            h.app.plans.pending_pdf.as_ref().unwrap().bytes,
            page.to_pdf().unwrap()
        );
        let mut parameters = h.app.editor.document.model().openings[&door]
            .parameters
            .clone();
        if let OpeningDefinition::Legacy { width, .. } = &mut parameters.definition {
            *width = 1.4;
        }
        h.app
            .editor
            .command(
                "Widen door",
                Command::UpdateOpening {
                    id: door,
                    parameters,
                },
            )
            .unwrap();
        assert!(
            h.app.sheet_page().is_err(),
            "stale sheet must fail before polling"
        );
        h.frame(vec![]);
        assert!(h.app.plans.pending_pdf.is_none());
        h.settle();
        let live = h.app.sheet_page().unwrap();
        for viewport in &exact.viewports {
            let mut rect = PaperViewport {
                center_mm: viewport.paper_center_mm,
                width_mm: viewport.width_mm,
                height_mm: viewport.height_mm,
                model_center_m: viewport.model_center_m,
                scale_denominator: viewport.scale_denominator,
            }
            .paper_rect()
            .unwrap();
            rect.max_mm.y -= 7.0; // Multi-view frames reserve a caption footer.
            let marks = |page: &SheetPage| {
                page.marks()
                    .iter()
                    .filter(|m| m.clip == Some(rect))
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_ne!(
                marks(&page),
                marks(&live),
                "source {} must refresh",
                viewport.view
            );
        }
        assert!(
            String::from_utf8(live.to_pdf().unwrap())
                .unwrap()
                .contains("<312E343030> Tj")
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("coordinated.osb");
        let saved = h.app.editor.document.model().clone();
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.open_sheet(sheet);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &saved);
        assert_eq!(h.app.sheet_page().unwrap(), live);
        h.app.plans.sheet_draft.as_mut().unwrap().selected = Some(exact.viewports[1].id);
        h.click("Edit source elevation");
        assert_eq!(h.app.plans.active, Some(views[1]));
        assert!(h.app.plans.active_sheet.is_none());
    }
}

#[test]
fn coordinated_sheet_cancel_invalid_fit_stale_and_export_fail_closed() {
    let (mut h, sheet, views, schedule, _) = fixture(egui::vec2(1000.0, 650.0), 1.5);
    let before = h.app.editor.document.model().clone();
    arrange(&mut h, views, schedule);
    let good = h.app.plans.sheet_draft.as_ref().unwrap().parameters.clone();
    for invalid in 0..5 {
        let draft = h.app.plans.sheet_draft.as_mut().unwrap();
        draft.parameters = good.clone();
        match invalid {
            0 => {
                draft.parameters.viewports[1].paper_center_mm =
                    draft.parameters.viewports[0].paper_center_mm
            }
            1 => draft.parameters.viewports[0].paper_center_mm.x = 0.0,
            2 => draft.parameters.viewports[0].scale_denominator = 0.0,
            3 => {
                draft.parameters.schedule_placements[0]
                    .paper_rect_mm
                    .max_mm
                    .y = 179.0
            }
            _ => draft.parameters.viewports[0].view = Id::new(),
        }
        h.click("Apply layout");
        assert!(h.app.status_error);
        assert_eq!(h.app.editor.document.model(), &before);
    }
    h.app.plans.sheet_draft.as_mut().unwrap().parameters = good;
    h.frame(vec![escape()]);
    assert!(h.app.plans.sheet_draft.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
    h.settle();
    let page = h.app.sheet_page().unwrap();
    h.app.plans.pdf_path = "draft.pdf".into();
    h.app
        .plans
        .sheet_draft
        .as_mut()
        .unwrap()
        .parameters
        .viewports[0]
        .scale_denominator = 200.0;
    h.app.prepare_sheet_pdf(&page);
    assert!(h.app.plans.pending_pdf.is_none());
    let mut stale = Draft::new(&h.app.editor, sheet);
    stale.parameters.viewports[0].scale_denominator = 150.0;
    let mut parameters = before.sheets[&sheet].parameters.clone();
    parameters.name = "Changed externally".into();
    h.app
        .editor
        .command(
            "Change",
            Command::UpdateSheet {
                id: sheet,
                parameters,
            },
        )
        .unwrap();
    assert!(!stale.current(&h.app.editor, Some(sheet)));
    assert!(h.app.apply_sheet_draft(&stale).is_err());
    h.app.plans.poll(&h.app.editor);
    assert!(h.app.plans.sheet_draft.is_none());
    assert!(!Draft::new(&h.app.editor, sheet).current(&h.app.editor, None));
}
