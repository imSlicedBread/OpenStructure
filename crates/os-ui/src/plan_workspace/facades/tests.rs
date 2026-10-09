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
        let out = self.frame(vec![]);
        let at = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing action {label}"));
        assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains(at));
        self.click_at(at);
    }
    fn click_at(&mut self, p: egui::Pos2) {
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerMoved(p), button(p, pressed)]);
        }
    }
    fn point(&self, p: Point2) -> egui::Pos2 {
        self.app
            .plans
            .test_screen_point(self.app.plans.active.unwrap(), p)
            .unwrap()
    }
    fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", self.app.plans.error);
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
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
fn fixture(size: egui::Vec2, scale: f32) -> (Harness, Id, Id) {
    let (mut app, _) = app_with_section();
    let wall = *app.editor.document.model().walls.keys().next().unwrap();
    let plan = app
        .editor
        .create_floor_plan("Facade authoring", app.active_level)
        .unwrap();
    app.focus_plan(Some(plan));
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let mut h = Harness {
        app,
        ctx,
        size,
        scale,
        time: 0.,
    };
    h.settle();
    h.app.plans.cameras.insert(
        plan,
        PlanCamera {
            center: Point2::new(4., 2.),
            pixels_per_metre: 35.,
        },
    );
    h.frame(vec![]);
    (h, plan, wall)
}
#[test]
fn facade_real_egui_baseline_preview_settings_history_marker_picking_save_sheet_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let (mut h, plan, wall) = fixture(size, scale);
        let original = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let scene = h.app.editor.scene.clone();
        h.click("Facade authoring");
        h.click("Place facade");
        let camera = h.app.plans.cameras[&plan];
        h.click_at(h.point(Point2::new(-1., 5.)));
        h.frame(vec![egui::Event::PointerMoved(
            h.point(Point2::new(9., 5.)),
        )]);
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.editor.scene, scene);
        h.click_at(h.point(Point2::new(9., 5.)));
        assert!(h.app.plans.facade.as_ref().unwrap().settings.is_some());
        assert_eq!(h.app.plans.cameras[&plan], camera);
        h.click("View from right");
        h.click("View from left");
        h.click("Apply facade");
        assert!(!h.app.status_error, "{}", h.app.status);
        let view = h.app.plans.active.unwrap();
        assert_ne!(view, plan);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let created = h.app.editor.document.model().clone();
        let settings = created.views[&view].parameters.facade.unwrap();
        h.app.editor.undo().unwrap();
        assert_eq!(h.app.editor.document.model(), &original);
        h.app.editor.redo().unwrap();
        assert_eq!(h.app.editor.document.model(), &created);
        h.app.focus_plan(Some(view));
        h.settle();
        let context = h.app.editor.native_facade_context(view).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        let line = drawing
            .provider_lines(context)
            .unwrap()
            .iter()
            .find(|l| l.entity == wall)
            .unwrap()
            .clone();
        h.click_at(h.point(Point2::new(
            (line.start.x + line.end.x) / 2.,
            (line.start.y + line.end.y) / 2.,
        )));
        assert_eq!(h.app.selected, Some(wall));
        let plan_drawing = h.app.editor.native_drawing(plan).unwrap();
        assert!(
            plan_drawing
                .provider_lines(h.app.editor.native_plan_context(plan).unwrap())
                .unwrap()
                .iter()
                .any(|l| l.entity == view),
            "linked plan marker uses the view identity"
        );
        h.click("Facade settings");
        let before = h.app.editor.document.model().clone();
        h.app
            .plans
            .facade
            .as_mut()
            .unwrap()
            .settings
            .as_mut()
            .unwrap()
            .far_depth = -1.;
        h.click("Apply facade");
        assert!(h.app.status_error);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app
            .plans
            .facade
            .as_mut()
            .unwrap()
            .settings
            .as_mut()
            .unwrap()
            .far_depth = 20.;
        h.click("Apply facade");
        assert!(!h.app.status_error, "{}", h.app.status);
        assert_eq!(h.app.editor.document.model().views[&view].id(), view);
        h.settle();
        h.click("Facade settings");
        h.app
            .plans
            .facade
            .as_mut()
            .unwrap()
            .settings
            .as_mut()
            .unwrap()
            .far_depth = 18.;
        let current = h.app.editor.document.model().clone();
        h.frame(vec![escape()]);
        assert!(h.app.plans.facade.is_none());
        assert_eq!(h.app.editor.document.model(), &current);
        let mut p = h.app.editor.document.model().walls[&wall]
            .parameters
            .clone();
        let a = p.start();
        let b = p.end();
        p.path = os_model::WallPath::Straight { start: b, end: a };
        h.app
            .editor
            .command(
                "Reverse host",
                Command::UpdateWall {
                    id: wall,
                    parameters: p,
                },
            )
            .unwrap();
        assert!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .items(h.app.editor.native_facade_context(view).unwrap())
                .is_err()
        );
        h.settle();
        assert_eq!(
            h.app.editor.document.model().views[&view]
                .parameters
                .facade
                .unwrap()
                .axis,
            settings.axis
        );
        h.app.create_sheet_from_active_view();
        h.settle();
        let sheet = h.app.plans.active_sheet.unwrap();
        let page = h.app.sheet_page().unwrap();
        h.app.plans.pdf_path = "facade.pdf".into();
        h.app.prepare_sheet_pdf(&page);
        assert_eq!(
            h.app.plans.pending_pdf.as_ref().unwrap().bytes,
            page.to_pdf().unwrap()
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("facade.osb");
        let saved = h.app.editor.document.model().clone();
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.open_sheet(sheet);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &saved);
        assert_eq!(h.app.sheet_page().unwrap(), page);
        h.click("Edit source facade");
        assert_eq!(h.app.plans.active, Some(view));
        assert!(h.app.plans.active_sheet.is_none());
        h.app.focus_plan(Some(plan));
        h.settle();
        h.click("Facade authoring");
        h.click("Place facade");
        h.click_at(h.point(Point2::new(0., 4.)));
        h.frame(vec![egui::Event::PointerGone]);
        assert!(h.app.plans.facade.is_none());
        assert_eq!(h.app.editor.document.model(), &saved);
    }
}

#[test]
fn facade_stale_settings_and_obsolete_worker_never_publish() {
    use std::sync::mpsc;
    let (mut h, plan, wall) = fixture(egui::vec2(1000., 650.), 1.5);
    let settings =
        FacadeViewSettings::new(Point2::new(0., 5.), Point2::new(10., 5.), -1., 10.).unwrap();
    let view = h
        .app
        .editor
        .create_facade_view("Facade", h.app.active_level, settings)
        .unwrap();
    h.app.focus_plan(Some(view));
    h.settle();
    h.click("Facade settings");
    let snapshot = h.app.editor.native_view_snapshot(view).unwrap();
    let context = h.app.editor.native_facade_context(view).unwrap();
    let (release, wait) = mpsc::channel();
    h.app.plans.pending = Some((
        std::thread::spawn(move || {
            wait.recv().unwrap();
            snapshot.derive()
        }),
        true,
    ));
    let mut p = h.app.editor.document.model().walls[&wall]
        .parameters
        .clone();
    p.height = 4.;
    h.app
        .editor
        .command(
            "Height",
            Command::UpdateWall {
                id: wall,
                parameters: p,
            },
        )
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.facade.is_none());
    assert!(!h.app.plans.pending.as_ref().unwrap().1);
    assert!(h.app.plans.drawing.is_none());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !h.app.plans.pending.as_ref().unwrap().0.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    h.app.plans.poll(&h.app.editor);
    assert!(h.app.plans.drawing.is_none());
    h.settle();
    assert!(
        h.app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .items(context)
            .is_err()
    );
    h.click("Facade settings");
    h.app
        .plans
        .facade
        .as_mut()
        .unwrap()
        .providers
        .push(("changed".into(), Id::new()));
    h.frame(vec![]);
    assert!(h.app.plans.facade.is_none());
    h.app.focus_plan(Some(plan));
    h.settle();
    h.click("Facade authoring");
    h.click("Place facade");
    h.app.focus_plan(Some(view));
    assert!(h.app.plans.facade.is_none());
}
