use super::super::sheet_tests::raw_input;
use super::*;
use std::time::{Duration, Instant};

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

struct Harness {
    app: DesktopApp,
    view: Id,
    ctx: egui::Context,
    size: egui::Vec2,
    scale: f32,
    time: f64,
}

impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let mut settings = os_model::SectionViewSettings::new(
            Point2::new(18., 20.),
            Point2::new(26., 26.),
            -10.,
            10.,
        );
        settings.min_station = -10.;
        settings.max_station = 10.;
        if scale > 1. {
            std::mem::swap(&mut settings.start, &mut settings.end);
            settings.side = os_model::ElevationSide::Right;
            settings.mode = os_model::SectionDisplayMode::CutAndProjection;
        }
        let view = app
            .editor
            .create_section_view("Parent section", app.active_level, settings)
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Self {
            app,
            view,
            ctx,
            size,
            scale,
            time: 0.0,
        };
        h.settle();
        h
    }

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
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing visible control {label:?}"));
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerMoved(at), button(at, pressed)]);
        }
    }

    fn open_draw(&mut self, reference: Option<Id>) {
        self.click("Section callouts");
        self.app.plans.section_callouts.reference = reference;
        self.click("Draw rectangular callout");
        assert!(self.app.plans.section_callouts.draft.is_some());
        assert!(!self.app.plans.section_callouts.open);
        self.frame(vec![]);
    }

    fn canvas_rect(&self) -> egui::Rect {
        self.app
            .plans
            .canvas_rect
            .expect("plan canvas was rendered")
    }

    fn drag_canvas(&mut self, start: egui::Pos2, end: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
        self.frame(vec![egui::Event::PointerMoved(end)]);
        self.frame(vec![egui::Event::PointerMoved(end)]);
        self.frame(vec![button(end, false)]);
    }

    fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() {
                return;
            }
            assert!(Instant::now() < deadline, "plan worker did not settle");
            std::thread::yield_now();
        }
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

#[test]
fn rectangular_callout_preview_commit_undo_and_redo_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.open_draw(None);
        let canvas = h.canvas_rect();
        let start = canvas.center() + egui::vec2(-48.0, 38.0);
        let end = canvas.center() + egui::vec2(63.0, -52.0);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();

        h.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
        h.frame(vec![egui::Event::PointerMoved(end)]);
        assert!(
            h.app
                .plans
                .section_callouts
                .draft
                .as_ref()
                .unwrap()
                .rectangle
                .is_some()
        );
        assert_eq!(
            h.app.editor.document.model(),
            &before,
            "preview is non-mutating"
        );
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.plans.cameras[&h.view], PlanCamera::default());

        h.frame(vec![button(end, false)]);
        h.settle();
        let after = h.app.editor.document.model().clone();
        assert_eq!(after.views.len(), before.views.len() + 1);
        assert_eq!(after.section_callouts.len(), 1);
        let marker = after.section_callouts.values().next().unwrap();
        assert_eq!(marker.parameters.parent, h.view);
        assert!(marker.parameters.owns_target);
        let target = &after.views[&marker.parameters.target];
        let mut expected = before.views[&h.view].parameters.section.unwrap();
        expected.min_station = marker.parameters.boundary.min.x;
        expected.max_station = marker.parameters.boundary.max.x;
        expected.bottom_elevation = marker.parameters.boundary.min.y;
        expected.top_elevation = marker.parameters.boundary.max.y;
        expected.scale_denominator = 25.;
        assert_eq!(target.parameters.section, Some(expected));
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1,
            "the view and marker are one transaction"
        );
        assert!(h.app.editor.document.undo());
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(h.app.editor.document.redo());
        assert_eq!(h.app.editor.document.model(), &after);
    }
}

#[test]
fn reference_only_callout_moves_and_navigates_without_owning_target() {
    let mut h = Harness::new(egui::vec2(1280.0, 800.0), 1.0);
    let target = h
        .app
        .editor
        .create_section_view(
            "Existing detail",
            h.app.active_level,
            h.app.editor.document.model().views[&h.view]
                .parameters
                .section
                .unwrap(),
        )
        .unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.settle();
    let views_before = h.app.editor.document.model().views.clone();
    h.open_draw(Some(target));
    let canvas = h.canvas_rect();
    let start = canvas.center() + egui::vec2(-48.0, 38.0);
    let end = canvas.center() + egui::vec2(63.0, -52.0);
    h.drag_canvas(start, end);
    let marker = h
        .app
        .editor
        .document
        .model()
        .section_callouts
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(h.app.editor.document.model().views, views_before);
    assert_eq!(marker.parameters.target, target);
    assert!(!marker.parameters.owns_target);

    h.click("Section callouts");
    h.click("Existing detail: Unplaced");
    h.click("Open target");
    assert_eq!(h.app.plans.active, Some(target));

    h.app.focus_plan(Some(h.view));
    h.click("Section callouts");
    h.click("Existing detail: Unplaced");
    h.click("Drag marker");
    let canvas = h.canvas_rect();
    let camera = h.app.plans.cameras[&h.view];
    let center = Point2::new(
        (marker.parameters.boundary.min.x + marker.parameters.boundary.max.x) * 0.5,
        (marker.parameters.boundary.min.y + marker.parameters.boundary.max.y) * 0.5,
    );
    let projected = camera
        .project(
            center,
            [f64::from(canvas.width()), f64::from(canvas.height())],
        )
        .unwrap();
    let start = canvas.min + egui::vec2(projected.x as f32, projected.y as f32);
    let end = start + egui::vec2(24.0, -12.0);
    let before_move = h.app.editor.document.model().clone();
    let history = h.app.editor.document.history_stats();
    h.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
    h.frame(vec![egui::Event::PointerMoved(end)]);
    assert_eq!(h.app.editor.document.model(), &before_move);
    h.frame(vec![button(end, false)]);
    let moved = &h.app.editor.document.model().section_callouts[&marker.id()].parameters;
    assert!(moved.boundary.min.x > marker.parameters.boundary.min.x);
    assert!(moved.boundary.max.x > marker.parameters.boundary.max.x);
    assert!(
        ((moved.boundary.max.x - moved.boundary.min.x)
            - (marker.parameters.boundary.max.x - marker.parameters.boundary.min.x))
            .abs()
            < 1e-12
    );
    assert_eq!(
        h.app.editor.document.history_stats().undo_entries,
        history.undo_entries + 1
    );

    h.click("Section callouts");
    h.click("Existing detail: Unplaced");
    h.click("Edit target crop");
    assert_eq!(h.app.plans.active, Some(target));
    assert_eq!(
        h.app.plans.section_placement.as_ref().unwrap().editing,
        Some(target)
    );
}

#[test]
fn escape_and_stale_revision_cancel_callout_drafts_without_committing() {
    let mut h = Harness::new(egui::vec2(1280.0, 800.0), 1.0);
    h.open_draw(None);
    let canvas = h.canvas_rect();
    let start = canvas.center() + egui::vec2(-40.0, 30.0);
    let end = canvas.center() + egui::vec2(45.0, -36.0);
    let before = h.app.editor.document.model().clone();
    let history = h.app.editor.document.history_stats();

    h.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
    h.frame(vec![egui::Event::PointerMoved(end)]);
    h.frame(vec![escape()]);
    assert!(h.app.plans.section_callouts.draft.is_none());
    h.frame(vec![button(end, false)]);
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.document.history_stats(), history);

    h.open_draw(None);
    let canvas = h.canvas_rect();
    let start = canvas.center() + egui::vec2(-40.0, 30.0);
    let end = canvas.center() + egui::vec2(45.0, -36.0);
    h.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
    h.frame(vec![egui::Event::PointerMoved(end)]);
    let mut changed = h.app.editor.document.model().views[&h.view]
        .parameters
        .clone();
    changed.name = "Changed during callout".into();
    h.app
        .editor
        .document
        .execute(
            "Concurrent plan edit",
            vec![Command::UpdateView {
                id: h.view,
                parameters: changed,
            }],
        )
        .unwrap();
    let external_model = h.app.editor.document.model().clone();
    let external_history = h.app.editor.document.history_stats();
    h.frame(vec![]);
    assert!(h.app.plans.section_callouts.draft.is_none());
    h.frame(vec![button(end, false)]);
    assert_eq!(h.app.editor.document.model(), &external_model);
    assert_eq!(h.app.editor.document.history_stats(), external_history);
    assert!(h.app.editor.document.model().section_callouts.is_empty());
}
