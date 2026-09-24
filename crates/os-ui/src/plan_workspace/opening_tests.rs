//! Real egui frames for pointer placement; no native-window or screenshot claim.
use super::*;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];
const KINDS: [OpeningKind; 2] = [OpeningKind::Door, OpeningKind::Window];

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
        app.draft.start = Point2::new(-4.0, 0.0);
        app.draft.end = Point2::new(4.0, 0.0);
        app.draft.height = 3.0;
        app.draft.thickness = 0.25;
        app.draft.name = "Opening host".into();
        app.apply_wall();
        let wall = app.selected.expect("fixture wall created");
        let view = app
            .editor
            .create_floor_plan("Opening plan", app.active_level)
            .unwrap();
        // Establish the fixture without leaving setup transactions in history.
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut harness = Self {
            app,
            ctx,
            output: Default::default(),
            size,
            scale,
            time: 0.0,
            wall,
            view,
        };
        harness.settle();
        // Keep both endpoints and several disjoint opening locations on canvas.
        let camera = harness.app.plans.cameras.get_mut(&view).unwrap();
        camera.center = Point2::new(0.0, 0.0);
        camera.pixels_per_metre = 35.0;
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
        self.output = self.ctx.run(input, |ctx| self.app.show(ctx));
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

    fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        self.hover(from);
        self.frame(vec![button(from, true)]);
        self.frame(vec![egui::Event::PointerMoved(to)]);
        assert!(self.app.wall_gesture.is_none());
        assert!(self.app.plans.endpoint_drag.is_none());
        self.frame(vec![button(to, false)]);
    }

    fn ribbon_click(&mut self, label: &str, band: std::ops::Range<f32>) {
        let position = self.output.shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == label
                && band.contains(&text.pos.y)
            {
                return Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center());
            }
            None
        });
        self.click(position.unwrap_or_else(|| panic!("missing ribbon button: {label}")));
        self.frame(vec![]);
    }

    fn begin(&mut self, kind: OpeningKind) {
        self.ribbon_click("Architecture", 30.0..58.0);
        self.ribbon_click(
            match kind {
                OpeningKind::Door => "Door",
                OpeningKind::Window => "Window",
            },
            58.0..180.0,
        );
        self.assert_active(kind);
        assert!(
            self.app.opening_draft.is_none(),
            "pointer tool, no modal form"
        );
    }

    fn assert_active(&self, kind: OpeningKind) {
        let draft = self
            .app
            .plans
            .opening_placement
            .as_ref()
            .expect("placement active");
        assert_eq!(draft.kind, kind);
        assert_eq!(
            draft.context,
            self.app.editor.native_plan_context(self.view).unwrap()
        );
    }

    fn has_text(&self, prefix: &str) -> bool {
        self.output.shapes.iter().any(|shape| {
            matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text.starts_with(prefix)
            )
        })
    }

    fn preview_lines(&self) -> usize {
        let rect = self.app.plans.canvas_rect.unwrap();
        self.output
            .shapes
            .iter()
            .filter(|shape| {
                matches!(
                    &shape.shape,
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.width == 2.0 && stroke.color == theme::ACCENT
                            && points.iter().all(|point| rect.contains(*point))
                )
            })
            .count()
    }

    fn handles(&self) -> usize {
        self.output.shapes.iter().filter(|shape| matches!(
            &shape.shape,
            egui::Shape::Circle(circle) if circle.radius == 6.0 && circle.fill == theme::ACCENT
        )).count()
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

impl Harness {
    fn movable(
        kind: OpeningKind,
        reversed: bool,
        legacy: bool,
        size: egui::Vec2,
        scale: f32,
    ) -> (Self, Id) {
        let mut h = Self::new(size, scale);
        let mut model = h.app.editor.document.model().clone();
        // A rotated host catches projection errors that horizontal-only tests miss.
        let wall = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
        wall.start = Point2::new(-3.2, -2.4);
        wall.end = Point2::new(3.2, 2.4);
        if reversed {
            std::mem::swap(&mut wall.start, &mut wall.end);
        }
        let (width, height, sill) = if kind == OpeningKind::Door {
            (0.9, 2.1, 0.0)
        } else {
            (1.2, 1.2, 0.9)
        };
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                family: Default::default(),
                name: "Drag type".into(),
                kind,
                width,
                height,
                sill,
                pane_position: Default::default(),
            },
        );
        let definition = if legacy {
            OpeningDefinition::Legacy {
                kind,
                width,
                height,
                sill,
            }
        } else {
            OpeningDefinition::Typed { type_id: ty.id() }
        };
        model.opening_types.insert(ty.id(), ty);
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Keep this name".into(),
                host: h.wall,
                offset: 1.5,
                definition,
                hinge: if kind == OpeningKind::Door {
                    os_model::DoorHinge::End
                } else {
                    Default::default()
                },
                swing: if kind == OpeningKind::Door {
                    os_model::DoorSwing::Right
                } else {
                    Default::default()
                },
            },
        );
        let id = opening.id();
        model.openings.insert(id, opening);
        h.app.editor.document = Document::from_model(model).unwrap();
        h.app.editor.pending_geometry.extend([h.wall, id]);
        h.app.editor.regenerate().unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.app.select(Some(id));
        h.settle();
        h.app.plans.cameras.insert(
            h.view,
            PlanCamera {
                center: Point2::new(0.0, 0.0),
                pixels_per_metre: 35.0,
            },
        );
        h.frame(vec![]);
        (h, id)
    }

    fn opening_point(&self, id: Id, offset: f64) -> egui::Pos2 {
        let model = self.app.editor.document.model();
        let p = model
            .resolve_opening(&model.openings[&id].parameters)
            .unwrap();
        self.point(os_geometry::openings::world(
            &model.resolve_wall(p.host).unwrap().parameters,
            offset + p.width * 0.5,
            0.0,
        ))
    }

    fn press_move(&mut self, id: Id, offset: f64) -> egui::Pos2 {
        let from = self.opening_point(
            id,
            self.app.editor.document.model().openings[&id]
                .parameters
                .offset,
        );
        self.hover(from);
        self.frame(vec![button(from, true)]);
        assert!(self.app.plans.opening_move.is_some());
        let to = self.opening_point(id, offset);
        self.hover(to);
        to
    }

    fn move_clean(&self) {
        assert!(self.app.plans.opening_move.is_none());
        assert!(!self.app.plans.opening_move_claimed);
        assert!(self.app.plans.endpoint_drag.is_none());
        assert!(self.app.wall_gesture.is_none());
    }
}

#[test]
fn opening_drag_preview_aperture_and_single_update_history() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                for legacy in [false, true] {
                    let (mut h, id) = Harness::movable(kind, reversed, legacy, size, scale);
                    assert_eq!(h.handles(), 1, "one selected opening grip");
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let camera = h.app.plans.cameras[&h.view];
                    let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                    let to = h.press_move(id, 2.5);
                    // A perpendicular excursion must not change the host offset.
                    h.hover(to + egui::vec2(-10.5, -14.0));
                    let off_axis = h
                        .app
                        .plans
                        .opening_move
                        .as_ref()
                        .unwrap()
                        .preview(&h.app.editor)
                        .unwrap();
                    h.hover(to);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.revision(), 0);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                    assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                    let draft = h.app.plans.opening_move.as_ref().unwrap();
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let preview = draft.preview(&h.app.editor).unwrap();
                    let off_axis_lines = off_axis.provider_lines(context).unwrap();
                    for (a, b) in off_axis_lines
                        .iter()
                        .zip(preview.provider_lines(context).unwrap())
                    {
                        assert!(a.start.distance(b.start) < 1e-5);
                        assert!(a.end.distance(b.end) < 1e-5);
                    }
                    let old = h.app.editor.native_wall_plan(h.view).unwrap();
                    assert_ne!(preview.items(context).unwrap(), old.items(context).unwrap());
                    assert_ne!(
                        preview.provider_lines(context).unwrap(),
                        old.provider_lines(context).unwrap()
                    );
                    // The real paint stream includes the preview's aperture polygons.
                    for item in preview.items(context).unwrap() {
                        let points: Vec<_> = item
                            .footprint
                            .vertices()
                            .iter()
                            .map(|p| h.point(*p))
                            .collect();
                        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                            egui::Shape::Path(path) if path.closed && path.points == points)));
                    }
                    for line in preview.provider_lines(context).unwrap() {
                        let points = [h.point(line.start), h.point(line.end)];
                        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                            egui::Shape::LineSegment { points: actual, stroke }
                                if *actual == points && stroke.color == theme::ACCENT)));
                    }
                    h.frame(vec![button(to, false)]);
                    h.settle();
                    h.move_clean();
                    assert_eq!(h.app.selected, Some(id));
                    assert_eq!(h.app.editor.document.revision(), 1);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                    let after = h.app.editor.document.model().clone();
                    let mut expected = before.clone();
                    expected.openings.get_mut(&id).unwrap().parameters.offset =
                        after.openings[&id].parameters.offset;
                    assert!((after.openings[&id].parameters.offset - 2.5).abs() < 1e-5);
                    assert_eq!(after, expected, "offset is the only persisted change");
                    let current = h.app.editor.native_plan_context(h.view).unwrap();
                    let committed = h.app.editor.native_wall_plan(h.view).unwrap();
                    assert_eq!(
                        preview.items(context).unwrap(),
                        committed.items(current).unwrap()
                    );
                    assert_eq!(
                        preview.provider_lines(context).unwrap(),
                        committed.provider_lines(current).unwrap()
                    );
                    assert_ne!(h.app.editor.scene, scene);
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &after);
                }
            }
        }
    }
}

#[test]
fn opening_drag_rejects_clearance_overlap_and_collapsing_pier() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                let (mut h, id) = Harness::movable(kind, reversed, false, size, scale);
                let mut model = h.app.editor.document.model().clone();
                let mut neighbor =
                    Opening::new("core.opening", model.openings[&id].parameters.clone());
                neighbor.parameters.offset = 4.5;
                model.openings.insert(neighbor.id(), neighbor);
                let width = model
                    .resolve_opening(&model.openings[&id].parameters)
                    .unwrap()
                    .width;
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app
                    .editor
                    .pending_geometry
                    .extend(h.app.editor.document.model().openings.keys().copied());
                h.app.editor.pending_geometry.insert(h.wall);
                h.app.editor.regenerate().unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.settle();
                h.app.plans.cameras.insert(
                    h.view,
                    PlanCamera {
                        center: Point2::new(0.0, 0.0),
                        pixels_per_metre: 35.0,
                    },
                );
                h.frame(vec![]);
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                for offset in [0.0005, 8.0 - width - 0.0005, 4.5, 4.5 - width - 0.0005] {
                    let to = h.press_move(id, offset);
                    assert!(h.has_text("Cannot move:"), "offset {offset}");
                    h.frame(vec![button(to, false)]);
                    h.frame(vec![]);
                    h.move_clean();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                }
                // A valid preview followed by an invalid release cannot commit the cached preview.
                h.press_move(id, 2.0);
                h.frame(vec![button(h.opening_point(id, 4.5), false)]);
                h.move_clean();
                assert_eq!(h.app.editor.document.model(), &before);
            }
        }
    }
}

#[test]
fn opening_drag_cancel_stale_outside_and_click_leave_no_residue() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reason in [
                "escape",
                "revision",
                "session",
                "view",
                "settings",
                "provider",
                "selection",
                "drawing",
                "outside",
                "lost",
            ] {
                let (mut h, id) = Harness::movable(kind, false, false, size, scale);
                let camera = h.app.plans.cameras[&h.view];
                let mut to = h.press_move(id, 2.5);
                match reason {
                    "escape" => h.frame(vec![escape()]),
                    "revision" => {
                        h.app
                            .editor
                            .command("Other edit", Command::RenameProject("Changed".into()))
                            .unwrap();
                    }
                    "session" => {
                        h.app.editor.document =
                            Document::from_model(h.app.editor.document.model().clone()).unwrap();
                    }
                    "view" => {
                        h.app.focus_plan(None);
                    }
                    "settings" => {
                        let mut settings = h.app.editor.document.model().views[&h.view]
                            .parameters
                            .plan
                            .unwrap();
                        settings.visibility.walls = false;
                        h.app
                            .editor
                            .update_floor_plan(h.view, "Hidden", h.app.active_level, settings)
                            .unwrap();
                    }
                    "provider" => {
                        h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                    }
                    "selection" => {
                        h.app.select(Some(h.wall));
                    }
                    "drawing" => {
                        h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap());
                    }
                    "outside" => {
                        to = egui::pos2(-10.0, -10.0);
                    }
                    "lost" => {
                        h.frame(vec![egui::Event::PointerGone]);
                        to = egui::pos2(-10.0, -10.0);
                    }
                    _ => unreachable!(),
                }
                let before_release = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                let scene = h.app.editor.scene.clone();
                h.frame(vec![egui::Event::PointerMoved(to), button(to, false)]);
                h.frame(vec![]);
                h.move_clean();
                assert_eq!(h.app.editor.document.model(), &before_release, "{reason}");
                assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
                assert_eq!(h.app.editor.scene, scene, "{reason}");
                if let Some(current) = h.app.plans.cameras.get(&h.view) {
                    assert_eq!(*current, camera, "{reason}");
                }
            }
            let (mut h, id) = Harness::movable(kind, false, false, size, scale);
            let point = h.opening_point(id, 1.5);
            h.click(point);
            h.move_clean();
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
            h.press_move(id, 2.5);
            h.frame(vec![egui::Event::PointerMoved(point), button(point, false)]);
            h.move_clean();
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        }
    }
}

#[test]
fn opening_drag_visibility_and_pointer_precedence() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let (mut h, id) = Harness::movable(kind, false, false, size, scale);
            let camera = h.app.plans.cameras[&h.view];
            let blank = h.point(Point2::new(0.0, 2.0));
            h.drag(blank, blank + egui::vec2(24.0, 12.0));
            assert_ne!(h.app.plans.cameras[&h.view], camera, "ordinary drag pans");
            h.app.select(Some(h.wall));
            h.frame(vec![]);
            assert_eq!(h.handles(), 2, "wall endpoint handles retained");
            h.app.select(Some(id));
            h.begin(kind);
            assert_eq!(h.handles(), 0, "placement owns pointer before opening grip");
            let from = h.opening_point(id, 1.5);
            h.drag(from, h.opening_point(id, 2.5));
            assert!(h.app.plans.opening_move.is_none());
            h.frame(vec![escape()]);
            for hidden in [false, true] {
                let mut settings = h.app.editor.document.model().views[&h.view]
                    .parameters
                    .plan
                    .unwrap();
                settings.visibility.walls = !hidden;
                settings.crop = (!hidden).then_some(os_model::PlanViewCrop {
                    min: Point2::new(0.0, -3.0),
                    max: Point2::new(4.0, 3.0),
                });
                h.app
                    .editor
                    .update_floor_plan(h.view, "Visibility", h.app.active_level, settings)
                    .unwrap();
                h.settle();
                assert_eq!(h.handles(), 0, "hidden/cropped opening has no grip");
            }
        }
    }
}

#[test]
fn opening_placement_ribbon_hover_repeat_commit_and_history() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let mut h = Harness::new(size, scale);
            let original = h.app.editor.document.model().clone();
            let original_scene = h.app.editor.scene.clone();
            h.begin(kind);
            let blank = h.point(Point2::new(0.0, 2.0));
            h.hover(blank);
            let idle_lines = h.preview_lines();
            let first = h.point(Point2::new(-2.0, 0.0));
            h.hover(first);
            assert!(h.has_text(match kind {
                OpeningKind::Door => "Door · 0.90 m wide · offset",
                OpeningKind::Window => "Window · 1.20 m wide · offset",
            }));
            assert!(
                h.preview_lines() > idle_lines,
                "opening symbol must be painted"
            );
            assert_eq!(h.app.editor.document.model(), &original);
            assert_eq!(h.app.editor.document.revision(), 0);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
            assert_eq!(h.app.editor.scene, original_scene);

            h.click(first);
            assert_eq!(h.app.editor.document.model().openings.len(), 1);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
            let first_id = h.app.selected.unwrap();
            assert_eq!(h.app.editor.document.model().opening_types.len(), 1);
            assert!(h.app.editor.scene.contains_key(&first_id));
            assert_ne!(
                h.app.editor.scene.get(&h.wall),
                original_scene.get(&h.wall),
                "host wall geometry regenerates around the committed opening"
            );
            let p = &h.app.editor.document.model().openings[&first_id].parameters;
            assert_eq!(p.host, h.wall);
            let resolved = h.app.editor.document.model().resolve_opening(p).unwrap();
            assert_eq!(resolved.kind, kind);
            let first_type = resolved
                .type_id
                .expect("pointer placement creates a reusable type");
            let (width, height, sill) = match kind {
                OpeningKind::Door => (0.9, 2.1, 0.0),
                OpeningKind::Window => (1.2, 1.2, 0.9),
            };
            assert_eq!(
                (resolved.width, resolved.height, resolved.sill),
                (width, height, sill)
            );
            assert!((p.offset - (2.0 - width * 0.5)).abs() < 1e-5);
            assert_eq!(h.app.editor.document.model().walls, original.walls);
            h.frame(vec![]);
            h.assert_active(kind);
            h.settle();
            let after_first = h.app.editor.document.model().clone();

            // A second click uses the regenerated drawing and refreshed context.
            h.click(h.point(Point2::new(2.0, 0.0)));
            assert_eq!(h.app.editor.document.model().openings.len(), 2);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
            let second_id = h.app.selected.unwrap();
            assert_ne!(first_id, second_id);
            assert_eq!(h.app.editor.document.model().opening_types.len(), 1);
            assert_eq!(
                h.app
                    .editor
                    .document
                    .model()
                    .resolve_opening(&h.app.editor.document.model().openings[&second_id].parameters)
                    .unwrap()
                    .type_id,
                Some(first_type),
            );
            h.frame(vec![]);
            h.assert_active(kind);
            h.settle();
            let after_second = h.app.editor.document.model().clone();
            h.frame(vec![escape()]);
            assert!(h.app.plans.opening_placement.is_none());
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after_first);
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &original);
            assert!(!h.app.editor.document.can_undo());
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after_first);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after_second);
        }
    }
}

#[test]
fn opening_placement_invalid_overlap_and_end_clearance_keep_history() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let mut h = Harness::new(size, scale);
            h.begin(kind);
            h.click(h.point(Point2::new(0.0, 0.0)));
            h.settle();
            let model = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            for world in [
                Point2::new(0.0, 0.0),
                Point2::new(-3.9, 0.0),
                Point2::new(3.9, 0.0),
            ] {
                let point = h.point(world);
                h.hover(point);
                assert!(h.has_text("Cannot place:"), "invalid preview at {world:?}");
                h.click(point);
                h.frame(vec![]);
                assert_eq!(h.app.editor.document.model(), &model);
                assert_eq!(h.app.editor.document.revision(), revision);
                assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                h.assert_active(kind);
            }
            // An invalid attempt does not poison the repeatable tool.
            h.click(h.point(Point2::new(2.0, 0.0)));
            assert_eq!(h.app.editor.document.model().openings.len(), 2);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
        }
    }
}

#[test]
fn opening_placement_suppresses_handles_and_pan_until_escape() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let mut h = Harness::new(size, scale);
            h.app.select(Some(h.wall));
            h.frame(vec![]);
            assert_eq!(
                h.handles(),
                2,
                "positive control: visible selected endpoints"
            );
            let model = h.app.editor.document.model().clone();
            let camera = h.app.plans.cameras[&h.view];
            h.begin(kind);
            assert_eq!(h.handles(), 0);
            for world in [Point2::new(-4.0, 0.0), Point2::new(0.0, 2.0)] {
                let from = h.point(world);
                h.drag(from, from + egui::vec2(24.0, 12.0));
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                assert_eq!(h.app.editor.document.model(), &model);
                assert!(!h.app.editor.document.can_undo());
                h.assert_active(kind);
            }
            h.hover(h.point(Point2::new(0.0, 0.0)));
            h.frame(vec![escape()]);
            assert!(h.app.plans.opening_placement.is_none());
            h.click(h.point(Point2::new(0.0, 0.0)));
            assert_eq!(h.app.editor.document.model(), &model);
            assert!(!h.app.editor.document.can_undo());
            let from = h.point(Point2::new(0.0, 2.0));
            h.drag(from, from + egui::vec2(24.0, 12.0));
            assert_ne!(
                h.app.plans.cameras[&h.view], camera,
                "ordinary canvas drag pans again"
            );
        }
    }
}

#[test]
fn opening_placement_rejects_hidden_and_cropped_hosts() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for hidden in [false, true] {
                let mut h = Harness::new(size, scale);
                let mut settings = h.app.editor.document.model().views[&h.view]
                    .parameters
                    .plan
                    .unwrap();
                if hidden {
                    settings.visibility.walls = false;
                } else {
                    // Keep half the host visible: clicking its cropped half is ineligible.
                    settings.crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(0.0, -1.0),
                        max: Point2::new(4.0, 1.0),
                    });
                }
                h.app
                    .editor
                    .update_floor_plan(h.view, "Visibility", h.app.active_level, settings)
                    .unwrap();
                h.settle();
                let model = h.app.editor.document.model().clone();
                let revision = h.app.editor.document.revision();
                let undo_entries = h.app.editor.document.history_stats().undo_entries;
                h.begin(kind);
                let point = h.point(Point2::new(-2.0, 0.0));
                h.hover(point);
                assert!(!h.has_text("Door ·") && !h.has_text("Window ·"));
                h.click(point);
                h.frame(vec![]);
                assert_eq!(h.app.editor.document.model(), &model);
                assert_eq!(h.app.editor.document.revision(), revision);
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    undo_entries
                );
                h.assert_active(kind);
                if !hidden {
                    h.click(h.point(Point2::new(2.0, 0.0)));
                    assert_eq!(h.app.editor.document.model().openings.len(), 1);
                }
            }
        }
    }
}

#[test]
fn opening_placement_stale_document_cancels_without_delayed_commit() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let mut h = Harness::new(size, scale);
            h.begin(kind);
            let point = h.point(Point2::new(0.0, 0.0));
            h.hover(point);
            h.frame(vec![button(point, true)]);
            h.app
                .editor
                .command("External edit", Command::RenameProject("Changed".into()))
                .unwrap();
            let model = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.frame(vec![button(point, false)]);
            h.settle();
            assert!(h.app.plans.opening_placement.is_none());
            assert_eq!(h.app.editor.document.model(), &model);
            assert_eq!(h.app.editor.document.revision(), revision);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
            h.click(h.point(Point2::new(0.0, 0.0)));
            assert_eq!(h.app.editor.document.model(), &model);
        }
    }
}

#[test]
fn opening_orientation_controls_preview_apply_escape_and_stale_cancel_in_real_frames() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.begin(OpeningKind::Door);
        h.click(h.point(Point2::new(0.0, 0.0)));
        h.settle();
        h.frame(vec![escape()]);
        let id = *h
            .app
            .editor
            .document
            .model()
            .openings
            .keys()
            .next()
            .unwrap();
        h.app.select(Some(id));
        let initial = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let edit = |h: &mut Harness| {
            h.app.begin_opening(None);
            h.frame(vec![]);
            h.frame(vec![]);
            assert!(h.has_text("Hinge"));
            assert!(h.has_text("Swing"));
            h.ribbon_click("Wall end", 0.0..size.y);
            h.ribbon_click("Right of wall", 0.0..size.y);
            h.frame(vec![]);
            assert!(h.has_text("Opening preview"));
        };
        edit(&mut h);
        assert_eq!(h.app.editor.document.model(), &initial);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.frame(vec![escape()]);
        assert!(h.app.opening_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &initial);
        edit(&mut h);
        h.ribbon_click("Apply opening", 0.0..size.y);
        h.settle();
        assert!(h.app.opening_draft.is_none());
        let applied = h.app.editor.document.model().clone();
        assert_eq!(
            applied.openings[&id].parameters.hinge,
            os_model::DoorHinge::End
        );
        assert_eq!(
            applied.openings[&id].parameters.swing,
            os_model::DoorSwing::Right
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &initial);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &applied);
        h.app.select(Some(id));
        edit(&mut h);
        h.app
            .editor
            .command("External edit", Command::RenameProject("Stale".into()))
            .unwrap();
        let external = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.opening_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &external);
    }
}

#[test]
fn opening_arc_selects_door_but_does_not_hijack_active_placement() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.begin(OpeningKind::Door);
        h.click(h.point(Point2::new(0.0, 0.0)));
        h.settle();
        h.frame(vec![escape()]);
        let id = *h
            .app
            .editor
            .document
            .model()
            .openings
            .keys()
            .next()
            .unwrap();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let drawing = h.app.editor.native_wall_plan(h.view).unwrap();
        let line = drawing
            .provider_lines(context)
            .unwrap()
            .iter()
            .find(|line| line.entity == id && line.feature == 11)
            .unwrap();
        let point = h.point(Point2::new(
            (line.start.x + line.end.x) * 0.5,
            (line.start.y + line.end.y) * 0.5,
        ));
        h.app.select(None);
        h.frame(vec![]);
        h.click(point);
        assert_eq!(h.app.selected, Some(id));
        h.begin(OpeningKind::Window);
        h.app.select(Some(h.wall));
        h.frame(vec![]);
        let model = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click(point);
        assert_eq!(h.app.selected, Some(h.wall));
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.assert_active(OpeningKind::Window);
    }
}
