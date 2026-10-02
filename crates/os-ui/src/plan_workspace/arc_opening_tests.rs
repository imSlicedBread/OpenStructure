use super::*;
use std::f64::consts::PI;

fn arc_harness(size: egui::Vec2, scale: f32) -> Harness {
    let mut h = Harness::new(size, scale);
    let mut model = h.app.editor.document.model().clone();
    model.walls.get_mut(&h.wall).unwrap().parameters.path = os_model::WallPath::CircularArc {
        center: Point2::default(),
        radius: 4.0,
        start_angle_rad: PI,
        signed_sweep_rad: -PI,
    };
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.insert(h.wall);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.frame(vec![]);
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(0.0, 2.0);
    h.app
        .plans
        .cameras
        .get_mut(&h.view)
        .unwrap()
        .pixels_per_metre = 35.0;
    h.settle();
    h
}

#[test]
fn curved_host_door_and_window_draw_width_preview_commit_plan_and_geometry_both_dpis() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let mut h = arc_harness(size, scale);
            let path = h.app.editor.document.model().walls[&h.wall].parameters.path;
            h.begin_width(kind);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            let from = h.point(path.point(4.8));
            // Keep the target outside the wall-midpoint snap aperture.
            let to = h.point(path.point(7.2));

            h.hover(from);
            h.frame(vec![button(from, true)]);
            h.hover(to);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert!(h.preview_lines() > 0);
            h.frame(vec![button(to, false)]);
            h.settle();

            let model = h.app.editor.document.model();
            assert_eq!(model.openings.len(), 1);
            let opening = model.openings.values().next().unwrap();
            let opening_id = opening.id();
            assert_eq!(opening.parameters.host, h.wall);
            let resolved = model.resolve_opening(&opening.parameters).unwrap();
            assert!((opening.parameters.offset - 4.8).abs() < 1e-4);
            assert!(
                (resolved.width - 2.4).abs() < 1e-4,
                "unexpected curved opening width: {} (offset {})",
                resolved.width,
                opening.parameters.offset
            );
            assert_eq!(resolved.kind, kind);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);

            let context = h.app.editor.native_plan_context(h.view).unwrap();
            let drawing = h.app.editor.native_wall_plan(h.view).unwrap();
            let symbols = drawing.provider_lines(context).unwrap();
            assert!(symbols.iter().any(|line| line.entity == opening.id()));
            if kind == OpeningKind::Door {
                // The rigid leaf is one Euclidean plan segment. A chord-sampled
                // leaf would instead produce many short segments along the host.
                assert_eq!(
                    symbols
                        .iter()
                        .filter(|line| {
                            line.entity == opening.id()
                                && (line.start.distance(line.end) - resolved.width).abs() < 1e-3
                        })
                        .count(),
                    1
                );
            }
            let native = os_geometry::walls::NativeWall::from_model(model, h.wall).unwrap();
            let gross = path.length() * 0.25 * 3.0;
            assert!(native.net_volume().unwrap() < gross);
            assert!(native.mesh().unwrap().signed_volume() < gross);

            // Center-drag remains a station edit on the analytic host path.
            let old_offset = opening.parameters.offset;
            h.frame(vec![escape()]);
            h.app.select(Some(opening_id));
            h.settle();
            let before_move = h.app.editor.document.model().clone();
            let scene_before_move = h.app.editor.scene.clone();
            let history_before_move = h.app.editor.document.history_stats();
            let from = h.opening_point(opening_id, old_offset);
            let target_offset = 6.2;
            let to = h.opening_point(opening_id, target_offset);
            h.hover(from);
            h.frame(vec![button(from, true)]);
            assert!(h.app.plans.opening_move.is_some());
            h.hover(to);
            assert_eq!(h.app.editor.document.model(), &before_move);
            assert_eq!(h.app.editor.scene, scene_before_move);
            assert!(
                h.app
                    .plans
                    .opening_move
                    .as_ref()
                    .unwrap()
                    .preview(&h.app.editor)
                    .is_ok()
            );
            h.frame(vec![button(to, false)]);
            h.settle();
            let after_move = h.app.editor.document.model().clone();
            assert!(
                (after_move.openings[&opening_id].parameters.offset - target_offset).abs() < 1e-4
            );
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history_before_move.undo_entries + 1
            );
            assert_ne!(h.app.editor.scene, scene_before_move);
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before_move);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after_move);

            // Same-host arrays advance in centerline stations and validate the
            // complete curved batch before creating any instances.
            h.app.select(Some(opening_id));
            h.settle();
            let before_array = h.app.editor.document.model().clone();
            let history_before_array = h.app.editor.document.history_stats();
            h.app.begin_opening_array();
            let array = h.app.plans.opening_array.as_mut().unwrap();
            array.count = 2;
            array.spacing = 3.4;
            array.toward_end = true;
            let preview = array.preview(&before_array).unwrap();
            assert!(preview.error.is_none(), "{:?}", preview.error);
            assert_eq!(preview.openings.len(), 1);
            h.frame(vec![]);
            h.frame(vec![]);
            assert_eq!(h.app.editor.document.model(), &before_array);
            h.text_click("Apply array");
            h.settle();
            let after_array = h.app.editor.document.model().clone();
            assert_eq!(after_array.openings.len(), 2, "{}", h.app.status);
            assert!((after_array.openings[&opening_id].parameters.offset - 6.2).abs() < 1e-4);
            let copy = after_array
                .openings
                .values()
                .find(|opening| opening.id() != opening_id)
                .unwrap();
            assert!((copy.parameters.offset - 9.6).abs() < 1e-4);
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history_before_array.undo_entries + 1
            );
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before_array);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after_array);
        }
    }
}

#[test]
fn arc_door_flip_high_zoom_rows_and_cropped_analytic_center() {
    for (size, scale) in PROFILES {
        let (mut h, id) = arc_door_flip_fixture(size, scale, false, true);
        let mut model = h.app.editor.document.model().clone();
        // Center the aperture on the top of a minor arc. Both jambs and their
        // chord midpoint are below the true center, exposing the old overlap.
        let path = os_model::WallPath::CircularArc {
            center: Point2::default(),
            radius: 2.0,
            start_angle_rad: PI / 2.0 - 1.2,
            signed_sweep_rad: 2.8,
        };
        model.walls.get_mut(&h.wall).unwrap().parameters.path = path;
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
                center: path.point(2.4),
                pixels_per_metre: 95.0,
            },
        );
        h.frame(vec![]);
        let [start, center, end] = arc_flip_anchors(&h, id);
        assert!(center.distance(start.lerp(end, 0.5)) > 18.0);
        for label in ["Hinge", "Swing"] {
            let rect = painted_flip_rect(&h, label);
            assert!((rect.center().y - (center.y - 30.0)).abs() < 0.01);
            for grip in [start, center, end] {
                assert!(
                    !rect
                        .expand(4.0)
                        .intersects(egui::Rect::from_center_size(grip, egui::vec2(20.0, 20.0)))
                );
            }
        }
        // With the center 25 logical pixels below the top, only the lower row fits.
        let canvas = h.app.plans.canvas_rect.unwrap();
        h.app.plans.cameras.get_mut(&h.view).unwrap().center.y -=
            f64::from(canvas.height() / 2.0 - 25.0) / 95.0;
        h.frame(vec![]);
        let [start, _, end] = arc_flip_anchors(&h, id);
        for label in ["Hinge", "Swing"] {
            assert!(
                (painted_flip_rect(&h, label).center().y - (start.y.max(end.y) + 30.0)).abs()
                    < 0.01
            );
        }
        // A fast click on the fallback row still commits exactly once.
        let pos = h.direct_door_button("Swing").unwrap();
        let mut expected = h.app.editor.document.model().clone();
        expected.openings.get_mut(&id).unwrap().parameters.swing = os_model::DoorSwing::Left;
        h.hover(pos);
        h.frame(vec![button(pos, true), button(pos, false)]);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &expected);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        h.move_clean();
        // Keep all anchors on canvas while neither full button row fits horizontally.
        h.app
            .plans
            .cameras
            .get_mut(&h.view)
            .unwrap()
            .pixels_per_metre = 35.0;
        h.app.plans.cameras.get_mut(&h.view).unwrap().center =
            Point2::new(f64::from(canvas.width() / 2.0 - 35.0) / 35.0, 2.0);
        h.frame(vec![]);
        let _ = arc_flip_anchors(&h, id);
        assert!(h.direct_door_button("Hinge").is_none());
        assert!(h.direct_door_button("Swing").is_none());
        h.app.plans.cameras.insert(
            h.view,
            PlanCamera {
                center: path.point(2.4),
                pixels_per_metre: 95.0,
            },
        );
        h.frame(vec![]);
        let old = h.direct_door_button("Hinge").unwrap();
        let mut settings = h.app.editor.document.model().views[&h.view]
            .parameters
            .plan
            .unwrap();
        settings.crop = Some(os_model::PlanViewCrop {
            min: Point2::new(-3.0, -3.0),
            max: Point2::new(3.0, (path.point(1.5).y + path.point(2.4).y) / 2.0),
        });
        h.app
            .editor
            .update_floor_plan(h.view, "Crop analytic center", h.app.active_level, settings)
            .unwrap();
        h.settle();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .provider_lines(context)
                .unwrap()
                .iter()
                .any(|line| line.entity == id)
        );
        assert!(
            h.direct_door_button("Hinge").is_none(),
            "visible jambs cannot stand in for a cropped center"
        );
        assert!(h.direct_door_button("Swing").is_none());
        let before = h.app.editor.document.model().clone();
        h.click(old);
        assert!(h.app.plans.opening_flip.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
    }
}

fn arc_door_flip_fixture(
    size: egui::Vec2,
    scale: f32,
    legacy: bool,
    positive_sweep: bool,
) -> (Harness, Id) {
    let (mut h, id) = Harness::movable(OpeningKind::Door, false, legacy, size, scale);
    let mut model = h.app.editor.document.model().clone();
    let sign = if positive_sweep { 1.0 } else { -1.0 };
    let path = os_model::WallPath::CircularArc {
        center: Point2::default(),
        radius: 2.0,
        start_angle_rad: sign * 5.8,
        signed_sweep_rad: sign * 1.5 * PI,
    };
    model.walls.get_mut(&h.wall).unwrap().parameters.path = path;
    let p = &mut model.openings.get_mut(&id).unwrap().parameters;
    match &mut p.definition {
        OpeningDefinition::Legacy { width, .. } => *width = 1.8,
        OpeningDefinition::Typed { type_id } => {
            p.width_override = Some(1.8);
            model
                .opening_types
                .get_mut(type_id)
                .unwrap()
                .parameters
                .family
                .frame_width = 0.04;
        }
    }
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
            center: path.point(2.4),
            pixels_per_metre: 35.0,
        },
    );
    h.frame(vec![]);
    (h, id)
}

fn arc_flip_anchors(h: &Harness, id: Id) -> [egui::Pos2; 3] {
    let model = h.app.editor.document.model();
    let p = model
        .resolve_opening(&model.openings[&id].parameters)
        .unwrap();
    let path = model.walls[&p.host].parameters.path;
    [0.0, 0.5, 1.0].map(|fraction| h.point(path.point(p.offset + p.width * fraction)))
}

fn painted_flip_rect(h: &Harness, label: &str) -> egui::Rect {
    let pos = h
        .direct_door_button(label)
        .expect("painted arc flip button");
    h.output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(r)
                if r.rect.contains(pos) && r.rect.size() == egui::vec2(58.0, 24.0) =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn arc_door_flip_independent_fields_geometry_history_both_dpis() {
    for (size, scale) in PROFILES {
        for legacy in [false, true] {
            for positive in [false, true] {
                let (mut h, id) = arc_door_flip_fixture(size, scale, legacy, positive);
                let [start, center, end] = arc_flip_anchors(&h, id);
                assert!(center.distance(start.lerp(end, 0.5)) > 5.0);
                let hinge = painted_flip_rect(&h, "Hinge");
                let swing = painted_flip_rect(&h, "Swing");
                assert!(((hinge.center().x + swing.center().x) / 2.0 - center.x).abs() < 0.01);
                assert!(!hinge.intersects(swing));
                for rect in [hinge, swing] {
                    assert!(
                        h.app
                            .plans
                            .canvas_rect
                            .unwrap()
                            .contains_rect(rect.expand(4.0))
                    );
                    for grip in [start, center, end] {
                        assert!(!rect.expand(4.0).intersects(egui::Rect::from_center_size(
                            grip,
                            egui::vec2(20.0, 20.0)
                        )));
                    }
                }
                for label in ["Hinge", "Swing", "Hinge", "Swing"] {
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let history = h.app.editor.document.history_stats();
                    let revision = h.app.editor.document.revision();
                    let camera = h.app.plans.cameras[&h.view];
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let drawing = h.app.plans.drawing.as_ref().unwrap();
                    let identity = drawing.identity();
                    let items = drawing.items(context).unwrap().to_vec();
                    let lines = drawing.provider_lines(context).unwrap().to_vec();
                    let pos = h.direct_door_button(label).unwrap();
                    h.hover(pos);
                    h.frame(vec![button(pos, true)]);
                    assert!(h.app.plans.opening_flip.is_some());
                    assert!(h.app.plans.opening_move.is_none());
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.editor.document.history_stats(), history);
                    assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), identity);
                    h.frame(vec![button(pos, false)]);
                    h.settle();
                    h.move_clean();
                    assert!(!h.app.plans.opening_flip_claimed);
                    assert!(h.app.plans.opening_flip.is_none());
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
                        "only the chosen field changes"
                    );
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        history.undo_entries + 1
                    );
                    assert_eq!(h.app.editor.document.revision(), revision + 1);
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                    assert_eq!(h.app.selected, Some(id));
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
                    let after_lines = drawing.provider_lines(context).unwrap().to_vec();
                    assert_ne!(after_lines, lines);
                    // The rigid leaf follows the local tangent at the stored-station
                    // hinge, even when the host wraps through zero on a major arc.
                    let p = expected
                        .resolve_opening(&expected.openings[&id].parameters)
                        .unwrap();
                    let wall = &expected.walls[&h.wall].parameters;
                    let station = p.offset
                        + if p.hinge == os_model::DoorHinge::End {
                            p.width
                        } else {
                            0.0
                        };
                    let tangent = wall.path.tangent(station);
                    let side = if p.swing == os_model::DoorSwing::Left {
                        1.0
                    } else {
                        -1.0
                    };
                    let anchor = wall.path.point(station);
                    let leaf = after_lines
                        .iter()
                        .find(|line| line.entity == id && line.feature == 2)
                        .unwrap();
                    let a = context.basis.plane_to_world(leaf.start).unwrap();
                    let b = context.basis.plane_to_world(leaf.end).unwrap();
                    assert!(
                        a.distance(Point2::new(
                            anchor.x
                                + side * (wall.thickness / 2.0 - p.family.frame_width) * tangent.y,
                            anchor.y
                                - side * (wall.thickness / 2.0 - p.family.frame_width) * tangent.x
                        )) < 1e-8
                    );
                    assert!((a.distance(b) - (p.width - 2.0 * p.family.frame_width)).abs() < 1e-8);
                    assert!(((b.x - a.x) * tangent.x + (b.y - a.y) * tangent.y).abs() < 1e-8);
                    assert_eq!(
                        tangent.x * (b.y - a.y) - tangent.y * (b.x - a.x) > 0.0,
                        p.swing == os_model::DoorSwing::Left
                    );
                    let after_scene = h.app.editor.scene.clone();
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(
                        h.app
                            .plans
                            .drawing
                            .as_ref()
                            .unwrap()
                            .provider_lines(h.app.editor.native_plan_context(h.view).unwrap())
                            .unwrap(),
                        lines
                    );
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &expected);
                    assert_eq!(h.app.editor.scene, after_scene);
                    assert_eq!(
                        h.app
                            .plans
                            .drawing
                            .as_ref()
                            .unwrap()
                            .provider_lines(h.app.editor.native_plan_context(h.view).unwrap())
                            .unwrap(),
                        after_lines
                    );
                }
            }
        }
    }
}

#[test]
fn arc_door_flip_selection_and_cancel_own_the_entire_press() {
    for (size, scale) in PROFILES {
        for reason in [
            "multiple",
            "nonmember",
            "escape",
            "lost",
            "center",
            "jamb",
            "other",
            "outside",
        ] {
            let (mut h, id) = arc_door_flip_fixture(size, scale, false, true);
            let pos = h.direct_door_button("Hinge").unwrap();
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            let camera = h.app.plans.cameras[&h.view];
            h.hover(pos);
            h.frame(vec![button(pos, true)]);
            assert!(h.app.plans.opening_flip.is_some());
            let mut release = pos;
            match reason {
                "multiple" => {
                    h.app.selected_ids.insert(h.wall);
                }
                "nonmember" => {
                    h.app.selected_ids = [h.wall].into();
                }
                "escape" => h.frame(vec![escape()]),
                "lost" => h.frame(vec![egui::Event::PointerGone]),
                "center" => release = arc_flip_anchors(&h, id)[1],
                "jamb" => release = arc_flip_anchors(&h, id)[0],
                "other" => release = h.direct_door_button("Swing").unwrap(),
                "outside" => release = egui::pos2(-10.0, -10.0),
                _ => unreachable!(),
            }
            h.hover(release);
            if matches!(reason, "multiple" | "nonmember") {
                assert!(h.direct_door_button("Hinge").is_none());
                assert!(h.direct_door_button("Swing").is_none());
            }
            assert!(h.app.plans.opening_flip.is_none(), "{reason}");
            assert!(
                h.app.plans.opening_flip_claimed,
                "cancel retains ownership: {reason}"
            );
            h.frame(vec![button(release, false)]);
            h.frame(vec![]);
            h.move_clean();
            assert!(!h.app.plans.opening_flip_claimed);
            assert_eq!(h.app.editor.document.model(), &before, "{reason}");
            assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
            assert_eq!(h.app.editor.scene, scene, "{reason}");
            assert_eq!(h.app.plans.cameras[&h.view], camera, "{reason}");
            assert_eq!(h.app.selected, Some(id), "{reason}");
            if matches!(reason, "multiple" | "nonmember") {
                h.hover(pos);
                h.frame(vec![button(pos, true)]);
                assert!(h.app.plans.opening_flip.is_none());
                h.frame(vec![button(pos, false)]);
                assert_eq!(h.app.editor.document.model(), &before);
            }
        }
    }
}

fn arc_window_flip_fixture(size: egui::Vec2, scale: f32, positive: bool) -> (Harness, Id) {
    let (mut h, id) = Harness::movable(OpeningKind::Window, false, false, size, scale);
    let mut model = h.app.editor.document.model().clone();
    let sign = if positive { 1.0 } else { -1.0 };
    let path = os_model::WallPath::CircularArc {
        center: Point2::default(),
        radius: 2.0,
        start_angle_rad: sign * 5.8,
        signed_sweep_rad: sign * 1.5 * PI,
    };
    model.walls.get_mut(&h.wall).unwrap().parameters.path = path;
    let materials = ["Primary pane", "Fixed lite", "Frame"].map(|name| {
        let material = os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: name.into(),
                density_kg_m3: 650.0,
                color: [100, 90, 80],
            },
        );
        let id = material.id();
        model.materials.insert(id, material);
        id
    });
    let p = &mut model.openings.get_mut(&id).unwrap().parameters;
    p.width_override = Some(1.8);
    let ty = &mut model
        .opening_types
        .get_mut(&p.type_id().unwrap())
        .unwrap()
        .parameters;
    ty.pane_position = os_model::WindowPanePosition::LeftFace;
    ty.window_operation = os_model::WindowOperation::Casement;
    ty.family.depth = 0.04;
    ty.family.frame_depth = 0.08;
    ty.family.frame_width = 0.04;
    ty.family.panel_material = Some(materials[0]);
    ty.family.frame_material = Some(materials[2]);
    ty.family.side_lite = Some(os_model::SideLite {
        side: os_model::LiteSide::Start,
        width_fraction: 0.25,
        mullion_width: 0.04,
        material: Some(materials[1]),
    });
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
            center: path.point(2.4),
            // Leave room for both flip controls in the compact high-DPI
            // viewport while still making the curved opening easy to target.
            pixels_per_metre: if scale > 1.0 { 66.0 } else { 70.0 },
        },
    );
    h.frame(vec![]);
    (h, id)
}

fn arc_window_flip_rect(h: &Harness, label: &str) -> egui::Rect {
    let pos = h.direct_door_button(label).expect("painted window control");
    let width = if label == "Flip lite" { 68.0 } else { 58.0 };
    h.output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(r)
                if r.rect.contains(pos)
                    && (r.rect.size() - egui::vec2(width, 24.0)).length() < 0.01 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .unwrap()
}

// Identify components by material and analytic radius/station, independently
// of tessellation order or provider feature indices.
fn assert_arc_window_flip_geometry(h: &Harness, id: Id) {
    let model = h.app.editor.document.model();
    let p = model
        .resolve_opening(&model.openings[&id].parameters)
        .unwrap();
    let wall = &model.resolve_wall(p.host).unwrap().parameters;
    let os_model::WallPath::CircularArc {
        center,
        radius,
        signed_sweep_rad,
        ..
    } = wall.path
    else {
        unreachable!();
    };
    let bays = p.family.bays(p.width).unwrap().unwrap();
    let lite = p.family.side_lite.as_ref().unwrap();
    let mesh = &h.app.editor.scene[&id];
    for (material, bay, depth) in [
        (p.family.panel_material, bays.primary, 0.04),
        (lite.material, bays.lite, 0.04),
        (p.family.frame_material, (0.0, p.width), 0.08),
    ] {
        let points: Vec<_> = mesh
            .triangles
            .iter()
            .zip(&mesh.surfaces)
            .filter(|(_, surface)| surface.material == material)
            .flat_map(|(triangle, _)| triangle.map(|i| mesh.vertices[i as usize]))
            .collect();
        assert!(!points.is_empty());
        let face = match p.pane_position {
            os_model::WindowPanePosition::LeftFace => 1.0,
            os_model::WindowPanePosition::RightFace => -1.0,
            os_model::WindowPanePosition::Center => 0.0,
        };
        let middle_radius =
            radius - signed_sweep_rad.signum() * face * (wall.thickness - depth) / 2.0;
        let radii: Vec<_> = points
            .iter()
            .map(|v| Point2::new(v.x, v.y).distance(center))
            .collect();
        assert!(
            (radii.iter().copied().fold(f64::INFINITY, f64::min) - (middle_radius - depth / 2.0))
                .abs()
                < 1e-7
        );
        assert!(
            (radii.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - (middle_radius + depth / 2.0))
                .abs()
                < 1e-7
        );
        let stations: Vec<_> = points
            .iter()
            .map(|v| wall.path.project(Point2::new(v.x, v.y)))
            .collect();
        assert!(
            (stations.iter().copied().fold(f64::INFINITY, f64::min) - (p.offset + bay.0)).abs()
                < 1e-7
        );
        assert!(
            (stations.iter().copied().fold(f64::NEG_INFINITY, f64::max) - (p.offset + bay.1)).abs()
                < 1e-7
        );
    }
}

#[test]
fn arc_window_flip_fields_geometry_and_history_both_dpis() {
    use os_model::{LiteSide, WindowPanePosition};
    for (size, scale) in PROFILES {
        for positive in [false, true] {
            let (mut h, id) = arc_window_flip_fixture(size, scale, positive);
            let [start, center, end] = arc_flip_anchors(&h, id);
            assert!((center.x - start.lerp(end, 0.5).x).abs() > 5.0);
            let side = arc_window_flip_rect(&h, "Side");
            let lite = arc_window_flip_rect(&h, "Flip lite");
            assert!((side.center().x - center.x).abs() < 0.01);
            assert!((lite.center().x - center.x).abs() < 0.01);
            let top = start.y.min(center.y).min(end.y) - 30.0;
            let bottom = start.y.max(center.y).max(end.y) + 30.0;
            let outward = if (side.center().y - top).abs() < 0.01 {
                -28.0
            } else {
                assert!((side.center().y - bottom).abs() < 0.01);
                28.0
            };
            assert!((lite.center().y - side.center().y - outward).abs() < 0.01);
            assert!(!side.intersects(lite));
            for rect in [side, lite] {
                assert!(
                    h.app
                        .plans
                        .canvas_rect
                        .unwrap()
                        .contains_rect(rect.expand(4.0))
                );
                for anchor in [start, center, end] {
                    assert!(
                        !rect.expand(4.0).intersects(egui::Rect::from_center_size(
                            anchor,
                            egui::vec2(20.0, 20.0)
                        ))
                    );
                }
            }
            assert!(h.direct_door_button("Hinge").is_none());
            assert!(h.direct_door_button("Swing").is_none());
            assert_arc_window_flip_geometry(&h, id);
            // Repeat each flip to prove effective pins are toggled, including
            // retaining an explicit pin when it returns to the type default.
            for label in ["Side", "Flip lite", "Side", "Flip lite"] {
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let history = h.app.editor.document.history_stats();
                let camera = h.app.plans.cameras[&h.view];
                let context = h.app.editor.native_plan_context(h.view).unwrap();
                let drawing = h.app.plans.drawing.as_ref().unwrap();
                let identity = drawing.identity();
                let items = drawing.items(context).unwrap().to_vec();
                let lines = drawing.provider_lines(context).unwrap().to_vec();
                let pos = h.direct_door_button(label).unwrap();
                h.hover(pos);
                h.frame(vec![button(pos, true)]);
                assert!(h.app.plans.opening_flip.is_some());
                assert!(h.app.plans.opening_move.is_none());
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), identity);
                h.frame(vec![button(pos, false)]);
                h.settle();
                h.move_clean();
                let resolved = before
                    .resolve_opening(&before.openings[&id].parameters)
                    .unwrap();
                let mut expected = before.clone();
                let p = &mut expected.openings.get_mut(&id).unwrap().parameters;
                if label == "Side" {
                    p.pane_position_override =
                        Some(if resolved.pane_position == WindowPanePosition::LeftFace {
                            WindowPanePosition::RightFace
                        } else {
                            WindowPanePosition::LeftFace
                        });
                } else {
                    p.lite_side_override = Some(
                        if resolved.family.side_lite.unwrap().side == LiteSide::Start {
                            LiteSide::End
                        } else {
                            LiteSide::Start
                        },
                    );
                }
                assert_eq!(
                    h.app.editor.document.model(),
                    &expected,
                    "only the instance override changes"
                );
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    history.undo_entries + 1
                );
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                assert_eq!(h.app.selected, Some(id));
                assert!(!h.app.plans.opening_flip_claimed);
                assert!(h.app.plans.opening_flip.is_none());
                assert_ne!(h.app.editor.scene[&id], scene[&id]);
                assert_eq!(
                    h.app.editor.scene[&h.wall], scene[&h.wall],
                    "aperture unchanged"
                );
                assert_arc_window_flip_geometry(&h, id);
                let context = h.app.editor.native_plan_context(h.view).unwrap();
                let drawing = h.app.plans.drawing.as_ref().unwrap();
                assert_ne!(drawing.identity(), identity);
                assert_eq!(drawing.items(context).unwrap(), items);
                let after_lines = drawing.provider_lines(context).unwrap().to_vec();
                assert_ne!(after_lines, lines);
                let after_scene = h.app.editor.scene.clone();
                h.app.history(false);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(
                    h.app
                        .plans
                        .drawing
                        .as_ref()
                        .unwrap()
                        .provider_lines(h.app.editor.native_plan_context(h.view).unwrap())
                        .unwrap(),
                    lines
                );
                h.app.history(true);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &expected);
                assert_eq!(h.app.editor.scene, after_scene);
                assert_eq!(
                    h.app
                        .plans
                        .drawing
                        .as_ref()
                        .unwrap()
                        .provider_lines(h.app.editor.native_plan_context(h.view).unwrap())
                        .unwrap(),
                    after_lines
                );
            }
        }
    }
}

#[test]
fn arc_window_flip_available_actions_and_fallback_both_dpis() {
    use os_model::WindowPanePosition::{Center, LeftFace};
    for (size, scale) in PROFILES {
        let (mut h, id) = arc_window_flip_fixture(size, scale, true);
        let canvas = h.app.plans.canvas_rect.unwrap();
        let top = arc_flip_anchors(&h, id)
            .into_iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);
        h.app.plans.cameras.get_mut(&h.view).unwrap().center.y +=
            f64::from(canvas.top() + 25.0 - top) / 70.0;
        h.frame(vec![]);
        let bottom = arc_flip_anchors(&h, id)
            .into_iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let side = arc_window_flip_rect(&h, "Side");
        let lite = arc_window_flip_rect(&h, "Flip lite");
        assert!((side.center().y - bottom - 30.0).abs() < 0.01);
        assert!((lite.center().y - side.center().y - 28.0).abs() < 0.01);
        let pos = lite.center();
        h.hover(pos);
        h.frame(vec![button(pos, true), button(pos, false)]);
        h.settle();
        assert_eq!(
            h.app.editor.document.model().openings[&id]
                .parameters
                .lite_side_override,
            Some(os_model::LiteSide::End)
        );
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        for (pane, has_lite, legacy) in [
            (Center, true, false),
            (Center, false, false),
            (LeftFace, false, false),
            (Center, false, true),
        ] {
            let (mut h, id) = arc_window_flip_fixture(size, scale, false);
            let mut model = h.app.editor.document.model().clone();
            let p = &mut model.openings.get_mut(&id).unwrap().parameters;
            let ty = &mut model
                .opening_types
                .get_mut(&p.type_id().unwrap())
                .unwrap()
                .parameters;
            // An explicit Center must suppress the Side button even with an
            // off-center reusable default.
            p.pane_position_override = Some(pane);
            if !has_lite {
                ty.family.side_lite = None;
            }
            if legacy {
                p.definition = OpeningDefinition::Legacy {
                    kind: OpeningKind::Window,
                    width: 1.8,
                    height: 1.2,
                    sill: 0.9,
                };
                p.width_override = None;
                p.pane_position_override = None;
            }
            h.app.editor.document = Document::from_model(model).unwrap();
            h.app.editor.pending_geometry.extend([h.wall, id]);
            h.app.editor.regenerate().unwrap();
            h.app.plans.poll(&h.app.editor);
            h.app.focus_plan(Some(h.view));
            h.app.select(Some(id));
            h.settle();
            if scale > 1.0 {
                let path = h.app.editor.document.model().walls[&h.wall].parameters.path;
                h.app.plans.cameras.insert(
                    h.view,
                    PlanCamera {
                        center: path.point(2.4),
                        pixels_per_metre: 40.0,
                    },
                );
                h.frame(vec![]);
            }
            let side_visible = h.direct_door_button("Side").is_some();
            assert_eq!(
                side_visible,
                pane != Center && !legacy,
                "pane={pane:?}, has_lite={has_lite}, legacy={legacy}, profile={size:?}/{scale}, camera={:?}, canvas={:?}, anchors={:?}",
                h.app.plans.cameras[&h.view],
                h.app.plans.canvas_rect,
                arc_flip_anchors(&h, id),
            );
            assert_eq!(
                h.direct_door_button("Flip lite").is_some(),
                has_lite && !legacy
            );
            assert!(h.direct_door_button("Hinge").is_none());
            assert!(h.direct_door_button("Swing").is_none());
            if has_lite {
                let top = arc_flip_anchors(&h, id)
                    .into_iter()
                    .map(|p| p.y)
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    (arc_window_flip_rect(&h, "Flip lite").center().y - top + 30.0).abs() < 0.01
                );
            }
        }
    }
}
