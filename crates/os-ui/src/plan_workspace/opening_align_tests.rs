//! Cross-wall center alignment through real egui pointer frames at both DPIs.
use super::*;
use std::f64::consts::TAU;

fn install(h: &mut Harness, model: Model, id: Id) {
    let entities: Vec<_> = model
        .walls
        .keys()
        .chain(model.openings.keys())
        .copied()
        .collect();
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend(entities);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();
    h.app.plans.cameras.insert(
        h.view,
        PlanCamera {
            center: Point2::new(0., 0.),
            pixels_per_metre: 35.,
        },
    );
    h.frame(vec![]);
}

fn fixture(
    kind: OpeningKind,
    legacy: bool,
    reversed: bool,
    antiparallel: bool,
    profile: usize,
) -> (Harness, Id, Id) {
    let (size, scale) = PROFILES[profile];
    let (mut h, id) = Harness::movable(kind, reversed, legacy, size, scale);
    let mut model = h.app.editor.document.model().clone();
    if !legacy {
        let p = &mut model.openings.get_mut(&id).unwrap().parameters;
        p.width_override = Some(1.1);
        p.height_override = Some(1.8);
        p.sill_override = (kind == OpeningKind::Window).then_some(0.7);
        p.lite_side_override = Some(os_model::LiteSide::End);
        let type_id = p.type_id().unwrap();
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
    let mut host = model.walls[&h.wall].parameters.clone();
    let shift = |p: Point2| Point2::new(p.x - 1.2, p.y + 1.6);
    let (a, b) = (shift(host.start()), shift(host.end()));
    host.path = os_model::WallPath::Straight {
        start: if antiparallel { b } else { a },
        end: if antiparallel { a } else { b },
    };
    let host = os_model::Wall::new(os_walls::WALL_TYPE, host);
    let host_id = host.id();
    model.walls.insert(host_id, host);
    let mut p = model.openings[&id].parameters.clone();
    p.host = host_id;
    p.offset = 4.;
    if legacy {
        if let OpeningDefinition::Legacy { width, .. } = &mut p.definition {
            *width = 1.6;
        }
    } else {
        p.width_override = Some(1.6);
    }
    let reference = Opening::new("core.opening", p);
    let reference_id = reference.id();
    model.openings.insert(reference_id, reference);
    install(&mut h, model, id);
    (h, id, reference_id)
}

fn reference_point(h: &Harness, id: Id) -> egui::Pos2 {
    let model = h.app.editor.document.model();
    let p = &model.openings[&id].parameters;
    // Click the visible jamb; alignment must use the aperture center, not the hit.
    h.point(model.walls[&p.host].parameters.path.point(p.offset))
}

fn center_camera_on_opening(h: &mut Harness, id: Id) {
    let model = h.app.editor.document.model();
    let opening = model
        .resolve_opening(&model.openings[&id].parameters)
        .unwrap();
    let world = model.walls[&opening.host]
        .parameters
        .path
        .point(opening.offset + opening.width * 0.5);
    let center = h
        .app
        .editor
        .native_plan_context(h.view)
        .unwrap()
        .basis
        .world_to_plane(world)
        .unwrap();
    h.app.plans.cameras.get_mut(&h.view).unwrap().center = center;
    h.frame(vec![]);
}

fn candidate(h: &Harness, point: egui::Pos2) -> Result<opening_align::Preview> {
    let rect = h.app.plans.canvas_rect.unwrap();
    h.app.plans.opening_align.as_ref().unwrap().candidate(
        &h.app.editor,
        h.app.plans.drawing.as_ref().unwrap(),
        h.app.plans.cameras[&h.view],
        [rect.width() as f64, rect.height() as f64],
        Point2::new(
            (point.x - rect.left()) as f64,
            (point.y - rect.top()) as f64,
        ),
    )
}

fn arc_fixture(
    kind: OpeningKind,
    legacy: bool,
    sign: f64,
    profile: usize,
) -> (Harness, Id, Id, f64) {
    let (mut h, id, reference) = fixture(kind, legacy, false, false, profile);
    let mut model = h.app.editor.document.model().clone();
    let source = model
        .resolve_opening(&model.openings[&id].parameters)
        .unwrap();
    let target = model
        .resolve_opening(&model.openings[&reference].parameters)
        .unwrap();
    let goal_angle = sign * 0.3;
    let source_start = sign * 5.8;
    let reference_sign = -sign;
    let reference_center_station = target.width * 0.5 + 3.0;
    let reference_start = goal_angle - reference_sign * reference_center_station / 6.0;
    model.walls.get_mut(&source.host).unwrap().parameters.path = os_model::WallPath::CircularArc {
        center: Point2::default(),
        radius: 4.0,
        start_angle_rad: source_start,
        signed_sweep_rad: sign * 4.7,
    };
    model.walls.get_mut(&target.host).unwrap().parameters.path = os_model::WallPath::CircularArc {
        center: Point2::default(),
        radius: 6.0,
        start_angle_rad: reference_start,
        signed_sweep_rad: reference_sign * 4.8,
    };
    model
        .openings
        .get_mut(&reference)
        .unwrap()
        .parameters
        .offset = reference_center_station - target.width * 0.5;
    install(&mut h, model, id);
    let station = 4.0 * ((goal_angle - source_start) * sign).rem_euclid(TAU);
    (h, id, reference, station)
}

#[test]
fn opening_align_projects_on_signed_major_arcs_and_commits_offset_only() {
    for profile in 0..2 {
        for kind in KINDS {
            for legacy in [false, true] {
                for sign in [-1.0, 1.0] {
                    let (mut h, id, reference, station) = arc_fixture(kind, legacy, sign, profile);
                    let before = h.app.editor.document.model().clone();
                    let scene_before = h.app.editor.scene.clone();
                    let history = h.app.editor.document.history_stats();
                    h.app.begin_opening_align();
                    assert!(h.app.plans.opening_align.is_some(), "{}", h.app.status);
                    let to = reference_point(&h, reference);
                    let preview = candidate(&h, to).unwrap();
                    let resolved = before
                        .resolve_opening(&before.openings[&id].parameters)
                        .unwrap();
                    assert!(
                        (preview.parameters.offset - (station - resolved.width * 0.5)).abs() < 1e-9
                    );
                    let selected_path = before.walls[&resolved.host].parameters.path;
                    let tangent = selected_path.tangent(station);
                    let delta = Point2::new(
                        preview.centers[1].x - preview.centers[0].x,
                        preview.centers[1].y - preview.centers[0].y,
                    );
                    assert!((delta.x * tangent.x + delta.y * tangent.y).abs() < 1e-9);
                    assert!((delta.x.hypot(delta.y) - 2.).abs() < 1e-9);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats(), history);
                    h.click(to);
                    h.settle();
                    let mut expected = before.clone();
                    expected.openings.get_mut(&id).unwrap().parameters.offset =
                        preview.parameters.offset;
                    assert_eq!(h.app.editor.document.model(), &expected, "offset only");
                    assert_ne!(h.app.editor.scene, scene_before);
                    assert_eq!(
                        h.app.editor.scene[&resolved.host],
                        crate::opening_tools::host_mesh(&expected, resolved.host).unwrap()
                    );
                    assert_eq!(
                        h.app.editor.scene[&id],
                        crate::opening_tools::panel_mesh(&expected, id).unwrap()
                    );
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let drawing = h.app.plans.drawing.as_ref().unwrap();
                    assert!(
                        drawing
                            .items(context)
                            .unwrap()
                            .iter()
                            .any(|item| item.entity == resolved.host)
                    );
                    assert!(
                        drawing
                            .provider_lines(context)
                            .unwrap()
                            .iter()
                            .any(|line| line.entity == id)
                    );
                    let scene_after = h.app.editor.scene.clone();
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        history.undo_entries + 1
                    );
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene_before);
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &expected);
                    assert_eq!(h.app.editor.scene, scene_after);
                }
            }
        }
    }
}

#[test]
fn opening_align_supports_mixed_straight_arc_hosts_and_rejects_out_of_sweep() {
    for profile in 0..2 {
        // Selected straight -> reference arc, then selected arc -> reference straight.
        for source_is_arc in [false, true] {
            let (mut h, id, reference, arc_station) =
                arc_fixture(OpeningKind::Window, false, 1.0, profile);
            let mut model = h.app.editor.document.model().clone();
            let source = model
                .resolve_opening(&model.openings[&id].parameters)
                .unwrap();
            let target = model
                .resolve_opening(&model.openings[&reference].parameters)
                .unwrap();
            let goal_angle: f64 = 0.3;
            let q = Point2::new(6.0 * goal_angle.cos(), 6.0 * goal_angle.sin());
            if source_is_arc {
                model.walls.get_mut(&target.host).unwrap().parameters.path =
                    os_model::WallPath::Straight {
                        start: Point2::new(q.x - 8.0, q.y),
                        end: Point2::new(q.x + 8.0, q.y),
                    };
                model
                    .openings
                    .get_mut(&reference)
                    .unwrap()
                    .parameters
                    .offset = 8.0 - target.width * 0.5;
            } else {
                model.walls.get_mut(&source.host).unwrap().parameters.path =
                    os_model::WallPath::Straight {
                        start: Point2::new(-10.0, q.y),
                        end: Point2::new(10.0, q.y),
                    };
            }
            install(&mut h, model, id);
            center_camera_on_opening(&mut h, reference);
            h.app.begin_opening_align();
            let to = reference_point(&h, reference);
            let preview = candidate(&h, to).unwrap();
            let before = h.app.editor.document.model();
            let source = before
                .resolve_opening(&before.openings[&id].parameters)
                .unwrap();
            let station = if source_is_arc {
                arc_station
            } else {
                q.x + 10.0
            };
            assert!((preview.parameters.offset - (station - source.width * 0.5)).abs() < 1e-8);
        }

        let (mut h, id, reference, _) = arc_fixture(OpeningKind::Door, true, 1.0, profile);
        let mut model = h.app.editor.document.model().clone();
        let source = model
            .resolve_opening(&model.openings[&id].parameters)
            .unwrap();
        let target = model
            .resolve_opening(&model.openings[&reference].parameters)
            .unwrap();
        let reference_sign = -1.0;
        let target_center_station = target.offset + target.width * 0.5;
        model.walls.get_mut(&source.host).unwrap().parameters.path =
            os_model::WallPath::CircularArc {
                center: Point2::default(),
                radius: 4.0,
                start_angle_rad: 0.0,
                signed_sweep_rad: 1.5,
            };
        model.walls.get_mut(&target.host).unwrap().parameters.path =
            os_model::WallPath::CircularArc {
                center: Point2::default(),
                radius: 6.0,
                start_angle_rad: 2.0 - reference_sign * target_center_station / 6.0,
                signed_sweep_rad: reference_sign * 4.8,
            };
        install(&mut h, model, id);
        center_camera_on_opening(&mut h, reference);
        h.app.begin_opening_align();
        let to = reference_point(&h, reference);
        assert!(candidate(&h, to).is_err(), "outside finite source sweep");
        assert!(h.app.editor.document.model().openings.contains_key(&id));
    }
}

#[test]
fn opening_align_rejects_undefined_radial_projection_at_arc_center() {
    for profile in 0..2 {
        let (mut h, id, reference, _) = arc_fixture(OpeningKind::Door, true, 1.0, profile);
        let mut model = h.app.editor.document.model().clone();
        let target = model
            .resolve_opening(&model.openings[&reference].parameters)
            .unwrap();
        model.walls.get_mut(&target.host).unwrap().parameters.path = os_model::WallPath::Straight {
            start: Point2::new(-10.0, 0.0),
            end: Point2::new(10.0, 0.0),
        };
        model
            .openings
            .get_mut(&reference)
            .unwrap()
            .parameters
            .offset = 10.0 - target.width * 0.5;
        install(&mut h, model, id);
        center_camera_on_opening(&mut h, reference);
        h.app.begin_opening_align();
        let to = reference_point(&h, reference);
        let error = candidate(&h, to).err().unwrap();
        assert!(error.to_string().contains("radial projection"), "{error}");
        assert!(h.app.plans.opening_align.is_some());
    }
}

#[test]
fn opening_align_projection_preview_preservation_history_regeneration_and_noop() {
    for profile in 0..2 {
        for kind in KINDS {
            for legacy in [false, true] {
                for reversed in [false, true] {
                    for antiparallel in [false, true] {
                        let (mut h, id, reference) =
                            fixture(kind, legacy, reversed, antiparallel, profile);
                        let before = h.app.editor.document.model().clone();
                        let scene = h.app.editor.scene.clone();
                        let history = h.app.editor.document.history_stats();
                        let identity = h.app.plans.drawing.as_ref().unwrap().identity();
                        let camera = h.app.plans.cameras[&h.view];
                        h.app.begin_opening_align();
                        let to = reference_point(&h, reference);
                        h.hover(to);
                        let preview = candidate(&h, to).unwrap();
                        let width = before
                            .resolve_opening(&before.openings[&id].parameters)
                            .unwrap()
                            .width;
                        let station = if antiparallel { 8. - 4.8 } else { 4.8 };
                        assert!((preview.parameters.offset - (station - width / 2.)).abs() < 1e-10);
                        let host = &before.walls[&h.wall].parameters;
                        let direction = host.path.tangent(0.);
                        let delta = Point2::new(
                            preview.centers[1].x - preview.centers[0].x,
                            preview.centers[1].y - preview.centers[0].y,
                        );
                        assert!((delta.x * direction.x + delta.y * direction.y).abs() < 1e-10);
                        assert!(
                            (preview.centers[0].distance(preview.centers[1]) - 2.).abs() < 1e-10
                        );
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        for item in preview.drawing.items(context).unwrap() {
                            let points: Vec<_> = item
                                .footprint
                                .vertices()
                                .iter()
                                .map(|p| h.point(*p))
                                .collect();
                            assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                                egui::Shape::Path(path) if path.closed && path.points == points)));
                        }
                        let centers = preview.centers.map(|p| h.point(p));
                        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                            egui::Shape::LineSegment { points, .. } if *points == centers)));
                        for center in centers {
                            assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                                egui::Shape::Circle(c) if c.center == center && c.radius == 5.)));
                        }
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.scene, scene);
                        assert_eq!(h.app.editor.document.history_stats(), history);
                        assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), identity);
                        h.click(to);
                        h.settle();
                        assert!(h.app.plans.opening_align.is_none(), "{}", h.app.status);
                        assert!(!h.app.plans.opening_align_claimed);
                        assert_eq!(h.app.selected, Some(id));
                        assert_eq!(h.app.plans.cameras[&h.view], camera);
                        assert_eq!(
                            h.app.editor.document.history_stats().undo_entries,
                            history.undo_entries + 1
                        );
                        let mut expected = before.clone();
                        expected.openings.get_mut(&id).unwrap().parameters.offset =
                            preview.parameters.offset;
                        assert_eq!(
                            h.app.editor.document.model(),
                            &expected,
                            "only offset changes"
                        );
                        assert_ne!(h.app.editor.scene[&h.wall], scene[&h.wall]);
                        assert_ne!(h.app.editor.scene[&id], scene[&id]);
                        assert_eq!(h.app.editor.scene[&reference], scene[&reference]);
                        assert_eq!(
                            h.app.editor.scene[&h.wall],
                            crate::opening_tools::host_mesh(&expected, h.wall).unwrap()
                        );
                        assert_eq!(
                            h.app.editor.scene[&id],
                            crate::opening_tools::panel_mesh(&expected, id).unwrap()
                        );
                        let current = h.app.editor.native_plan_context(h.view).unwrap();
                        let drawing = h.app.plans.drawing.as_ref().unwrap();
                        assert_eq!(
                            preview.drawing.items(context).unwrap(),
                            drawing
                                .items(current)
                                .unwrap()
                                .iter()
                                .filter(|i| i.entity == h.wall)
                                .cloned()
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(
                            preview.drawing.provider_lines(context).unwrap(),
                            drawing
                                .provider_lines(current)
                                .unwrap()
                                .iter()
                                .filter(|l| l.entity == id)
                                .cloned()
                                .collect::<Vec<_>>()
                        );
                        let after_scene = h.app.editor.scene.clone();
                        let after_history = h.app.editor.document.history_stats();
                        h.app.begin_opening_align();
                        h.click(to);
                        h.settle();
                        assert!(h.app.plans.opening_align.is_none());
                        assert_eq!(
                            h.app.editor.document.history_stats(),
                            after_history,
                            "no-op"
                        );
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
}

#[test]
fn opening_align_rejects_invalid_references_and_candidates_without_clamping() {
    for profile in 0..2 {
        for reason in [
            "same_host",
            "self",
            "nonparallel",
            "arc_outside",
            "fit",
            "clearance",
            "overlap",
            "crop",
            "hidden",
            "phase",
            "level",
        ] {
            let (mut h, id, reference) = fixture(OpeningKind::Door, true, false, false, profile);
            let mut model = h.app.editor.document.model().clone();
            let other = model.openings[&reference].parameters.host;
            match reason {
                "same_host" => model.openings.get_mut(&reference).unwrap().parameters.host = h.wall,
                "nonparallel" => {
                    *model
                        .walls
                        .get_mut(&other)
                        .unwrap()
                        .parameters
                        .path
                        .straight_end_mut()
                        .unwrap() = Point2::new(3., 5.)
                }
                "arc_outside" => {
                    let source = model
                        .resolve_opening(&model.openings[&id].parameters)
                        .unwrap();
                    let target = model
                        .resolve_opening(&model.openings[&reference].parameters)
                        .unwrap();
                    model.walls.get_mut(&source.host).unwrap().parameters.path =
                        os_model::WallPath::CircularArc {
                            center: Point2::default(),
                            radius: 4.,
                            start_angle_rad: 0.,
                            signed_sweep_rad: 1.5,
                        };
                    model.walls.get_mut(&other).unwrap().parameters.path =
                        os_model::WallPath::CircularArc {
                            center: Point2::default(),
                            radius: 6.,
                            start_angle_rad: 2. + (target.offset + target.width * 0.5) / 6.,
                            signed_sweep_rad: -4.8,
                        };
                }
                "fit" | "clearance" => {
                    let delta = if reason == "fit" { 5. } else { -4.6 };
                    let w = &mut model.walls.get_mut(&other).unwrap().parameters;
                    let start = w.start();
                    let end = w.end();
                    w.path = os_model::WallPath::Straight {
                        start: Point2::new(start.x + delta * 0.8, start.y + delta * 0.6),
                        end: Point2::new(end.x + delta * 0.8, end.y + delta * 0.6),
                    };
                }
                "overlap" => {
                    let mut p = model.openings[&id].parameters.clone();
                    p.offset = 4.;
                    let o = Opening::new("core.opening", p);
                    model.openings.insert(o.id(), o);
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
                        min: Point2::new(-5., -4.),
                        max: Point2::new(5., 0.),
                    })
                }
                "hidden" => {
                    let p = &mut model.openings.get_mut(&reference).unwrap().parameters;
                    if let OpeningDefinition::Legacy { kind, .. } = &mut p.definition {
                        *kind = OpeningKind::Window;
                    }
                    p.hinge = Default::default();
                    p.swing = Default::default();
                    model
                        .views
                        .get_mut(&h.view)
                        .unwrap()
                        .parameters
                        .plan
                        .as_mut()
                        .unwrap()
                        .visibility
                        .windows = false;
                }
                "phase" => {
                    let order = model
                        .phases
                        .values()
                        .map(|phase| phase.parameters.order)
                        .max()
                        .unwrap_or(0)
                        + 1;
                    let phase = os_model::new_phase("Future", order);
                    model.element_lifecycles.insert(
                        reference,
                        os_model::ElementLifecycle {
                            created_in: phase.id(),
                            demolished_in: None,
                        },
                    );
                    model.phases.insert(phase.id(), phase);
                }
                "level" => {
                    let level = os_model::Level::new(
                        "core.level",
                        model.levels[&h.app.active_level].parameters.clone(),
                    );
                    model.walls.get_mut(&other).unwrap().parameters.level = level.id();
                    model.levels.insert(level.id(), level);
                }
                "self" => {}
                _ => unreachable!(),
            }
            install(&mut h, model, id);
            let hit_id = if reason == "self" { id } else { reference };
            if reason != "crop" {
                let target = h
                    .app
                    .editor
                    .document
                    .model()
                    .resolve_opening(&h.app.editor.document.model().openings[&hit_id].parameters)
                    .unwrap();
                let world_center = h.app.editor.document.model().walls[&target.host]
                    .parameters
                    .path
                    .point(target.offset + target.width * 0.5);
                let center = h
                    .app
                    .editor
                    .native_plan_context(h.view)
                    .unwrap()
                    .basis
                    .world_to_plane(world_center)
                    .unwrap();
                h.app.plans.cameras.get_mut(&h.view).unwrap().center = center;
                h.frame(vec![]);
            }
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let camera = h.app.plans.cameras[&h.view];
            h.app.begin_opening_align();
            assert!(
                h.app.plans.opening_align.is_some(),
                "{reason}: {}",
                h.app.status
            );
            let to = if reason == "crop" {
                h.app.plans.canvas_rect.unwrap().center()
            } else {
                reference_point(&h, hit_id)
            };
            h.hover(to);
            assert!(candidate(&h, to).is_err(), "{reason}");
            assert!(h.has_text("Cannot align:"), "{reason}");
            h.click(to);
            assert!(h.app.plans.opening_align.is_some(), "retain after {reason}");
            assert!(h.app.status_error, "{reason}");
            assert_eq!(h.app.editor.document.model(), &before, "{reason}");
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
            assert_eq!(h.app.selected, Some(id));
            assert_eq!(h.app.plans.cameras[&h.view], camera);
            h.frame(vec![escape()]);
            assert!(h.app.plans.opening_align.is_none());
        }
    }
}

#[test]
fn opening_align_stale_escape_pointer_loss_and_selection_pan_precedence() {
    for profile in 0..2 {
        for reason in [
            "escape",
            "lost",
            "revision",
            "session",
            "view",
            "settings",
            "provider",
            "selection",
            "multi",
            "drawing",
            "outside",
        ] {
            let (mut h, id, reference) = fixture(OpeningKind::Window, false, true, true, profile);
            let camera = h.app.plans.cameras[&h.view];
            h.app.begin_opening_align();
            let mut to = reference_point(&h, reference);
            h.hover(to);
            h.frame(vec![button(to, true)]);
            assert!(h.app.plans.opening_align_claimed);
            match reason {
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
                "selection" => h.app.select(Some(h.wall)),
                "multi" => {
                    h.app.selected_ids.insert(reference);
                }
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                "outside" => to = egui::pos2(-10., -10.),
                _ => unreachable!(),
            }
            let model = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            let selected = h.app.selected;
            h.hover(to + egui::vec2(20., 0.));
            h.frame(vec![button(to, false)]);
            h.frame(vec![]);
            assert!(h.app.plans.opening_align.is_none(), "{reason}");
            assert!(!h.app.plans.opening_align_claimed, "{reason}");
            assert_eq!(h.app.editor.document.model(), &model, "{reason}");
            assert_eq!(h.app.editor.scene, scene, "{reason}");
            assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
            assert_eq!(h.app.selected, selected, "no delayed selection: {reason}");
            assert_eq!(
                h.app.editor.document.model().openings[&id]
                    .parameters
                    .offset,
                1.5
            );
            if let Some(current) = h.app.plans.cameras.get(&h.view) {
                assert_eq!(*current, camera, "no delayed pan: {reason}");
            }
        }
    }
}

#[test]
fn opening_align_action_and_single_visible_native_source_requirement() {
    for profile in 0..2 {
        let (mut h, id, reference) = fixture(OpeningKind::Door, false, false, false, profile);
        h.ribbon_click("Architecture", 30.0..58.0);
        h.ribbon_click("Align opening center", 58.0..180.0);
        assert!(h.app.plans.opening_align.is_some());
        h.frame(vec![escape()]);
        h.app.selected_ids.insert(reference);
        h.app.begin_opening_align();
        assert!(h.app.plans.opening_align.is_none());
        h.app.select(Some(id));
        let mut curved = h.app.editor.document.model().clone();
        let host = curved.openings[&id].parameters.host;
        curved.walls.get_mut(&host).unwrap().parameters.path = os_model::WallPath::CircularArc {
            center: Point2::default(),
            radius: 8.0,
            start_angle_rad: 0.0,
            signed_sweep_rad: 2.0,
        };
        install(&mut h, curved, id);
        h.app.begin_opening_align();
        assert!(h.app.plans.opening_align.is_some(), "{}", h.app.status);
        h.frame(vec![escape()]);
        let mut model = h.app.editor.document.model().clone();
        model
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .visibility
            .doors = false;
        install(&mut h, model, id);
        h.app.begin_opening_align();
        assert!(h.app.plans.opening_align.is_none());
    }
}
