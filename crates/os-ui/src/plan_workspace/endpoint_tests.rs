//! Headless real-egui desktop frames; no native-window inspection implied.
use super::*;
#[path = "crop_tests.rs"]
mod crop_tests;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

#[test]
fn angular_authoring_preview_cancel_and_single_transaction_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let mut params = h.app.editor.document.model().walls[&h.wall]
            .parameters
            .clone();
        params.start = Point2::new(0.0, -1.0);
        params.end = Point2::new(0.0, 1.0);
        let wall = os_model::Wall::new(
            &h.app.editor.document.model().walls[&h.wall].header.type_id,
            params,
        );
        let second = wall.id();
        h.app
            .editor
            .command("Fixture axis", Command::AddWall(wall))
            .unwrap();
        h.settle();
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        h.app
            .begin_dimension(h.view, os_model::DimensionLayout::Angular);
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.7, 0.0)));
        h.frame(vec![]);
        h.click(h.point(Point2::new(-0.7, 0.0)));
        assert!(
            h.app
                .plans
                .dimension_draft
                .as_ref()
                .unwrap()
                .second
                .is_none()
        );
        assert!(h.app.status_error);
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.0, 0.7)));
        assert!(h.app.plans.dimension_draft.as_ref().unwrap().placing);
        h.frame(vec![egui::Event::PointerMoved(
            h.point(Point2::new(0.5, 0.5)),
        )]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        h.frame(vec![key_pressed(egui::Key::Backspace)]);
        assert!(!h.app.plans.dimension_draft.as_ref().unwrap().placing);
        h.frame(vec![escape()]);
        h.frame(vec![]);
        h.clean();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app
            .begin_dimension(h.view, os_model::DimensionLayout::Angular);
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.7, 0.0)));
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.0, 0.7)));
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.5, 0.5)));
        h.settle();
        h.clean();
        assert!(!h.app.status_error, "{}", h.app.status);
        let id = h.app.selected.unwrap();
        let committed = h.app.editor.document.model().clone();
        let d = &committed.dimensions[&id];
        assert_eq!(d.parameters.layout, os_model::DimensionLayout::Angular);
        assert_eq!(d.parameters.first.wall, h.wall);
        assert_eq!(d.parameters.second.wall, second);
        assert!((d.parameters.resolve_angular(&committed).unwrap().degrees() - 90.0).abs() < 1e-8);
        assert_eq!(committed.dimensions.len(), before.dimensions.len() + 1);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        let mut without_dimension = committed.clone();
        without_dimension.dimensions.remove(&id);
        assert_eq!(without_dimension, before);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &committed);
    }
}

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    output: egui::FullOutput,
    size: egui::Vec2,
    scale: f32,
    time: f64,
    wall: Id,
    view: Id,
}
impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let material = os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: "Fixture concrete".into(),
                density_kg_m3: 2400.0,
            },
        );
        app.draft.material = Some(material.id());
        let mut model = app.editor.document.model().clone();
        model.materials.insert(material.id(), material);
        app.editor.document = Document::from_model(model).unwrap();
        app.draft.start = Point2::new(-1.0, 0.0);
        app.draft.end = Point2::new(1.0, 0.0);
        app.draft.name = "Endpoint fixture".into();
        app.draft.height = 3.7;
        app.draft.thickness = 0.27;
        app.apply_wall();
        let wall = app.selected.unwrap();
        let view = app
            .editor
            .create_floor_plan("Endpoint plan", app.active_level)
            .unwrap();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Self {
            app,
            ctx,
            output: Default::default(),
            size,
            scale,
            time: 0.0,
            wall,
            view,
        };
        h.settle();
        h
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
        self.output = self.ctx.run(input, |ctx| self.app.show(ctx));
    }
    fn settle(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() && !self.app.editor.plugin_work_pending() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "plan/worker did not settle"
            );
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
    fn point(&self, world: Point2) -> egui::Pos2 {
        let rect = self.app.plans.canvas_rect.unwrap();
        let context = self.app.editor.native_plan_context(self.view).unwrap();
        let p = self.app.plans.cameras[&self.view]
            .project(
                context.basis.world_to_plane(world).unwrap(),
                [rect.width() as f64, rect.height() as f64],
            )
            .unwrap();
        rect.min + egui::vec2(p.x as f32, p.y as f32)
    }
    fn press(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
        self.frame(vec![button(p, true)]);
    }
    fn release(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p), button(p, false)]);
    }
    fn click(&mut self, p: egui::Pos2) {
        self.press(p);
        self.release(p);
    }
    fn click_text_in_band(&mut self, text: &str, y_min: f32, y_max: f32) {
        let position = self
            .output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(galley)
                    if galley.galley.job.text == text && (y_min..y_max).contains(&galley.pos.y) =>
                {
                    Some(egui::Rect::from_min_size(galley.pos, galley.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("text not rendered in band: {text}"));
        self.click(position);
    }
    fn circles(&self) -> Vec<egui::Pos2> {
        self.output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c) if c.radius == 6.0 && c.fill == theme::ACCENT => {
                    Some(c.center)
                }
                _ => None,
            })
            .collect()
    }
    fn clean(&self) {
        assert!(self.app.plans.endpoint_drag.is_none());
        assert!(self.app.wall_gesture.is_none());
        assert!(self.app.plans.dimension_draft.is_none());
        assert!(!self.app.plans.endpoint_pointer_claimed);
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
fn key_pressed(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn endpoint_handles_render_and_hit_in_logical_points() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        assert_eq!(h.ctx.pixels_per_point(), scale);
        let start = h.point(Point2::new(-1.0, 0.0));
        let end = h.point(Point2::new(1.0, 0.0));
        assert_eq!(h.circles(), vec![start, end]);
        for (delta, hit) in [
            (egui::vec2(10.0, 0.0), true),
            (egui::vec2(6.0, 8.0), true),
            (egui::vec2(10.1, 0.0), false),
            (egui::vec2(8.0, 8.0), false),
        ] {
            let p = h.point(Point2::new(-1.0, 0.0)) + delta;
            h.press(p);
            assert_eq!(
                h.app.plans.endpoint_drag.is_some(),
                hit,
                "{delta:?} at {scale}"
            );
            if hit {
                assert_eq!(
                    h.app.wall_gesture.as_ref().unwrap().edit_mode(),
                    Some(WallEdit::ResizeStart)
                );
            }
            h.release(p);
            h.clean();
            assert!(!h.app.editor.document.can_undo());
            h.app.select(Some(h.wall));
            h.frame(vec![]);
        }
        // Overlapping hit circles: Euclidean nearest, then start on exact tie.
        let handles = [
            (WallEdit::ResizeStart, start),
            (WallEdit::ResizeEnd, start + egui::vec2(12.0, 0.0)),
        ];
        assert_eq!(
            hit_endpoint(&handles, start + egui::vec2(6.0, 0.0)),
            Some(WallEdit::ResizeStart)
        );
        assert_eq!(
            hit_endpoint(&handles, start + egui::vec2(7.0, 0.0)),
            Some(WallEdit::ResizeEnd)
        );
        h.app
            .plans
            .cameras
            .get_mut(&h.view)
            .unwrap()
            .pixels_per_metre = 6.0;
        h.frame(vec![]);
        for (dx, expected) in [(0.0, WallEdit::ResizeStart), (1.0, WallEdit::ResizeEnd)] {
            let p = h.point(Point2::new(0.0, 0.0)) + egui::vec2(dx, 0.0);
            h.press(p);
            assert_eq!(
                h.app.wall_gesture.as_ref().unwrap().edit_mode(),
                Some(expected)
            );
            h.release(p);
            h.clean();
            assert!(!h.app.editor.document.can_undo());
            h.frame(vec![]);
        }
    }
}

#[test]
fn endpoint_drag_preview_commit_identity_properties_and_history() {
    for (size, scale) in PROFILES {
        for mode in [WallEdit::ResizeStart, WallEdit::ResizeEnd] {
            let mut h = Harness::new(size, scale);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let camera = h.app.plans.cameras[&h.view];
            let original = &before.walls[&h.wall];
            let moving = if mode == WallEdit::ResizeStart {
                original.parameters.start
            } else {
                original.parameters.end
            };
            h.press(h.point(moving));
            assert_eq!(
                h.app.wall_gesture.as_ref().unwrap().snap_exclusion(),
                Some(h.wall)
            );
            let destination = Point2::new(0.0, 1.0);
            let p = h.point(destination);
            h.frame(vec![egui::Event::PointerMoved(p)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.revision(), 0);
            assert!(!h.app.editor.document.can_undo());
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.plans.cameras[&h.view], camera);
            let expected = h
                .app
                .wall_gesture
                .as_ref()
                .unwrap()
                .parameters(destination)
                .unwrap();
            assert_eq!(expected.height, original.parameters.height);
            assert_eq!(expected.thickness, original.parameters.thickness);
            assert_eq!(expected.name, original.parameters.name);
            assert_eq!(expected.material, original.parameters.material);
            assert_eq!(expected.level, original.parameters.level);
            if mode == WallEdit::ResizeStart {
                assert_eq!(expected.end, original.parameters.end);
            } else {
                assert_eq!(expected.start, original.parameters.start);
            }
            let a = h.point(expected.start);
            let b = h.point(expected.end);
            assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::LineSegment { points, stroke } if *points == [a,b] && stroke.width == 2.0 && stroke.color == theme::ACCENT)));
            h.release(p);
            h.clean();
            assert!(!h.app.status_error, "{}", h.app.status);
            let edited = h.app.editor.document.model().clone();
            assert_eq!(edited.walls.len(), 1);
            assert_eq!(edited.walls[&h.wall].header, original.header);
            assert_eq!(edited.walls[&h.wall].parameters, expected);
            assert_eq!(h.app.editor.document.revision(), 1);
            assert_eq!(h.app.selected, Some(h.wall));
            assert_ne!(h.app.editor.scene, scene);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &edited);
        }
    }
}

#[test]
fn endpoint_cancel_invalid_outside_and_stale_leave_no_residue() {
    for (size, scale) in PROFILES {
        for reason in [
            "click",
            "long press",
            "outside",
            "escape",
            "collapse",
            "invalid exact",
            "revision",
            "settings",
            "view",
            "selection",
            "provider",
            "session",
        ] {
            let mut h = Harness::new(size, scale);
            let before = h.app.editor.document.model().clone();
            let camera = h.app.plans.cameras[&h.view];
            let start = h.point(Point2::new(-1.0, 0.0));
            h.press(start);
            let mut p = h.point(Point2::new(0.0, 1.0));
            if reason != "click" && reason != "long press" {
                h.frame(vec![egui::Event::PointerMoved(p)]);
            }
            match reason {
                "click" => p = start,
                "long press" => {
                    p = start;
                    h.time += 2.0;
                    h.frame(vec![]);
                }
                "outside" => {
                    p = h.app.plans.canvas_rect.unwrap().right_bottom() + egui::vec2(30.0, 30.0)
                }
                "escape" => h.frame(vec![escape()]),
                "collapse" => p = h.point(Point2::new(1.0, 0.0)),
                "invalid exact" => h.app.wall_gesture.as_mut().unwrap().length = "invalid".into(),
                "revision" => {
                    h.app
                        .editor
                        .command("revision", Command::RenameProject("New name".into()))
                        .unwrap();
                }
                "settings" => {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    settings.visibility.walls = false;
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Changed", h.app.active_level, settings)
                        .unwrap();
                }
                "view" => h.app.focus_plan(None),
                "selection" => h.app.select(None),
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "session" => h.app.editor.document = Document::from_model(before.clone()).unwrap(),
                _ => unreachable!(),
            }
            let expected = h.app.editor.document.model().clone();
            if reason == "escape" {
                h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(40.0, 20.0))]);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
            }
            h.release(p);
            h.clean();
            assert_eq!(h.app.editor.document.model(), &expected, "{reason}");
            assert_eq!(
                h.app.editor.document.can_undo(),
                reason == "revision" || reason == "settings",
                "{reason}"
            );
            if let Some(current) = h.app.plans.cameras.get(&h.view) {
                assert_eq!(*current, camera, "{reason}");
            }
            // No delayed commit on a subsequent ordinary pointer release.
            h.frame(vec![egui::Event::PointerMoved(p)]);
            h.release(p);
            h.clean();
            assert_eq!(h.app.editor.document.model(), &expected);
        }
    }
}

#[test]
fn endpoint_handles_require_selected_checked_visible_native_wall() {
    for (size, scale) in PROFILES {
        for case in [
            "unselected",
            "other id",
            "off canvas",
            "hidden",
            "cropped",
            "partial crop",
            "absent drawing",
            "stale drawing",
        ] {
            let mut h = Harness::new(size, scale);
            match case {
                "unselected" => h.app.select(None),
                "other id" => h.app.selected = Some(Id::new()),
                "off canvas" => {
                    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(100.0, 100.0)
                }
                "hidden" | "cropped" | "partial crop" => {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    if case == "hidden" {
                        settings.visibility.walls = false;
                    } else {
                        settings.crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(if case == "cropped" { 10.0 } else { 0.0 }, -1.0),
                            max: Point2::new(12.0, 1.0),
                        });
                    }
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Visibility", h.app.active_level, settings)
                        .unwrap();
                    h.settle();
                }
                "absent drawing" => {
                    let context = h.app.plans.desired.unwrap();
                    h.app.plans.drawing =
                        Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap());
                }
                "stale drawing" => {
                    let mut context = h.app.plans.desired.unwrap();
                    context.model_revision += 1;
                    h.app.plans.drawing =
                        Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap());
                }
                _ => unreachable!(),
            }
            h.frame(vec![]);
            assert_eq!(
                h.circles().len(),
                usize::from(case == "partial crop"),
                "{case}"
            );
            if case == "partial crop" {
                assert_eq!(h.circles(), vec![h.point(Point2::new(1.0, 0.0))]);
            }
            let p = h.point(Point2::new(-1.0, 0.0));
            let model = h.app.editor.document.model().clone();
            h.press(p);
            assert!(h.app.plans.endpoint_drag.is_none(), "{case}");
            h.release(p);
            h.clean();
            assert_eq!(h.app.editor.document.model(), &model);
        }
    }
}

#[test]
fn endpoint_drag_reuses_snapping_and_exact_input_precedence() {
    for (size, scale) in PROFILES {
        for exact in [false, true] {
            let mut h = Harness::new(size, scale);
            let mut target = h.app.editor.document.model().walls[&h.wall]
                .parameters
                .clone();
            target.start = Point2::new(0.0, 1.0);
            target.end = Point2::new(1.0, 1.0);
            h.app
                .editor
                .wall_command("Snap target", Request::CreateWall(target))
                .unwrap();
            h.settle();
            let before = h.app.editor.document.model().clone();
            h.press(h.point(Point2::new(1.0, 0.0)));
            if exact {
                let gesture = h.app.wall_gesture.as_mut().unwrap();
                gesture.length = "3".into();
                gesture.angle_degrees = "90".into();
            }
            let near = h.point(Point2::new(0.0, 1.0)) + egui::vec2(3.0, 2.0);
            h.frame(vec![egui::Event::PointerMoved(near)]);
            assert_eq!(h.app.editor.document.model(), &before);
            h.release(near);
            h.clean();
            let wall = &h.app.editor.document.model().walls[&h.wall].parameters;
            assert_eq!(wall.start, before.walls[&h.wall].parameters.start);
            let expected = if exact {
                Point2::new(-1.0, 3.0)
            } else {
                Point2::new(0.0, 1.0)
            };
            assert!(wall.end.distance(expected) < 1e-10, "{:?}", wall.end);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
        }
    }
}

#[test]
fn aligned_dimensions_author_live_endpoint_references_with_one_history_step() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.select(Some(h.wall));
        let before = h.app.editor.document.model().clone();
        let before_revision = h.app.editor.document.revision();
        let view = h.view;
        h.frame(vec![]);
        h.click_text_in_band("Architecture", 28.0, 60.0);
        h.click_text_in_band("Aligned dimension", 60.0, 150.0);
        assert!(h.app.plans.dimension_draft.is_some());
        h.frame(vec![]);
        let canvas = h.app.plans.canvas_rect.unwrap();
        let pan_from = canvas.center() + egui::vec2(100.0, 90.0);
        let pan_to = pan_from + egui::vec2(35.0, 20.0);
        let camera = h.app.plans.cameras[&view];
        h.press(pan_from);
        h.frame(vec![egui::Event::PointerMoved(pan_to)]);
        h.release(pan_to);
        assert_eq!(h.app.plans.cameras[&view], camera);
        assert!(
            h.app
                .plans
                .dimension_draft
                .as_ref()
                .unwrap()
                .first
                .is_none()
        );
        let start = h.point(Point2::new(-1.0, 0.0));
        let end = h.point(Point2::new(1.0, 0.0));
        let placement = h.point(Point2::new(0.0, 0.5));

        h.click(start);
        assert!(h.app.plans.endpoint_drag.is_none());
        assert!(h.app.wall_gesture.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), before_revision);
        assert!(!h.app.editor.document.can_undo());

        h.click(end);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), before_revision);
        h.frame(vec![egui::Event::PointerMoved(placement)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.plans.cameras[&view], camera);
        assert!(h.output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "2.000 m"
        )));

        h.click(placement);
        h.clean();
        assert!(!h.app.status_error, "{}", h.app.status);
        let id = h.app.selected.expect("new dimension remains selected");
        let committed = h.app.editor.document.model();
        assert_eq!(committed.dimensions.len(), 1);
        let dimension = committed.dimensions[&id].clone();
        assert_eq!(dimension.parameters.view, view);
        assert_eq!(dimension.parameters.first.wall, h.wall);
        assert_eq!(
            dimension.parameters.first.endpoint,
            DimensionEndpoint::Start
        );
        assert_eq!(dimension.parameters.second.wall, h.wall);
        assert_eq!(dimension.parameters.second.endpoint, DimensionEndpoint::End);
        assert!((dimension.parameters.offset_m - 0.5).abs() < 1e-9);
        assert_eq!(committed.walls, before.walls);
        assert_eq!(h.app.editor.document.revision(), before_revision + 1);
        assert!(h.app.editor.document.can_undo());

        h.settle();
        assert!(h.output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "2.000 m"
        )));
        h.app.select(Some(h.wall));
        // Wall selection switches the Modify ribbon and changes canvas bounds.
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.0, 0.5)));
        assert_eq!(
            h.app.selected,
            Some(id),
            "dimension wins annotation picking"
        );

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dimension.osb");
        h.app.editor.save(&path).unwrap();
        let mut reopened = Editor::new().unwrap();
        reopened.open(&path).unwrap();
        assert_eq!(reopened.document.model().dimensions[&id], dimension);

        h.app.history(false);
        assert!(!h.app.editor.document.model().dimensions.contains_key(&id));
        h.app.history(true);
        assert_eq!(h.app.editor.document.model().dimensions[&id], dimension);
    }
}

#[test]
fn aligned_dimension_offset_is_editable_and_measurement_follows_wall_edits() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    h.click(h.point(Point2::new(1.0, 0.0)));
    h.click(h.point(Point2::new(0.0, 0.5)));
    let id = h.app.selected.unwrap();

    h.app.dimension_offset_draft = 0.75;
    h.app.apply_dimension_properties();
    assert!(!h.app.status_error, "{}", h.app.status);
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .offset_m
            - 0.75)
            .abs()
            < 1e-9
    );
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .orphan_hint
            .y
            - 0.75)
            .abs()
            < 1e-9
    );
    h.app.history(false);
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .offset_m
            - 0.5)
            .abs()
            < 1e-9
    );
    h.app.history(true);
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .offset_m
            - 0.75)
            .abs()
            < 1e-9
    );

    let mut wall = h.app.editor.document.model().walls[&h.wall]
        .parameters
        .clone();
    wall.end = Point2::new(2.0, 0.0);
    h.app
        .editor
        .command(
            "Extend dimensioned wall",
            Command::UpdateWall {
                id: h.wall,
                parameters: wall,
            },
        )
        .unwrap();
    h.settle();
    let current = &h.app.editor.document.model().dimensions[&id];
    assert_eq!(current.parameters.first.endpoint, DimensionEndpoint::Start);
    assert_eq!(current.parameters.second.endpoint, DimensionEndpoint::End);
    assert_eq!(
        current
            .parameters
            .resolve(h.app.editor.document.model())
            .unwrap()
            .length_metres,
        3.0
    );
    assert!(h.output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "3.000 m"
    )));
    assert_eq!(h.app.selected, Some(id));
    h.app.history(false);
    h.settle();
    assert_eq!(
        h.app.editor.document.model().dimensions[&id]
            .parameters
            .resolve(h.app.editor.document.model())
            .unwrap()
            .length_metres,
        2.0
    );
}

#[test]
fn chain_and_baseline_dimensions_collect_ordered_anchors_and_commit_once() {
    for layout in [
        os_model::DimensionLayout::Chain,
        os_model::DimensionLayout::Baseline,
    ] {
        for (size, scale) in PROFILES {
            let mut h = Harness::new(size, scale);
            let source = &h.app.editor.document.model().walls[&h.wall];
            let wall_type = source.header.type_id.clone();
            let mut params = source.parameters.clone();
            params.name = "Dimension continuation".into();
            params.start = Point2::new(2.0, 0.0);
            params.end = Point2::new(3.0, 0.0);
            let continuation = os_model::Wall::new(&wall_type, params);
            let continuation_id = continuation.id();
            let mut off_axis_params = source.parameters.clone();
            off_axis_params.name = "Off-axis dimension test wall".into();
            off_axis_params.start = Point2::new(2.0, 1.0);
            off_axis_params.end = Point2::new(3.0, 1.0);
            let off_axis = os_model::Wall::new(&wall_type, off_axis_params);
            h.app
                .editor
                .document
                .execute(
                    "Add dimension test walls",
                    vec![Command::AddWall(continuation), Command::AddWall(off_axis)],
                )
                .unwrap();
            h.settle();

            let before = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.app.begin_dimension(h.view, layout);
            h.frame(vec![]);
            h.click(h.point(Point2::new(-1.0, 0.0)));
            h.frame(vec![]);
            h.click(h.point(Point2::new(1.0, 0.0)));
            h.frame(vec![]);
            h.click(h.point(Point2::new(2.0, 1.0)));
            assert_eq!(
                h.app
                    .plans
                    .dimension_draft
                    .as_ref()
                    .unwrap()
                    .additional
                    .len(),
                0,
                "invalid off-axis anchor must leave the draft unchanged"
            );
            assert!(h.app.status_error);
            h.frame(vec![]);
            h.click(h.point(Point2::new(2.0, 0.0)));
            assert_eq!(
                h.app.editor.document.model(),
                &before,
                "anchor selection is preview-only"
            );
            assert_eq!(h.app.editor.document.revision(), revision);
            h.frame(vec![]);
            h.frame(vec![key_pressed(egui::Key::Backspace)]);
            assert!(
                h.app
                    .plans
                    .dimension_draft
                    .as_ref()
                    .unwrap()
                    .additional
                    .is_empty()
            );
            assert_eq!(h.app.editor.document.model(), &before);
            h.frame(vec![]);
            h.click(h.point(Point2::new(2.0, 0.0)));
            h.frame(vec![]);
            h.click_text_in_band("Finish anchors", 0.0, h.size.y);
            h.frame(vec![]);
            assert!(h.app.plans.dimension_draft.as_ref().unwrap().placing);
            assert_eq!(
                h.app.editor.document.model(),
                &before,
                "finishing anchors is not a commit"
            );

            h.click(h.point(Point2::new(0.0, 0.5)));
            h.settle();
            h.clean();
            assert!(!h.app.status_error, "{}", h.app.status);
            let id = h.app.selected.expect("dimension remains selected");
            let committed = h.app.editor.document.model();
            assert_eq!(committed.dimensions.len(), 1);
            let dimension = committed.dimensions[&id].clone();
            assert_eq!(dimension.parameters.layout, layout);
            assert_eq!(dimension.parameters.first.wall, h.wall);
            assert_eq!(
                dimension.parameters.first.endpoint,
                DimensionEndpoint::Start
            );
            assert_eq!(dimension.parameters.second.wall, h.wall);
            assert_eq!(dimension.parameters.second.endpoint, DimensionEndpoint::End);
            assert_eq!(dimension.parameters.additional.len(), 1);
            assert_eq!(dimension.parameters.additional[0].wall, continuation_id);
            assert_eq!(
                dimension.parameters.additional[0].endpoint,
                DimensionEndpoint::Start
            );
            assert_eq!(dimension.parameters.baseline_spacing_m, 0.25);
            assert_eq!(h.app.editor.document.revision(), revision + 1);

            let context = h.app.editor.native_plan_context(h.view).unwrap();
            let drawing = h.app.plans.drawing.as_ref().unwrap();
            let spans = drawing.dimensions(context).unwrap();
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].spans.len(), 1);
            assert_eq!(spans[0].entity, id);
            assert_eq!(spans[0].spans[0].entity, id);
            match layout {
                os_model::DimensionLayout::Chain => {
                    assert_eq!(spans[0].value_m, Some(2.0));
                    assert_eq!(spans[0].spans[0].value_m, Some(1.0));
                    assert!(spans[0].spans[0].shared_start_witness);
                }
                os_model::DimensionLayout::Baseline => {
                    assert_eq!(spans[0].value_m, Some(2.0));
                    assert_eq!(spans[0].spans[0].value_m, Some(3.0));
                    assert!(!spans[0].spans[0].shared_start_witness);
                    assert!(
                        (spans[0].spans[0].line_start.y - spans[0].line_start.y - 0.25).abs()
                            < 1e-9
                    );
                }
                os_model::DimensionLayout::Aligned => unreachable!(),
                os_model::DimensionLayout::Angular => unreachable!(),
            }

            h.app.history(false);
            assert!(!h.app.editor.document.model().dimensions.contains_key(&id));
            assert_eq!(h.app.editor.document.revision(), revision + 2);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model().dimensions[&id], dimension);
        }
    }
}

#[test]
fn aligned_dimension_cancel_and_context_change_leave_no_draft_or_partial_entity() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let before = h.app.editor.document.model().clone();
    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    assert_eq!(h.app.editor.document.model(), &before);
    h.frame(vec![escape()]);
    h.frame(vec![]);
    assert!(h.app.plans.dimension_draft.is_none());
    assert_eq!(h.app.editor.document.model(), &before);

    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    h.app
        .editor
        .command("Change document", Command::RenameProject("Changed".into()))
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.dimension_draft.is_none());
    assert!(h.app.editor.document.model().dimensions.is_empty());
    assert_eq!(
        h.app.editor.document.model().project.parameters.name,
        "Changed"
    );
    h.release(h.point(Point2::new(0.0, 0.5)));
    assert!(h.app.editor.document.model().dimensions.is_empty());
}

#[test]
fn dimension_draft_is_bound_to_its_native_plan_drawing_identity() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let before = h.app.editor.document.model().clone();
    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let drawing_identity = h.app.plans.drawing.as_ref().unwrap().identity();
    h.app.begin_aligned_dimension(h.view);
    assert_eq!(
        h.app
            .plans
            .dimension_draft
            .as_ref()
            .unwrap()
            .drawing_identity,
        Some(drawing_identity)
    );

    h.app.plans.drawing =
        Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new()).unwrap());
    h.frame(vec![]);
    assert!(h.app.plans.dimension_draft.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
}

#[test]
fn aligned_dimension_shows_recoverable_orphan_without_blocking_wall_deletion() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    h.click(h.point(Point2::new(1.0, 0.0)));
    h.click(h.point(Point2::new(0.0, 0.5)));
    let id = h.app.selected.unwrap();
    assert_eq!(h.app.editor.document.model().dimensions.len(), 1);

    h.app
        .editor
        .command("Delete referenced wall", Command::RemoveWall(h.wall))
        .unwrap();
    h.settle();
    assert!(h.app.editor.document.model().dimensions.contains_key(&id));
    assert_eq!(
        h.app.editor.document.model().dimensions[&id]
            .parameters
            .resolve(h.app.editor.document.model()),
        Err(os_model::DimensionDiagnostic::MissingWall)
    );
    assert!(h.output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "Dimension lost reference"
    )));
    assert!(!h.app.status_error);

    h.app.history(false);
    h.settle();
    assert!(h.app.editor.document.model().walls.contains_key(&h.wall));
    assert_eq!(
        h.app.editor.document.model().dimensions[&id]
            .parameters
            .resolve(h.app.editor.document.model())
            .unwrap()
            .length_metres,
        2.0
    );
}

#[cfg(feature = "external-plugins")]
#[test]
#[ignore = "requires explicitly installed independent Wall guest via OPENSTRUCTURE_WALL_TEST_PLUGIN"]
fn endpoint_installed_drag_submits_and_worker_owns_completion() {
    let directory =
        std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN").expect("installed Wall directory");
    for (size, scale) in PROFILES {
        for mode in [WallEdit::ResizeStart, WallEdit::ResizeEnd] {
            for cancel in [false, true] {
                let mut h = Harness::new(size, scale);
                h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                h.app
                    .editor
                    .host
                    .load_wasm_directory(
                        Path::new(&directory),
                        [
                            Permission::ModelRead,
                            Permission::ModelWrite,
                            Permission::UiTool,
                        ]
                        .into(),
                    )
                    .unwrap();
                h.settle();
                // Move the floating plugin window off the canvas with real input.
                let title = h
                    .output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.job.text == "Plugin tools" => {
                            Some(t.pos + egui::vec2(25.0, 5.0))
                        }
                        _ => None,
                    })
                    .unwrap();
                h.press(title);
                let to = egui::pos2(40.0, 90.0);
                h.frame(vec![egui::Event::PointerMoved(to)]);
                h.release(to);
                h.frame(vec![]);
                let before = h.app.editor.document.model().clone();
                let moving = if mode == WallEdit::ResizeStart {
                    before.walls[&h.wall].parameters.start
                } else {
                    before.walls[&h.wall].parameters.end
                };
                h.press(h.point(moving));
                assert!(
                    h.app
                        .wall_gesture
                        .as_ref()
                        .expect("handle claimed")
                        .is_installed()
                );
                let destination = Point2::new(0.0, 1.0);
                let expected = h
                    .app
                    .wall_gesture
                    .as_ref()
                    .unwrap()
                    .parameters(destination)
                    .unwrap();
                let p = h.point(destination);
                h.frame(vec![egui::Event::PointerMoved(p)]);
                assert_eq!(h.app.editor.document.model(), &before);
                h.release(p);
                h.clean();
                assert!(h.app.editor.plugin_work_pending(), "{}", h.app.status);
                assert_eq!(h.app.editor.document.model(), &before);
                assert!(!h.app.editor.document.can_undo());
                if cancel {
                    h.frame(vec![escape()]);
                }
                h.settle();
                if cancel {
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert!(!h.app.editor.document.can_undo());
                } else {
                    assert!(!h.app.status_error, "{}", h.app.status);
                    let edited = h.app.editor.document.model().clone();
                    assert_eq!(edited.walls[&h.wall].header, before.walls[&h.wall].header);
                    assert_eq!(edited.walls[&h.wall].parameters, expected);
                    h.app.history(false);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert!(!h.app.editor.document.can_undo());
                    h.app.history(true);
                    assert_eq!(h.app.editor.document.model(), &edited);
                }
                h.clean();
            }
        }
    }
}

#[test]
fn endpoint_body_and_empty_canvas_drag_keep_pan_precedence() {
    for (size, scale) in PROFILES {
        for body in [true, false] {
            let mut h = Harness::new(size, scale);
            let model = h.app.editor.document.model().clone();
            let camera = h.app.plans.cameras[&h.view];
            let p = h.point(Point2::new(0.0, if body { 0.0 } else { -1.0 }));
            h.press(p);
            assert!(h.app.plans.endpoint_drag.is_none());
            let target = p + egui::vec2(40.0, 30.0);
            h.frame(vec![egui::Event::PointerMoved(target)]);
            h.release(target);
            h.clean();
            assert_ne!(h.app.plans.cameras[&h.view], camera);
            assert_eq!(h.app.selected, Some(h.wall));
            assert_eq!(h.app.editor.document.model(), &model);
            assert!(!h.app.editor.document.can_undo());
            h.app.select(None);
            h.frame(vec![]);
            let body = h.point(Point2::new(0.0, 0.0));
            h.press(body);
            h.release(body);
            assert_eq!(h.app.selected, Some(h.wall));
        }
    }
}

#[test]
fn room_boundaries_and_placement_obey_view_crop_without_changing_area() {
    let crop = os_geometry::plan::PlanCrop {
        min: Point2::new(0.0, 0.0),
        max: Point2::new(1.0, 1.0),
    };
    assert_eq!(
        clip_plan_segment(Point2::new(-1.0, 0.5), Point2::new(2.0, 0.5), Some(crop),),
        Some((Point2::new(0.0, 0.5), Point2::new(1.0, 0.5)))
    );
    assert_eq!(
        clip_plan_segment(Point2::new(-1.0, 2.0), Point2::new(2.0, 2.0), Some(crop),),
        None
    );
    let context = PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: Default::default(),
        range: Default::default(),
        crop: Some(crop),
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    };
    assert!(point_in_plan_crop(context, Point2::new(0.5, 0.5)));
    assert!(!point_in_plan_crop(context, Point2::new(1.5, 0.5)));
    assert!(point_in_plan_crop(
        PlanContext {
            crop: None,
            ..context
        },
        Point2::new(1.5, 0.5)
    ));
}

#[test]
fn annotation_label_and_marker_bounds_stay_inside_the_plan_crop() {
    let crop = os_geometry::plan::PlanCrop {
        min: Point2::new(0.0, 0.0),
        max: Point2::new(1.0, 1.0),
    };
    let context = PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: Default::default(),
        range: Default::default(),
        crop: Some(crop),
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    };
    let camera = PlanCamera::default();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let size = [800.0, 600.0];
    let near_left_crop_edge = camera.project(Point2::new(0.05, 0.5), size).unwrap();
    let marker = rect.min + egui::vec2(near_left_crop_edge.x as f32, near_left_crop_edge.y as f32);
    assert!(
        !screen_rect_inside_plan_crop(
            context,
            camera,
            rect,
            size,
            egui::Rect::from_center_size(marker, egui::vec2(12.0, 12.0)),
        )
        .unwrap()
    );
    let comfortably_inside = camera.project(Point2::new(0.5, 0.5), size).unwrap();
    let label = rect.min + egui::vec2(comfortably_inside.x as f32, comfortably_inside.y as f32);
    assert!(
        screen_rect_inside_plan_crop(
            context,
            camera,
            rect,
            size,
            egui::Rect::from_center_size(label, egui::vec2(30.0, 18.0)),
        )
        .unwrap()
    );
}
