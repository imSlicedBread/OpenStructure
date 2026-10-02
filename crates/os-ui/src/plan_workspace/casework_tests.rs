//! Headless egui acceptance for native casework types and instances.
use super::*;
use os_model::{Casework, CaseworkParams, CaseworkType, CaseworkTypeParams};

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
            .create_floor_plan("Casework plan", app.active_level)
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
        assert!(rect.contains(position));
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
                assert!(self.app.casework_properties(ui));
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
fn place_shared_casework_edit_type_and_sync_plan_scene_history_and_storage_at_both_profiles() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.plans.split = true;
        h.frame(vec![]);
        let original = h.app.editor.document.model().clone();
        let original_revision = h.app.editor.document.revision();

        let first_center = Point2::new(1.0, 2.0);
        h.app.begin_casework(h.view);
        h.frame(vec![]);
        let output = h.hover(h.point(first_center));
        assert!(!output.shapes.is_empty(), "preview is drawn on the plan");
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.revision(), original_revision);
        assert!(h.app.editor.scene.is_empty(), "preview does not mutate 3D");

        h.click(h.point(first_center));
        let first_id = h
            .app
            .selected
            .unwrap_or_else(|| panic!("placed casework remains selected: {}", h.app.status));
        let first = h.app.editor.document.model().casework[&first_id].clone();
        assert_eq!(first.header.type_id, "core.casework");
        assert_eq!(first.parameters.center, first_center);
        assert_eq!(first.parameters.level, h.app.active_level);
        let type_id = first.parameters.type_id;
        let original_type = h.app.editor.document.model().casework_types[&type_id].clone();
        assert_eq!(original_type.parameters.width, 0.6);
        assert_eq!(original_type.parameters.depth, 0.6);
        assert_eq!(original_type.parameters.height, 0.9);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        h.settle();
        assert!(
            h.app.editor.scene.contains_key(&first_id),
            "committed casework missing from scene: {:?}",
            h.app.editor.scene.keys().collect::<Vec<_>>()
        );
        assert!((h.app.editor.scene[&first_id].signed_volume() - 0.6 * 0.6 * 0.9).abs() < 1e-10);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert_eq!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .casework(context)
                .unwrap()
                .len(),
            1
        );

        let second_center = Point2::new(3.0, 2.0);
        h.app.begin_casework(h.view);
        h.frame(vec![]);
        h.click(h.point(second_center));
        let second_id = h.app.selected.expect("second instance selected");
        let second = h.app.editor.document.model().casework[&second_id].clone();
        assert_eq!(
            h.app.editor.document.model().casework[&second_id]
                .parameters
                .type_id,
            type_id,
            "repeat placement reuses the selected instance's type"
        );
        assert_eq!(h.app.editor.document.model().casework_types.len(), 1);
        h.settle();

        h.app.select(Some(second_id));
        assert!(!h.properties_frame().shapes.is_empty());
        h.app
            .plans
            .casework_edit
            .as_mut()
            .unwrap()
            .type_parameters
            .width = 0.8;
        h.app.plans.casework_edit.as_mut().unwrap().parameters.yaw = std::f64::consts::FRAC_PI_2;
        h.app.apply_casework_properties();
        assert_eq!(
            h.app.editor.document.model().casework[&second_id].header,
            second.header
        );
        assert_eq!(
            h.app.editor.document.model().casework_types[&type_id]
                .parameters
                .width,
            0.8
        );
        assert_eq!(
            h.app.editor.document.model().casework[&first_id]
                .parameters
                .type_id,
            type_id
        );
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 3);
        h.settle();
        assert!((h.app.editor.scene[&first_id].signed_volume() - 0.8 * 0.6 * 0.9).abs() < 1e-10);
        assert!((h.app.editor.scene[&second_id].signed_volume() - 0.8 * 0.6 * 0.9).abs() < 1e-10);

        h.app.history(false);
        h.settle();
        assert_eq!(
            h.app.editor.document.model().casework_types[&type_id],
            original_type
        );
        assert!((h.app.editor.scene[&first_id].signed_volume() - 0.6 * 0.6 * 0.9).abs() < 1e-10);
        h.app.history(true);
        h.settle();
        assert_eq!(
            h.app.editor.document.model().casework_types[&type_id]
                .parameters
                .width,
            0.8
        );

        let committed = h.app.editor.document.model().clone();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("casework.osb");
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert!(h.app.editor.scene.contains_key(&first_id));
        assert!(h.app.editor.scene.contains_key(&second_id));
        assert!((h.app.editor.scene[&first_id].signed_volume() - 0.8 * 0.6 * 0.9).abs() < 1e-10);
    }
}

#[test]
fn escape_stale_context_and_drag_claim_cancel_casework_without_model_or_pan_changes() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let original = h.app.editor.document.model().clone();
    let camera_center = h.app.plans.cameras[&h.view].center;
    h.app.begin_casework(h.view);
    h.frame(vec![]);
    let target = h.point(Point2::new(1.0, 1.0));
    h.frame(vec![button(target, true)]);
    let drag_to = target + egui::vec2(48.0, 0.0);
    h.frame(vec![egui::Event::PointerMoved(drag_to)]);
    h.frame(vec![button(drag_to, false)]);
    assert_eq!(h.app.plans.cameras[&h.view].center, camera_center);
    assert!(h.app.editor.document.model().casework.is_empty());
    h.frame(vec![escape()]);
    assert!(h.app.plans.casework_placement.is_none());
    assert_eq!(h.app.editor.document.model(), &original);

    h.app.begin_casework(h.view);
    let external_type = CaseworkType::new(
        "core.casework_type",
        CaseworkTypeParams {
            name: "Concurrent cabinet".into(),
            width: 0.5,
            depth: 0.5,
            height: 0.8,
            material: None,
        },
    );
    h.app
        .editor
        .command(
            "External casework edit",
            Command::AddCaseworkType(external_type),
        )
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.casework_placement.is_none());
    assert!(h.app.editor.document.model().casework.is_empty());
}

#[test]
fn casework_participates_in_its_own_selection_filter_category() {
    let mut model = Model::new("Casework filter");
    let level = *model.levels.keys().next().unwrap();
    let casework_type = CaseworkType::new(
        "core.casework_type",
        CaseworkTypeParams {
            name: "Cabinet".into(),
            width: 0.6,
            depth: 0.6,
            height: 0.9,
            material: None,
        },
    );
    let type_id = casework_type.id();
    model.casework_types.insert(type_id, casework_type);
    let casework = Casework::new(
        "core.casework",
        CaseworkParams {
            name: "Cabinet 1".into(),
            type_id,
            level,
            center: Point2::new(1.0, 2.0),
            yaw: 0.0,
            base_offset: 0.0,
        },
    );
    let id = casework.id();
    model.casework.insert(id, casework);
    model.validate().unwrap();

    assert_eq!(
        selection_filters::Category::classify(&model, id),
        selection_filters::Category::Casework
    );
    let mut filters = selection_filters::SelectionFilters::default();
    assert!(filters.allows(&model, id));
    assert!(filters.set_enabled(selection_filters::Category::Casework, false));
    assert!(!filters.allows(&model, id));
}

#[test]
fn starting_casework_cancels_a_ceiling_draft_and_claims_plan_input() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let reflected = h
        .app
        .editor
        .create_reflected_ceiling_plan("Ceiling plan", h.app.active_level)
        .unwrap();
    h.app.focus_plan(Some(reflected));
    h.settle();

    h.app.begin_ceiling(reflected);
    assert!(h.app.plans.ceiling_draft.is_some());
    h.app.begin_casework(reflected);

    assert!(h.app.plans.ceiling_draft.is_none());
    assert!(h.app.plans.casework_placement.is_some());
}
