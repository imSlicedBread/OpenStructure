//! Headless egui acceptance for native rectangular column authoring.
use super::*;
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
            .create_floor_plan("Column plan", app.active_level)
            .unwrap();
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
        let position = rect.min + egui::vec2(local.x as f32, local.y as f32);
        assert!(
            rect.contains(position),
            "fixture point must be on the plan canvas"
        );
        position
    }

    fn hover(&mut self, point: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(point)])
    }

    fn click(&mut self, point: egui::Pos2) {
        self.hover(point);
        self.frame(vec![button(point, true)]);
        self.frame(vec![button(point, false)]);
    }

    fn properties_frame(&mut self) -> egui::FullOutput {
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
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(self.app.column_properties(ui));
            });
        })
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
fn snapped_column_preview_commit_properties_undo_redo_and_save_reopen() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.plans.split = true;
        h.frame(vec![]);
        let original = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        let center = Point2::new(1.0, 2.0);
        h.app.begin_column(h.view);
        h.frame(vec![]);
        let output = h.hover(h.point(center));
        assert!(h.app.editor.document.model().columns.is_empty());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert!(
            !output.shapes.is_empty(),
            "transient preview paints on the plan"
        );

        h.click(h.point(center));
        let id = h.app.selected.expect("placed column stays selected");
        let column = h.app.editor.document.model().columns[&id].clone();
        assert_eq!(column.header.type_id, "core.column");
        assert_eq!(column.parameters.center, center);
        assert_eq!(column.parameters.level, h.app.active_level);
        assert_eq!(column.parameters.width, 0.4);
        assert_eq!(column.parameters.depth, 0.4);
        assert_eq!(column.parameters.height, 3.0);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        h.settle();
        assert!((h.app.editor.scene[&id].signed_volume() - 0.4 * 0.4 * 3.0).abs() < 1e-10);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert_eq!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .columns(context)
                .unwrap()
                .len(),
            1
        );

        h.app.select(Some(id));
        let output = h.properties_frame();
        assert!(!output.shapes.is_empty());
        let draft = h.app.plans.column_edit.as_mut().unwrap();
        draft.parameters.width = 0.5;
        draft.parameters.base_offset = 0.2;
        h.app.apply_column_properties();
        assert_eq!(
            h.app.editor.document.model().columns[&id].header,
            column.header
        );
        assert_eq!(
            h.app.editor.document.model().columns[&id].parameters.width,
            0.5
        );
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
        h.settle();
        assert!((h.app.editor.scene[&id].signed_volume() - 0.5 * 0.4 * 3.0).abs() < 1e-10);

        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model().columns[&id], column);
        h.app.history(true);
        h.settle();
        assert_eq!(
            h.app.editor.document.model().columns[&id].parameters.width,
            0.5
        );

        h.app.delete_column();
        assert!(!h.app.editor.document.model().columns.contains_key(&id));
        h.app.history(false);
        h.settle();
        assert!(h.app.editor.document.model().columns.contains_key(&id));

        let committed = h.app.editor.document.model().clone();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("column.osb");
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert!(h.app.editor.scene.contains_key(&id));
        assert!((h.app.editor.scene[&id].signed_volume() - 0.5 * 0.4 * 3.0).abs() < 1e-10);
    }
}

#[test]
fn column_center_uses_existing_wall_endpoint_snap() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Column snap target".into(),
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
        .command("Add snap target", Command::AddWall(wall))
        .unwrap();
    h.settle();
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(10.0, 10.0);
    h.app
        .plans
        .cameras
        .get_mut(&h.view)
        .unwrap()
        .pixels_per_metre = 40.0;
    h.app.plans.snaps.enabled = true;
    h.frame(vec![]);
    h.app.begin_column(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(10.05, 10.05)));
    let placed = h
        .app
        .editor
        .document
        .model()
        .columns
        .values()
        .next()
        .unwrap();
    assert_eq!(placed.parameters.center, Point2::new(10.0, 10.0));
}

#[test]
fn escape_and_stale_document_cancel_column_placement_without_model_changes() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let original = h.app.editor.document.model().clone();
    h.app.begin_column(h.view);
    h.frame(vec![]);
    h.frame(vec![escape()]);
    assert!(h.app.plans.column_placement.is_none());
    assert_eq!(h.app.editor.document.model(), &original);

    h.app.begin_column(h.view);
    h.frame(vec![]);
    let level = h.app.active_level;
    let mut parameters = h.app.editor.document.model().levels[&level]
        .parameters
        .clone();
    parameters.elevation = 1.0;
    h.app
        .editor
        .command(
            "Change level",
            Command::UpdateLevel {
                id: level,
                parameters,
            },
        )
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.column_placement.is_none());
    assert!(h.app.editor.document.model().columns.is_empty());
}

#[test]
fn escape_while_pointer_is_claimed_does_not_pan_or_place_column() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let before = h.app.plans.cameras[&h.view].center;
    let point = h.point(Point2::new(1.0, 1.0));
    h.app.begin_column(h.view);
    h.frame(vec![]);
    h.hover(point);
    h.frame(vec![button(point, true)]);
    let moved = point + egui::vec2(45.0, 0.0);
    h.frame(vec![egui::Event::PointerMoved(moved), escape()]);
    h.frame(vec![button(moved, false)]);
    assert!(h.app.plans.column_placement.is_none());
    assert!(h.app.editor.document.model().columns.is_empty());
    assert_eq!(h.app.plans.cameras[&h.view].center, before);
}
