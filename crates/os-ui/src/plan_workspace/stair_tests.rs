//! Headless egui acceptance for native straight stair authoring.
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
            .create_floor_plan("Stair plan", app.active_level)
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
        self.properties_events(vec![])
    }

    fn properties_events(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
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
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(self.app.stair_properties(ui));
            });
        })
    }

    fn property_button(&mut self, label: &str) {
        self.properties_frame();
        let output = self.properties_frame();
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Properties button missing: {label}"));
        self.properties_events(vec![egui::Event::PointerMoved(position)]);
        self.properties_events(vec![button(position, true)]);
        self.properties_events(vec![button(position, false)]);
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
fn stair_two_click_preview_commit_repeat_pick_and_pan_at_both_profiles() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.plans.split = true;
        h.frame(vec![]);
        let original = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        let scene_count = h.app.editor.scene.len();
        let camera = h.app.plans.cameras[&h.view];
        h.app.begin_stair(h.view);
        h.frame(vec![]);
        let start = Point2::new(-2.0, 0.0);
        let end = Point2::new(2.0, 0.0);
        h.click(h.point(start));
        assert_eq!(
            h.app.plans.stair_placement.as_ref().unwrap().first,
            Some(start)
        );
        let output = h.hover(h.point(end));
        assert!(!output.shapes.is_empty());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        assert_eq!(h.app.editor.scene.len(), scene_count);
        let draft = h.app.plans.stair_placement.as_ref().unwrap();
        let preview = stairs::preview(&h.app.editor, draft, end).unwrap().unwrap();
        let preview_item = preview
            .stairs(h.app.editor.native_plan_context(h.view).unwrap())
            .unwrap()[0]
            .clone();

        // Neither press nor pointer motion may write; only the second click release commits.
        let end_screen = h.point(end);
        h.frame(vec![button(end_screen, true)]);
        assert_eq!(h.app.editor.document.model(), &original);
        h.frame(vec![button(end_screen, false)]);
        let id = h.app.selected.expect("placed stair selected");
        let stair = h.app.editor.document.model().stairs[&id].clone();
        assert_eq!(stair.parameters.start, start);
        assert_eq!(stair.parameters.end, end);
        assert_eq!(
            stair.parameters.lower_level,
            h.app.editor.document.model().views[&h.view]
                .parameters
                .level
                .unwrap()
        );
        assert_eq!(stair.parameters.width, 1.2);
        assert_eq!(stair.parameters.riser_count, 15);
        assert_eq!(stair.parameters.structural_thickness, 0.2);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        assert!(h.app.editor.scene[&id].signed_volume() > 0.0);
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        assert!(
            h.app
                .plans
                .stair_placement
                .as_ref()
                .unwrap()
                .first
                .is_none()
        );
        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        assert_eq!(drawing.stairs(context).unwrap()[0], preview_item);
        assert_eq!(
            drawing
                .pick(
                    context,
                    context
                        .basis
                        .world_to_plane(Point2::new(-1.7, 0.15))
                        .unwrap()
                )
                .unwrap(),
            Some(id)
        );

        h.click(h.point(Point2::new(-2.0, -2.0)));
        h.click(h.point(Point2::new(2.0, -2.0)));
        assert_eq!(h.app.editor.document.model().stairs.len(), 2);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
        h.frame(vec![escape()]);
        h.settle();
        assert!(h.app.plans.stair_placement.is_none());
        h.click(h.point(Point2::new(-1.7, 0.15)));
        assert_eq!(h.app.selected, Some(id));
        let before_pan = h.app.plans.cameras[&h.view];
        let p = h.point(Point2::new(-1.0, 0.0));
        h.frame(vec![egui::Event::PointerMoved(p), button(p, true)]);
        h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(30.0, 15.0))]);
        h.frame(vec![button(p + egui::vec2(30.0, 15.0), false)]);
        assert_ne!(h.app.plans.cameras[&h.view], before_pan);
        assert_eq!(h.app.editor.document.model().stairs[&id], stair);
        h.app.editor.undo().unwrap();
        assert_eq!(h.app.editor.document.model().stairs.len(), 1);
        h.app.editor.undo().unwrap();
        assert_eq!(h.app.editor.document.model(), &original);
        h.app.editor.redo().unwrap();
        assert_eq!(h.app.editor.document.model().stairs[&id], stair);
    }
}

#[test]
fn stair_properties_apply_cancel_delete_and_history_preserve_identity() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.begin_stair(h.view);
        h.app.stair_click(Point2::new(-2.0, 0.0));
        h.app.stair_click(Point2::new(2.0, 0.0));
        let id = h.app.selected.unwrap();
        h.frame(vec![escape()]);
        h.settle();
        let original = h.app.editor.document.model().stairs[&id].clone();
        let revision = h.app.editor.document.revision();
        h.properties_frame();
        h.app.plans.stair_edit.as_mut().unwrap().parameters.width = 1.6;
        h.properties_frame();
        assert_eq!(h.app.editor.document.model().stairs[&id], original);
        assert_eq!(h.app.editor.document.revision(), revision);
        h.property_button("Apply stair");
        let edited = h.app.editor.document.model().stairs[&id].clone();
        assert_eq!(edited.header, original.header);
        assert_eq!(edited.parameters.width, 1.6);
        assert_eq!(edited.parameters.material, original.parameters.material);
        assert_eq!(
            edited.parameters.lower_level,
            original.parameters.lower_level
        );
        assert_eq!(
            edited.parameters.upper_level,
            original.parameters.upper_level
        );
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
        h.app.editor.undo().unwrap();
        assert_eq!(h.app.editor.document.model().stairs[&id], original);
        h.app.editor.redo().unwrap();
        assert_eq!(h.app.editor.document.model().stairs[&id], edited);

        h.properties_frame();
        h.app.plans.stair_edit.as_mut().unwrap().parameters.width = -1.0;
        h.app.apply_stair_properties();
        assert!(h.app.status_error);
        assert_eq!(h.app.editor.document.model().stairs[&id], edited);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
        h.property_button("Cancel stair edits");
        h.properties_frame();
        assert_eq!(
            h.app.plans.stair_edit.as_ref().unwrap().parameters,
            edited.parameters
        );
        h.property_button("Delete stair");
        assert!(!h.app.editor.document.model().stairs.contains_key(&id));
        assert!(!h.app.editor.scene.contains_key(&id));
        assert_eq!(h.app.selected, None);
        h.app.editor.undo().unwrap();
        assert_eq!(h.app.editor.document.model().stairs[&id], edited);
        assert!(h.app.editor.scene.contains_key(&id));
        h.app.editor.redo().unwrap();
        assert!(!h.app.editor.document.model().stairs.contains_key(&id));
    }
}

#[test]
fn stair_invalid_release_escape_and_stale_contexts_do_not_write() {
    for (size, scale) in PROFILES {
        for change in 0..5 {
            let mut h = Harness::new(size, scale);
            h.app.begin_stair(h.view);
            h.frame(vec![]);
            let start = h.point(Point2::new(-1.0, 0.0));
            h.click(start);
            h.click(start);
            assert!(h.app.status_error, "coincident endpoints rejected");
            assert!(h.app.editor.document.model().stairs.is_empty());
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
            let pointer = h.point(Point2::new(2.0, 0.0));
            h.frame(vec![
                egui::Event::PointerMoved(pointer),
                button(pointer, true),
            ]);
            match change {
                0 => {
                    h.frame(vec![escape()]);
                }
                1 => h.app.focus_plan(None),
                2 => h.app.add_level(),
                3 => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap();
                }
                _ => {
                    h.frame(vec![egui::Event::PointerGone]);
                }
            }
            h.frame(vec![button(pointer, false)]);
            assert!(h.app.plans.stair_placement.is_none(), "change {change}");
            assert!(
                h.app.editor.document.model().stairs.is_empty(),
                "change {change}"
            );
        }
    }
}

#[test]
fn stair_snaps_to_existing_wall_endpoint_and_upper_level_selection_is_valid() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let lower = h.app.active_level;
        let wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: "Snap target".into(),
                level: lower,
                path: os_model::WallPath::Straight {
                    start: Point2::new(-2.0, 0.0),
                    end: Point2::new(2.0, 0.0),
                },
                height: 3.0,
                thickness: 0.2,
                material: None,
            },
        );
        h.app
            .editor
            .command("Snap wall", Command::AddWall(wall))
            .unwrap();
        h.settle();
        h.app.plans.snaps.enabled = true;
        h.app.begin_stair(h.view);
        h.frame(vec![]);
        h.click(h.point(Point2::new(-2.0, 0.0)) + egui::vec2(2.0, 1.0));
        assert_eq!(
            h.app.plans.stair_placement.as_ref().unwrap().first,
            Some(Point2::new(-2.0, 0.0))
        );
        h.click(h.point(Point2::new(2.0, 0.0)) + egui::vec2(-2.0, 1.0));
        let stair = &h.app.editor.document.model().stairs[&h.app.selected.unwrap()];
        assert_eq!(stair.parameters.end, Point2::new(2.0, 0.0));
        let model = h.app.editor.document.model();
        assert!(
            model.levels[&stair.parameters.upper_level]
                .parameters
                .elevation
                > model.levels[&lower].parameters.elevation
        );
        assert_eq!(
            model.levels[&stair.parameters.upper_level]
                .parameters
                .building,
            model.levels[&lower].parameters.building
        );
    }
}

#[test]
fn stair_without_upper_level_explains_prerequisite_and_creates_nothing() {
    let mut app = DesktopApp::new().unwrap();
    let view = app
        .editor
        .create_floor_plan("Plan", app.active_level)
        .unwrap();
    app.focus_plan(Some(view));
    let model = app.editor.document.model().clone();
    let revision = app.editor.document.revision();
    app.begin_stair(view);
    assert!(app.plans.stair_placement.is_none());
    assert!(app.status_error);
    assert!(app.status.contains("higher level"));
    assert_eq!(app.editor.document.model(), &model);
    assert_eq!(app.editor.document.revision(), revision);
}
