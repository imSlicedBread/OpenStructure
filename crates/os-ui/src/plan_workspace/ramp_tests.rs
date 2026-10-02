//! Headless egui acceptance for native straight-run ramp authoring.
use super::*;
const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    scale: f32,
    time: f64,
    view: Id,
}

impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let view = app
            .editor
            .create_floor_plan("Ramp plan", app.active_level)
            .unwrap();
        app.add_level();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        app.plans.snaps.enabled = false;
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut result = Self {
            app,
            ctx,
            size,
            scale,
            time: 0.0,
            view,
        };
        result.settle();
        let camera = result.app.plans.cameras.get_mut(&view).unwrap();
        camera.center = Point2::new(0.0, 0.0);
        camera.pixels_per_metre = 30.0;
        result.frame(vec![]);
        assert_eq!(result.ctx.pixels_per_point(), scale);
        result
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.scale);
        self.ctx.run(input, |ctx| self.app.show(ctx))
    }

    fn settle(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() && !self.app.editor.plugin_work_pending() {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "plan did not settle");
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }

    fn point(&self, world: Point2) -> egui::Pos2 {
        let rect = self.app.plans.canvas_rect.unwrap();
        let context = self.app.editor.native_plan_context(self.view).unwrap();
        let local = self.app.plans.cameras[&self.view]
            .project(
                context.basis.world_to_plane(world).unwrap(),
                [f64::from(rect.width()), f64::from(rect.height())],
            )
            .unwrap();
        let point = rect.min + egui::vec2(local.x as f32, local.y as f32);
        assert!(rect.contains(point), "fixture point must lie in canvas");
        point
    }

    fn hover(&mut self, point: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(point)])
    }

    fn click(&mut self, point: egui::Pos2) {
        self.hover(point);
        self.frame(vec![button(point, true)]);
        self.frame(vec![button(point, false)]);
    }

    fn properties_frame(&mut self) {
        self.time += 1.0 / 60.0;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.scale);
        let _ = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.app.ramp_properties(ui);
            });
        });
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
fn two_click_preview_commit_scene_plan_pick_and_history_at_both_profiles() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.plans.split = true;
        h.frame(vec![]);
        let original = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        let scene_count = h.app.editor.scene.len();
        h.app.begin_ramp(h.view);
        h.frame(vec![]);
        let start = Point2::new(-2.0, 0.0);
        let end = Point2::new(2.0, 1.0);
        h.click(h.point(start));
        assert_eq!(
            h.app.plans.ramp_placement.as_ref().unwrap().first,
            Some(start)
        );
        let end_screen = h.point(end);
        let output = h.hover(end_screen);
        assert!(!output.shapes.is_empty());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        assert_eq!(h.app.editor.scene.len(), scene_count);
        let draft = h.app.plans.ramp_placement.as_ref().unwrap();
        let preview = ramps::preview(&h.app.editor, draft, end).unwrap().unwrap();
        let ramp_id = preview
            .ramps(h.app.editor.native_plan_context(h.view).unwrap())
            .unwrap()[0]
            .entity;

        h.frame(vec![button(end_screen, true)]);
        assert_eq!(h.app.editor.document.model(), &original);
        h.frame(vec![button(end_screen, false)]);
        assert_eq!(h.app.selected, Some(ramp_id));
        let ramp = h.app.editor.document.model().ramps[&ramp_id].clone();
        assert_eq!(ramp.parameters.start, start);
        assert_eq!(ramp.parameters.end, end);
        assert_eq!(ramp.parameters.width, 1.5);
        assert_eq!(ramp.parameters.structural_thickness, 0.18);
        assert!(h.app.editor.scene[&ramp_id].signed_volume() > 0.0);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        assert_eq!(drawing.ramps(context).unwrap().len(), 1);
        assert_eq!(
            drawing
                .pick(
                    context,
                    context.basis.world_to_plane(Point2::new(0.0, 0.5)).unwrap()
                )
                .unwrap(),
            Some(ramp_id)
        );
        assert_eq!(
            drawing.pick_ramp(context, Point2::new(0.0, 0.5)).unwrap(),
            Some(ramp_id)
        );

        h.app.editor.undo().unwrap();
        assert!(!h.app.editor.scene.contains_key(&ramp_id));
        h.app.editor.redo().unwrap();
        assert_eq!(h.app.editor.document.model().ramps[&ramp_id], ramp);
        assert!(h.app.editor.scene.contains_key(&ramp_id));
    }
}

#[test]
fn invalid_release_escape_stale_edit_and_ordinary_pan_do_not_leak_drafts() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        h.app.begin_ramp(h.view);
        h.app.ramp_click(Point2::new(-1.0, 0.0));
        h.app.ramp_click(Point2::new(-1.0, 0.0));
        assert!(h.app.status_error, "coincident endpoints are rejected");
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        h.app.ramp_click(Point2::new(-1.0, 0.0));
        h.frame(vec![escape()]);
        assert!(h.app.plans.ramp_placement.is_none());
        assert_eq!(h.app.editor.document.model(), &before);

        h.app.begin_ramp(h.view);
        h.app.ramp_click(Point2::new(-1.0, 0.0));
        let upper = *h
            .app
            .editor
            .document
            .model()
            .levels
            .keys()
            .find(|id| **id != h.app.active_level)
            .unwrap();
        let mut level = h.app.editor.document.model().levels[&upper]
            .parameters
            .clone();
        level.elevation += 0.25;
        h.app
            .editor
            .command(
                "Raise upper level",
                Command::UpdateLevel {
                    id: upper,
                    parameters: level,
                },
            )
            .unwrap();
        h.frame(vec![]);
        assert!(h.app.plans.ramp_placement.is_none());
        assert!(h.app.editor.document.model().ramps.is_empty());
        // The cancellation status can wrap and shift the canvas at high DPI.
        h.frame(vec![]);

        let point = h.point(Point2::new(-1.0, -1.0));
        let before_camera = h.app.plans.cameras[&h.view];
        h.hover(point);
        h.frame(vec![button(point, true)]);
        h.frame(vec![egui::Event::PointerMoved(
            point + egui::vec2(24.0, 12.0),
        )]);
        h.frame(vec![button(point + egui::vec2(24.0, 12.0), false)]);
        assert_ne!(h.app.plans.cameras[&h.view], before_camera);
        assert!(h.app.editor.document.model().ramps.is_empty());
    }
}

#[test]
fn ramp_property_edit_preserves_identity_and_stale_drafts_cannot_overwrite() {
    let mut h = Harness::new(egui::vec2(1280.0, 800.0), 1.0);
    h.app.begin_ramp(h.view);
    h.app.ramp_click(Point2::new(-2.0, 0.0));
    h.app.ramp_click(Point2::new(2.0, 1.0));
    let id = h.app.selected.unwrap();
    h.frame(vec![escape()]);
    let original = h.app.editor.document.model().ramps[&id].clone();
    h.properties_frame();
    h.app.plans.ramp_edit.as_mut().unwrap().parameters.width = 1.8;
    h.properties_frame();
    assert_eq!(h.app.editor.document.model().ramps[&id], original);
    h.app.apply_ramp_properties();
    let edited = h.app.editor.document.model().ramps[&id].clone();
    assert_eq!(edited.header, original.header);
    assert_eq!(edited.parameters.width, 1.8);
    assert_eq!(edited.parameters.material, original.parameters.material);
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
    h.app.editor.undo().unwrap();
    assert_eq!(h.app.editor.document.model().ramps[&id], original);
    h.app.editor.redo().unwrap();
    assert_eq!(h.app.editor.document.model().ramps[&id], edited);

    h.properties_frame();
    h.app.plans.ramp_edit.as_mut().unwrap().parameters.width = 2.0;
    let mut level = h.app.editor.document.model().levels[&edited.parameters.upper_level]
        .parameters
        .clone();
    level.elevation += 0.25;
    h.app
        .editor
        .command(
            "Raise ramp level",
            Command::UpdateLevel {
                id: edited.parameters.upper_level,
                parameters: level,
            },
        )
        .unwrap();
    h.app.apply_ramp_properties();
    assert!(h.app.status_error);
    assert_eq!(
        h.app.editor.document.model().ramps[&id].parameters.width,
        1.8
    );
}
