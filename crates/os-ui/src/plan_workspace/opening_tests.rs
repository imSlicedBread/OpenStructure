//! Real egui frames for pointer placement; no native-window or screenshot claim.
use super::*;

#[path = "opening_array_tests.rs"]
mod array_tests;

#[path = "window_pane_tests.rs"]
mod window_pane_tests;

#[path = "lite_handedness_tests.rs"]
mod lite_handedness_tests;

#[path = "opening_spacing_tests.rs"]
mod opening_spacing_tests;

#[path = "opening_visibility_tests.rs"]
mod opening_visibility_tests;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];
const KINDS: [OpeningKind; 2] = [OpeningKind::Door, OpeningKind::Window];

impl Harness {
    fn width_point(&self, station: f64) -> egui::Pos2 {
        self.point(os_geometry::openings::world(
            &self.app.editor.document.model().walls[&self.wall].parameters,
            station,
            0.0,
        ))
    }

    fn begin_width(&mut self, kind: OpeningKind) {
        self.begin(kind);
        assert!(
            !self
                .app
                .plans
                .opening_placement
                .as_ref()
                .unwrap()
                .draw_width
        );
        self.text_click("Draw opening width");
        assert!(
            self.app
                .plans
                .opening_placement
                .as_ref()
                .unwrap()
                .draw_width
        );
    }
}

#[test]
fn draw_opening_width_preview_commit_history_and_apertures() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                for typed_rotated in [false, true] {
                    for backwards in [false, true] {
                        let mut h = if typed_rotated {
                            Harness::movable(kind, reversed, false, size, scale).0
                        } else {
                            let mut h = Harness::new(size, scale);
                            if reversed {
                                let mut model = h.app.editor.document.model().clone();
                                let wall = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
                                wall.path = os_model::WallPath::Straight {
                                    start: wall.end(),
                                    end: wall.start(),
                                };
                                h.app.editor.document = Document::from_model(model).unwrap();
                                h.app.editor.pending_geometry.insert(h.wall);
                                h.app.editor.regenerate().unwrap();
                                h.app.plans.poll(&h.app.editor);
                                h.app.focus_plan(Some(h.view));
                                h.settle();
                            }
                            h
                        };
                        h.app.plans.split = true;
                        h.app.plans.cameras.insert(
                            h.view,
                            PlanCamera {
                                center: Point2::new(0.0, 0.0),
                                pixels_per_metre: 20.0,
                            },
                        );
                        h.frame(vec![]);
                        h.begin_width(kind);
                        h.app.plans.snaps.enabled = false;
                        let before = h.app.editor.document.model().clone();
                        let scene = h.app.editor.scene.clone();
                        let history = h.app.editor.document.history_stats();
                        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                        let camera = h.app.plans.cameras[&h.view];
                        let (a, b) = if backwards { (6.1, 4.7) } else { (4.7, 6.1) };
                        let from = h.width_point(a);
                        let to = h.width_point(b);
                        h.hover(from);
                        h.frame(vec![button(from, true)]);
                        h.hover(to);
                        assert!(h.app.plans.opening_width_claimed);
                        assert!(h.app.plans.opening_move.is_none());
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.scene, scene);
                        assert_eq!(h.app.editor.document.history_stats(), history);
                        assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                        assert_eq!(h.app.plans.cameras[&h.view], camera);
                        assert!(h.has_text(&format!("{kind:?} · 1.40 m wide")));
                        assert!(h.preview_lines() > 0);
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        let rect = h.app.plans.canvas_rect.unwrap();
                        let preview = opening_placement_preview(
                            &before,
                            h.app.plans.drawing.as_ref().unwrap(),
                            context,
                            camera,
                            [f64::from(rect.width()), f64::from(rect.height())],
                            Point2::new(
                                f64::from(to.x - rect.left()),
                                f64::from(to.y - rect.top()),
                            ),
                            h.app.plans.opening_placement.as_ref().unwrap(),
                        )
                        .unwrap()
                        .unwrap();
                        let mut candidate = before.clone();
                        let opening = Opening::new("core.opening", preview.parameters);
                        let preview_id = opening.id();
                        candidate.openings.insert(preview_id, opening);
                        let plan = crate::opening_tools::opening_edit_preview(
                            &candidate,
                            preview_id,
                            &[h.wall],
                            context,
                        )
                        .unwrap();
                        for item in plan.items(context).unwrap() {
                            let points: Vec<_> = item
                                .footprint
                                .vertices()
                                .iter()
                                .map(|p| h.point(*p))
                                .collect();
                            assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                                egui::Shape::Path(path) if path.closed && path.points == points)));
                        }
                        h.frame(vec![button(to, false)]);
                        h.settle();
                        let after = h.app.editor.document.model().clone();
                        assert_eq!(after.openings.len(), before.openings.len() + 1);
                        let id = h.app.selected.unwrap();
                        let p = &after.openings[&id].parameters;
                        let resolved = after.resolve_opening(p).unwrap();
                        assert!((resolved.width - 1.4).abs() < 1e-5);
                        assert!((p.offset - 4.7).abs() < 1e-5);
                        assert_eq!(p.width_override, Some(resolved.width));
                        assert_eq!(resolved.kind, kind);
                        if typed_rotated {
                            assert_eq!(after.opening_types, before.opening_types);
                        } else {
                            assert_eq!(after.opening_types.len(), 1);
                            assert_eq!(
                                after.opening_types[&p.type_id().unwrap()].parameters.width,
                                if kind == OpeningKind::Door { 0.9 } else { 1.2 }
                            );
                        }
                        assert_eq!(
                            h.app.editor.document.history_stats().undo_entries,
                            history.undo_entries + 1
                        );
                        let current = h.app.editor.native_plan_context(h.view).unwrap();
                        let committed = h.app.editor.native_wall_plan(h.view).unwrap();
                        assert_eq!(
                            plan.items(context).unwrap(),
                            committed.items(current).unwrap()
                        );
                        assert_ne!(h.app.editor.scene[&h.wall], scene[&h.wall]);
                        assert_eq!(
                            h.app.editor.scene[&h.wall],
                            crate::opening_tools::host_mesh(&after, h.wall).unwrap()
                        );
                        assert_eq!(
                            h.app.editor.scene[&id],
                            crate::opening_tools::panel_mesh(&after, id).unwrap()
                        );
                        let after_scene = h.app.editor.scene.clone();
                        h.app.history(false);
                        h.settle();
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.scene, scene);
                        h.app.history(true);
                        h.settle();
                        assert_eq!(h.app.editor.document.model(), &after);
                        assert_eq!(h.app.editor.scene, after_scene);
                    }
                }
            }
        }
    }
}

#[test]
fn draw_opening_width_invalid_release_and_stale_press_ownership() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reason in [
                "short",
                "collision",
                "clearance",
                "off_host",
                "escape",
                "lost",
                "revision",
                "session",
                "view",
                "settings",
                "provider",
                "drawing",
                "camera",
                "canvas",
            ] {
                let (mut h, _) = Harness::movable(kind, false, false, size, scale);
                h.begin_width(kind);
                h.app.plans.snaps.enabled = false;
                let from = h.width_point(4.7);
                let mut to = h.width_point(6.1);
                h.hover(from);
                h.frame(vec![button(from, true)]);
                h.hover(to);
                match reason {
                    "short" => to = from,
                    "collision" => to = h.width_point(2.0),
                    "clearance" => to = h.width_point(8.0),
                    "off_host" => to += egui::vec2(0.0, 70.0),
                    "escape" => h.frame(vec![escape()]),
                    "lost" => h.frame(vec![egui::Event::PointerGone]),
                    "revision" => h
                        .app
                        .editor
                        .command("Other edit", Command::RenameProject("Changed".into()))
                        .unwrap(),
                    "session" => {
                        h.app.editor.document =
                            Document::from_model(h.app.editor.document.model().clone()).unwrap()
                    }
                    "view" => h.app.focus_plan(None),
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
                    "drawing" => {
                        h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                    }
                    "camera" => {
                        h.app
                            .plans
                            .cameras
                            .get_mut(&h.view)
                            .unwrap()
                            .pixels_per_metre += 1.0;
                    }
                    "canvas" => h.size.x += 120.0,
                    _ => unreachable!(),
                }
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let history = h.app.editor.document.history_stats();
                let selection = h.app.selected;
                let camera = h.app.plans.cameras[&h.view];
                // Move after cancellation before releasing: the press must not become a pan.
                h.hover(to);
                h.frame(vec![button(to, false)]);
                h.frame(vec![]);
                assert_eq!(h.app.editor.document.model(), &before, "{reason}");
                assert_eq!(h.app.editor.scene, scene, "{reason}");
                assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
                assert_eq!(h.app.selected, selection, "{reason}");
                if let Some(current) = h.app.plans.cameras.get(&h.view) {
                    assert_eq!(*current, camera, "{reason}");
                }
                assert!(!h.app.plans.opening_width_claimed);
            }
        }
    }
}

#[test]
fn draw_opening_width_visibility_snapping_and_return_to_pan() {
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
                h.begin_width(kind);
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                let camera = h.app.plans.cameras[&h.view];
                h.drag(h.width_point(2.0), h.width_point(5.5));
                h.frame(vec![]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                // A valid start with a cropped/hidden end cannot reuse a valid hover.
                h.drag(h.width_point(5.5), h.width_point(2.0));
                assert_eq!(h.app.editor.document.model(), &before);
            }
            let mut h = Harness::new(size, scale);
            h.begin_width(kind);
            h.app.plans.snaps.enabled = true;
            h.app.plans.snaps.midpoints = true;
            h.drag(h.width_point(4.08), h.width_point(5.5));
            h.settle();
            let model = h.app.editor.document.model();
            let p = &model.openings[&h.app.selected.unwrap()].parameters;
            assert!(
                (p.offset - 4.0).abs() < 1e-5,
                "first jamb snaps to midpoint"
            );
            assert!((model.resolve_opening(p).unwrap().width - 1.5).abs() < 1e-5);
            h.frame(vec![escape()]);
            h.frame(vec![]);
            let camera = h.app.plans.cameras[&h.view];
            let a = h.point(Point2::new(-2.0, -1.0));
            h.drag(a, a + egui::vec2(25.0, 12.0));
            assert_ne!(
                h.app.plans.cameras[&h.view], camera,
                "ordinary drag pans after exit"
            );
        }
    }
}

// These tests acquire controls from painted text, then drive real pointer frames.
impl Harness {
    fn direct_door_button(&self, label: &str) -> Option<egui::Pos2> {
        let canvas = self.app.plans.canvas_rect?;
        self.output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text == label && canvas.contains(text.pos) =>
                {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                }
                _ => None,
            })
    }

    fn direct_door_fixture(
        size: egui::Vec2,
        scale: f32,
        reversed: bool,
        legacy: bool,
    ) -> (Self, Id) {
        let (mut h, id) = Self::movable(OpeningKind::Door, reversed, legacy, size, scale);
        let model = h.app.editor.document.model();
        let p = model
            .resolve_opening(&model.openings[&id].parameters)
            .unwrap();
        let center = os_geometry::openings::world(
            &model.resolve_wall(h.wall).unwrap().parameters,
            p.offset + p.width / 2.,
            0.,
        );
        h.app.plans.cameras.insert(
            h.view,
            PlanCamera {
                center,
                pixels_per_metre: 35.,
            },
        );
        h.frame(vec![]);
        (h, id)
    }
}

#[test]
fn direct_plan_door_independent_toggles_preserve_model_and_regenerate_with_history() {
    for (size, scale) in PROFILES {
        for reversed in [false, true] {
            for legacy in [false, true] {
                let (mut h, id) = Harness::direct_door_fixture(size, scale, reversed, legacy);
                if !legacy {
                    let mut model = h.app.editor.document.model().clone();
                    let p = &mut model.openings.get_mut(&id).unwrap().parameters;
                    p.width_override = Some(0.95);
                    p.height_override = Some(2.05);
                    let ty = p.type_id().unwrap();
                    let panel = os_model::Material::new(
                        "core.material",
                        os_model::MaterialParams {
                            name: "Door panel".into(),
                            density_kg_m3: 650.,
                            color: [140, 90, 40],
                        },
                    );
                    let frame = os_model::Material::new(
                        "core.material",
                        os_model::MaterialParams {
                            name: "Door frame".into(),
                            density_kg_m3: 8000.,
                            color: [100, 110, 120],
                        },
                    );
                    let family = &mut model.opening_types.get_mut(&ty).unwrap().parameters.family;
                    family.frame_width = 0.04;
                    family.panel_material = Some(panel.id());
                    family.frame_material = Some(frame.id());
                    model.materials.insert(panel.id(), panel);
                    model.materials.insert(frame.id(), frame);
                    h.app.editor.document = Document::from_model(model).unwrap();
                    h.app.editor.pending_geometry.extend([h.wall, id]);
                    h.app.editor.regenerate().unwrap();
                    let camera = h.app.plans.cameras[&h.view];
                    h.app.plans.poll(&h.app.editor);
                    h.app.focus_plan(Some(h.view));
                    h.app.select(Some(id));
                    h.settle();
                    h.app.plans.cameras.insert(h.view, camera);
                    h.frame(vec![]);
                }
                // Each control toggles twice, including each combination of the other field.
                for label in ["Hinge", "Swing", "Hinge", "Swing"] {
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let camera = h.app.plans.cameras[&h.view];
                    let history = h.app.editor.document.history_stats().undo_entries;
                    let revision = h.app.editor.document.revision();
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let drawing = h.app.plans.drawing.as_ref().unwrap();
                    let identity = drawing.identity();
                    let items = drawing.items(context).unwrap().to_vec();
                    let lines = drawing.provider_lines(context).unwrap().to_vec();
                    let pos = h.direct_door_button(label).expect("visible direct control");
                    h.hover(pos);
                    assert_eq!(
                        h.output.platform_output.cursor_icon,
                        egui::CursorIcon::PointingHand
                    );
                    h.frame(vec![button(pos, true)]);
                    assert!(h.app.plans.opening_flip.is_some());
                    assert!(h.app.plans.opening_move.is_none());
                    assert_eq!(h.app.editor.document.model(), &before, "press is transient");
                    h.frame(vec![button(pos, false)]);
                    assert!(
                        format!("{:?}", h.output.platform_output.events).contains(
                            if label == "Hinge" {
                                "Flip door hinge"
                            } else {
                                "Flip door swing"
                            }
                        ),
                        "button exposes its accessible action label"
                    );
                    h.settle();
                    h.move_clean();
                    assert!(h.app.plans.opening_flip.is_none());
                    assert!(!h.app.plans.opening_flip_claimed);
                    let mut expected = before.clone();
                    let p = &mut expected.openings.get_mut(&id).unwrap().parameters;
                    if label == "Hinge" {
                        p.hinge = match p.hinge {
                            os_model::DoorHinge::Start => os_model::DoorHinge::End,
                            os_model::DoorHinge::End => os_model::DoorHinge::Start,
                        };
                    } else {
                        p.swing = match p.swing {
                            os_model::DoorSwing::Left => os_model::DoorSwing::Right,
                            os_model::DoorSwing::Right => os_model::DoorSwing::Left,
                        };
                    }
                    assert_eq!(
                        h.app.editor.document.model(),
                        &expected,
                        "only chosen field changes"
                    );
                    assert_eq!(h.app.selected, Some(id));
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        history + 1
                    );
                    assert_eq!(h.app.editor.document.revision(), revision + 1);
                    assert_ne!(h.app.editor.scene[&id], scene[&id]);
                    assert_eq!(
                        h.app.editor.scene[&id],
                        crate::opening_tools::panel_mesh(&expected, id).unwrap()
                    );
                    assert_eq!(
                        h.app.editor.scene[&h.wall], scene[&h.wall],
                        "aperture preserved"
                    );
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let drawing = h.app.plans.drawing.as_ref().unwrap();
                    assert_ne!(drawing.identity(), identity);
                    assert_eq!(drawing.items(context).unwrap(), items);
                    assert_ne!(drawing.provider_lines(context).unwrap(), lines);
                    // Check the leaf's world direction against the stored host axis.
                    let leaf = drawing
                        .provider_lines(context)
                        .unwrap()
                        .iter()
                        .find(|l| l.entity == id && l.feature == 2)
                        .unwrap();
                    let a = context.basis.plane_to_world(leaf.start).unwrap();
                    let b = context.basis.plane_to_world(leaf.end).unwrap();
                    let wall = &expected.walls[&h.wall].parameters;
                    let along = ((a.x - wall.start().x) * (wall.end().x - wall.start().x)
                        + (a.y - wall.start().y) * (wall.end().y - wall.start().y))
                        / wall.length();
                    let p = &expected.openings[&id].parameters;
                    let resolved = expected.resolve_opening(p).unwrap();
                    let station = p.offset
                        + if p.hinge == os_model::DoorHinge::End {
                            resolved.width
                        } else {
                            0.0
                        };
                    assert!((along - station).abs() < 1e-9);
                    let cross = (wall.end().x - wall.start().x) * (b.y - a.y)
                        - (wall.end().y - wall.start().y) * (b.x - a.x);
                    assert_eq!(cross > 0.0, p.swing == os_model::DoorSwing::Left);
                    assert_eq!(
                        drawing
                            .provider_lines(context)
                            .unwrap()
                            .iter()
                            .filter(|l| l.entity == id && (3..19).contains(&l.feature))
                            .count(),
                        16
                    );
                    let after_scene = h.app.editor.scene.clone();
                    let after_lines = drawing.provider_lines(context).unwrap().to_vec();
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    assert_eq!(
                        h.app
                            .plans
                            .drawing
                            .as_ref()
                            .unwrap()
                            .provider_lines(context)
                            .unwrap(),
                        lines
                    );
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &expected);
                    assert_eq!(h.app.editor.scene, after_scene);
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    assert_eq!(
                        h.app
                            .plans
                            .drawing
                            .as_ref()
                            .unwrap()
                            .provider_lines(context)
                            .unwrap(),
                        after_lines
                    );
                }
            }
        }
    }
}

#[test]
fn direct_plan_door_controls_hidden_and_tool_precedence() {
    for (size, scale) in PROFILES {
        for reason in [
            "window",
            "unselected",
            "wall",
            "hidden",
            "crop",
            "partial_crop",
            "level",
            "edge",
            "placement",
            "rehost",
            "properties",
            "room",
            "crop_tool",
        ] {
            let (mut h, id) = Harness::direct_door_fixture(size, scale, false, false);
            let old = h.direct_door_button("Hinge").unwrap();
            match reason {
                "window" => {
                    (h, _) = Harness::movable(OpeningKind::Window, false, false, size, scale);
                    h.use_move_anchor(0.0);
                }
                "unselected" => h.app.select(None),
                "wall" => h.app.select(Some(h.wall)),
                "hidden" | "crop" | "partial_crop" | "level" => {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    let mut level_id = h.app.active_level;
                    if reason == "hidden" {
                        settings.visibility.walls = false;
                    } else if reason == "level" {
                        let mut parameters = h.app.editor.document.model().levels
                            [&h.app.active_level]
                            .parameters
                            .clone();
                        parameters.name = "Other level".into();
                        let level = os_model::Level::new("core.level", parameters);
                        level_id = level.id();
                        h.app
                            .editor
                            .command("Other level", Command::AddLevel(level))
                            .unwrap();
                    } else {
                        settings.crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(
                                if reason == "partial_crop" { -1.7 } else { 0.0 },
                                -3.0,
                            ),
                            max: Point2::new(4.0, 3.0),
                        });
                    }
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Hidden", level_id, settings)
                        .unwrap();
                    h.settle();
                }
                "edge" => h.app.plans.cameras.get_mut(&h.view).unwrap().center.y -= 20.0,
                "placement" => h.begin(OpeningKind::Window),
                "rehost" => h.app.begin_opening_rehost(),
                "properties" => h.app.begin_opening(None),
                "room" => h.app.plans.room_placement_active = true,
                "crop_tool" => {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    settings.crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(-4., -3.),
                        max: Point2::new(4., 3.),
                    });
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Crop", h.app.active_level, settings)
                        .unwrap();
                    h.settle();
                    h.app.plans.crop.mode = Some(h.view);
                }
                _ => unreachable!(),
            }
            h.frame(vec![]);
            assert!(h.direct_door_button("Hinge").is_none(), "{reason}");
            assert!(h.direct_door_button("Swing").is_none(), "{reason}");
            let before = h.app.editor.document.model().openings.clone();
            h.click(old);
            assert!(h.app.plans.opening_flip.is_none(), "{reason}");
            assert_eq!(h.app.editor.document.model().openings, before, "{reason}");
            if reason != "window" {
                assert_eq!(
                    h.app.editor.document.model().openings[&id].parameters.hinge,
                    os_model::DoorHinge::End
                );
            }
        }
    }
}

#[test]
fn direct_plan_door_click_cancellation_retains_pointer_ownership() {
    for (size, scale) in PROFILES {
        for reason in [
            "drag",
            "escape",
            "revision",
            "session",
            "selection",
            "drawing",
            "provider",
            "lost",
            "camera",
            "tool",
        ] {
            let (mut h, _) = Harness::direct_door_fixture(size, scale, false, false);
            let pos = h.direct_door_button("Swing").unwrap();
            h.hover(pos);
            h.frame(vec![button(pos, true)]);
            assert!(h.app.plans.opening_flip.is_some());
            match reason {
                "drag" => h.hover(pos + egui::vec2(25.0, 20.0)),
                "escape" => h.frame(vec![escape()]),
                "revision" => h
                    .app
                    .editor
                    .command("Other edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "selection" => h.app.select(Some(h.wall)),
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "lost" => h.frame(vec![egui::Event::PointerGone]),
                "camera" => {
                    h.app
                        .plans
                        .cameras
                        .get_mut(&h.view)
                        .unwrap()
                        .pixels_per_metre += 1.0
                }
                "tool" => h.app.begin_opening_rehost(),
                _ => unreachable!(),
            }
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let camera = h.app.plans.cameras[&h.view];
            let selection = h.app.selected;
            h.hover(pos);
            h.frame(vec![button(pos, false)]);
            h.frame(vec![]);
            assert!(h.app.plans.opening_flip.is_none(), "{reason}");
            assert!(!h.app.plans.opening_flip_claimed, "{reason}");
            assert!(h.app.plans.opening_move.is_none(), "{reason}");
            assert_eq!(h.app.editor.document.model(), &before, "{reason}");
            assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
            assert_eq!(h.app.selected, selection, "{reason}");
            if let Some(current) = h.app.plans.cameras.get(&h.view) {
                assert_eq!(*current, camera, "{reason}");
            }
        }
    }
}

#[test]
fn direct_plan_door_controls_and_grips_have_distinct_pointer_targets() {
    for (size, scale) in PROFILES {
        let (mut h, id) = Harness::direct_door_fixture(size, scale, false, false);
        for label in ["Hinge", "Swing"] {
            let pos = h.direct_door_button(label).unwrap();
            let button_style = |h: &Harness| {
                h.output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Rect(r)
                            if r.rect.contains(pos) && r.rect.size() == egui::vec2(58., 24.) =>
                        {
                            Some((r.fill, r.stroke))
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            h.hover(pos + egui::vec2(0., 35.));
            let idle = button_style(&h);
            h.hover(pos);
            let hovered = button_style(&h);
            assert_ne!(idle, hovered, "hover is visible");
            for _ in 0..90 {
                h.frame(vec![]);
            }
            assert!(
                h.has_text(if label == "Hinge" {
                    "Flip door hinge:"
                } else {
                    "Flip door swing:"
                }),
                "host-relative tooltip"
            );
            h.frame(vec![button(pos, true)]);
            assert_ne!(hovered, button_style(&h), "press is visible");
            assert!(h.app.plans.opening_move.is_none());
            h.frame(vec![escape()]);
            h.frame(vec![button(pos, false)]);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        }
        // Jamb resizing has its own clearance threshold; the flip buttons above
        // were already verified at the fixture's ordinary 35 px/m zoom.
        h.app
            .plans
            .cameras
            .get_mut(&h.view)
            .unwrap()
            .pixels_per_metre = 70.0;
        h.frame(vec![]);
        // Actual center/jamb presses still own move/resize, including cancellation.
        for anchor in [0.0, 0.5, 1.0] {
            h.frame(vec![]);
            h.move_anchor = anchor;
            let grip = h.opening_point(id, 1.5);
            for label in ["Hinge", "Swing"] {
                let rect = egui::Rect::from_center_size(
                    h.direct_door_button(label).unwrap(),
                    egui::vec2(58., 24.),
                );
                assert!(
                    !rect
                        .expand(4.)
                        .intersects(egui::Rect::from_center_size(grip, egui::vec2(20., 20.)))
                );
            }
            h.hover(grip);
            h.frame(vec![button(grip, true)]);
            assert!(
                h.app.plans.opening_move.is_some(),
                "anchor={anchor}, selected={:?}, flip_claimed={}, status={}",
                h.app.selected,
                h.app.plans.opening_flip_claimed,
                h.app.status
            );
            assert!(h.app.plans.opening_flip.is_none());
            assert!(!h.app.plans.opening_flip_claimed);
            h.frame(vec![]);
            assert!(h.direct_door_button("Hinge").is_none());
            h.frame(vec![escape()]);
            h.frame(vec![button(grip, false)]);
            h.move_clean();
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        }
        // Dragging either button onto a grip or the other button cannot transfer ownership.
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let camera = h.app.plans.cameras[&h.view];
        for label in ["Hinge", "Swing"] {
            for target in ["other", "center", "jamb", "outside"] {
                let from = h.direct_door_button(label).unwrap();
                h.move_anchor = if target == "jamb" { 0. } else { 0.5 };
                let to = match target {
                    "other" => h
                        .direct_door_button(if label == "Hinge" { "Swing" } else { "Hinge" })
                        .unwrap(),
                    "outside" => egui::pos2(-10., -10.),
                    _ => h.opening_point(id, 1.5),
                };
                h.drag(from, to);
                assert!(h.app.plans.opening_flip.is_none());
                assert!(!h.app.plans.opening_flip_claimed);
                h.move_clean();
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                assert_eq!(h.app.selected, Some(id));
            }
        }
    }
}

#[test]
fn direct_plan_door_fast_click_is_one_update() {
    for (size, scale) in PROFILES {
        for label in ["Hinge", "Swing"] {
            let (mut h, id) = Harness::direct_door_fixture(size, scale, false, false);
            let pos = h.direct_door_button(label).unwrap();
            let mut expected = h.app.editor.document.model().clone();
            let p = &mut expected.openings.get_mut(&id).unwrap().parameters;
            if label == "Hinge" {
                p.hinge = os_model::DoorHinge::Start;
            } else {
                p.swing = os_model::DoorSwing::Left;
            }
            h.hover(pos);
            h.frame(vec![button(pos, true), button(pos, false)]);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &expected);
            assert_eq!(h.app.editor.document.revision(), 1);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
            assert!(h.app.plans.opening_flip.is_none());
            assert!(!h.app.plans.opening_flip_claimed);
            h.move_clean();
        }
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
    move_anchor: f64,
}

impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        *app.draft.path.straight_start_mut().unwrap() = Point2::new(-4.0, 0.0);
        *app.draft.path.straight_end_mut().unwrap() = Point2::new(4.0, 0.0);
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
            move_anchor: 0.5,
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
        assert!(
            rect.contains(position),
            "fixture point {position:?} must lie on canvas {rect:?}"
        );
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

    fn text_click(&mut self, label: &str) {
        let position = self.output.shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == label
            {
                return Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center());
            }
            None
        });
        self.click(position.unwrap_or_else(|| panic!("missing button: {label}")));
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

impl Harness {
    fn rehostable(
        kind: OpeningKind,
        legacy: bool,
        reversed: bool,
        lite_override: bool,
        size: egui::Vec2,
        scale: f32,
    ) -> (Self, Id, Id) {
        let (mut h, id) = Self::movable(kind, false, legacy, size, scale);
        let mut model = h.app.editor.document.model().clone();
        if lite_override {
            let opening = model.openings.get_mut(&id).unwrap();
            let type_id = opening.parameters.type_id().unwrap();
            opening.parameters.lite_side_override = Some(os_model::LiteSide::End);
            model
                .opening_types
                .get_mut(&type_id)
                .unwrap()
                .parameters
                .family
                .side_lite = Some(os_model::SideLite {
                side: os_model::LiteSide::Start,
                width_fraction: 0.25,
                mullion_width: 0.04,
                material: None,
            });
        }
        let mut parameters = model.walls[&h.wall].parameters.clone();
        parameters.name = "New host".into();
        *parameters.path.straight_start_mut().unwrap() = Point2::new(-4.0, 2.0);
        *parameters.path.straight_end_mut().unwrap() = Point2::new(4.0, 2.0);
        if reversed {
            parameters.path = os_model::WallPath::Straight {
                start: parameters.end(),
                end: parameters.start(),
            };
        }
        let target = os_model::Wall::new(os_walls::WALL_TYPE, parameters);
        let target_id = target.id();
        model.walls.insert(target_id, target);
        h.app.editor.document = Document::from_model(model).unwrap();
        if lite_override {
            h.app
                .editor
                .pending_geometry
                .extend([id, h.wall, target_id]);
        } else {
            h.app.editor.pending_geometry.insert(target_id);
        }
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
        (h, id, target_id)
    }

    fn rehost_clean(&self) {
        assert!(self.app.plans.opening_rehost.is_none());
        assert!(!self.app.plans.opening_rehost_claimed);
        self.move_clean();
    }
}

#[test]
fn opening_rehost_preview_commit_history_and_both_host_regeneration() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for legacy in [false, true] {
                for reversed in [false, true] {
                    let (mut h, id, target) =
                        Harness::rehostable(kind, legacy, reversed, !legacy, size, scale);
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let camera = h.app.plans.cameras[&h.view];
                    let identity = h.app.plans.drawing.as_ref().unwrap().identity();
                    h.ribbon_click("Architecture", 30.0..58.0);
                    h.ribbon_click("Rehost", 58.0..180.0);
                    assert!(h.app.plans.opening_rehost.is_some());
                    let to = h.point(Point2::new(-1.0, 2.0));
                    h.hover(to);
                    let rect = h.app.plans.canvas_rect.unwrap();
                    let pointer =
                        Point2::new(f64::from(to.x - rect.left()), f64::from(to.y - rect.top()));
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let drawing = h.app.plans.drawing.as_ref().unwrap();
                    let pick_before = drawing
                        .pick_screen(
                            context,
                            camera,
                            [f64::from(rect.width()), f64::from(rect.height())],
                            pointer,
                            7.0,
                        )
                        .unwrap();
                    let (parameters, preview) = h
                        .app
                        .plans
                        .opening_rehost
                        .as_ref()
                        .unwrap()
                        .candidate(
                            &h.app.editor,
                            drawing,
                            camera,
                            [f64::from(rect.width()), f64::from(rect.height())],
                            pointer,
                        )
                        .unwrap();
                    assert_eq!(parameters.host, target);
                    for host in [h.wall, target] {
                        let old: Vec<_> = drawing
                            .items(context)
                            .unwrap()
                            .iter()
                            .filter(|i| i.entity == host)
                            .collect();
                        let new: Vec<_> = preview
                            .items(context)
                            .unwrap()
                            .iter()
                            .filter(|i| i.entity == host)
                            .collect();
                        assert_ne!(old, new, "both apertures change");
                    }
                    for item in preview.items(context).unwrap() {
                        let points: Vec<_> = item
                            .footprint
                            .vertices()
                            .iter()
                            .map(|p| h.point(*p))
                            .collect();
                        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Path(path) if path.closed && path.points == points)));
                    }
                    for line in preview.provider_lines(context).unwrap() {
                        let points = [h.point(line.start), h.point(line.end)];
                        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { points: actual, stroke } if *actual == points && stroke.color == theme::ACCENT)));
                    }
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.revision(), 0);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(drawing.identity(), identity);
                    assert_eq!(
                        drawing
                            .pick_screen(
                                context,
                                camera,
                                [f64::from(rect.width()), f64::from(rect.height())],
                                pointer,
                                7.0
                            )
                            .unwrap(),
                        pick_before
                    );
                    h.click(to);
                    h.settle();
                    h.rehost_clean();
                    assert_eq!(h.app.selected, Some(id));
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                    let mut expected = before.clone();
                    expected.openings.get_mut(&id).unwrap().parameters = parameters;
                    assert_eq!(
                        h.app.editor.document.model(),
                        &expected,
                        "only host and offset change"
                    );
                    for entity in [h.wall, target, id] {
                        assert_ne!(h.app.editor.scene.get(&entity), scene.get(&entity));
                    }
                    let current = h.app.editor.native_plan_context(h.view).unwrap();
                    let committed = h.app.plans.drawing.as_ref().unwrap();
                    assert_eq!(
                        preview.items(context).unwrap(),
                        committed.items(current).unwrap()
                    );
                    assert_eq!(
                        preview.provider_lines(context).unwrap(),
                        committed.provider_lines(current).unwrap()
                    );
                    let after_scene = h.app.editor.scene.clone();
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &expected);
                    assert_eq!(h.app.editor.scene, after_scene);
                }
            }
        }
    }
}

#[test]
fn opening_rehost_invalid_absent_target_and_final_click_revalidation() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let (mut h, id, target) = Harness::rehostable(kind, false, false, false, size, scale);
            let mut neighbor = Opening::new(
                "core.opening",
                h.app.editor.document.model().openings[&id]
                    .parameters
                    .clone(),
            );
            neighbor.parameters.host = target;
            neighbor.parameters.offset = 4.5;
            h.app
                .editor
                .command("Neighbor", Command::AddOpening(neighbor))
                .unwrap();
            h.settle();
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let scene = h.app.editor.scene.clone();
            h.app.begin_opening_rehost();
            for world in [
                Point2::new(-3.95, 2.0),
                Point2::new(3.95, 2.0),
                Point2::new(1.0, 2.0),
                Point2::new(-1.0, -2.0),
            ] {
                let bad = h.point(world);
                h.hover(bad);
                assert!(h.has_text("Cannot rehost:"));
                h.click(bad);
                assert!(h.app.plans.opening_rehost.is_some());
                assert!(!h.app.plans.opening_rehost_claimed);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(h.app.editor.scene, scene);
            }
            // A release at a nearby invalid point must not reuse the valid press preview.
            let width = before
                .resolve_opening(&before.openings[&id].parameters)
                .unwrap()
                .width;
            let valid = h.point(Point2::new(-4.0 + width * 0.5 + 0.02, 2.0));
            let invalid = h.point(Point2::new(-4.0 + width * 0.5 - 0.02, 2.0));
            h.hover(valid);
            h.frame(vec![button(valid, true)]);
            h.frame(vec![button(invalid, false)]);
            assert!(h.app.status_error, "the invalid final click is evaluated");
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(h.app.plans.opening_rehost.is_some());
            h.click(h.point(Point2::new(-1.0, 2.0)));
            h.settle();
            h.rehost_clean();
            assert_eq!(
                h.app.editor.document.model().openings[&id].parameters.host,
                target
            );
        }
    }
}

#[test]
fn opening_rehost_cancel_stale_and_pointer_ownership() {
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
                let (mut h, id, _) = Harness::rehostable(kind, false, false, false, size, scale);
                let camera = h.app.plans.cameras[&h.view];
                h.app.begin_opening_rehost();
                let mut to = h.point(Point2::new(-1.0, 2.0));
                h.hover(to);
                h.frame(vec![button(to, true)]);
                assert!(h.app.plans.opening_rehost_claimed);
                match reason {
                    "escape" => h.frame(vec![escape()]),
                    "revision" => h
                        .app
                        .editor
                        .command("Other edit", Command::RenameProject("Changed".into()))
                        .unwrap(),
                    "session" => {
                        h.app.editor.document =
                            Document::from_model(h.app.editor.document.model().clone()).unwrap()
                    }
                    "view" => h.app.focus_plan(None),
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
                    "selection" => h.app.select(Some(h.wall)),
                    "drawing" => {
                        h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                    }
                    "outside" => to = egui::pos2(-10.0, -10.0),
                    "lost" => {
                        h.frame(vec![egui::Event::PointerGone]);
                        to = egui::pos2(-10.0, -10.0);
                    }
                    _ => unreachable!(),
                }
                let model = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let history = h.app.editor.document.history_stats();
                let selected = h.app.selected;
                h.frame(vec![egui::Event::PointerMoved(to + egui::vec2(20.0, 0.0))]);
                h.frame(vec![button(to, false)]);
                h.frame(vec![]);
                h.rehost_clean();
                assert_eq!(h.app.editor.document.model(), &model, "{reason}");
                assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
                assert_eq!(h.app.editor.scene, scene, "{reason}");
                assert_eq!(h.app.selected, selected, "no delayed selection: {reason}");
                assert_eq!(model.openings[&id].parameters.host, h.wall);
                if let Some(current) = h.app.plans.cameras.get(&h.view) {
                    assert_eq!(*current, camera, "no delayed pan: {reason}");
                }
            }
        }
    }
}

#[test]
fn opening_rehost_target_visibility_level_crop_and_fit() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reason in ["hidden", "crop", "level", "height", "same_host"] {
                let (mut h, id, target) =
                    Harness::rehostable(kind, false, false, false, size, scale);
                let mut model = h.app.editor.document.model().clone();
                match reason {
                    "hidden" => {
                        model
                            .views
                            .get_mut(&h.view)
                            .unwrap()
                            .parameters
                            .plan
                            .as_mut()
                            .unwrap()
                            .visibility
                            .walls = false
                    }
                    "crop" => {
                        model
                            .views
                            .get_mut(&h.view)
                            .unwrap()
                            .parameters
                            .plan
                            .as_mut()
                            .unwrap()
                            .crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(-4.0, -3.0),
                            max: Point2::new(0.0, 3.0),
                        })
                    }
                    "level" => {
                        // Same elevation means visibility alone cannot enforce active-level targeting.
                        let mut parameters = model.levels[&h.app.active_level].parameters.clone();
                        parameters.name = "Other level".into();
                        let level = os_model::Level::new("core.level", parameters);
                        model.walls.get_mut(&target).unwrap().parameters.level = level.id();
                        model.levels.insert(level.id(), level);
                    }
                    "height" => model.walls.get_mut(&target).unwrap().parameters.height = 1.0,
                    "same_host" => {}
                    _ => unreachable!(),
                }
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.editor.pending_geometry.insert(target);
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
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                h.app.begin_opening_rehost();
                if reason == "hidden" {
                    assert!(h.app.plans.opening_rehost.is_none());
                    assert!(h.app.status_error);
                } else {
                    let to = if reason == "same_host" {
                        h.opening_point(id, 1.5)
                    } else {
                        h.point(Point2::new(1.0, 2.0))
                    };
                    h.hover(to);
                    assert!(h.has_text("Cannot rehost:"), "{reason}");
                    h.click(to);
                    assert!(h.app.plans.opening_rehost.is_some(), "{reason}");
                    assert!(!h.app.plans.opening_rehost_claimed);
                    h.frame(vec![escape()]);
                }
                h.rehost_clean();
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                assert_eq!(h.app.editor.scene, scene);
            }
        }
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
        *wall.path.straight_start_mut().unwrap() = Point2::new(-3.2, -2.4);
        *wall.path.straight_end_mut().unwrap() = Point2::new(3.2, 2.4);
        if reversed {
            wall.path = os_model::WallPath::Straight {
                start: wall.end(),
                end: wall.start(),
            };
        }
        let (width, height, sill) = if kind == OpeningKind::Door {
            (0.9, 2.1, 0.0)
        } else {
            (1.2, 1.2, 0.9)
        };
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                window_operation: Default::default(),
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
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
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
            offset + p.width * self.move_anchor,
            0.0,
        ))
    }

    fn use_move_anchor(&mut self, anchor: f64) {
        self.move_anchor = anchor;
        if anchor != 0.5 {
            self.app
                .plans
                .cameras
                .get_mut(&self.view)
                .unwrap()
                .pixels_per_metre = 45.0;
            self.app.plans.snaps.enabled = false;
            self.frame(vec![]);
        }
    }

    fn jamb_handles(&self) -> usize {
        self.output.shapes.iter().filter(|shape| matches!(&shape.shape,
            egui::Shape::Path(path) if path.closed && path.points.len() == 4
                && path.stroke.width == 2.0 && path.stroke.color == egui::epaint::ColorMode::Solid(theme::ACCENT)
                && (path.points[0].distance(path.points[2]) - 12.0).abs() < 0.01
                && (path.points[1].distance(path.points[3]) - 12.0).abs() < 0.01
        )).count()
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
    drag_preview_history(0.5);
}

#[test]
fn opening_jamb_preview_aperture_and_single_update_history() {
    for anchor in [0.0, 1.0] {
        drag_preview_history(anchor);
    }
}

#[test]
fn opening_jamb_off_center_press_keeps_opposite_fixed_and_return_to_origin_is_noop() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                for legacy in [false, true] {
                    for anchor in [0.0, 1.0] {
                        let (mut h, id) = Harness::movable(kind, reversed, legacy, size, scale);
                        h.use_move_anchor(anchor);
                        let before = h.app.editor.document.model().clone();
                        let scene = h.app.editor.scene.clone();
                        let old = before
                            .resolve_opening(&before.openings[&id].parameters)
                            .unwrap();
                        let jamb = h.opening_point(id, old.offset);
                        let direction = (h.opening_point(id, old.offset + 1.) - jamb).normalized();
                        let grab = jamb + direction * 6.;
                        h.hover(grab);
                        h.frame(vec![button(grab, true)]);
                        assert!(h.app.plans.opening_move.is_some());
                        h.hover(grab + direction * 13.5);
                        let draft = h.app.plans.opening_move.as_ref().unwrap();
                        let p = draft.parameters();
                        let effective = before.resolve_opening(&p).unwrap();
                        let expected_width = old.width + if anchor == 0.0 { -0.3 } else { 0.3 };
                        assert!(
                            (effective.width - expected_width).abs() < 1e-5,
                            "press offset must not add 6 pixels"
                        );
                        if anchor == 0.0 {
                            assert!(
                                (effective.offset + effective.width - old.offset - old.width).abs()
                                    < 1e-5
                            );
                        } else {
                            assert_eq!(effective.offset, old.offset);
                        }
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.scene, scene);
                        // A real excursion followed by release at the original grab must not pin.
                        h.frame(vec![egui::Event::PointerMoved(grab), button(grab, false)]);
                        h.settle();
                        h.move_clean();
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                        assert_eq!(h.app.editor.scene, scene);
                    }
                }
            }
        }
    }
}

fn drag_preview_history(anchor: f64) {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                for legacy in [false, true] {
                    let (mut h, id) = Harness::movable(kind, reversed, legacy, size, scale);
                    h.use_move_anchor(anchor);
                    assert_eq!(h.handles(), 1, "one selected opening grip");
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let camera = h.app.plans.cameras[&h.view];
                    let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                    let target = if anchor == 0.0 { 1.0 } else { 2.5 };
                    let to = h.press_move(id, target);
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
                    let old = before
                        .resolve_opening(&before.openings[&id].parameters)
                        .unwrap();
                    let new = after
                        .resolve_opening(&after.openings[&id].parameters)
                        .unwrap();
                    let expected_offset = if anchor == 1.0 { old.offset } else { target };
                    assert!((new.offset - expected_offset).abs() < 1e-5);
                    if anchor != 0.5 {
                        let expected_width = old.width + if anchor == 0.0 { 0.5 } else { 1.0 };
                        assert!((new.width - expected_width).abs() < 1e-5);
                        if anchor == 0.0 {
                            assert!((new.offset + new.width - old.offset - old.width).abs() < 1e-5);
                        } else {
                            assert_eq!(new.offset, old.offset);
                        }
                        let p = &mut expected.openings.get_mut(&id).unwrap().parameters;
                        match &mut p.definition {
                            OpeningDefinition::Legacy { width, .. } => *width = new.width,
                            OpeningDefinition::Typed { .. } => p.width_override = Some(new.width),
                        }
                    }
                    assert_eq!(after, expected, "only offset/effective width change");
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
    drag_invalid(0.5);
}

#[test]
fn opening_jamb_rejects_clearance_overlap_and_collapsing_pier() {
    for anchor in [0.0, 1.0] {
        drag_invalid(anchor);
    }
}

#[test]
fn opening_jamb_rejects_effective_frame_fit_and_start_side_overlap() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for anchor in [0.0, 1.0] {
                let (mut h, id) = Harness::movable(kind, true, false, size, scale);
                let model = h.app.editor.document.model();
                let ty = model.openings[&id].parameters.type_id().unwrap();
                let width = model
                    .resolve_opening(&model.openings[&id].parameters)
                    .unwrap()
                    .width;
                let mut parameters = model.opening_types[&ty].parameters.clone();
                parameters.family.frame_width = 0.3;
                h.app
                    .editor
                    .command("Frame", Command::UpdateOpeningType { id: ty, parameters })
                    .unwrap();
                h.settle();
                h.use_move_anchor(anchor);
                let before = h.app.editor.document.model().clone();
                let stats = h.app.editor.document.history_stats();
                let scene = h.app.editor.scene.clone();
                let target = if anchor == 0.0 {
                    1.5 + width - 0.5
                } else {
                    1.5 + 0.5 - width
                };
                let to = h.press_move(id, target);
                assert!(h.has_text("Cannot move:"));
                h.frame(vec![button(to, false)]);
                h.move_clean();
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), stats);
                assert_eq!(h.app.editor.scene, scene);
            }
            let (mut h, id) = Harness::movable(kind, false, false, size, scale);
            let mut neighbor = Opening::new(
                "core.opening",
                h.app.editor.document.model().openings[&id]
                    .parameters
                    .clone(),
            );
            neighbor.parameters.offset = 0.05;
            h.app
                .editor
                .command("Left neighbor", Command::AddOpening(neighbor))
                .unwrap();
            h.settle();
            h.use_move_anchor(0.0);
            let before = h.app.editor.document.model().clone();
            let stats = h.app.editor.document.history_stats();
            let to = h.press_move(id, 0.5);
            assert!(h.has_text("Cannot move:"));
            h.frame(vec![button(to, false)]);
            h.move_clean();
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), stats);
        }
    }
}

fn drag_invalid(anchor: f64) {
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
                h.use_move_anchor(anchor);
                // Exercise the raw clearance boundary separately from snap acquisition.
                h.app.plans.snaps.enabled = false;
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let invalid = if anchor == 0.0 {
                    [0.0005, 1.5 + width, 1.5 + width - 0.0005, 4.5]
                } else if anchor == 1.0 {
                    [
                        8.0 - width - 0.0005,
                        4.5 - width - 0.0005,
                        1.5 - width,
                        1.5 - width + 0.0005,
                    ]
                } else {
                    [0.0005, 8.0 - width - 0.0005, 4.5, 4.5 - width - 0.0005]
                };
                for offset in invalid {
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
    drag_cancel(0.5);
}

#[test]
fn opening_jamb_cancel_stale_outside_and_click_leave_no_residue() {
    for anchor in [0.0, 1.0] {
        drag_cancel(anchor);
    }
}

fn drag_cancel(anchor: f64) {
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
                h.use_move_anchor(anchor);
                let camera = h.app.plans.cameras[&h.view];
                let width = h
                    .app
                    .editor
                    .document
                    .model()
                    .resolve_opening(&h.app.editor.document.model().openings[&id].parameters)
                    .unwrap()
                    .width;
                let mut to = h.press_move(
                    id,
                    if anchor == 0.5 {
                        4.08 - width * 0.5
                    } else {
                        2.5
                    },
                );
                if anchor == 0.5 {
                    assert!(
                        (h.app
                            .plans
                            .opening_move
                            .as_ref()
                            .unwrap()
                            .parameters()
                            .offset
                            + width * 0.5
                            - 4.0)
                            .abs()
                            < 1e-5,
                        "cancel an acquired snap"
                    );
                }
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
                        assert!(h.app.plans.opening_move.is_none());
                        assert!(h.app.plans.opening_move_claimed);
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
            h.use_move_anchor(anchor);
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
fn opening_jamb_projection_radius_collision_and_tool_precedence() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for anchor in [0.0, 1.0] {
                let (mut h, id) = Harness::movable(kind, true, false, size, scale);
                h.use_move_anchor(anchor);
                assert_eq!(h.jamb_handles(), 2);
                let pos = h.opening_point(id, 1.5);
                // A perpendicular acquisition checks a logical-pixel disc, independent of DPI.
                for distance in [10.1, 9.9] {
                    let grab = pos + egui::vec2(-0.6, -0.8) * distance;
                    h.hover(grab);
                    h.frame(vec![button(grab, true)]);
                    assert_eq!(h.app.plans.opening_move.is_some(), distance < 10.0);
                    h.frame(vec![button(grab, false)]);
                    h.move_clean();
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                }
                h.hover(pos);
                assert!(h.has_text("Resize opening"));
                assert_eq!(
                    h.output.platform_output.cursor_icon,
                    egui::CursorIcon::ResizeNeSw
                );
                h.app
                    .plans
                    .cameras
                    .get_mut(&h.view)
                    .unwrap()
                    .pixels_per_metre = 20.0;
                h.frame(vec![]);
                assert_eq!(
                    h.jamb_handles(),
                    0,
                    "overlapping acquisition discs suppressed"
                );
                assert_eq!(h.handles(), 1, "center remains");
                h.use_move_anchor(anchor);
                let camera = h.app.plans.cameras[&h.view];
                let blank = h.point(Point2::new(0.0, 1.5));
                h.drag(blank, blank + egui::vec2(20.0, 10.0));
                assert_ne!(h.app.plans.cameras[&h.view], camera);
                h.begin(kind);
                assert_eq!(h.jamb_handles(), 0);
                let from = h.opening_point(id, 1.5);
                h.drag(from, h.opening_point(id, 2.5));
                assert!(h.app.plans.opening_move.is_none());
                h.frame(vec![escape()]);
                h.app.select(Some(id));
                h.frame(vec![]);
                h.ribbon_click("Rehost", 58.0..180.0);
                assert!(h.app.plans.opening_rehost.is_some());
                assert_eq!(h.jamb_handles(), 0);
                h.frame(vec![escape()]);
                h.app.select(Some(h.wall));
                h.frame(vec![]);
                assert_eq!(h.jamb_handles(), 0);
                assert_eq!(h.handles(), 2);
            }
        }
    }
}

#[test]
fn opening_jamb_snap_uses_host_station_and_excludes_edited_aperture() {
    opening_grip_snap(&[0.0, 1.0]);
}

#[test]
fn opening_center_snap_uses_host_station_and_excludes_edited_aperture() {
    opening_grip_snap(&[0.5]);
}

fn opening_grip_snap(anchors: &[f64]) {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                for legacy in [false, true] {
                    for &anchor in anchors {
                        let (mut h, id) = Harness::movable(kind, reversed, legacy, size, scale);
                        if anchor == 0.5 && !legacy {
                            let mut parameters = h.app.editor.document.model().openings[&id]
                                .parameters
                                .clone();
                            parameters.width_override = Some(if kind == OpeningKind::Door {
                                0.96
                            } else {
                                1.26
                            });
                            parameters.height_override =
                                Some(if kind == OpeningKind::Door { 2.05 } else { 1.1 });
                            parameters.sill_override = (kind == OpeningKind::Window).then_some(0.8);
                            h.app
                                .editor
                                .command(
                                    "Instance overrides",
                                    Command::UpdateOpening { id, parameters },
                                )
                                .unwrap();
                            h.settle();
                        }
                        let original_offset = if anchor == 0.0 { 4.5 } else { 1.5 };
                        if anchor == 0.0 {
                            let mut parameters = h.app.editor.document.model().openings[&id]
                                .parameters
                                .clone();
                            parameters.offset = original_offset;
                            h.app
                                .editor
                                .command(
                                    "Prepare start jamb",
                                    Command::UpdateOpening { id, parameters },
                                )
                                .unwrap();
                            h.settle();
                        }
                        h.use_move_anchor(anchor);
                        h.app.plans.snaps.enabled = true;
                        let width = h
                            .app
                            .editor
                            .document
                            .model()
                            .resolve_opening(
                                &h.app.editor.document.model().openings[&id].parameters,
                            )
                            .unwrap()
                            .width;
                        let target = 4.0 - width * anchor;
                        let before = h.app.editor.document.model().clone();
                        let scene = h.app.editor.scene.clone();
                        let history = h.app.editor.document.history_stats();
                        let camera = h.app.plans.cameras[&h.view];
                        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                        let to = h.press_move(id, target + 0.08);
                        let draft = h.app.plans.opening_move.as_ref().unwrap();
                        let preview = draft.preview(&h.app.editor).unwrap();
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.document.history_stats(), history);
                        assert_eq!(h.app.editor.scene, scene);
                        assert_eq!(h.app.plans.cameras[&h.view], camera);
                        assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                        h.frame(vec![button(to, false)]);
                        h.settle();
                        h.move_clean();
                        let model = h.app.editor.document.model();
                        let changed = model
                            .resolve_opening(&model.openings[&id].parameters)
                            .unwrap();
                        assert!((changed.offset + changed.width * anchor - 4.0).abs() < 1e-5);
                        let after = model.clone();
                        if anchor == 0.5 {
                            let mut expected = before.clone();
                            expected.openings.get_mut(&id).unwrap().parameters.offset =
                                changed.offset;
                            assert_eq!(after, expected, "center move edits only offset");
                        }
                        assert_eq!(
                            h.app.editor.document.history_stats().undo_entries,
                            history.undo_entries + 1
                        );
                        assert_ne!(h.app.editor.scene, scene);
                        let committed_context = h.app.editor.native_plan_context(h.view).unwrap();
                        let committed = h.app.plans.drawing.as_ref().unwrap();
                        assert_eq!(
                            preview.items(context).unwrap(),
                            committed.items(committed_context).unwrap()
                        );
                        assert_eq!(
                            preview.provider_lines(context).unwrap(),
                            committed.provider_lines(committed_context).unwrap()
                        );
                        h.app.history(false);
                        h.settle();
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.scene, scene);
                        h.app.history(true);
                        h.settle();
                        assert_eq!(h.app.editor.document.model(), &after);
                        h.app.history(false);
                        h.settle();
                        let to = h.press_move(id, original_offset + 0.2);
                        h.frame(vec![button(to, false)]);
                        h.settle();
                        let model = h.app.editor.document.model();
                        let changed = model
                            .resolve_opening(&model.openings[&id].parameters)
                            .unwrap();
                        assert!(
                            (changed.offset + changed.width * anchor
                                - (original_offset + width * anchor + 0.2))
                                .abs()
                                < 1e-5,
                            "selected symbol/aperture cannot snap back to itself"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn opening_center_snap_corrects_grab_offset_and_respects_preferences_and_crop() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                for mode in ["enabled", "disabled", "no midpoint", "cropped"] {
                    let (mut h, id) = Harness::movable(kind, reversed, false, size, scale);
                    h.app.plans.snaps.enabled = mode != "disabled";
                    h.app.plans.snaps.midpoints = mode != "no midpoint";
                    if mode == "cropped" {
                        let camera = h.app.plans.cameras[&h.view];
                        let mut settings = h.app.editor.document.model().views[&h.view]
                            .parameters
                            .plan
                            .unwrap();
                        // Keep the source and station 3.8 visible, but exclude the host midpoint.
                        settings.crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(if reversed { 0.04 } else { -4.0 }, -3.0),
                            max: Point2::new(if reversed { 4.0 } else { -0.04 }, 3.0),
                        });
                        h.app
                            .editor
                            .update_floor_plan(
                                h.view,
                                "Crop midpoint",
                                h.app.active_level,
                                settings,
                            )
                            .unwrap();
                        h.settle();
                        h.app.plans.cameras.insert(h.view, camera);
                        h.frame(vec![]);
                    }
                    let before = h.app.editor.document.model().clone();
                    let history = h.app.editor.document.history_stats();
                    let scene = h.app.editor.scene.clone();
                    let p = before
                        .resolve_opening(&before.openings[&id].parameters)
                        .unwrap();
                    let center = h.opening_point(id, p.offset);
                    let direction = (h.opening_point(id, p.offset + 1.0) - center).normalized();
                    let correction = -direction * 6.0;
                    let grab = center + correction;
                    h.hover(grab);
                    h.frame(vec![button(grab, true)]);
                    assert_eq!(
                        h.app.plans.opening_move.as_ref().unwrap().parameters(),
                        before.openings[&id].parameters
                    );
                    // The corrected center is 7 px from midpoint; the raw pointer is 13 px away.
                    let to = h.opening_point(id, 3.8 - p.width * 0.5) + correction;
                    h.hover(to);
                    let candidate = h.app.plans.opening_move.as_ref().unwrap().parameters();
                    let expected = if mode == "enabled" { 4.0 } else { 3.8 };
                    assert!(
                        (candidate.offset + p.width * 0.5 - expected).abs() < 1e-5,
                        "{mode}"
                    );
                    // Returning to the original off-center press is a no-op even with snaps on.
                    h.frame(vec![egui::Event::PointerMoved(grab), button(grab, false)]);
                    h.move_clean();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats(), history);
                    assert_eq!(h.app.editor.scene, scene);
                }
            }
        }
    }
}

#[test]
fn opening_center_snap_revalidates_endpoint_and_neighbor_on_release() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reversed in [false, true] {
                let (mut h, id) = Harness::movable(kind, reversed, false, size, scale);
                let mut neighbor = Opening::new(
                    "core.opening",
                    h.app.editor.document.model().openings[&id]
                        .parameters
                        .clone(),
                );
                neighbor.parameters.offset = 5.0;
                h.app
                    .editor
                    .command("Neighbor", Command::AddOpening(neighbor))
                    .unwrap();
                h.settle();
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                let scene = h.app.editor.scene.clone();
                let width = before
                    .resolve_opening(&before.openings[&id].parameters)
                    .unwrap()
                    .width;
                for station in [0.0, 5.0] {
                    h.press_move(id, 4.08 - width * 0.5);
                    assert!(
                        h.app
                            .plans
                            .opening_move
                            .as_ref()
                            .unwrap()
                            .preview(&h.app.editor)
                            .is_ok()
                    );
                    let to = h.opening_point(id, station + 0.08 - width * 0.5);
                    h.hover(to);
                    let draft = h.app.plans.opening_move.as_ref().unwrap();
                    assert!((draft.parameters().offset + width * 0.5 - station).abs() < 1e-5);
                    assert!(
                        draft.preview(&h.app.editor).is_err(),
                        "snap still requires host fit and clearance"
                    );
                    // Restore a valid preview, then release directly at the invalid snapped target.
                    h.hover(h.opening_point(id, 4.08 - width * 0.5));
                    h.frame(vec![button(to, false)]);
                    h.move_clean();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats(), history);
                    assert_eq!(h.app.editor.scene, scene);
                }
            }
        }
    }
}

#[test]
fn opening_jamb_hidden_crop_and_host_level_visibility() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for reason in ["hidden", "crop", "level"] {
                let (mut h, id) = Harness::movable(kind, false, false, size, scale);
                h.use_move_anchor(0.0);
                assert_eq!(h.jamb_handles(), 2);
                let mut model = h.app.editor.document.model().clone();
                let parameters = &mut model.views.get_mut(&h.view).unwrap().parameters;
                match reason {
                    "hidden" => parameters.plan.as_mut().unwrap().visibility.walls = false,
                    "crop" => {
                        parameters.plan.as_mut().unwrap().crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(0.0, -3.0),
                            max: Point2::new(4.0, 3.0),
                        })
                    }
                    "level" => {
                        let mut level = model.levels[&h.app.active_level].clone();
                        level = os_model::Level::new("core.level", level.parameters);
                        level.parameters.name = "Other".into();
                        level.parameters.elevation = 4.0;
                        parameters.level = Some(level.id());
                        model.levels.insert(level.id(), level);
                    }
                    _ => unreachable!(),
                }
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.settle();
                assert_eq!(h.jamb_handles(), 0, "{reason}");
                // A press where the old jamb was cannot acquire a hidden opening.
                let pos = h.opening_point(id, 1.5);
                h.hover(pos);
                h.frame(vec![button(pos, true)]);
                assert!(h.app.plans.opening_move.is_none(), "{reason}");
                h.frame(vec![button(pos, false)]);
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
fn opening_copy_preserves_instance_and_type_on_another_wall() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for legacy in [false, true] {
                let (mut h, source_id, target_wall) =
                    Harness::rehostable(kind, legacy, false, false, size, scale);
                let mut model = h.app.editor.document.model().clone();
                let source = &mut model.openings.get_mut(&source_id).unwrap().parameters;
                source.name = "Preserved instance name".into();
                if !legacy {
                    source.width_override = Some(if kind == OpeningKind::Door { 1.0 } else { 1.3 });
                    source.height_override =
                        Some(if kind == OpeningKind::Door { 2.0 } else { 1.1 });
                    source.lite_side_override = Some(os_model::LiteSide::End);
                    let type_id = source.type_id().unwrap();
                    model
                        .opening_types
                        .get_mut(&type_id)
                        .unwrap()
                        .parameters
                        .family
                        .side_lite = Some(os_model::SideLite {
                        side: os_model::LiteSide::Start,
                        width_fraction: 0.25,
                        mullion_width: 0.04,
                        material: None,
                    });
                    if kind == OpeningKind::Window {
                        source.sill_override = Some(0.75);
                    }
                }
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.editor.pending_geometry.extend([source_id, h.wall]);
                h.app.editor.regenerate().unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.app.select(Some(source_id));
                h.settle();
                h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(0.0, 0.5);
                h.frame(vec![]);

                let before = h.app.editor.document.model().clone();
                let before_scene = h.app.editor.scene.clone();
                let source_params = before.openings[&source_id].parameters.clone();
                let source_resolved = before.resolve_opening(&source_params).unwrap();
                let history = h.app.editor.document.history_stats();
                h.text_click("Copy opening");
                h.assert_active(kind);
                assert_eq!(
                    h.app.plans.opening_placement.as_ref().unwrap().source,
                    Some(source_id)
                );

                // An overlapping placement is rejected without changing model/history/scene.
                let source_point = h.opening_point(source_id, source_params.offset);
                h.hover(source_point);
                h.frame(vec![button(source_point, true)]);
                h.frame(vec![button(source_point, false)]);
                h.frame(vec![]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(h.app.editor.scene, before_scene);
                h.assert_active(kind);

                // Place on a different visible wall, preserving source data exactly
                // except for the new host and projected first-jamb offset.
                let target_point = h.point(Point2::new(-2.0, 2.0));
                h.hover(target_point);
                assert!(h.has_text(match kind {
                    OpeningKind::Door => "Door ·",
                    OpeningKind::Window => "Window ·",
                }));
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, before_scene);
                h.click(target_point);
                h.settle();
                let copy_id = h.app.selected.unwrap();
                assert_ne!(copy_id, source_id);
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    history.undo_entries + 1
                );
                let after = h.app.editor.document.model().clone();
                let copied = &after.openings[&copy_id].parameters;
                let mut expected = source_params.clone();
                expected.host = target_wall;
                expected.offset = copied.offset;
                assert_eq!(copied, &expected);
                let target_parameters = &after.walls[&target_wall].parameters;
                let expected_offset = 2.0 - source_resolved.width * 0.5;
                assert!((copied.offset - expected_offset).abs() < 0.03);
                assert_eq!(copied.host, target_wall);
                assert_eq!(
                    target_parameters.level,
                    before.walls[&source_params.host].parameters.level
                );
                let copied_resolved = after.resolve_opening(copied).unwrap();
                assert_eq!(copied_resolved.kind, kind);
                assert_eq!(copied_resolved.width, source_resolved.width);
                assert_eq!(copied_resolved.height, source_resolved.height);
                assert_eq!(copied_resolved.sill, source_resolved.sill);
                assert_eq!(copied_resolved.type_id, source_resolved.type_id);
                if !legacy {
                    assert_eq!(
                        copied_resolved.family.side_lite.as_ref().unwrap().side,
                        os_model::LiteSide::End,
                        "copy retains the pinned per-instance lite end"
                    );
                }
                assert_eq!(after.openings.len(), before.openings.len() + 1);
                assert!(h.app.editor.scene.contains_key(&copy_id));
                h.frame(vec![]);
                h.assert_active(kind);
                assert_eq!(
                    h.app.plans.opening_placement.as_ref().unwrap().source,
                    Some(source_id)
                );

                h.app.history(false);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &before);
                h.app.history(true);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &after);
                h.frame(vec![escape()]);
                assert!(h.app.plans.opening_placement.is_none());
            }
        }
    }
}

#[test]
fn opening_copy_cancels_when_provider_context_changes() {
    for (size, scale) in PROFILES {
        let (mut h, source_id) = Harness::movable(OpeningKind::Door, false, false, size, scale);
        let original = h.app.editor.document.model().clone();
        h.app.begin_opening_copy(source_id);
        h.frame(vec![]);
        assert!(h.app.plans.opening_placement.is_some());
        h.app
            .plans
            .opening_placement
            .as_mut()
            .unwrap()
            .provider_signature
            .push(("org.example.changed-provider".into(), h.wall));
        h.frame(vec![]);
        assert!(h.app.plans.opening_placement.is_none());
        assert!(h.app.status.contains("provider changed"));
        assert_eq!(h.app.editor.document.model(), &original);
        h.click(h.point(Point2::new(0.0, 2.0)));
        assert_eq!(h.app.editor.document.model(), &original);
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
            for _ in 0..20 {
                if h.has_text("Opening preview") {
                    break;
                }
                h.frame(vec![
                    egui::Event::PointerMoved(egui::pos2(size.x / 2., size.y / 2.)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., -45.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                for _ in 0..8 {
                    h.frame(vec![]);
                }
            }
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
