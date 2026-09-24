//! Real egui frames for native floor boundary authoring; no native-window claim.
use super::*;
use os_model::Wall;

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
            .create_floor_plan("Floor authoring", app.active_level)
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

    fn hover(&mut self, point: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(point)]);
    }

    fn click(&mut self, point: egui::Pos2) {
        self.hover(point);
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
fn floor_sketch_is_preview_only_then_commits_to_plan_and_split_scene_once() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let original_model = h.app.editor.document.model().clone();
        let original_scene = h.app.editor.scene.clone();
        h.app.plans.split = true;
        h.app.plans.snaps.enabled = false;
        h.frame(vec![]);
        h.app.begin_floor_sketch(h.view);
        h.frame(vec![]);
        let boundary = [
            Point2::new(18.0, 19.0),
            Point2::new(22.0, 19.0),
            Point2::new(22.0, 22.0),
            Point2::new(18.0, 22.0),
        ];
        for vertex in boundary {
            h.click(h.point(vertex));
            assert_eq!(h.app.editor.document.model(), &original_model);
            assert_eq!(h.app.editor.document.revision(), 0);
            assert_eq!(h.app.editor.scene, original_scene);
        }
        h.click(h.point(boundary[0]));
        assert_eq!(
            h.app.editor.document.model().floors.len(),
            1,
            "draft vertices: {:?}; status: {}",
            h.app
                .plans
                .floor_sketch
                .as_ref()
                .map(|draft| draft.points.len()),
            h.app.status
        );
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        let floor_id = h.app.selected.expect("new floor selected");
        let floor = &h.app.editor.document.model().floors[&floor_id];
        assert_eq!(floor.parameters.level, h.app.active_level);
        assert_eq!(floor.parameters.thickness, 0.2);
        assert_eq!(floor.parameters.top_offset, 0.0);
        assert_eq!(floor.parameters.area(), 12.0);
        let mesh = &h.app.editor.scene[&floor_id];
        assert!((mesh.signed_volume() - 2.4).abs() < 1e-9);
        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert_eq!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .floors(context)
                .unwrap()
                .len(),
            1
        );

        let committed = h.app.editor.document.model().clone();
        h.app.history(false);
        h.settle();
        assert!(h.app.editor.document.model().floors.is_empty());
        assert!(!h.app.editor.scene.contains_key(&floor_id));
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert!(h.app.editor.scene.contains_key(&floor_id));

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("floor.osb");
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert!(h.app.editor.scene.contains_key(&floor_id));
        assert!((h.app.editor.scene[&floor_id].signed_volume() - 2.4).abs() < 1e-9);
    }
}

#[test]
fn escape_stale_context_and_invalid_boundary_never_partially_commit() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let original = h.app.editor.document.model().clone();
    h.app.plans.snaps.enabled = false;
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(0.0, 0.0);
    h.frame(vec![]);
    h.app.begin_floor_sketch(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(-2.0, -1.0)));
    h.click(h.point(Point2::new(2.0, -1.0)));
    h.frame(vec![escape()]);
    assert!(h.app.plans.floor_sketch.is_none());
    assert_eq!(h.app.editor.document.model(), &original);

    h.app.begin_floor_sketch(h.view);
    h.frame(vec![]);
    for point in [
        Point2::new(-2.0, -2.0),
        Point2::new(2.0, 2.0),
        Point2::new(-2.0, 2.0),
        Point2::new(2.0, -2.0),
    ] {
        h.click(h.point(point));
    }
    h.click(h.point(Point2::new(-2.0, -2.0)));
    assert_eq!(h.app.editor.document.model(), &original);
    assert!(
        h.app.plans.floor_sketch.is_some(),
        "invalid finish leaves the draft repairable"
    );
    h.frame(vec![escape()]);
    assert_eq!(h.app.editor.document.model(), &original);

    h.app.begin_floor_sketch(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(-1.0, -1.0)));
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Concurrent edit".into(),
            start: Point2::new(-3.0, 0.0),
            end: Point2::new(3.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level: h.app.active_level,
            material: None,
        },
    );
    h.app
        .editor
        .command("Concurrent edit", Command::AddWall(wall))
        .unwrap();
    h.frame(vec![]);
    assert!(
        h.app.plans.floor_sketch.is_none(),
        "document revision change cancels the draft"
    );
    assert!(h.app.editor.document.model().floors.is_empty());

    h.settle();
    h.app.begin_floor_sketch(h.view);
    h.frame(vec![]);
    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
    h.frame(vec![]);
    assert!(
        h.app.plans.floor_sketch.is_none(),
        "provider activation change cancels the draft"
    );
}

#[test]
fn floor_vertices_use_the_active_plan_endpoint_snaps() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Snap target".into(),
            start: Point2::new(10.0, 10.0),
            end: Point2::new(14.0, 10.0),
            thickness: 0.2,
            height: 3.0,
            level: h.app.active_level,
            material: None,
        },
    );
    h.app
        .editor
        .command("Snap target", Command::AddWall(wall))
        .unwrap();
    h.settle();
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(10.0, 10.0);
    h.app
        .plans
        .cameras
        .get_mut(&h.view)
        .unwrap()
        .pixels_per_metre = 40.0;
    h.frame(vec![]);
    h.app.begin_floor_sketch(h.view);
    h.frame(vec![]);

    let near_endpoint = h.point(Point2::new(10.1, 10.1));
    h.click(near_endpoint);
    assert_eq!(
        h.app.plans.floor_sketch.as_ref().unwrap().points,
        [Point2::new(10.0, 10.0)]
    );
    assert!(h.app.editor.document.model().floors.is_empty());
    h.frame(vec![escape()]);
    assert!(h.app.plans.floor_sketch.is_none());
}
