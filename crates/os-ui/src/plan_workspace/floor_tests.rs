//! Real egui frames for native floor boundary authoring; no native-window claim.
use super::*;
use os_model::{Floor, FloorParams, Wall};
#[path = "floor_properties_tests.rs"]
mod floor_properties_tests;

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
    output: egui::FullOutput,
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
            output: egui::FullOutput::default(),
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
        self.frame(vec![egui::Event::PointerMoved(to), button(to, false)]);
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
            path: os_model::WallPath::Straight {
                start: Point2::new(-3.0, 0.0),
                end: Point2::new(3.0, 0.0),
            },
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

    #[cfg(feature = "external-plugins")]
    {
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
}

#[test]
fn floor_vertices_use_the_active_plan_endpoint_snaps() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Snap target".into(),
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

fn add_floor(harness: &mut Harness, boundary: Vec<Point2>) -> Id {
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Editable slab".into(),
            level: harness.app.active_level,
            material: None,
            boundary,
            holes: Vec::new(),
            thickness: 0.27,
            top_offset: 0.12,
        },
    );
    let id = floor.id();
    harness
        .app
        .editor
        .command("Floor edit fixture", Command::AddFloor(floor))
        .unwrap();
    harness.settle();
    harness
        .app
        .plans
        .cameras
        .get_mut(&harness.view)
        .unwrap()
        .center = Point2::new(0.0, 0.0);
    harness
        .app
        .plans
        .cameras
        .get_mut(&harness.view)
        .unwrap()
        .pixels_per_metre = 30.0;
    harness.app.plans.snaps.enabled = false;
    harness.app.select(Some(id));
    harness.frame(vec![]);
    id
}

#[test]
fn floor_drag_contexts_contain_original_params_until_one_commit_and_history_step() {
    for (profile_index, (size, scale)) in PROFILES.into_iter().enumerate() {
        let mut h = Harness::new(size, scale);
        h.app.plans.split = true;
        let boundary = if profile_index == 0 {
            vec![
                Point2::new(-2.0, -2.0),
                Point2::new(2.0, -2.0),
                Point2::new(2.0, 2.0),
                Point2::new(-2.0, 2.0),
            ]
        } else {
            vec![
                Point2::new(-2.0, -2.0),
                Point2::new(2.0, -2.0),
                Point2::new(0.5, 0.0),
                Point2::new(2.0, 2.0),
                Point2::new(-2.0, 2.0),
            ]
        };
        let floor_id = add_floor(&mut h, boundary);
        let original_model = h.app.editor.document.model().clone();
        let original_scene = h.app.editor.scene.clone();
        let original = original_model.floors[&floor_id].parameters.clone();
        let revision = h.app.editor.document.revision();
        let undo_entries = h.app.editor.document.history_stats().undo_entries;
        let index = if profile_index == 0 { 0 } else { 2 };
        let destination = if index == 0 {
            Point2::new(-3.0, -2.5)
        } else {
            Point2::new(0.75, 0.25)
        };
        let from = h.point(original.boundary[index]);
        let to = h.point(destination);
        let camera_center = h.app.plans.cameras[&h.view].center;

        h.hover(from);
        h.frame(vec![button(from, true)]);
        assert_eq!(
            h.app
                .plans
                .floor_vertex_drag
                .as_ref()
                .map(|draft| draft.vertex_index),
            Some(index)
        );
        h.frame(vec![egui::Event::PointerMoved(to)]);
        assert_eq!(h.app.editor.document.model(), &original_model);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            undo_entries
        );
        assert_eq!(h.app.editor.scene, original_scene);
        assert_eq!(h.app.plans.cameras[&h.view].center, camera_center);

        let expected_world = h
            .app
            .editor
            .native_plan_context(h.view)
            .unwrap()
            .basis
            .plane_to_world(destination)
            .unwrap();
        h.frame(vec![egui::Event::PointerMoved(to), button(to, false)]);
        let committed = h.app.editor.document.model().clone();
        let committed_parameters = &committed.floors[&floor_id].parameters;
        assert_eq!(committed_parameters.name, original.name);
        assert_eq!(committed_parameters.level, original.level);
        assert_eq!(committed_parameters.material, original.material);
        assert_eq!(committed_parameters.thickness, original.thickness);
        assert_eq!(committed_parameters.top_offset, original.top_offset);
        assert!(committed_parameters.boundary[index].distance(expected_world) < 1e-8);
        assert_eq!(committed_parameters.boundary.len(), original.boundary.len());
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);
        assert_ne!(h.app.editor.scene, original_scene);
        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let projected_floor = h
            .app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .floors(context)
            .unwrap()
            .iter()
            .find(|floor| floor.entity == floor_id)
            .unwrap();
        assert!(projected_floor.boundary[index].distance(destination) < 1e-8);

        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &original_model);
        assert_eq!(h.app.editor.scene, original_scene);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert_ne!(h.app.editor.scene, original_scene);
    }
}

#[test]
fn floor_vertex_escape_invalid_release_stale_context_and_canvas_pan_are_safe() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let floor_id = add_floor(
        &mut h,
        vec![
            Point2::new(-2.0, -2.0),
            Point2::new(2.0, -2.0),
            Point2::new(2.0, 2.0),
            Point2::new(-2.0, 2.0),
        ],
    );
    let original = h.app.editor.document.model().clone();
    let original_floor = original.floors[&floor_id].parameters.clone();
    let start = h.point(original_floor.boundary[0]);
    let target = h.point(Point2::new(-3.0, -2.5));
    h.hover(start);
    h.frame(vec![button(start, true)]);
    h.frame(vec![egui::Event::PointerMoved(target)]);
    h.frame(vec![escape()]);
    assert!(h.app.plans.floor_vertex_drag.is_none());
    h.frame(vec![
        egui::Event::PointerMoved(target),
        button(target, false),
    ]);
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);

    // A vertex cannot collapse onto its neighbor; the invalid draft is canceled.
    let collapse_target = h.point(original_floor.boundary[1]);
    h.drag(start, collapse_target);
    assert_eq!(h.app.editor.document.model(), &original);
    assert!(h.app.status_error);
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);

    // A document revision change invalidates the draft before release.
    h.hover(start);
    h.frame(vec![button(start, true)]);
    h.frame(vec![egui::Event::PointerMoved(target)]);
    let mut concurrent = original_floor.clone();
    concurrent.name = "Concurrent slab edit".into();
    h.app
        .editor
        .command(
            "Concurrent floor edit",
            Command::UpdateFloor {
                id: floor_id,
                parameters: concurrent.clone(),
            },
        )
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.floor_vertex_drag.is_none());
    h.frame(vec![
        egui::Event::PointerMoved(target),
        button(target, false),
    ]);
    assert_eq!(
        h.app.editor.document.model().floors[&floor_id].parameters,
        concurrent
    );
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);

    // A drag that starts on floor body, away from a handle, remains canvas pan.
    // The concurrent edit above invalidates the asynchronous plan drawing.
    // Wait for the current drawing before sending input to its canvas; a fixed
    // number of empty frames races worker completion under the full test load.
    h.settle();
    let center = h.app.plans.cameras[&h.view].center;
    let body = h.point(Point2::new(0.0, 0.0));
    let moved = body + egui::vec2(30.0, 12.0);
    h.hover(body);
    h.frame(vec![button(body, true)]);
    h.frame(vec![egui::Event::PointerMoved(moved)]);
    assert_ne!(h.app.plans.cameras[&h.view].center, center);
    h.frame(vec![egui::Event::PointerMoved(moved), button(moved, false)]);
    assert_eq!(
        h.app.editor.document.model().floors[&floor_id].parameters,
        concurrent
    );
}

#[test]
fn floor_vertex_projection_and_hit_testing_respect_crop_and_logical_radius() {
    let mut h = Harness::new(PROFILES[1].0, PROFILES[1].1);
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(0.0, 0.0);
    h.frame(vec![]);
    let mut context = h.app.editor.native_plan_context(h.view).unwrap();
    context.crop = Some(os_geometry::plan::PlanCrop {
        min: Point2::new(-1.0, -1.0),
        max: Point2::new(1.0, 1.0),
    });
    let boundary = vec![
        Point2::new(-0.5, -0.5),
        Point2::new(2.0, -0.5),
        Point2::new(2.0, 2.0),
        Point2::new(-0.5, 2.0),
    ];
    let holes = vec![vec![
        Point2::new(0.0, 0.0),
        Point2::new(0.5, 0.0),
        Point2::new(0.5, 0.5),
        Point2::new(0.0, 0.5),
    ]];
    let vertices = boundary.iter().chain(holes[0].iter()).copied().collect();
    let floor = PlanFloorItem {
        entity: Id::new(),
        boundary: boundary.clone(),
        holes,
        vertices,
        triangles: vec![],
        area_m2: 1.0,
    };
    let rect = h.app.plans.canvas_rect.unwrap();
    let handles = floor_vertex_handles(&floor, context, h.app.plans.cameras[&h.view], rect);
    assert_eq!(
        handles
            .iter()
            .map(|(ring, index, _)| (*ring, *index))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 0), (1, 1), (1, 2), (1, 3)]
    );
    let (ring, index, point) = handles[1];
    assert_eq!((ring, index), (1, 0));
    assert_eq!(
        hit_floor_vertex(&handles, point + egui::vec2(6.0, 8.0)),
        Some((1, 0, 100.0))
    );
    assert!(hit_floor_vertex(&handles, handles[0].2 + egui::vec2(10.1, 0.0)).is_none());
}

#[test]
fn slab_opening_vertices_drag_and_preserve_other_ring_and_floor_parameters() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let floor_id = add_floor(
            &mut h,
            vec![
                Point2::new(-4.0, -4.0),
                Point2::new(4.0, -4.0),
                Point2::new(4.0, 4.0),
                Point2::new(-4.0, 4.0),
            ],
        );
        let mut parameters = h.app.editor.document.model().floors[&floor_id]
            .parameters
            .clone();
        parameters.holes.push(vec![
            Point2::new(-1.0, -1.0),
            Point2::new(1.0, -1.0),
            Point2::new(1.0, 1.0),
            Point2::new(-1.0, 1.0),
        ]);
        h.app
            .editor
            .command(
                "Opening vertex fixture",
                Command::UpdateFloor {
                    id: floor_id,
                    parameters,
                },
            )
            .unwrap();
        h.settle();
        let original_model = h.app.editor.document.model().clone();
        let original = original_model.floors[&floor_id].parameters.clone();
        let origin = h.point(original.holes[0][0]);
        let destination = Point2::new(-1.5, -1.25);
        let target = h.point(destination);

        h.hover(origin);
        h.frame(vec![button(origin, true)]);
        assert_eq!(
            h.app
                .plans
                .floor_vertex_drag
                .as_ref()
                .map(|draft| (draft.ring_index, draft.vertex_index)),
            Some((1, 0))
        );
        h.frame(vec![egui::Event::PointerMoved(target)]);
        assert_eq!(h.app.editor.document.model(), &original_model);
        h.frame(vec![
            egui::Event::PointerMoved(target),
            button(target, false),
        ]);
        let committed = h.app.editor.document.model().clone();
        let edited = &committed.floors[&floor_id].parameters;
        assert_eq!(edited.boundary, original.boundary);
        assert_eq!(edited.holes[0][1..], original.holes[0][1..]);
        assert!(edited.holes[0][0].distance(destination) < 1e-8);
        assert_eq!(edited.thickness, original.thickness);
        assert_eq!(edited.top_offset, original.top_offset);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 3);
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &original_model);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
    }
}

#[test]
fn slab_opening_sketch_is_preview_only_then_one_undoable_2d_3d_transaction() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.plans.split = true;
        let floor_id = add_floor(
            &mut h,
            vec![
                Point2::new(-4.0, -4.0),
                Point2::new(4.0, -4.0),
                Point2::new(4.0, 4.0),
                Point2::new(-4.0, 4.0),
            ],
        );
        let original_model = h.app.editor.document.model().clone();
        let original_scene = h.app.editor.scene.clone();
        let original_params = original_model.floors[&floor_id].parameters.clone();
        let revision = h.app.editor.document.revision();
        let history = h.app.editor.document.history_stats().undo_entries;

        h.app.begin_floor_hole_sketch(h.view, floor_id);
        assert!(h.app.plans.floor_hole_sketch.is_some());
        h.frame(vec![]);
        for (expected_count, point) in [
            Point2::new(-1.0, -1.0),
            Point2::new(1.0, -1.0),
            Point2::new(1.0, 1.0),
            Point2::new(-1.0, 1.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, point)| (index + 1, point))
        {
            h.click(h.point(point));
            assert_eq!(
                h.app
                    .plans
                    .floor_hole_sketch
                    .as_ref()
                    .map(|draft| draft.points.len()),
                Some(expected_count),
                "{}",
                h.app.status
            );
            assert_eq!(h.app.editor.document.model(), &original_model);
            assert_eq!(h.app.editor.document.revision(), revision);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, history);
            assert_eq!(h.app.editor.scene, original_scene);
        }
        let close_position = h.point(Point2::new(-1.0, -1.0));
        let rect = h.app.plans.canvas_rect.unwrap();
        let first = h.app.plans.floor_hole_sketch.as_ref().unwrap().points[0];
        let expected_screen = h.app.plans.cameras[&h.view]
            .project(first, [f64::from(rect.width()), f64::from(rect.height())])
            .unwrap();
        assert!(
            expected_screen.distance(Point2::new(
                f64::from(close_position.x - rect.left()),
                f64::from(close_position.y - rect.top()),
            )) <= 0.01,
            "first {:?}, projected {:?}, pointer {:?}",
            first,
            expected_screen,
            close_position
        );
        h.click(close_position);
        assert!(
            h.app.plans.floor_hole_sketch.is_none(),
            "opening close did not commit: {} · points {:?}",
            h.app.status,
            h.app
                .plans
                .floor_hole_sketch
                .as_ref()
                .map(|draft| draft.points.len())
        );

        let committed = h.app.editor.document.model().clone();
        let parameters = &committed.floors[&floor_id].parameters;
        assert_eq!(parameters.name, original_params.name);
        assert_eq!(parameters.level, original_params.level);
        assert_eq!(parameters.material, original_params.material);
        assert_eq!(parameters.thickness, original_params.thickness);
        assert_eq!(parameters.top_offset, original_params.top_offset);
        assert_eq!(parameters.holes.len(), 1);
        assert_eq!(parameters.area(), 60.0);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history + 1
        );
        assert_ne!(h.app.editor.scene, original_scene);
        assert!((h.app.editor.scene[&floor_id].signed_volume() - 16.2).abs() < 1e-8);

        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        assert_eq!(
            drawing.pick_floor(context, Point2::new(0.0, 0.0)).unwrap(),
            None
        );
        assert_eq!(
            drawing.pick_floor(context, Point2::new(2.0, 0.0)).unwrap(),
            Some(floor_id)
        );

        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &original_model);
        assert_eq!(h.app.editor.scene, original_scene);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
    }
}

#[test]
fn slab_opening_escape_invalid_finish_and_removal_preserve_transaction_boundaries() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let floor_id = add_floor(
        &mut h,
        vec![
            Point2::new(-4.0, -4.0),
            Point2::new(4.0, -4.0),
            Point2::new(4.0, 4.0),
            Point2::new(-4.0, 4.0),
        ],
    );
    let original = h.app.editor.document.model().clone();
    h.app.begin_floor_hole_sketch(h.view, floor_id);
    h.app.plans.floor_hole_sketch.as_mut().unwrap().points = vec![
        Point2::new(-1.0, -1.0),
        Point2::new(1.0, 1.0),
        Point2::new(-1.0, 1.0),
        Point2::new(1.0, -1.0),
    ];
    h.app.finish_floor_hole_sketch();
    assert!(h.app.status_error);
    assert!(h.app.plans.floor_hole_sketch.is_some());
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
    h.frame(vec![escape()]);
    assert!(h.app.plans.floor_hole_sketch.is_none());
    assert_eq!(h.app.editor.document.model(), &original);

    let mut parameters = original.floors[&floor_id].parameters.clone();
    parameters.holes.push(vec![
        Point2::new(-1.0, -1.0),
        Point2::new(1.0, -1.0),
        Point2::new(1.0, 1.0),
        Point2::new(-1.0, 1.0),
    ]);
    h.app
        .editor
        .command(
            "Opening removal fixture",
            Command::UpdateFloor {
                id: floor_id,
                parameters,
            },
        )
        .unwrap();
    h.settle();
    h.app.remove_floor_hole(floor_id, 0);
    assert!(
        h.app.editor.document.model().floors[&floor_id]
            .parameters
            .holes
            .is_empty()
    );
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 3);
}

#[test]
fn floor_vertex_drag_snaps_to_a_different_native_wall() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let floor_id = add_floor(
        &mut h,
        vec![
            Point2::new(-2.0, -2.0),
            Point2::new(2.0, -2.0),
            Point2::new(2.0, 2.0),
            Point2::new(-2.0, 2.0),
        ],
    );
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Vertex snap target".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(4.0, 0.0),
                end: Point2::new(4.0, 2.0),
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
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(0.0, 0.0);
    h.app
        .plans
        .cameras
        .get_mut(&h.view)
        .unwrap()
        .pixels_per_metre = 40.0;
    h.app.plans.snaps = SnapOptions::default();
    h.app.select(Some(floor_id));
    h.frame(vec![]);
    let from = h.point(Point2::new(2.0, 2.0));
    let to = h.point(Point2::new(3.95, 2.02));
    h.drag(from, to);
    assert_eq!(
        h.app.editor.document.model().floors[&floor_id]
            .parameters
            .boundary[2],
        Point2::new(4.0, 2.0)
    );
}

#[cfg(feature = "external-plugins")]
#[test]
fn floor_vertex_drag_is_revoked_when_a_provider_unloads() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let floor_id = add_floor(
        &mut h,
        vec![
            Point2::new(-2.0, -2.0),
            Point2::new(2.0, -2.0),
            Point2::new(2.0, 2.0),
            Point2::new(-2.0, 2.0),
        ],
    );
    let original = h.app.editor.document.model().floors[&floor_id]
        .parameters
        .clone();
    let from = h.point(original.boundary[0]);
    let to = h.point(Point2::new(-3.0, -2.5));
    h.hover(from);
    h.frame(vec![button(from, true)]);
    h.frame(vec![egui::Event::PointerMoved(to)]);
    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.floor_vertex_drag.is_none());
    h.frame(vec![egui::Event::PointerMoved(to), button(to, false)]);
    assert_eq!(
        h.app.editor.document.model().floors[&floor_id].parameters,
        original
    );
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
}

#[cfg(feature = "external-plugins")]
#[test]
fn slab_opening_sketch_is_revoked_when_a_provider_unloads() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let floor_id = add_floor(
        &mut h,
        vec![
            Point2::new(-4.0, -4.0),
            Point2::new(4.0, -4.0),
            Point2::new(4.0, 4.0),
            Point2::new(-4.0, 4.0),
        ],
    );
    let original = h.app.editor.document.model().clone();
    h.app.begin_floor_hole_sketch(h.view, floor_id);
    h.frame(vec![]);
    assert!(h.app.plans.floor_hole_sketch.is_some());
    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.floor_hole_sketch.is_none());
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
}
