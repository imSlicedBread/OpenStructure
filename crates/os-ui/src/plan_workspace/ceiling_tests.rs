//! Real egui frames for native ceiling authoring at the required desktop profiles.
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
            .create_reflected_ceiling_plan("RCP", app.active_level)
            .unwrap();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        Self::from_app(app, view, size, scale)
    }

    fn from_app(mut app: DesktopApp, view: Id, size: egui::Vec2, scale: f32) -> Self {
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        app.plans.snaps.enabled = false;
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

    fn with_room(size: egui::Vec2, scale: f32) -> (Self, Id, Id, Id) {
        let (mut app, room, _, _, partition) = crate::opening_schedule::rooms::tests::fixture();
        app.opening_schedule.open = false;
        let material = crate::opening_schedule::rooms::tests::add_material(&mut app);
        let mut room_parameters = app.editor.document.model().rooms[&room].parameters.clone();
        room_parameters.ceiling_material = Some(material);
        app.editor
            .command(
                "Assign ceiling material",
                Command::UpdateRoom {
                    id: room,
                    parameters: room_parameters,
                },
            )
            .unwrap();
        let view = app
            .editor
            .create_reflected_ceiling_plan("RCP", app.active_level)
            .unwrap();
        app.focus_plan(Some(view));
        let mut harness = Self::from_app(app, view, size, scale);
        harness.app.select(Some(room));
        harness.settle();
        (harness, room, material, partition)
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
        assert!(
            rect.contains(position),
            "fixture point must lie on the canvas"
        );
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

#[test]
fn ceiling_pointer_preview_and_apply_work_at_both_desktop_profiles() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        h.app.begin_ceiling(h.view);
        h.frame(vec![]);
        let boundary = [
            Point2::new(18.0, 19.0),
            Point2::new(22.0, 19.0),
            Point2::new(22.0, 22.0),
            Point2::new(18.0, 22.0),
        ];
        for vertex in boundary {
            h.click(h.point(vertex));
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.revision(), revision);
        }
        h.click(h.point(boundary[0]));
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(
            h.app.plans.ceiling_draft.as_ref().unwrap().phase,
            ceilings::Phase::Ready,
            "points={:?}; status={}",
            h.app.plans.ceiling_draft.as_ref().unwrap().points,
            h.app.status
        );

        h.app.apply_ceiling();
        assert_eq!(h.app.editor.document.model().ceilings.len(), 1);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        let id = *h
            .app
            .editor
            .document
            .model()
            .ceilings
            .keys()
            .next()
            .unwrap();
        let ceiling = &h.app.editor.document.model().ceilings[&id];
        assert_eq!(ceiling.parameters.level, h.app.active_level);
        assert_eq!(ceiling.parameters.elevation_offset, 2.55);
        assert_eq!(ceiling.parameters.thickness, 0.12);
        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert_eq!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .ceilings(context)
                .unwrap()
                .len(),
            1
        );
        assert!(h.app.editor.scene.contains_key(&id));
    }
}

#[test]
fn selected_room_creates_a_preview_only_ceiling_with_one_step_history_at_both_dpis() {
    for (size, scale) in PROFILES {
        let (mut h, room, material, _) = Harness::with_room(size, scale);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let expected_boundary = h
            .app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .rooms(context)
            .unwrap()
            .iter()
            .find(|graphic| graphic.entity == room)
            .unwrap()
            .boundary
            .iter()
            .map(|point| context.basis.plane_to_world(*point).unwrap())
            .collect::<Vec<_>>();
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        let history = h.app.editor.document.history_stats();

        h.app.begin_ceiling_from_selected_room(h.view);
        let draft = h.app.plans.ceiling_draft.as_ref().unwrap();
        assert_eq!(draft.phase, ceilings::Phase::Ready);
        assert_eq!(draft.parameters.boundary, expected_boundary);
        assert_eq!(draft.parameters.level, h.app.active_level);
        assert_eq!(draft.parameters.material, Some(material));
        assert_eq!(draft.parameters.boundary_room, Some(room));
        assert_eq!(draft.parameters.name, "Ceiling · Room 1");
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);

        let ceiling_id = draft.id;
        h.app.apply_ceiling();
        let ceiling = &h.app.editor.document.model().ceilings[&ceiling_id];
        assert_eq!(ceiling.parameters.boundary, expected_boundary);
        assert_eq!(ceiling.parameters.level, h.app.active_level);
        assert_eq!(ceiling.parameters.material, Some(material));
        assert_eq!(ceiling.parameters.boundary_room, Some(room));
        assert_eq!(ceiling.parameters.thickness, 0.12);
        assert_eq!(ceiling.parameters.elevation_offset, 2.55);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert!(h.app.editor.document.undo());
        assert!(
            !h.app
                .editor
                .document
                .model()
                .ceilings
                .contains_key(&ceiling_id)
        );
        assert!(h.app.editor.document.redo());
        assert!(
            h.app
                .editor
                .document
                .model()
                .ceilings
                .contains_key(&ceiling_id)
        );
    }
}

#[test]
fn room_backed_ceiling_tracks_boundary_in_plan_section_and_scene_then_detaches() {
    let (mut h, room, _, partition) = Harness::with_room(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_ceiling_from_selected_room(h.view);
    let ceiling_id = h.app.plans.ceiling_draft.as_ref().unwrap().id;
    h.app.apply_ceiling();
    h.settle();

    let level = h.app.active_level;
    let section = h
        .app
        .editor
        .create_section_view(
            "Ceiling section",
            level,
            os_model::SectionViewSettings::new(
                Point2::new(0.0, 1.5),
                Point2::new(4.0, 1.5),
                0.0,
                3.0,
            ),
        )
        .unwrap();
    let section_lines = |app: &DesktopApp| {
        let context = app.editor.native_section_context(section).unwrap();
        app.editor
            .native_drawing(section)
            .unwrap()
            .provider_lines(context)
            .unwrap()
            .iter()
            .filter(|line| line.entity == ceiling_id)
            .map(|line| line.start.x.max(line.end.x))
            .fold(None, |max, x| Some(max.map_or(x, |max: f64| max.max(x))))
    };
    assert!((section_lines(&h.app).unwrap() - 4.0).abs() < 1e-8);

    let mut partition_parameters = h.app.editor.document.model().room_separation_lines[&partition]
        .parameters
        .clone();
    partition_parameters.start.x = 3.0;
    partition_parameters.end.x = 3.0;
    h.app
        .editor
        .command(
            "Move room boundary",
            Command::UpdateRoomSeparationLine {
                id: partition,
                parameters: partition_parameters,
            },
        )
        .unwrap();
    h.settle();

    let plan_context = h.app.editor.native_plan_context(h.view).unwrap();
    let plan_drawing = h.app.editor.native_drawing(h.view).unwrap();
    let plan_ceiling = plan_drawing
        .ceilings(plan_context)
        .unwrap()
        .iter()
        .find(|item| item.entity == ceiling_id)
        .unwrap();
    assert!(
        (plan_ceiling
            .boundary
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max)
            - 3.0)
            .abs()
            < 1e-8
    );
    assert!((section_lines(&h.app).unwrap() - 3.0).abs() < 1e-8);
    assert!(
        (h.app.editor.scene[&ceiling_id]
            .vertices
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max)
            - 3.0)
            .abs()
            < 1e-8
    );

    h.app
        .editor
        .command(
            "Break room boundary",
            Command::RemoveRoomSeparationLine(partition),
        )
        .unwrap();
    h.settle();
    assert!(!h.app.editor.scene.contains_key(&ceiling_id));
    assert!(
        h.app
            .editor
            .native_drawing(h.view)
            .unwrap()
            .ceilings(h.app.editor.native_plan_context(h.view).unwrap())
            .unwrap()
            .iter()
            .all(|item| item.entity != ceiling_id)
    );
    assert_eq!(section_lines(&h.app), None);

    let cached_boundary = h.app.editor.document.model().ceilings[&ceiling_id]
        .parameters
        .boundary
        .clone();
    h.app.select(Some(ceiling_id));
    h.frame(vec![]);
    h.app.detach_ceiling_boundary(ceiling_id);
    let draft = h.app.plans.ceiling_draft.as_ref().unwrap();
    assert_eq!(draft.parameters.boundary_room, None);
    assert_eq!(draft.parameters.boundary, cached_boundary);
    assert!(
        h.app.editor.document.model().ceilings[&ceiling_id]
            .parameters
            .boundary_room
            .is_some()
    );
    h.app.apply_ceiling();
    assert!(h.app.editor.scene.contains_key(&ceiling_id));
    assert_eq!(
        h.app.editor.document.model().ceilings[&ceiling_id]
            .parameters
            .boundary_room,
        None
    );
    assert_eq!(
        h.app.editor.document.model().rooms[&room].parameters.name,
        "Room 1"
    );
}

#[test]
fn linked_ceiling_openings_validate_live_boundary_and_detach_current_room_outline() {
    let (mut h, _, _, partition) = Harness::with_room(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_ceiling_from_selected_room(h.view);
    let ceiling_id = h.app.plans.ceiling_draft.as_ref().unwrap().id;
    h.app.apply_ceiling();
    h.settle();

    let mut partition_parameters = h.app.editor.document.model().room_separation_lines[&partition]
        .parameters
        .clone();
    partition_parameters.start.x = 3.0;
    partition_parameters.end.x = 3.0;
    h.app
        .editor
        .command(
            "Move room boundary",
            Command::UpdateRoomSeparationLine {
                id: partition,
                parameters: partition_parameters,
            },
        )
        .unwrap();
    h.settle();

    h.app.select(Some(ceiling_id));
    h.frame(vec![]);
    let draft = h.app.plans.ceiling_draft.as_mut().unwrap();
    draft.parameters.holes = vec![vec![
        Point2::new(3.2, 1.0),
        Point2::new(3.7, 1.0),
        Point2::new(3.7, 2.0),
        Point2::new(3.2, 2.0),
    ]];
    let revision = h.app.editor.document.revision();
    h.app.apply_ceiling();
    assert_eq!(h.app.editor.document.revision(), revision);
    assert!(h.app.status_error);
    assert!(h.app.plans.ceiling_draft.is_some());

    h.app.detach_ceiling_boundary(ceiling_id);
    let draft = h.app.plans.ceiling_draft.as_ref().unwrap();
    assert_eq!(draft.parameters.boundary_room, None);
    assert!(
        (draft
            .parameters
            .boundary
            .iter()
            .map(|p| p.x)
            .fold(0.0, f64::max)
            - 3.0)
            .abs()
            < 1e-8
    );
    assert_eq!(draft.parameters.holes.len(), 1);

    h.app
        .plans
        .ceiling_draft
        .as_mut()
        .unwrap()
        .parameters
        .holes
        .clear();
    h.app.apply_ceiling();
    assert!(h.app.editor.scene.contains_key(&ceiling_id));
    assert_eq!(
        h.app.editor.document.model().ceilings[&ceiling_id]
            .parameters
            .boundary_room,
        None
    );
}

#[test]
fn room_ceiling_creation_rejects_unresolved_room_and_cancels_on_stale_document() {
    let (mut h, room, _, partition) = Harness::with_room(PROFILES[0].0, PROFILES[0].1);
    h.app
        .editor
        .command(
            "Remove room partition",
            Command::RemoveRoomSeparationLine(partition),
        )
        .unwrap();
    h.settle();
    h.app.select(Some(room));
    h.app.begin_ceiling_from_selected_room(h.view);
    assert!(h.app.plans.ceiling_draft.is_none());
    assert!(h.app.status_error);

    let (mut h, _, _, _) = Harness::with_room(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_ceiling_from_selected_room(h.view);
    assert!(h.app.plans.ceiling_draft.is_some());
    crate::opening_schedule::rooms::tests::add_material(&mut h.app);
    h.app.validate_ceiling_interaction(&h.ctx);
    assert!(h.app.plans.ceiling_draft.is_none());

    let (mut h, room, _, _) = Harness::with_room(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_ceiling_from_selected_room(h.view);
    let ceiling = h.app.plans.ceiling_draft.as_ref().unwrap().id;
    h.app.apply_ceiling();
    assert!(h.app.editor.scene.contains_key(&ceiling));
    h.app
        .editor
        .command("Delete source room", Command::RemoveRoom(room))
        .unwrap();
    assert!(!h.app.editor.scene.contains_key(&ceiling));
    assert!(
        h.app.editor.document.model().ceilings[&ceiling]
            .parameters
            .boundary_room
            .is_some()
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unresolved-room-ceiling.osb");
    h.app.editor.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert!(!reopened.scene.contains_key(&ceiling));
    assert_eq!(
        reopened.document.model().ceilings[&ceiling]
            .parameters
            .boundary_room,
        Some(room)
    );
}
