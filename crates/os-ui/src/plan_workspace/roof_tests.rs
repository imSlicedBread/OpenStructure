//! Native roof workflow acceptance in real egui frames; no native-window claim.
use super::*;
use os_core::Point2;
use os_model::{Wall, WallParams};

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
            .create_floor_plan("Roof authoring", app.active_level)
            .unwrap();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut harness = Self {
            app,
            ctx,
            size,
            scale,
            time: 0.0,
            view,
        };
        harness.settle();
        let camera = harness.app.plans.cameras.get_mut(&view).unwrap();
        camera.center = Point2::new(20.0, 20.0);
        camera.pixels_per_metre = 30.0;
        harness.frame(vec![]);
        assert_eq!(harness.ctx.pixels_per_point(), scale);
        harness
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
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
        let _ = self.ctx.run(input, |ctx| self.app.show(ctx));
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
        let position = rect.min + egui::vec2(local.x as f32, local.y as f32);
        assert!(rect.contains(position), "fixture point must lie on canvas");
        position
    }

    fn click(&mut self, point: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(point)]);
        self.frame(vec![button(point, true)]);
        self.frame(vec![button(point, false)]);
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
fn roof_sketch_is_preview_only_then_commits_edits_history_plan_and_scene() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let original = h.app.editor.document.model().clone();
        let original_scene = h.app.editor.scene.clone();
        h.app.plans.split = true;
        h.app.plans.snaps.enabled = false;
        h.app.begin_roof(h.view);
        h.frame(vec![]);
        let boundary = [
            Point2::new(18.0, 19.0),
            Point2::new(22.0, 19.0),
            Point2::new(22.0, 22.0),
            Point2::new(18.0, 22.0),
        ];
        for vertex in boundary {
            h.click(h.point(vertex));
            assert_eq!(h.app.editor.document.model(), &original);
            assert_eq!(h.app.editor.document.revision(), 0);
            assert_eq!(h.app.editor.scene, original_scene);
        }
        assert_eq!(
            h.app.plans.roof_draft.as_ref().unwrap().points.len(),
            boundary.len(),
            "draft points; status: {}",
            h.app.status
        );

        h.app.finish_roof_loop();
        assert_eq!(
            h.app.plans.roof_draft.as_ref().unwrap().phase,
            roofs::Phase::SlopeStart
        );
        h.click(h.point(Point2::new(20.0, 20.0)));
        h.click(h.point(Point2::new(20.0, 23.0)));
        assert_eq!(h.app.editor.document.model(), &original);

        h.app.apply_roof();
        h.settle();
        let roof_id = h.app.selected.expect("committed roof selected");
        let roof = &h.app.editor.document.model().roofs[&roof_id];
        assert_eq!(roof.parameters.level, h.app.active_level);
        assert_eq!(roof.parameters.thickness, 0.2);
        assert_eq!(roof.parameters.top_offset, 0.0);
        assert_eq!(roof.parameters.rise_per_run, 0.25);
        assert_eq!(roof.parameters.slope_start, Point2::new(20.0, 20.0));
        assert_eq!(roof.parameters.slope_end, Point2::new(20.0, 23.0));
        assert!((h.app.editor.scene[&roof_id].signed_volume() - 2.4).abs() < 1e-9);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .items(context)
                .unwrap()
                .iter()
                .any(|i| i.entity == roof_id)
        );
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);

        // Properties opens an edit draft for the selected roof. Change just its
        // slope and verify identity, host level, thickness and offset survive.
        h.frame(vec![]);
        let before_edit = h.app.editor.document.model().roofs[&roof_id]
            .parameters
            .clone();
        h.app
            .plans
            .roof_draft
            .as_mut()
            .unwrap()
            .parameters
            .rise_per_run = -0.5;
        h.app.apply_roof();
        h.settle();
        let edited = &h.app.editor.document.model().roofs[&roof_id].parameters;
        assert_eq!(edited.rise_per_run, -0.5);
        assert_eq!(edited.level, before_edit.level);
        assert_eq!(edited.thickness, before_edit.thickness);
        assert_eq!(edited.top_offset, before_edit.top_offset);
        assert_eq!(edited.material, before_edit.material);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);

        h.app.editor.undo().unwrap();
        h.settle();
        assert_eq!(
            h.app.editor.document.model().roofs[&roof_id].parameters,
            before_edit
        );
        h.app.editor.redo().unwrap();
        h.settle();
        assert_eq!(
            h.app.editor.document.model().roofs[&roof_id]
                .parameters
                .rise_per_run,
            -0.5
        );

        h.app.editor.undo().unwrap();
        h.settle();
        assert_eq!(
            h.app.editor.document.model().roofs[&roof_id].parameters,
            before_edit
        );
        h.app.editor.undo().unwrap();
        h.settle();
        assert!(!h.app.editor.document.model().roofs.contains_key(&roof_id));
        assert!(!h.app.editor.scene.contains_key(&roof_id));
        h.app.editor.redo().unwrap();
        h.settle();
        assert!(h.app.editor.scene.contains_key(&roof_id));
    }
}

#[test]
fn roof_escape_invalid_loop_and_document_revision_cancel_without_partial_commit() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    h.app.plans.snaps.enabled = false;
    let original = h.app.editor.document.model().clone();

    h.app.begin_roof(h.view);
    h.click(h.point(Point2::new(18.0, 18.0)));
    h.click(h.point(Point2::new(22.0, 18.0)));
    h.frame(vec![escape()]);
    assert!(h.app.plans.roof_draft.is_none());
    assert_eq!(h.app.editor.document.model(), &original);

    h.app.begin_roof(h.view);
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(0.0, 0.0);
    h.frame(vec![]);
    for vertex in [
        Point2::new(-2.0, -2.0),
        Point2::new(2.0, 2.0),
        Point2::new(-2.0, 2.0),
        Point2::new(2.0, -2.0),
    ] {
        h.click(h.point(vertex));
    }
    h.app.finish_roof_loop();
    assert!(
        h.app.plans.roof_draft.is_some(),
        "invalid loop remains repairable"
    );
    assert!(
        h.app
            .plans
            .roof_draft
            .as_ref()
            .unwrap()
            .parameters
            .boundary
            .is_empty()
    );
    assert_eq!(h.app.editor.document.model(), &original);
    h.frame(vec![escape()]);
    assert!(h.app.plans.roof_draft.is_none());

    h.app.begin_roof(h.view);
    h.click(h.point(Point2::new(-1.0, -1.0)));
    h.app
        .editor
        .command(
            "Concurrent edit",
            Command::RenameProject("Concurrent edit".into()),
        )
        .unwrap();
    h.frame(vec![]);
    assert!(
        h.app.plans.roof_draft.is_none(),
        "document revision invalidates the draft"
    );
    assert!(h.app.editor.document.model().roofs.is_empty());
}

#[test]
fn roof_sketch_uses_existing_wall_endpoint_snaps() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Roof snap target".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(10.0, 10.0),
                end: Point2::new(14.0, 10.0),
            },
            thickness: 0.2,
            height: 3.0,
            level: h.app.active_level,
            material: None,
        },
    );
    h.app
        .editor
        .command("Add snap target", Command::AddWall(wall))
        .unwrap();
    h.settle();
    let camera = h.app.plans.cameras.get_mut(&h.view).unwrap();
    camera.center = Point2::new(10.0, 10.0);
    camera.pixels_per_metre = 40.0;
    h.frame(vec![]);
    h.app.begin_roof(h.view);
    h.click(h.point(Point2::new(10.0, 10.0)));
    assert_eq!(
        h.app.plans.roof_draft.as_ref().unwrap().points[0],
        Point2::new(10.0, 10.0)
    );
    h.frame(vec![escape()]);
    assert!(h.app.editor.document.model().roofs.is_empty());
}
