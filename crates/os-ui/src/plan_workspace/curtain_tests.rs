//! egui acceptance tests for native curtain plan placement.
use super::*;
use os_model::{Wall, WallParams, WallPath};
use os_storage::{StorageBackend, ZipJsonStorage};

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
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let view = app
            .editor
            .create_floor_plan("Curtain plan", app.active_level)
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
            shapes: Vec::new(),
        };
        harness.settle();
        let camera = harness.app.plans.cameras.get_mut(&view).unwrap();
        camera.center = Point2::new(20.0, 20.0);
        camera.pixels_per_metre = 30.0;
        harness.app.plans.snaps.enabled = false;
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
        self.shapes = self.ctx.run(input, |ctx| self.app.show(ctx)).shapes;
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

    fn click_modal_button(&mut self, label: &str) {
        self.frame(vec![]);
        let position = self
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    let size = text.galley.size();
                    let position = if label.starts_with("Add ") {
                        text.pos + egui::vec2(size.x / 2.0, -size.y - 1.5)
                    } else {
                        text.pos + size * 0.5
                    };
                    Some(position)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing modal button: {label}"));
        self.click(position);
        self.frame(vec![]);
    }

    fn click_remove_vertical_grid(&mut self) {
        self.frame(vec![]);
        let response_rect = |h: &Harness| {
            h.app
                .plans
                .curtain_properties
                .as_ref()
                .and_then(|draft| draft.remove_vertical_rect)
                .expect("visible remove-grid response")
        };
        let mut position = response_rect(self).center();
        for _ in 0..3 {
            self.hover(position);
            let next = response_rect(self).center();
            if next.distance(position) < 0.5 {
                position = next;
                break;
            }
            position = next;
        }
        self.frame(vec![button(position, true)]);
        self.frame(vec![button(position, false)]);
        self.frame(vec![]);
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

fn place_test_curtain(h: &mut Harness) -> Id {
    h.app.begin_curtain(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(18.0, 20.0)));
    h.click(h.point(Point2::new(24.0, 20.0)));
    h.settle();
    h.app.selected.expect("placed curtain selected")
}

#[test]
fn curtain_placement_is_preview_only_then_one_undoable_plan_pickable_commit() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let original_model = h.app.editor.document.model().clone();
        let original_scene = h.app.editor.scene.clone();
        h.app.begin_curtain(h.view);
        h.frame(vec![]);

        let start = Point2::new(18.0, 20.0);
        let end = Point2::new(24.0, 20.0);
        h.click(h.point(start));
        assert_eq!(h.app.editor.document.model(), &original_model);
        assert_eq!(h.app.editor.document.revision(), 0);
        assert_eq!(h.app.editor.scene, original_scene);

        h.hover(h.point(end));
        assert_eq!(h.app.editor.document.model(), &original_model);
        assert_eq!(h.app.editor.scene, original_scene);
        h.click(h.point(end));

        let model = h.app.editor.document.model();
        assert_eq!(model.curtain_systems.len(), 1);
        assert_eq!(model.curtain_panel_types.len(), 1);
        assert_eq!(model.curtain_mullion_types.len(), 1);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        let (id, curtain) = model.curtain_systems.iter().next().unwrap();
        let id = *id;
        assert_eq!(curtain.parameters.start, start);
        assert_eq!(curtain.parameters.end, end);
        assert_eq!(curtain.parameters.height, 3.0);
        assert_eq!(curtain.parameters.vertical.len(), 2);
        assert_eq!(curtain.parameters.horizontal.len(), 2);
        assert!(h.app.editor.scene.contains_key(&id));
        assert_eq!(h.app.selected, Some(id));

        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        assert!(
            drawing
                .provider_lines(context)
                .unwrap()
                .iter()
                .any(|line| line.entity == id)
        );
        let pointer = h.point(Point2::new(21.0, 20.0));
        let rect = h.app.plans.canvas_rect.unwrap();
        let local = Point2::new(
            f64::from(pointer.x - rect.left()),
            f64::from(pointer.y - rect.top()),
        );
        assert!(
            drawing
                .hits_screen(
                    context,
                    h.app.plans.cameras[&h.view],
                    [f64::from(rect.width()), f64::from(rect.height())],
                    local,
                    6.0,
                )
                .unwrap()
                .contains(&id)
        );
        h.click(pointer);
        assert_eq!(h.app.selected, Some(id));

        let committed = h.app.editor.document.model().clone();
        h.app.history(false);
        h.settle();
        assert!(h.app.editor.document.model().curtain_systems.is_empty());
        assert!(h.app.editor.document.model().curtain_panel_types.is_empty());
        assert!(
            h.app
                .editor
                .document
                .model()
                .curtain_mullion_types
                .is_empty()
        );
        assert!(!h.app.editor.scene.contains_key(&id));
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert!(h.app.editor.scene.contains_key(&id));
    }
}

#[test]
fn curtain_cancel_and_invalid_baseline_leave_model_and_history_untouched() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let original = h.app.editor.document.model().clone();
        let original_scene = h.app.editor.scene.clone();
        h.app.begin_curtain(h.view);
        h.frame(vec![]);
        let point = h.point(Point2::new(18.0, 20.0));
        h.click(point);
        h.click(point);
        assert!(h.app.plans.curtain_draft.is_some());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        h.frame(vec![escape()]);
        assert!(h.app.plans.curtain_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.scene, original_scene);
    }
}

#[test]
fn curtain_snaps_endpoints_claims_canvas_drag_and_cancels_on_document_change() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: "Curtain snap target".into(),
                path: WallPath::Straight {
                    start: Point2::new(18.0, 20.0),
                    end: Point2::new(22.0, 20.0),
                },
                thickness: 0.2,
                height: 3.0,
                level: h.app.active_level,
                material: None,
            },
        );
        h.app
            .editor
            .command("Curtain snap target", Command::AddWall(wall))
            .unwrap();
        h.settle();
        h.app.plans.snaps.enabled = true;
        h.app.begin_curtain(h.view);
        h.frame(vec![]);

        h.click(h.point(Point2::new(18.1, 20.05)));
        assert_eq!(
            h.app.plans.curtain_draft.as_ref().unwrap().first,
            Some(Point2::new(18.0, 20.0))
        );
        let camera = h.app.plans.cameras[&h.view];
        h.drag(
            h.point(Point2::new(18.0, 20.0)),
            h.point(Point2::new(18.0, 23.0)),
        );
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        assert_eq!(
            h.app.plans.curtain_draft.as_ref().unwrap().first,
            Some(Point2::new(18.0, 20.0))
        );
        assert!(h.app.editor.document.model().curtain_systems.is_empty());
        h.frame(vec![escape()]);
        assert!(h.app.plans.curtain_draft.is_none());

        h.app.begin_curtain(h.view);
        h.frame(vec![]);
        h.click(h.point(Point2::new(18.0, 20.0)));
        let concurrent_wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: "Concurrent edit".into(),
                path: WallPath::Straight {
                    start: Point2::new(15.0, 17.0),
                    end: Point2::new(16.0, 17.0),
                },
                thickness: 0.2,
                height: 3.0,
                level: h.app.active_level,
                material: None,
            },
        );
        h.app
            .editor
            .command("Concurrent edit", Command::AddWall(concurrent_wall))
            .unwrap();
        h.frame(vec![]);
        assert!(h.app.plans.curtain_draft.is_none());
        assert!(h.app.editor.document.model().curtain_systems.is_empty());
    }
}

#[test]
fn curtain_properties_edit_grids_preflight_and_commit_once_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.begin_curtain(h.view);
        h.frame(vec![]);
        h.click(h.point(Point2::new(18.0, 20.0)));
        h.click(h.point(Point2::new(24.0, 20.0)));
        h.settle();

        let id = h.app.selected.expect("placed curtain selected");
        let before = h.app.editor.document.model().clone();
        let before_scene = h.app.editor.scene.clone();
        let before_revision = h.app.editor.document.revision();
        let undo_before = h.app.editor.document.history_stats().undo_entries;
        let original = before.curtain_systems[&id].parameters.clone();
        let start_grid = original.vertical[0].id;
        let end_grid = original.vertical[1].id;
        let original_panel_ids: BTreeSet<_> =
            original.panels.iter().map(|panel| panel.id).collect();

        assert!(h.app.can_edit_curtain_properties(id));
        h.app.begin_curtain_properties(id);
        h.frame(vec![]);
        assert!(h.app.plans.curtain_properties.is_some());
        h.click_modal_button("Add vertical grid");
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, before_scene);
        assert_eq!(h.app.editor.document.revision(), before_revision);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            undo_before
        );
        assert_eq!(
            h.app
                .plans
                .curtain_properties
                .as_ref()
                .unwrap()
                .parameters
                .vertical
                .len(),
            3,
            "status: {}; draft: {:?}",
            h.app.status,
            h.app.plans.curtain_properties.as_ref().unwrap().validation
        );
        h.click_modal_button("Apply");
        assert!(h.app.plans.curtain_properties.is_none(), "{}", h.app.status);
        assert!(!h.app.status_error, "{}", h.app.status);
        h.settle();

        let after = h.app.editor.document.model().clone();
        let edited = &after.curtain_systems[&id].parameters;
        assert_eq!(edited.vertical.len(), 3);
        assert_eq!(edited.vertical[0].id, start_grid);
        assert_eq!(edited.vertical[2].id, end_grid);
        assert_eq!(edited.panels.len(), 2);
        assert!(
            edited
                .panels
                .iter()
                .all(|panel| !original_panel_ids.contains(&panel.id))
        );
        assert_eq!(h.app.editor.document.revision(), before_revision + 1);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            undo_before + 1
        );
        assert!(h.app.editor.scene.contains_key(&id));

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("curtain-properties.osb");
        ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);

        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &after);
    }
}

#[test]
fn curtain_component_types_are_staged_and_do_not_change_existing_defaults_implicitly() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let id = place_test_curtain(&mut h);
        let original = h.app.editor.document.model().curtain_systems[&id]
            .parameters
            .clone();
        let original_mullion = original.mullions[0].mullion_type;
        let model = h.app.editor.document.model().clone();

        h.app.begin_curtain_properties(id);
        let draft = h.app.plans.curtain_properties.as_mut().unwrap();
        draft.panel_name = "Interior opaque panel".into();
        draft.panel_kind = os_model::CurtainPanelKind::Opaque;
        draft.panel_thickness = "0".into();
        assert!(draft.add_panel_type(&model).is_err());
        assert!(draft.panel_types.is_empty());
        draft.panel_thickness = "0.015".into();
        draft.add_panel_type(&model).unwrap();
        let new_default = draft.panel_types[0].0;
        draft.panel_name = "Custom vision glass".into();
        draft.panel_kind = os_model::CurtainPanelKind::Glazing;
        draft.panel_thickness = "0.018".into();
        draft.add_panel_type(&model).unwrap();
        let new_assigned = draft.panel_types[1].0;
        draft.mullion_name = "Narrow frame".into();
        draft.mullion_width = "0.03".into();
        draft.mullion_depth = "0.06".into();
        draft.add_mullion_type(&model).unwrap();
        let new_mullion = draft.mullion_types[0].0;
        draft.parameters.panel_type = new_default;
        draft.parameters.panels[0].panel_type = new_assigned;
        draft.parameters.mullions[0].mullion_type = new_mullion;
        draft.refresh(&h.app.editor);
        assert!(draft.validation.is_ok(), "{:?}", draft.validation);

        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let revision = h.app.editor.document.revision();
        let undo_entries = h.app.editor.document.history_stats().undo_entries;
        assert_eq!(before.curtain_systems[&id].parameters, original);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            undo_entries
        );

        h.app.apply_curtain_properties().unwrap();
        h.settle();
        let model = h.app.editor.document.model();
        let edited = &model.curtain_systems[&id].parameters;
        assert_eq!(edited.panel_type, new_default);
        assert_eq!(edited.panels[0].panel_type, new_assigned);
        assert_ne!(edited.panels[0].panel_type, edited.panel_type);
        assert_eq!(edited.mullions[0].mullion_type, new_mullion);
        assert!(
            edited
                .mullions
                .iter()
                .skip(1)
                .all(|member| member.mullion_type == original_mullion)
        );
        assert!(model.curtain_panel_types.contains_key(&new_default));
        assert!(model.curtain_panel_types.contains_key(&new_assigned));
        assert!(model.curtain_mullion_types.contains_key(&new_mullion));
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            undo_entries + 1
        );
        // Reuse the same assembly UUID and keep the component assignments through storage.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("curtain-assignments.osb");
        ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), model);
    }
}

#[test]
fn curtain_horizontal_grid_stations_are_numeric_and_keep_their_uuids() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let id = place_test_curtain(&mut h);
        let before = h.app.editor.document.model().curtain_systems[&id]
            .parameters
            .clone();
        h.app.begin_curtain_properties(id);
        let draft = h.app.plans.curtain_properties.as_mut().unwrap();
        draft.add_grid(false).unwrap();
        let interior_id = draft.parameters.horizontal[1].id;
        draft.parameters.horizontal[1].position = 1.1;
        draft.refresh(&h.app.editor);
        assert!(draft.validation.is_ok(), "{:?}", draft.validation);
        h.app.apply_curtain_properties().unwrap();
        h.settle();

        let edited = &h.app.editor.document.model().curtain_systems[&id].parameters;
        assert_eq!(edited.horizontal.len(), 3);
        assert_eq!(edited.horizontal[0].id, before.horizontal[0].id);
        assert_eq!(edited.horizontal[1].id, interior_id);
        assert_eq!(edited.horizontal[1].position, 1.1);
        assert_eq!(edited.horizontal[2].id, before.horizontal[1].id);
        assert_eq!(edited.panels.len(), 2);
        assert!(edited.resolve(h.app.editor.document.model()).is_ok());
    }
}

#[test]
fn reverting_a_staged_grid_add_restores_original_component_ids() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let id = place_test_curtain(&mut h);
        let before = h.app.editor.document.model().curtain_systems[&id]
            .parameters
            .clone();
        h.app.begin_curtain_properties(id);
        let draft = h.app.plans.curtain_properties.as_mut().unwrap();
        draft.add_grid(true).unwrap();
        draft.refresh(&h.app.editor);
        let temporary_grid = draft.parameters.vertical[1].id;
        draft.remove_grid(true, temporary_grid);
        draft.parameters.name = "Reverted grid edit".into();
        draft.refresh(&h.app.editor);
        assert!(draft.validation.is_ok(), "{:?}", draft.validation);
        assert_eq!(draft.parameters.panels, before.panels);
        assert_eq!(draft.parameters.mullions, before.mullions);

        h.app.apply_curtain_properties().unwrap();
        let edited = &h.app.editor.document.model().curtain_systems[&id].parameters;
        assert_eq!(edited.name, "Reverted grid edit");
        assert_eq!(edited.vertical, before.vertical);
        assert_eq!(edited.panels, before.panels);
        assert_eq!(edited.mullions, before.mullions);
    }
}

#[test]
fn curtain_customized_removed_bays_require_explicit_reset_in_one_transaction() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let id = place_test_curtain(&mut h);
        h.app.begin_curtain_properties(id);
        h.frame(vec![]);
        {
            let draft = h.app.plans.curtain_properties.as_mut().unwrap();
            draft.add_grid(true).unwrap();
            draft.refresh(&h.app.editor);
            assert!(draft.validation.is_ok(), "{:?}", draft.validation);
        }
        h.app.apply_curtain_properties().unwrap();
        h.settle();

        let model = h.app.editor.document.model().clone();
        let original = model.curtain_systems[&id].parameters.clone();
        let custom_type = os_model::CurtainPanelType::new(
            "core.curtain_panel_type",
            os_model::CurtainPanelTypeParams {
                name: "Customized panel".into(),
                kind: os_model::CurtainPanelKind::Opaque,
                thickness: 0.015,
                material: None,
            },
        );
        let custom_type_id = custom_type.id();
        let custom_mullion_type = os_model::CurtainMullionType::new(
            "core.curtain_mullion_type",
            os_model::CurtainMullionTypeParams {
                name: "Customized mullion".into(),
                width: 0.035,
                depth: 0.06,
                material: None,
            },
        );
        let custom_mullion_type_id = custom_mullion_type.id();
        let mut custom_parameters = original.clone();
        let customized_panel = custom_parameters.panels[0].id;
        custom_parameters.panels[0].panel_type = custom_type_id;
        let customized_mullion = custom_parameters
            .mullions
            .iter_mut()
            .find(|member| member.grid == original.vertical[1].id)
            .expect("interior vertical mullion")
            .id;
        custom_parameters
            .mullions
            .iter_mut()
            .find(|member| member.id == customized_mullion)
            .unwrap()
            .mullion_type = custom_mullion_type_id;
        h.app
            .editor
            .document
            .execute(
                "Customize curtain panel",
                vec![
                    Command::AddCurtainPanelType(custom_type),
                    Command::AddCurtainMullionType(custom_mullion_type),
                    Command::UpdateCurtainSystem {
                        id,
                        parameters: custom_parameters,
                    },
                ],
            )
            .unwrap();
        h.app.editor.regenerate().unwrap();
        h.settle();
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let undo_entries = h.app.editor.document.history_stats().undo_entries;
        h.app.begin_curtain_properties(id);
        h.frame(vec![]);
        let new_panel_default = Id::new();
        let new_mullion_default = Id::new();
        {
            let draft = h.app.plans.curtain_properties.as_mut().unwrap();
            draft.panel_types.push((
                new_panel_default,
                os_model::CurtainPanelTypeParams {
                    name: "Replacement default panel".into(),
                    kind: os_model::CurtainPanelKind::Glazing,
                    thickness: 0.012,
                    material: None,
                },
            ));
            draft.mullion_types.push((
                new_mullion_default,
                os_model::CurtainMullionTypeParams {
                    name: "Replacement default frame".into(),
                    width: 0.025,
                    depth: 0.05,
                    material: None,
                },
            ));
            draft.parameters.panel_type = new_panel_default;
            draft.parameters.mullion_type = new_mullion_default;
            draft.refresh(&h.app.editor);
            assert!(draft.validation.is_ok(), "{:?}", draft.validation);
        }
        h.frame(vec![]);
        h.click_remove_vertical_grid();
        {
            let draft = h.app.plans.curtain_properties.as_mut().unwrap();
            assert_eq!(
                draft.parameters.vertical.len(),
                2,
                "grid removal click did not apply"
            );
            assert!(draft.validation.is_err(), "{:?}", draft.validation);
            assert_eq!(
                draft
                    .parameters
                    .panels
                    .iter()
                    .find(|panel| panel.id == customized_panel)
                    .unwrap()
                    .panel_type,
                custom_type_id
            );
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert!(draft.reset_removed_custom_assignments().is_ok());
            draft.refresh(&h.app.editor);
            assert!(draft.validation.is_ok(), "{:?}", draft.validation);
        }
        h.app.apply_curtain_properties().unwrap();
        h.settle();
        let committed = h.app.editor.document.model().clone();
        let result = &committed.curtain_systems[&id].parameters;
        assert_eq!(result.vertical.len(), 2);
        assert_eq!(result.panels.len(), 1);
        assert_eq!(result.panel_type, new_panel_default);
        assert_eq!(result.mullion_type, new_mullion_default);
        assert_eq!(result.panels[0].panel_type, new_panel_default);
        assert!(
            result
                .mullions
                .iter()
                .any(|member| member.mullion_type == new_mullion_default)
        );
        assert!(
            !result
                .panels
                .iter()
                .any(|panel| panel.panel_type == custom_type_id)
        );
        assert!(
            !result
                .mullions
                .iter()
                .any(|member| member.mullion_type == custom_mullion_type_id)
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            undo_entries + 1
        );
        assert!(!h.app.status_error, "{}", h.app.status);
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
    }
}

#[test]
fn curtain_property_invalid_inputs_and_stale_selection_never_apply_the_draft() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let id = place_test_curtain(&mut h);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let revision = h.app.editor.document.revision();
        h.app.begin_curtain_properties(id);
        h.frame(vec![]);
        {
            let draft = h.app.plans.curtain_properties.as_mut().unwrap();
            draft.height = "0".into();
            draft.refresh(&h.app.editor);
            assert!(draft.validation.is_err());
        }
        assert!(h.app.apply_curtain_properties().is_err());
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.revision(), revision);
        h.frame(vec![escape()]);
        assert!(h.app.plans.curtain_properties.is_none());

        h.app.begin_curtain_properties(id);
        h.app.select(None);
        h.frame(vec![]);
        assert!(h.app.plans.curtain_properties.is_none());
        assert_eq!(
            h.app.editor.document.model().curtain_systems[&id].parameters,
            before.curtain_systems[&id].parameters
        );

        h.app.select(Some(id));
        h.settle();
        h.app.begin_curtain_properties(id);
        h.frame(vec![]);
        h.app.plans.drawing = Some(h.app.editor.native_drawing(h.view).unwrap());
        h.frame(vec![]);
        assert!(h.app.plans.curtain_properties.is_none());
        assert_eq!(
            h.app.editor.document.model().curtain_systems[&id].parameters,
            before.curtain_systems[&id].parameters
        );
    }
}
