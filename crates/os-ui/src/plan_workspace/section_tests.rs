//! Real egui-frame section marker authoring and cancellation evidence.
use super::*;
use os_model::{SectionViewSettings, ViewKind, Wall, WallParams};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    time: f64,
    view: Id,
}

impl Harness {
    fn new() -> Self {
        let mut app = DesktopApp::new().unwrap();
        let wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: "Section fixture".into(),
                start: Point2::new(18.0, 20.0),
                end: Point2::new(22.0, 20.0),
                thickness: 0.2,
                height: 4.0,
                level: app.active_level,
                material: None,
            },
        );
        app.editor
            .command("Add section fixture", Command::AddWall(wall))
            .unwrap();
        let view = app
            .editor
            .create_floor_plan("Section placement", app.active_level)
            .unwrap();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut harness = Self {
            app,
            ctx,
            time: 0.0,
            view,
        };
        harness.settle();
        let camera = harness.app.plans.cameras.get_mut(&view).unwrap();
        camera.center = Point2::new(20.0, 20.0);
        camera.pixels_per_metre = 30.0;
        harness.frame(vec![]);
        harness.app.plans.snaps.enabled = false;
        harness
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
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
        assert!(rect.contains(position));
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
fn two_click_section_is_preview_only_then_one_undoable_linked_view() {
    let mut h = Harness::new();
    let original = h.app.editor.document.model().clone();
    let revision = h.app.editor.document.revision();
    h.app.begin_section_placement(h.view);
    h.frame(vec![]);

    h.click(h.point(Point2::new(18.0, 20.0)));
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.revision(), revision);
    assert!(
        h.app
            .plans
            .section_placement
            .as_ref()
            .unwrap()
            .first
            .is_some()
    );

    h.click(h.point(Point2::new(18.0, 20.0)));
    assert!(h.app.status_error, "degenerate marker should be rejected");
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.revision(), revision);
    assert!(h.app.plans.section_placement.is_some());

    let end = h.point(Point2::new(22.0, 20.0));
    h.frame(vec![egui::Event::PointerMoved(end)]);
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.revision(), revision);
    h.click(end);

    let section = h.app.plans.active.unwrap();
    assert_eq!(h.app.editor.document.revision(), revision + 1);
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
    assert_eq!(
        h.app.editor.document.model().views[&section]
            .parameters
            .kind,
        ViewKind::Section
    );
    let params = &h.app.editor.document.model().views[&section].parameters;
    assert_eq!(params.level, Some(h.app.active_level));
    let definition = params.section.unwrap();
    assert_eq!(definition.start, Point2::new(18.0, 20.0));
    assert_eq!(definition.end, Point2::new(22.0, 20.0));
    assert!((definition.bottom_elevation + 0.25).abs() < 1e-9);
    assert!((definition.top_elevation - 4.25).abs() < 1e-9);

    h.settle();
    let context = h.app.editor.native_section_context(section).unwrap();
    assert!(
        !h.app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .provider_lines(context)
            .unwrap()
            .is_empty()
    );

    let committed = h.app.editor.document.model().clone();
    h.app.history(false);
    assert!(!h.app.editor.document.model().views.contains_key(&section));
    h.app.history(true);
    assert_eq!(h.app.editor.document.model(), &committed);
}

#[test]
fn escape_and_stale_document_cancel_section_marker_without_partial_view() {
    let mut h = Harness::new();
    let original = h.app.editor.document.model().clone();
    h.app.begin_section_placement(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(18.0, 20.0)));
    h.frame(vec![escape()]);
    assert!(h.app.plans.section_placement.is_none());
    assert_eq!(h.app.editor.document.model(), &original);

    h.app.begin_section_placement(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(18.0, 20.0)));
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Concurrent model edit".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(1.0, 0.0),
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
    assert!(h.app.plans.section_placement.is_none());
    assert_eq!(
        h.app.editor.document.model().views.len(),
        original.views.len()
    );
}

#[test]
fn section_elevation_bounds_use_building_wall_and_floor_extents() {
    let mut model = os_model::Model::new("section bounds");
    let level_id = *model.levels.keys().next().unwrap();
    model
        .levels
        .get_mut(&level_id)
        .unwrap()
        .parameters
        .elevation = 2.0;
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Tall wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.0, 0.0),
            thickness: 0.2,
            height: 4.0,
            level: level_id,
            material: None,
        },
    );
    model.walls.insert(wall.id(), wall);
    let floor = os_model::Floor::new(
        "core.floor",
        os_model::FloorParams {
            name: "Lower slab".into(),
            level: level_id,
            material: None,
            boundary: vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 2.0),
            ],
            thickness: 0.3,
            top_offset: -0.5,
        },
    );
    model.floors.insert(floor.id(), floor);

    assert_eq!(
        section_elevation_bounds(&model, level_id).unwrap(),
        (0.95, 6.25)
    );
    assert!(
        SectionViewSettings::new(Point2::new(0.0, 0.0), Point2::new(4.0, 0.0), 0.95, 6.25,)
            .validate()
            .is_ok()
    );
}
