//! Headless DesktopApp egui input, plus geometry checks; no native-window claim.
use super::*;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280., 800.), 1.),
    (egui::vec2(1000., 650.), 1.5),
];

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    output: egui::FullOutput,
    size: egui::Vec2,
    scale: f32,
    time: f64,
    view: Id,
    wall: Id,
    door: Id,
    window: Id,
    floor: Id,
}

fn ring(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point2> {
    vec![
        Point2::new(x0, y0),
        Point2::new(x1, y0),
        Point2::new(x1, y1),
        Point2::new(x0, y1),
    ]
}

impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        *app.draft.path.straight_start_mut().unwrap() = Point2::new(-3., 0.);
        *app.draft.path.straight_end_mut().unwrap() = Point2::new(3., 0.);
        app.apply_wall();
        let wall = app.selected.unwrap();
        let view = app
            .editor
            .create_floor_plan("Area selection", app.active_level)
            .unwrap();
        let mut model = app.editor.document.model().clone();
        let mut opening = |kind, offset| {
            let opening = Opening::new(
                "core.opening",
                OpeningParams {
                    open_state: Default::default(),
                    name: format!("{kind:?}"),
                    host: wall,
                    offset,
                    definition: OpeningDefinition::Legacy {
                        kind,
                        width: 0.8,
                        height: 1.8,
                        sill: if kind == OpeningKind::Window { 0.6 } else { 0. },
                    },
                    width_override: None,
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
                    hinge: Default::default(),
                    swing: Default::default(),
                },
            );
            let id = opening.id();
            model.openings.insert(id, opening);
            id
        };
        let door = opening(OpeningKind::Door, 0.6);
        let window = opening(OpeningKind::Window, 3.8);
        let floor = os_model::Floor::new(
            "core.floor",
            os_model::FloorParams {
                name: "Perforated slab".into(),
                level: app.active_level,
                material: None,
                boundary: ring(-2., -2., 2., -0.6),
                holes: vec![ring(-0.4, -1.6, 0.4, -1.)],
                thickness: 0.2,
                top_offset: 0.,
            },
        );
        let floor_id = floor.id();
        model.floors.insert(floor_id, floor);
        app.editor.document = Document::from_model(model).unwrap();
        app.editor
            .pending_geometry
            .extend([wall, door, window, floor_id]);
        app.editor.regenerate().unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        app.select(None);
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Self {
            app,
            ctx,
            output: Default::default(),
            size,
            scale,
            time: 0.,
            view,
            wall,
            door,
            window,
            floor: floor_id,
        };
        h.settle();
        h.app.plans.cameras.insert(
            view,
            PlanCamera {
                center: Point2::new(0., -0.5),
                pixels_per_metre: 28.,
            },
        );
        h.frame(vec![]);
        h
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1. / 60.;
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
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
    fn point(&self, x: f64, y: f64) -> egui::Pos2 {
        self.app
            .plans
            .test_screen_point(self.view, Point2::new(x, y))
            .unwrap()
    }
    fn press(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
        self.frame(vec![button(p, true)]);
    }
    fn press_area(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
        self.frame(vec![shift_button(p, true)]);
    }
    fn release(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p), button(p, false)]);
    }
    fn area(&mut self, a: egui::Pos2, b: egui::Pos2) {
        self.press_area(a);
        self.frame(vec![egui::Event::PointerMoved(b)]);
        self.release(b);
        self.frame(vec![]);
    }
    fn drag(&mut self, a: egui::Pos2, b: egui::Pos2) {
        self.press(a);
        self.frame(vec![egui::Event::PointerMoved(b)]);
        self.release(b);
        self.frame(vec![]);
    }
    fn focus_canvas(&mut self, point: egui::Pos2) -> Option<Id> {
        self.press(point);
        self.release(point);
        self.app.selected
    }
    fn all(&self) -> BTreeSet<Id> {
        [self.wall, self.door, self.window, self.floor].into()
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
fn shift_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
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
fn tab(shift: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift,
            ..Default::default()
        },
    }
}
fn overlap_point(h: &Harness, opening: Id) -> (egui::Pos2, Vec<Id>) {
    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let drawing = h.app.plans.drawing.as_ref().unwrap();
    let camera = h.app.plans.cameras[&h.view];
    let canvas = h.app.plans.canvas_rect.unwrap();
    let viewport = [f64::from(canvas.width()), f64::from(canvas.height())];
    for line in drawing
        .provider_lines(context)
        .unwrap()
        .iter()
        .filter(|line| line.entity == opening)
    {
        let a = camera.project(line.start, viewport).unwrap();
        let b = camera.project(line.end, viewport).unwrap();
        for step in 0..=10 {
            let t = f64::from(step) / 10.0;
            let local = Point2::new(a.x * (1.0 - t) + b.x * t, a.y * (1.0 - t) + b.y * t);
            let hits = drawing
                .hits_screen(context, camera, viewport, local, 6.0)
                .unwrap();
            if hits.len() == 2 && hits[0] == opening && hits[1] == h.wall {
                return (
                    canvas.min + egui::vec2(local.x as f32, local.y as f32),
                    hits,
                );
            }
        }
    }
    panic!("native opening symbol did not overlap its host wall in screen picking");
}
fn has_canvas_text(h: &Harness, expected: &str) -> bool {
    let canvas = h.app.plans.canvas_rect.unwrap();
    h.output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Text(text) => text.galley.job.text == expected && canvas.contains(text.pos),
        _ => false,
    })
}
#[test]
fn area_geometry_touch_containment_clipping_and_bbox_false_positive() {
    let p = |x, y| egui::pos2(x, y);
    let triangle = vec![p(0., 0.), p(10., 0.), p(0., 10.)];
    assert!(matches_area(
        std::slice::from_ref(&triangle),
        p(-1., -1.),
        p(11., 11.)
    ));
    assert!(!matches_area(
        std::slice::from_ref(&triangle),
        p(-1., -1.),
        p(9., 11.)
    ));
    assert!(!matches_area(
        std::slice::from_ref(&triangle),
        p(9., 8.),
        p(8., 9.)
    ));
    assert!(matches_area(
        std::slice::from_ref(&triangle),
        p(11., 0.),
        p(10., 1.)
    ));
    assert!(matches_area(
        std::slice::from_ref(&triangle),
        p(3., 2.),
        p(2., 3.)
    ));
    let clipped = clip_polygon(triangle, egui::Rect::from_min_max(p(2., 2.), p(4., 4.)));
    assert!(matches_area(&[clipped], p(2., 2.), p(4., 4.)));
    assert!(!matches_area(&[], p(0., 0.), p(10., 10.)));
    let symbol_segment = vec![p(5., -2.), p(5., 2.)];
    assert!(!matches_area(
        std::slice::from_ref(&symbol_segment),
        p(4., -1.),
        p(6., 1.)
    ));
    assert!(matches_area(&[symbol_segment], p(6., 1.), p(4., -1.)));
    assert!(matches_area(
        &[vec![p(5., -0.5), p(5., 0.5)]],
        p(4., -1.),
        p(6., 1.)
    ));
}

#[test]
fn area_desktop_window_crossing_group_highlights_primary_and_no_history() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let model = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        let camera = h.app.plans.cameras[&h.view];
        let (a, b) = (h.point(-3.5, 1.), h.point(3.5, -2.5));
        h.press_area(a);
        h.frame(vec![egui::Event::PointerMoved(b)]);
        assert!(h.app.selected_ids.is_empty());
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        h.release(b);
        h.frame(vec![]);
        assert_eq!(h.app.selected_ids, h.all());
        assert_eq!(h.app.selected, None);
        assert!(
            h.output.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="4 selected")
            )
        );
        assert!(
            h.output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Path(p) if p.fill==theme::SELECTED))
        );
        // Same narrow rectangle: window rejects partial walls, crossing includes one.
        h.area(h.point(-0.3, 0.4), h.point(0.3, -0.4));
        assert!(h.app.selected_ids.is_empty());
        h.area(h.point(0.3, 0.4), h.point(-0.3, -0.4));
        assert_eq!(h.app.selected_ids, BTreeSet::from([h.wall]));
        assert_eq!(h.app.selected, Some(h.wall));
        // A region fully inside the slab's hole selects nothing.
        h.app.select(None);
        h.frame(vec![]);
        h.area(h.point(0.3, -1.1), h.point(-0.3, -1.5));
        assert!(h.app.selected_ids.is_empty());
        // Stationary click uses the existing footprint pick.
        let p = h.point(0., 0.);
        h.press(p);
        h.release(p);
        assert_eq!(h.app.selected, Some(h.wall));
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert!(!h.app.editor.document.can_undo());
        assert!(!h.app.editor.document.can_redo());
    }
}

#[test]
fn area_desktop_pan_and_endpoint_precedence() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let camera = h.app.plans.cameras[&h.view];
        let p = h.point(0., 1.);
        h.drag(p, p + egui::vec2(30., 10.));
        assert_ne!(h.app.plans.cameras[&h.view], camera);
        h.app.select(Some(h.wall));
        h.frame(vec![]);
        let p = h.point(-3., 0.);
        h.press_area(p);
        assert!(h.app.plans.endpoint_pointer_claimed);
        assert!(!h.app.plans.area_selection.claimed);
        h.frame(vec![escape()]);
        h.release(p);
        assert!(!h.app.plans.area_selection.claimed);
    }
}

#[test]
fn area_desktop_cancel_stale_and_outside_preserve_selection_and_camera() {
    for (size, scale) in PROFILES {
        for cause in 0..8 {
            let mut h = Harness::new(size, scale);
            h.app.select_area(h.all());
            h.frame(vec![]);
            let ids = h.app.selected_ids.clone();
            let camera = h.app.plans.cameras[&h.view];
            let (a, b) = (h.point(-3.5, 1.), h.point(3.5, -2.5));
            h.press_area(a);
            h.frame(vec![egui::Event::PointerMoved(b)]);
            assert!(h.app.plans.area_selection.draft.is_some());
            match cause {
                0 => h.frame(vec![escape()]),
                1 => h.frame(vec![egui::Event::PointerGone]),
                2 => {
                    h.release(egui::pos2(-10., -10.));
                }
                3 => {
                    h.app.focus_plan(None);
                    h.frame(vec![]);
                    h.app.focus_plan(Some(h.view));
                }
                4 => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap());
                    h.frame(vec![]);
                }
                5 => {
                    let v = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .clone();
                    let mut s = v.plan.unwrap();
                    s.scale_denominator = 75.;
                    h.app
                        .editor
                        .update_floor_plan(h.view, &v.name, v.level.unwrap(), s)
                        .unwrap();
                    h.frame(vec![]);
                }
                6 => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap();
                    h.frame(vec![]);
                }
                _ => {
                    h.app
                        .plans
                        .area_selection
                        .draft
                        .as_mut()
                        .unwrap()
                        .providers
                        .push(("stale".into(), Id::new()));
                    h.frame(vec![]);
                }
            }
            assert!(h.app.plans.area_selection.draft.is_none(), "cause {cause}");
            h.release(b);
            assert_eq!(h.app.selected_ids, ids, "cause {cause}");
            assert!(!h.app.plans.area_selection.claimed);
            if cause != 6 {
                assert_eq!(h.app.plans.cameras[&h.view], camera, "cause {cause}");
            }
        }
    }
}

#[test]
fn overlap_cycle_preview_and_click_select_doors_windows_or_their_host_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let model = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        for (id, label) in [(h.door, "Door"), (h.window, "Window")] {
            let (point, hits) = overlap_point(&h, id);
            assert_eq!(hits, vec![id, h.wall]);
            assert_eq!(h.focus_canvas(point), Some(id));
            h.frame(vec![egui::Event::PointerMoved(point)]);
            h.frame(vec![tab(false)]);
            assert!(has_canvas_text(&h, "2 of 2 · Wall"));
            assert_eq!(
                h.app.selected,
                Some(id),
                "preview must not replace selection"
            );
            assert_eq!(h.app.editor.document.model(), &model);
            assert_eq!(h.app.editor.document.history_stats(), history);
            h.frame(vec![tab(true)]);
            assert!(has_canvas_text(&h, &format!("1 of 2 · {label}")));
            assert_eq!(h.app.selected, Some(id));
            h.frame(vec![tab(false)]);
            assert!(has_canvas_text(&h, "2 of 2 · Wall"));
            h.frame(vec![button(point, true)]);
            h.frame(vec![button(point, false)]);
            assert_eq!(h.app.selected, Some(h.wall));
            assert_eq!(h.app.editor.document.model(), &model);
            assert_eq!(h.app.editor.document.history_stats(), history);
            h.app.select(None);
            h.frame(vec![]);
            // Two forward cycles wrap back to the ordinary top-priority opening.
            h.frame(vec![egui::Event::PointerMoved(point)]);
            h.frame(vec![tab(false)]);
            h.frame(vec![tab(false)]);
            assert!(has_canvas_text(&h, &format!("1 of 2 · {label}")));
            h.frame(vec![escape()]);
            assert!(!has_canvas_text(&h, &format!("1 of 2 · {label}")));
            assert_eq!(h.app.editor.document.model(), &model);
            assert_eq!(h.app.editor.document.history_stats(), history);
            h.app.select(None);
        }
    }
}

#[test]
fn overlap_cycle_stale_pointer_and_active_tool_never_steal_input() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let model = h.app.editor.document.model().clone();
        let (point, _) = overlap_point(&h, h.door);
        assert_eq!(h.focus_canvas(point), Some(h.door));
        h.frame(vec![egui::Event::PointerMoved(point)]);
        h.frame(vec![tab(false)]);
        assert!(has_canvas_text(&h, "2 of 2 · Wall"));
        h.frame(vec![egui::Event::PointerMoved(point + egui::vec2(12., 0.))]);
        assert!(!has_canvas_text(&h, "2 of 2 · Wall"));

        // A revision change invalidates the preview; the subsequent click falls
        // through to the current ordinary pick rather than selecting stale data.
        h.frame(vec![egui::Event::PointerMoved(point)]);
        h.frame(vec![tab(false)]);
        h.app
            .editor
            .command(
                "Rename while cycling",
                os_document::Command::RenameProject("Changed".into()),
            )
            .unwrap();
        h.settle();
        h.frame(vec![]);
        assert!(!has_canvas_text(&h, "2 of 2 · Wall"));
        assert_eq!(h.app.selected, Some(h.door));
        assert_eq!(h.app.editor.document.model().openings, model.openings);
        h.press(point);
        h.release(point);
        assert_eq!(
            h.app.selected,
            Some(h.door),
            "stale cycle falls back to ordinary pick"
        );
        h.app.select(None);
        h.frame(vec![]);

        // Active placement owns pointer/keyboard behavior.
        h.app.plans.room_placement_active = true;
        h.frame(vec![egui::Event::PointerMoved(point), tab(false)]);
        assert!(!has_canvas_text(&h, "2 of 2 · Wall"));
        h.app.plans.room_placement_active = false;
        assert_eq!(h.app.editor.document.model().openings, model.openings);
    }
}

#[test]
fn selection_filters_click_through_disabled_openings_and_clear_when_none_are_enabled() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let model = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let filters = &mut h.app.plans.selection_filters;
        assert!(filters.set_enabled(selection_filters::Category::Door, false));

        let (door_point, _) = overlap_point(&h, h.door);
        h.press(door_point);
        h.release(door_point);
        assert_eq!(
            h.app.selected,
            Some(h.wall),
            "a disabled foreground door must not block its enabled host"
        );
        let (wall_point, _) = overlap_point(&h, h.door);
        h.frame(vec![egui::Event::PointerMoved(wall_point)]);
        h.frame(vec![tab(false)]);
        assert!(!has_canvas_text(&h, "2 of 2 · Wall"));
        assert_eq!(h.app.selected, Some(h.wall));
        h.app.select(None);
        h.frame(vec![]);

        for category in selection_filters::Category::ALL {
            h.app.plans.selection_filters.set_enabled(category, false);
        }
        let filters = &mut h.app.plans.selection_filters;
        filters.set_enabled(selection_filters::Category::Window, true);
        let (window_point, hits) = overlap_point(&h, h.window);
        assert_eq!(hits, vec![h.window, h.wall]);
        assert!(
            h.app
                .plans
                .selection_filters
                .allows(h.app.editor.document.model(), h.window)
        );
        assert!(
            !h.app
                .plans
                .selection_filters
                .allows(h.app.editor.document.model(), h.wall)
        );
        h.press(window_point);
        h.release(window_point);
        assert_eq!(
            h.app.selected,
            Some(h.window),
            "window point {window_point:?}"
        );

        h.app.select(None);
        h.frame(vec![]);
        let (empty_point, _) = overlap_point(&h, h.door);
        h.press(empty_point);
        h.release(empty_point);
        assert_eq!(
            h.app.selected, None,
            "an all-disabled filter makes clicks empty"
        );
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.history_stats(), history);
    }
}

#[test]
fn selection_filters_apply_to_window_and_crossing_marquees_without_mutation() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let model = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        for category in selection_filters::Category::ALL {
            h.app.plans.selection_filters.set_enabled(category, false);
        }
        h.app
            .plans
            .selection_filters
            .set_enabled(selection_filters::Category::Window, true);
        let (left, right) = (h.point(-3.5, 1.), h.point(3.5, -2.5));
        h.area(left, right);
        assert_eq!(h.app.selected_ids, BTreeSet::from([h.window]));

        h.app
            .plans
            .selection_filters
            .set_enabled(selection_filters::Category::Window, false);
        h.app
            .plans
            .selection_filters
            .set_enabled(selection_filters::Category::Door, true);
        h.area(right, left);
        assert_eq!(h.app.selected_ids, BTreeSet::from([h.door]));
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.history_stats(), history);
    }
}

#[test]
fn changing_selection_filters_cancels_cycle_and_marquee_without_pruning_selection() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let model = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let (point, _) = overlap_point(&h, h.door);
        assert_eq!(h.focus_canvas(point), Some(h.door));
        h.frame(vec![tab(false)]);
        assert!(has_canvas_text(&h, "2 of 2 · Wall"));
        h.app
            .plans
            .selection_filters
            .set_enabled(selection_filters::Category::Wall, false);
        h.frame(vec![]);
        assert!(!has_canvas_text(&h, "2 of 2 · Wall"));
        assert_eq!(h.app.selected, Some(h.door));

        h.app.select(None);
        h.frame(vec![]);
        let (a, b) = (h.point(-3.5, 1.), h.point(3.5, -2.5));
        h.press_area(a);
        h.frame(vec![egui::Event::PointerMoved(b)]);
        assert!(h.app.plans.area_selection.draft.is_some());
        h.app
            .plans
            .selection_filters
            .set_enabled(selection_filters::Category::Door, false);
        h.release(b);
        assert!(h.app.plans.area_selection.draft.is_none());
        assert!(h.app.selected_ids.is_empty());
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.history_stats(), history);
    }
}

#[test]
fn selection_filters_survive_plan_switches_and_reset_for_a_new_document_session() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let door = h.door;
    let model = h.app.editor.document.model().clone();
    let old_session = h.app.editor.document.session_id();
    h.app
        .plans
        .selection_filters
        .set_enabled(selection_filters::Category::Door, false);

    let second_view = h
        .app
        .editor
        .create_floor_plan("Second plan", h.app.active_level)
        .unwrap();
    h.app.focus_plan(Some(second_view));
    h.frame(vec![]);
    assert!(
        !h.app
            .plans
            .selection_filters
            .allows(h.app.editor.document.model(), door)
    );

    h.app.editor.document = Document::from_model(model).unwrap();
    assert_ne!(h.app.editor.document.session_id(), old_session);
    h.frame(vec![]);
    assert!(
        h.app
            .plans
            .selection_filters
            .allows(h.app.editor.document.model(), door)
    );
}

#[test]
fn area_desktop_crop_visibility_and_canvas_clipping() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let v = h.app.editor.document.model().views[&h.view]
            .parameters
            .clone();
        let mut settings = v.plan.unwrap();
        settings.crop = Some(os_model::PlanViewCrop {
            min: Point2::new(-0.5, -0.5),
            max: Point2::new(0.5, 0.5),
        });
        h.app
            .editor
            .update_floor_plan(h.view, &v.name, v.level.unwrap(), settings)
            .unwrap();
        h.settle();
        h.area(h.point(-0.6, 0.6), h.point(0.6, -0.6));
        assert_eq!(h.app.selected_ids, BTreeSet::from([h.wall]));
        settings.visibility.walls = false;
        h.app
            .editor
            .update_floor_plan(h.view, &v.name, v.level.unwrap(), settings)
            .unwrap();
        h.settle();
        h.area(h.point(-0.6, 0.6), h.point(0.6, -0.6));
        assert!(h.app.selected_ids.is_empty());
        settings.visibility.walls = true;
        settings.crop = None;
        h.app
            .editor
            .update_floor_plan(h.view, &v.name, v.level.unwrap(), settings)
            .unwrap();
        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(20., 20.));
        let polygons = candidates(
            &h.app.editor,
            drawing,
            context,
            PlanCamera {
                center: Point2::new(0., 0.),
                pixels_per_metre: 65.,
            },
            canvas,
        );
        assert_eq!(
            polygons.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([h.wall])
        );
        assert!(matches_area(&polygons[&h.wall], canvas.min, canvas.max));
    }
}
