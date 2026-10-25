//! Real-egui pose gestures; no native-window qualification.
use super::*;
use os_geometry::openings::{PoseLeaf, PoseMotion, pose_handle, pose_handle_for_leaf};
use os_model::{DoorHinge, DoorSwing, OpeningState, WindowOperation};
use os_storage::{StorageBackend, ZipJsonStorage};

fn fixture(profile: usize, operation: usize, reversed: bool, handing: bool) -> (Harness, Id) {
    let (size, scale) = PROFILES[profile];
    let kind = if operation == 0 {
        OpeningKind::Door
    } else {
        OpeningKind::Window
    };
    let (mut h, id) = Harness::movable(kind, reversed, false, size, scale);
    let mut model = h.app.editor.document.model().clone();
    let opening = &mut model.openings.get_mut(&id).unwrap().parameters;
    opening.offset = 3.;
    opening.width_override = Some(1.5);
    opening.hinge = if operation == 0 && handing {
        DoorHinge::End
    } else {
        DoorHinge::Start
    };
    opening.swing = if operation == 0 && handing != reversed {
        DoorSwing::Right
    } else {
        DoorSwing::Left
    };
    if operation != 0 {
        opening.pane_position_override = Some(if handing {
            os_model::WindowPanePosition::RightFace
        } else {
            os_model::WindowPanePosition::LeftFace
        });
    }
    opening.lite_side_override = Some(os_model::LiteSide::End);
    let ty = opening.type_id().unwrap();
    let p = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
    p.window_operation = match operation {
        1 => WindowOperation::Casement,
        2 => WindowOperation::Sliding,
        _ => WindowOperation::Fixed,
    };
    p.family.frame_width = 0.04;
    p.family.side_lite = Some(os_model::SideLite {
        side: os_model::LiteSide::Start,
        width_fraction: 0.2,
        mullion_width: 0.04,
        material: None,
    });
    let mut solid = os_model::OpeningExtrusion::new("Pose accessory");
    if operation != 0 {
        solid.attachment = os_model::OpeningAttachment::MovingSash;
    }
    p.family.extrusions.push(solid);
    refresh(&mut h, model, id);
    (h, id)
}

fn refresh(h: &mut Harness, model: Model, id: Id) {
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend([id, h.wall]);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();
    h.app.plans.cameras.insert(
        h.view,
        PlanCamera {
            center: Point2::default(),
            pixels_per_metre: 55.,
        },
    );
    h.frame(vec![]);
}

fn geometry(h: &Harness, id: Id, value: Option<f64>) -> os_geometry::openings::OpeningPoseHandle {
    let m = h.app.editor.document.model();
    let mut p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
    if let Some(value) = value {
        p.open_state = match (p.kind, p.window_operation) {
            (OpeningKind::Door, _) => OpeningState::DoorAngle(value),
            (_, WindowOperation::Sliding) => OpeningState::SlidingFraction(value),
            _ => OpeningState::CasementAngle(value),
        };
    }
    pose_handle(&p, &m.resolve_wall(p.host).unwrap().parameters).unwrap()
}

fn start(h: &mut Harness, id: Id, offset: egui::Vec2) -> egui::Pos2 {
    assert!(h.app.opening_pose_available());
    let canvas = h.app.plans.canvas_rect;
    h.text_click("Adjust opening pose");
    h.frame(vec![]);
    assert_eq!(
        h.app.plans.canvas_rect, canvas,
        "pose controls must not resize the canvas"
    );
    assert!(
        h.app.plans.opening_pose.is_some(),
        "{}; geometry {:?}, canvas {:?}, camera {:?}, size {:?}",
        h.app.status,
        geometry(h, id, None),
        h.app.plans.canvas_rect,
        h.app.plans.cameras[&h.view],
        h.size
    );
    assert_eq!(h.handles(), 1);
    assert!(h.direct_door_button("Hinge").is_none());
    assert!(h.direct_door_button("Swing").is_none());
    let p = h.point(geometry(h, id, None).point) + offset;
    h.hover(p);
    h.frame(vec![button(p, true)]);
    assert!(h.app.plans.opening_pose_claimed);
    assert!(h.app.plans.opening_move.is_none());
    p
}

#[test]
fn pose_handles_both_profiles_rotated_reversed_handing_preview_commit_history_and_reopen() {
    for profile in 0..2 {
        for operation in 0..3 {
            for reversed in [false, true] {
                for handing in [false, true] {
                    let (mut h, id) = fixture(profile, operation, reversed, handing);
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let camera = h.app.plans.cameras[&h.view];
                    let drawing_id = h.app.plans.drawing.as_ref().unwrap().identity();
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let apertures = h
                        .app
                        .plans
                        .drawing
                        .as_ref()
                        .unwrap()
                        .items(context)
                        .unwrap()
                        .to_vec();
                    let grab = egui::vec2(3., -2.);
                    start(&mut h, id, grab);
                    let value = if operation == 2 { 0.63 } else { 37. };
                    let target_geometry = geometry(&h, id, Some(value));
                    let target = h.point(target_geometry.point) + grab;
                    h.frame(vec![egui::Event::PointerMoved(target)]);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing_id);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                    assert_eq!(h.handles(), 1);
                    let preview_shapes = h.output.shapes.clone();
                    h.frame(vec![button(target, false)]);
                    assert!(!h.app.status_error, "{}", h.app.status);
                    assert!((geometry(&h, id, None).value - value).abs() < 1e-4);
                    let after = h.app.editor.document.model().clone();
                    let mut expected = before.clone();
                    expected
                        .openings
                        .get_mut(&id)
                        .unwrap()
                        .parameters
                        .open_state = after.openings[&id].parameters.open_state;
                    assert_eq!(after, expected, "only open_state may change");
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                    h.settle();
                    // Production symbol and component meshes agree with the exact candidate.
                    let p = after
                        .resolve_opening(&after.openings[&id].parameters)
                        .unwrap();
                    let wall = after.resolve_wall(p.host).unwrap();
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let elevation = after.levels[&wall.parameters.level].parameters.elevation;
                    let mut lines = crate::opening_tools::plan_symbol(
                        id,
                        &p,
                        &wall.parameters,
                        elevation,
                        context,
                    )
                    .unwrap();
                    let mut actual: Vec<_> = h
                        .app
                        .plans
                        .drawing
                        .as_ref()
                        .unwrap()
                        .provider_lines(context)
                        .unwrap()
                        .iter()
                        .filter(|l| l.entity == id)
                        .cloned()
                        .collect();
                    lines.sort_by_key(|l| l.feature);
                    actual.sort_by_key(|l| l.feature);
                    assert_eq!(actual, lines);
                    assert_eq!(
                        h.app
                            .plans
                            .drawing
                            .as_ref()
                            .unwrap()
                            .items(context)
                            .unwrap(),
                        apertures
                    );
                    // Every Cut symbol segment in the eventual committed drawing was already
                    // painted at those coordinates by the disposable preview. Accessories
                    // retain their normal production stroke treatment.
                    for line in actual
                        .iter()
                        .filter(|l| l.role == os_geometry::plan::PlanRole::Cut)
                    {
                        let a = h.point(context.basis.plane_to_world(line.start).unwrap());
                        let b = h.point(context.basis.plane_to_world(line.end).unwrap());
                        assert!(
                            preview_shapes.iter().any(
                                |s| matches!(&s.shape,egui::Shape::LineSegment{points,..}
                if (points[0].distance(a)<0.01 && points[1].distance(b)<0.01)
                || (points[1].distance(a)<0.01 && points[0].distance(b)<0.01))
                            ),
                            "preview must equal committed symbol feature {}",
                            line.feature
                        );
                    }
                    let meshes =
                        os_geometry::openings::component_solids(&p, &wall.parameters, elevation)
                            .unwrap();
                    let point = geometry(&h, id, None).point;
                    assert!(
                        meshes.iter().any(|mesh| mesh.vertices.iter().any(|a| mesh
                            .vertices
                            .iter()
                            .any(|b| (a.z - b.z).abs() < 1e-8
                                && Point2::new((a.x + b.x) / 2., (a.y + b.y) / 2.)
                                    .distance(point)
                                    < 1e-7))),
                        "handle must lie at a production component edge"
                    );
                    h.app.editor.document.undo();
                    assert_eq!(h.app.editor.document.model(), &before);
                    h.app.editor.document.redo();
                    assert_eq!(h.app.editor.document.model(), &after);
                    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
                    let path = dir.path().join("pose.osb");
                    ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
                    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);
                }
            }
        }
    }
}

#[test]
fn pose_handles_unwrap_opposite_ray_both_directions_handing_and_shift() {
    for profile in 0..2 {
        for operation in 0..2 {
            for reversed in [false, true] {
                for direction in [-1., 1.] {
                    for shift in [false, true] {
                        let (mut h, id) = fixture(profile, operation, reversed, true);
                        let mut model = h.app.editor.document.model().clone();
                        model.openings.get_mut(&id).unwrap().parameters.open_state =
                            if operation == 0 {
                                OpeningState::DoorAngle(45.)
                            } else {
                                OpeningState::CasementAngle(45.)
                            };
                        refresh(&mut h, model, id);
                        let g = geometry(&h, id, None);
                        let PoseMotion::Angular { pivot, sense } = g.motion else {
                            unreachable!()
                        };
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        h.app.plans.cameras.insert(
                            h.view,
                            PlanCamera {
                                center: context.basis.world_to_plane(pivot).unwrap(),
                                pixels_per_metre: 35.,
                            },
                        );
                        h.frame(vec![]);
                        let before = h.app.editor.document.model().clone();
                        let offset = egui::vec2(3., -2.);
                        start(&mut h, id, offset);
                        // Cross the initial vector's opposite ray, then retrace it.
                        // The endpoint stays clamped until accumulated motion returns.
                        for degrees in [37_f64, 100., 170., 179., 181., 190., 181., 179., 100., 37.]
                        {
                            let angle = (degrees * direction * sense).to_radians();
                            let x = g.point.x - pivot.x;
                            let y = g.point.y - pivot.y;
                            let target = h.point(Point2::new(
                                pivot.x + x * angle.cos() - y * angle.sin(),
                                pivot.y + x * angle.sin() + y * angle.cos(),
                            )) + offset;
                            let expected = 45. + degrees * direction;
                            let expected = if shift {
                                (expected / 15.).round() * 15.
                            } else {
                                expected
                            }
                            .clamp(0., 90.);
                            h.time += 1. / 60.;
                            let mut raw = egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    h.size,
                                )),
                                time: Some(h.time),
                                events: vec![egui::Event::PointerMoved(target)],
                                modifiers: if shift {
                                    egui::Modifiers::SHIFT
                                } else {
                                    egui::Modifiers::NONE
                                },
                                ..Default::default()
                            };
                            raw.viewports
                                .get_mut(&egui::ViewportId::ROOT)
                                .unwrap()
                                .native_pixels_per_point = Some(h.scale);
                            h.output = h.ctx.run(raw, |ctx| h.app.show(ctx));
                            assert!(
                                h.has_text(&format!("{expected:.1}°")),
                                "profile {profile}, operation {operation}, reversed {reversed}, direction {direction}, shift {shift}, degrees {degrees}"
                            );
                            assert_eq!(h.app.editor.document.model(), &before);
                            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                        }
                        h.frame(vec![escape()]);
                        assert!(h.app.plans.opening_pose.is_none());
                        assert_eq!(h.app.editor.document.model(), &before);
                        // A fresh gesture releases beyond the opposite ray and commits
                        // the same clamped endpoint that was previewed.
                        h.frame(vec![button(h.point(g.point) + offset, false)]);
                        // The cancelled press remains owned through its release frame.
                        // Render the next frame before acquiring the restored action.
                        h.frame(vec![]);
                        start(&mut h, id, offset);
                        for degrees in [90_f64, 170., 190.] {
                            let angle = (degrees * direction * sense).to_radians();
                            let x = g.point.x - pivot.x;
                            let y = g.point.y - pivot.y;
                            let target = h.point(Point2::new(
                                pivot.x + x * angle.cos() - y * angle.sin(),
                                pivot.y + x * angle.sin() + y * angle.cos(),
                            )) + offset;
                            h.frame(vec![egui::Event::PointerMoved(target)]);
                            if degrees == 190. {
                                h.frame(vec![button(target, false)]);
                            }
                        }
                        let value = if direction > 0. { 90. } else { 0. };
                        let mut expected = before.clone();
                        expected
                            .openings
                            .get_mut(&id)
                            .unwrap()
                            .parameters
                            .open_state = if operation == 0 {
                            OpeningState::DoorAngle(value)
                        } else {
                            OpeningState::CasementAngle(value)
                        };
                        assert_eq!(h.app.editor.document.model(), &expected);
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                    }
                }
            }
        }
    }
}

#[test]
fn pose_handles_noop_default_explicit_endpoints_and_shift() {
    for profile in 0..2 {
        for operation in 0..3 {
            for explicit in [false, true] {
                let (mut h, id) = fixture(profile, operation, false, false);
                if explicit {
                    let mut m = h.app.editor.document.model().clone();
                    m.openings.get_mut(&id).unwrap().parameters.open_state = match operation {
                        0 => OpeningState::DoorAngle(90.),
                        1 => OpeningState::CasementAngle(0.),
                        _ => OpeningState::SlidingFraction(0.),
                    };
                    refresh(&mut h, m, id);
                }
                let original = h.app.editor.document.model().clone();
                let startpoint = start(&mut h, id, egui::vec2(4., 1.));
                let intermediate = h
                    .point(geometry(&h, id, Some(if operation == 2 { 0.4 } else { 40. })).point)
                    + egui::vec2(4., 1.);
                h.frame(vec![egui::Event::PointerMoved(intermediate)]);
                h.frame(vec![egui::Event::PointerMoved(startpoint)]);
                h.frame(vec![button(startpoint, false)]);
                assert_eq!(h.app.editor.document.model(), &original);
                assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                for value in [0., if operation == 2 { 1. } else { 90. }] {
                    h.settle();
                    start(&mut h, id, egui::Vec2::ZERO);
                    let target = h.point(geometry(&h, id, Some(value)).point);
                    h.frame(vec![egui::Event::PointerMoved(target)]);
                    h.frame(vec![button(target, false)]);
                    assert!((geometry(&h, id, None).value - value).abs() < 1e-4);
                }
                h.settle();
                start(&mut h, id, egui::Vec2::ZERO);
                let target =
                    h.point(geometry(&h, id, Some(if operation == 2 { 0.36 } else { 37. })).point);
                // Harness uses RawInput modifiers for quantization, as real input does.
                h.time += 1. / 60.;
                let mut raw = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, h.size)),
                    time: Some(h.time),
                    events: vec![egui::Event::PointerMoved(target), button(target, false)],
                    modifiers: egui::Modifiers::SHIFT,
                    ..Default::default()
                };
                raw.viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .native_pixels_per_point = Some(h.scale);
                h.output = h.ctx.run(raw, |ctx| h.app.show(ctx));
                assert!(
                    (geometry(&h, id, None).value - if operation == 2 { 0.4 } else { 30. }).abs()
                        < 1e-6
                );
            }
        }
    }
}

#[test]
fn pose_handles_cancel_stale_camera_layout_hidden_tools_and_invalid_release() {
    for profile in 0..2 {
        for paired in [false, true] {
            for reason in [
                "escape",
                "gone",
                "focus",
                "revision",
                "selection",
                "view",
                "session",
                "camera",
                "layout",
                "hide",
                "crop",
                "tool",
                "modal",
                "provider",
                "drawing",
                "outside",
                "pivot",
            ] {
                let (mut h, id) = if paired {
                    pair_fixture(profile, false, false, OpeningState::Default)
                } else {
                    fixture(profile, 0, false, false)
                };
                let from = if paired {
                    start_pair(&mut h, id, PoseLeaf::Inactive, egui::Vec2::ZERO)
                } else {
                    start(&mut h, id, egui::Vec2::ZERO)
                };
                let selected_geometry = |h: &Harness, value| {
                    if paired {
                        pair_geometry(h, id, PoseLeaf::Inactive, value)
                    } else {
                        geometry(h, id, value)
                    }
                };
                let target = h.point(selected_geometry(&h, Some(35.)).point);
                h.frame(vec![egui::Event::PointerMoved(target)]);
                match reason {
                    "escape" => h.frame(vec![escape()]),
                    "gone" => h.frame(vec![egui::Event::PointerGone]),
                    "focus" => h.frame(vec![egui::Event::WindowFocused(false)]),
                    "revision" => {
                        h.app
                            .editor
                            .command("Rename", Command::RenameProject("Updated".into()))
                            .unwrap();
                    }
                    "selection" => h.app.select(Some(h.wall)),
                    "view" => h.app.focus_plan(None),
                    "session" => {
                        let m = h.app.editor.document.model().clone();
                        h.app.editor.document = Document::from_model(m).unwrap();
                    }
                    "camera" => h.app.plans.cameras.get_mut(&h.view).unwrap().center.x += 0.2,
                    "layout" => h.size.x -= 50.,
                    "hide" => h
                        .app
                        .editor
                        .command(
                            "Hide",
                            Command::SetPlanHiddenOpenings {
                                view: h.view,
                                hidden: std::collections::BTreeSet::from([id]),
                            },
                        )
                        .unwrap(),
                    "crop" => h.app.plans.crop.mode = Some(h.view),
                    "tool" => h.app.begin_opening_placement(OpeningKind::Window),
                    "modal" => h.app.cancel_plan_authoring_for_modal(),
                    "provider" => h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap(),
                    "drawing" => {
                        h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                    }
                    "outside" => {}
                    "pivot" => {}
                    _ => unreachable!(),
                }
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                let release = if reason == "outside" {
                    egui::pos2(-10., -10.)
                } else if reason == "pivot" {
                    let PoseMotion::Angular { pivot, .. } = selected_geometry(&h, None).motion
                    else {
                        unreachable!()
                    };
                    h.point(pivot)
                } else {
                    target
                };
                h.frame(vec![egui::Event::PointerMoved(release)]);
                let camera = h.app.plans.cameras.get(&h.view).copied();
                h.frame(vec![button(release, false)]);
                assert_eq!(h.app.editor.document.model(), &before, "{reason}");
                assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
                assert_eq!(
                    h.app.plans.cameras.get(&h.view).copied(),
                    camera,
                    "cancel must not pan: {reason}"
                );
                assert!(
                    h.app.plans.opening_pose.is_none(),
                    "{reason}, from {from:?}"
                );
            }
        }
    }
}

#[test]
fn pose_handles_eligibility_fixed_arc_profile_phase_category_instance_range_crop_canvas() {
    for profile in 0..2 {
        for paired in [false, true] {
            for reason in [
                "fixed", "arc", "profile", "phase", "category", "hidden", "range", "crop",
                "canvas", "multi", "rcp",
            ] {
                if paired && matches!(reason, "fixed" | "arc" | "profile") {
                    continue;
                }
                let (mut h, id) = if paired {
                    pair_fixture(profile, false, false, OpeningState::Default)
                } else {
                    fixture(profile, if reason == "fixed" { 1 } else { 0 }, false, false)
                };
                let mut m = h.app.editor.document.model().clone();
                let ty = m.openings[&id].parameters.type_id().unwrap();
                let settings = m
                    .views
                    .get_mut(&h.view)
                    .unwrap()
                    .parameters
                    .plan
                    .as_mut()
                    .unwrap();
                match reason {
                    "fixed" => {
                        let p = &mut m.opening_types.get_mut(&ty).unwrap().parameters;
                        p.window_operation = WindowOperation::Fixed;
                        p.family.extrusions[0].attachment =
                            os_model::OpeningAttachment::PrimaryPanel;
                    }
                    "arc" => {
                        m.opening_types
                            .get_mut(&ty)
                            .unwrap()
                            .parameters
                            .family
                            .extrusions
                            .clear();
                        m.walls.get_mut(&h.wall).unwrap().parameters.path =
                            os_model::WallPath::CircularArc {
                                center: Point2::default(),
                                radius: 6.,
                                start_angle_rad: 0.,
                                signed_sweep_rad: 1.4,
                            };
                    }
                    "profile" => {
                        let f = &mut m.opening_types.get_mut(&ty).unwrap().parameters.family;
                        f.extrusions.clear();
                        f.side_lite = None;
                        f.frame_width = 0.;
                        f.profile = vec![
                            Point2::new(0., 0.),
                            Point2::new(1., 0.),
                            Point2::new(0.5, 1.),
                        ];
                        m.openings
                            .get_mut(&id)
                            .unwrap()
                            .parameters
                            .lite_side_override = None;
                    }
                    "phase" => settings.phase_filter = os_model::PhaseFilter::ShowDemolished,
                    "category" => settings.visibility.doors = false,
                    "hidden" => {
                        m.plan_hidden_openings
                            .insert(h.view, std::collections::BTreeSet::from([id]));
                    }
                    "range" => {
                        settings.range = os_model::PlanViewRange {
                            top: 5.,
                            cut: 4.,
                            bottom: 3.5,
                            depth: 3.,
                        }
                    }
                    "crop" => {
                        settings.crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(2., 2.),
                            max: Point2::new(4., 4.),
                        })
                    }
                    "rcp" => {
                        settings.view_type = os_model::PlanViewType::ReflectedCeilingPlan;
                        settings.range = os_model::PlanViewRange {
                            bottom: 0.,
                            cut: 1.2,
                            top: 2.5,
                            depth: 3.,
                        };
                    }
                    _ => {}
                }
                refresh(&mut h, m, id);
                if reason == "multi" {
                    h.app.selected_ids.insert(h.wall);
                }
                if reason == "canvas" {
                    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(50., 50.);
                }
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                h.app.begin_opening_pose();
                h.frame(vec![]);
                assert!(h.app.plans.opening_pose.is_none(), "{reason}");
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
            }
        }
    }
}

fn pair_fixture(
    profile: usize,
    reversed: bool,
    handing: bool,
    state: OpeningState,
) -> (Harness, Id) {
    let (mut h, id) = fixture(profile, 0, reversed, handing);
    let mut m = h.app.editor.document.model().clone();
    let p = &mut m.openings.get_mut(&id).unwrap().parameters;
    p.open_state = state;
    let ty = p.type_id().unwrap();
    let family = &mut m.opening_types.get_mut(&ty).unwrap().parameters.family;
    family.door_leaves = os_model::DoorLeaves::Paired {
        active_fraction: if handing { 0.65 } else { 0.5 },
    };
    family.extrusions.clear();
    for attachment in [
        os_model::OpeningAttachment::ActiveLeaf,
        os_model::OpeningAttachment::InactiveLeaf,
    ] {
        let mut part = os_model::OpeningExtrusion::new("Leaf accessory");
        part.attachment = attachment;
        family.extrusions.push(part);
    }
    refresh(&mut h, m, id);
    (h, id)
}

fn pair_geometry(
    h: &Harness,
    id: Id,
    leaf: PoseLeaf,
    value: Option<f64>,
) -> os_geometry::openings::OpeningPoseHandle {
    let m = h.app.editor.document.model();
    let mut p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
    let wall = m.resolve_wall(p.host).unwrap();
    if let Some(value) = value {
        let leaves = os_geometry::openings::door_pair(&p).unwrap().unwrap();
        p.open_state = OpeningState::DoorPairAngles {
            active_degrees: if leaf == PoseLeaf::Active {
                value
            } else {
                leaves[0].door_angle(&wall.parameters)
            },
            inactive_degrees: if leaf == PoseLeaf::Inactive {
                value
            } else {
                leaves[1].door_angle(&wall.parameters)
            },
        };
    }
    pose_handle_for_leaf(&p, &wall.parameters, leaf).unwrap()
}

fn start_pair(h: &mut Harness, id: Id, leaf: PoseLeaf, grab: egui::Vec2) -> egui::Pos2 {
    assert!(h.app.opening_pose_available());
    let canvas = h.app.plans.canvas_rect;
    h.text_click("Adjust opening pose");
    h.frame(vec![]);
    assert!(h.has_text("Active leaf") && h.has_text("Inactive leaf"));
    if leaf == PoseLeaf::Inactive {
        h.text_click("Inactive leaf");
        h.frame(vec![]);
    }
    assert_eq!(h.app.plans.canvas_rect, canvas);
    assert_eq!(h.handles(), 1);
    let point = h.point(pair_geometry(h, id, leaf, None).point) + grab;
    h.hover(point);
    h.frame(vec![button(point, true)]);
    assert!(h.app.plans.opening_pose_claimed);
    point
}

#[test]
fn paired_pose_both_leaves_profiles_states_preview_history_reopen_and_accessories() {
    for profile in 0..2 {
        for reversed in [false, true] {
            for handing in [false, true] {
                for leaf in [PoseLeaf::Active, PoseLeaf::Inactive] {
                    for state in [
                        OpeningState::Default,
                        OpeningState::DoorAngle(0.),
                        OpeningState::DoorPairAngles {
                            active_degrees: 20.,
                            inactive_degrees: 70.,
                        },
                    ] {
                        let (mut h, id) = pair_fixture(profile, reversed, handing, state);
                        let before = h.app.editor.document.model().clone();
                        let scene = h.app.editor.scene.clone();
                        let camera = h.app.plans.cameras[&h.view];
                        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        let apertures = h
                            .app
                            .plans
                            .drawing
                            .as_ref()
                            .unwrap()
                            .items(context)
                            .unwrap()
                            .to_vec();
                        let other = if leaf == PoseLeaf::Active {
                            PoseLeaf::Inactive
                        } else {
                            PoseLeaf::Active
                        };
                        let unchanged = pair_geometry(&h, id, other, None).value;
                        let grab = egui::vec2(3., -2.);
                        start_pair(&mut h, id, leaf, grab);
                        let target = h.point(pair_geometry(&h, id, leaf, Some(37.)).point) + grab;
                        h.frame(vec![egui::Event::PointerMoved(target)]);
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.scene, scene);
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                        assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                        assert_eq!(h.app.plans.cameras[&h.view], camera);
                        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                            egui::Shape::LineSegment { stroke, .. } if stroke.color == theme::ACCENT)),
                            "selected symbol preview is painted");
                        let preview = h.output.shapes.clone();
                        h.frame(vec![button(target, false)]);
                        assert!(!h.app.status_error, "{}", h.app.status);
                        // egui pointer positions are f32; short leaves amplify
                        // subpixel rounding in degrees. Compare the actual free
                        // edge to the requested pose at the gesture's pixel precision.
                        let actual = pair_geometry(&h, id, leaf, None);
                        let requested = pair_geometry(&h, id, leaf, Some(37.));
                        assert!(
                            actual.point.distance(requested.point) * camera.pixels_per_metre
                                < 0.001,
                            "profile {profile}, reversed {reversed}, handing {handing}, leaf {leaf:?}, state {state:?}: actual {}, status {}",
                            actual.value,
                            h.app.status
                        );
                        assert_eq!(pair_geometry(&h, id, other, None).value, unchanged);
                        let after = h.app.editor.document.model().clone();
                        let mut expected = before.clone();
                        expected
                            .openings
                            .get_mut(&id)
                            .unwrap()
                            .parameters
                            .open_state = after.openings[&id].parameters.open_state;
                        assert_eq!(after, expected, "only selected pose may change");
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                        h.settle();
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        let drawing = h.app.plans.drawing.as_ref().unwrap();
                        assert_eq!(drawing.items(context).unwrap(), apertures);
                        for line in drawing.provider_lines(context).unwrap().iter().filter(|l| {
                            l.entity == id && l.role == os_geometry::plan::PlanRole::Cut
                        }) {
                            let a = h.point(context.basis.plane_to_world(line.start).unwrap());
                            let b = h.point(context.basis.plane_to_world(line.end).unwrap());
                            assert!(
                                preview.iter().any(
                                    |s| matches!(&s.shape, egui::Shape::LineSegment { points, .. }
                                if (points[0].distance(a)<0.01 && points[1].distance(b)<0.01)
                                || (points[1].distance(a)<0.01 && points[0].distance(b)<0.01))
                                ),
                                "preview matches committed symbol"
                            );
                        }
                        let solids = |m: &Model| {
                            let p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
                            os_geometry::openings::component_solids(
                                &p,
                                &m.resolve_wall(p.host).unwrap().parameters,
                                0.,
                            )
                            .unwrap()
                        };
                        let a = solids(&before);
                        let b = solids(&after);
                        let selected = if leaf == PoseLeaf::Active { 0 } else { 1 };
                        // Authored solids follow base solids, in family order.
                        assert_ne!(a[a.len() - 2 + selected], b[b.len() - 2 + selected]);
                        assert_eq!(
                            a[a.len() - 2 + (1 - selected)],
                            b[b.len() - 2 + (1 - selected)]
                        );
                        assert_ne!(h.app.editor.scene, scene, "committed 3D regenerates");
                        h.app.editor.document.undo();
                        assert_eq!(h.app.editor.document.model(), &before);
                        h.app.editor.document.redo();
                        assert_eq!(h.app.editor.document.model(), &after);
                        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
                        let path = dir.path().join("paired-pose.osb");
                        ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
                        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);
                    }
                }
            }
        }
    }
}

#[test]
fn paired_pose_noop_representation_quantization_and_selector_cancellation() {
    for profile in 0..2 {
        for leaf in [PoseLeaf::Active, PoseLeaf::Inactive] {
            for state in [
                OpeningState::Default,
                OpeningState::DoorAngle(0.),
                OpeningState::DoorPairAngles {
                    active_degrees: 20.,
                    inactive_degrees: 70.,
                },
            ] {
                let (mut h, id) = pair_fixture(profile, false, false, state);
                let before = h.app.editor.document.model().clone();
                let grab = egui::vec2(4., 1.);
                let origin = start_pair(&mut h, id, leaf, grab);
                let target = h.point(pair_geometry(&h, id, leaf, Some(37.)).point) + grab;
                h.frame(vec![egui::Event::PointerMoved(target)]);
                h.frame(vec![egui::Event::PointerMoved(origin)]);
                h.frame(vec![button(origin, false)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                h.settle();
                start_pair(&mut h, id, leaf, egui::Vec2::ZERO);
                let target = h.point(pair_geometry(&h, id, leaf, Some(37.)).point);
                h.time += 1. / 60.;
                let mut raw = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, h.size)),
                    time: Some(h.time),
                    events: vec![egui::Event::PointerMoved(target), button(target, false)],
                    modifiers: egui::Modifiers::SHIFT,
                    ..Default::default()
                };
                raw.viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .native_pixels_per_point = Some(h.scale);
                h.output = h.ctx.run(raw, |ctx| h.app.show(ctx));
                assert_eq!(pair_geometry(&h, id, leaf, None).value, 30.);
                h.settle();
                start_pair(&mut h, id, leaf, egui::Vec2::ZERO);
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                let target = h.point(pair_geometry(&h, id, leaf, Some(60.)).point);
                h.frame(vec![egui::Event::PointerMoved(target)]);
                let other = if leaf == PoseLeaf::Active {
                    "Inactive leaf"
                } else {
                    "Active leaf"
                };
                let selector = h.app.plans.canvas_rect.unwrap().right_top()
                    + egui::vec2(
                        if leaf == PoseLeaf::Active {
                            -60.
                        } else {
                            -168.
                        },
                        20.,
                    );
                h.frame(vec![
                    egui::Event::PointerMoved(selector),
                    button(selector, false),
                ]);
                h.text_click(other);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert!(h.app.plans.opening_pose.is_some());
                h.frame(vec![escape()]);
            }
        }
    }
}

#[test]
fn paired_pose_unwrap_opposite_ray_both_directions_and_shift() {
    for profile in 0..2 {
        for operation in 0..2 {
            for reversed in [false, true] {
                for direction in [-1., 1.] {
                    for shift in [false, true] {
                        let leaf = if operation == 0 {
                            PoseLeaf::Active
                        } else {
                            PoseLeaf::Inactive
                        };
                        let (mut h, id) =
                            pair_fixture(profile, reversed, true, OpeningState::DoorAngle(45.));
                        let g = pair_geometry(&h, id, leaf, None);
                        let PoseMotion::Angular { pivot, sense } = g.motion else {
                            unreachable!()
                        };
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        h.app.plans.cameras.insert(
                            h.view,
                            PlanCamera {
                                center: context.basis.world_to_plane(pivot).unwrap(),
                                pixels_per_metre: 35.,
                            },
                        );
                        h.frame(vec![]);
                        let before = h.app.editor.document.model().clone();
                        let offset = egui::vec2(3., -2.);
                        start_pair(&mut h, id, leaf, offset);
                        // Cross the initial vector's opposite ray, then retrace it.
                        // The endpoint stays clamped until accumulated motion returns.
                        for degrees in [37_f64, 100., 170., 179., 181., 190., 181., 179., 100., 37.]
                        {
                            let angle = (degrees * direction * sense).to_radians();
                            let x = g.point.x - pivot.x;
                            let y = g.point.y - pivot.y;
                            let target = h.point(Point2::new(
                                pivot.x + x * angle.cos() - y * angle.sin(),
                                pivot.y + x * angle.sin() + y * angle.cos(),
                            )) + offset;
                            let expected = 45. + degrees * direction;
                            let expected = if shift {
                                (expected / 15.).round() * 15.
                            } else {
                                expected
                            }
                            .clamp(0., 90.);
                            h.time += 1. / 60.;
                            let mut raw = egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    h.size,
                                )),
                                time: Some(h.time),
                                events: vec![egui::Event::PointerMoved(target)],
                                modifiers: if shift {
                                    egui::Modifiers::SHIFT
                                } else {
                                    egui::Modifiers::NONE
                                },
                                ..Default::default()
                            };
                            raw.viewports
                                .get_mut(&egui::ViewportId::ROOT)
                                .unwrap()
                                .native_pixels_per_point = Some(h.scale);
                            h.output = h.ctx.run(raw, |ctx| h.app.show(ctx));
                            assert!(
                                h.has_text(&format!(
                                    "{} · {expected:.1}°",
                                    if leaf == PoseLeaf::Active {
                                        "Active leaf"
                                    } else {
                                        "Inactive leaf"
                                    }
                                )),
                                "profile {profile}, operation {operation}, reversed {reversed}, direction {direction}, shift {shift}, degrees {degrees}"
                            );
                            assert_eq!(h.app.editor.document.model(), &before);
                            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                        }
                        h.frame(vec![escape()]);
                        assert!(h.app.plans.opening_pose.is_none());
                        assert_eq!(h.app.editor.document.model(), &before);
                        // A fresh gesture releases beyond the opposite ray and commits
                        // the same clamped endpoint that was previewed.
                        h.frame(vec![button(h.point(g.point) + offset, false)]);
                        // The cancelled press remains owned through its release frame.
                        // Render the next frame before acquiring the restored action.
                        h.frame(vec![]);
                        start_pair(&mut h, id, leaf, offset);
                        for degrees in [90_f64, 170., 190.] {
                            let angle = (degrees * direction * sense).to_radians();
                            let x = g.point.x - pivot.x;
                            let y = g.point.y - pivot.y;
                            let target = h.point(Point2::new(
                                pivot.x + x * angle.cos() - y * angle.sin(),
                                pivot.y + x * angle.sin() + y * angle.cos(),
                            )) + offset;
                            h.frame(vec![egui::Event::PointerMoved(target)]);
                            if degrees == 190. {
                                h.frame(vec![button(target, false)]);
                            }
                        }
                        let value = if direction > 0. { 90. } else { 0. };
                        let mut expected = before.clone();
                        expected
                            .openings
                            .get_mut(&id)
                            .unwrap()
                            .parameters
                            .open_state = if operation == 0 {
                            OpeningState::DoorPairAngles {
                                active_degrees: value,
                                inactive_degrees: 45.,
                            }
                        } else {
                            OpeningState::DoorPairAngles {
                                active_degrees: 45.,
                                inactive_degrees: value,
                            }
                        };
                        assert_eq!(h.app.editor.document.model(), &expected);
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                    }
                }
            }
        }
    }
}

#[test]
fn paired_pose_hit_radius_and_split_canvas_ownership() {
    for profile in 0..2 {
        for leaf in [PoseLeaf::Active, PoseLeaf::Inactive] {
            for distance in [9.9, 10.1] {
                let (mut h, id) = pair_fixture(profile, false, false, OpeningState::DoorAngle(0.));
                h.app.plans.split = true;
                h.frame(vec![]);
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let camera = h.app.plans.cameras[&h.view];
                let offset = egui::vec2(0., distance);
                start_pair(&mut h, id, leaf, offset);
                let target = h.point(pair_geometry(&h, id, leaf, Some(45.)).point) + offset;
                h.frame(vec![egui::Event::PointerMoved(target)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                h.frame(vec![button(target, false)]);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                if distance < 10. {
                    assert!((pair_geometry(&h, id, leaf, None).value - 45.).abs() < 1e-4);
                    h.settle();
                    assert_ne!(h.app.editor.scene, scene);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                } else {
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                }
            }
        }
    }
}

fn casement_pair_fixture(
    profile: usize,
    reversed: bool,
    unequal: bool,
    state: OpeningState,
) -> (Harness, Id) {
    let (mut h, id) = fixture(profile, 1, reversed, unequal);
    let mut m = h.app.editor.document.model().clone();
    let p = &mut m.openings.get_mut(&id).unwrap().parameters;
    p.open_state = state;
    let ty = p.type_id().unwrap();
    let p = &mut m.opening_types.get_mut(&ty).unwrap().parameters;
    p.window_operation = WindowOperation::PairedCasement {
        start_fraction: if unequal { 0.65 } else { 0.5 },
        mullion_width: 0.04,
    };
    p.family.extrusions.clear();
    for attachment in [
        os_model::OpeningAttachment::StartSash,
        os_model::OpeningAttachment::EndSash,
    ] {
        let mut solid = os_model::OpeningExtrusion::new("Sash accessory");
        solid.attachment = attachment;
        p.family.extrusions.push(solid);
    }
    refresh(&mut h, m, id);
    (h, id)
}
fn casement_pair_geometry(
    h: &Harness,
    id: Id,
    leaf: PoseLeaf,
    value: Option<f64>,
) -> os_geometry::openings::OpeningPoseHandle {
    let m = h.app.editor.document.model();
    let mut p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
    if let Some(value) = value {
        let [start, end] = p.casement_pair_angles();
        p.open_state = OpeningState::CasementPairAngles {
            start_degrees: if leaf == PoseLeaf::StartSash {
                value
            } else {
                start
            },
            end_degrees: if leaf == PoseLeaf::EndSash {
                value
            } else {
                end
            },
        };
    }
    pose_handle_for_leaf(&p, &m.resolve_wall(p.host).unwrap().parameters, leaf).unwrap()
}
fn start_casement_pair(h: &mut Harness, id: Id, leaf: PoseLeaf, grab: egui::Vec2) -> egui::Pos2 {
    assert!(h.app.opening_pose_available());
    let canvas = h.app.plans.canvas_rect;
    h.text_click("Adjust opening pose");
    h.frame(vec![]);
    assert!(h.has_text("Start sash") && h.has_text("End sash"));
    if leaf == PoseLeaf::EndSash {
        h.text_click("End sash");
        h.frame(vec![]);
    }
    assert_eq!(h.app.plans.canvas_rect, canvas);
    assert_eq!(h.handles(), 1);
    let point = h.point(casement_pair_geometry(h, id, leaf, None).point) + grab;
    h.hover(point);
    h.frame(vec![button(point, true)]);
    assert!(h.app.plans.opening_pose_claimed);
    point
}

#[test]
fn paired_casement_pose_both_leaves_profiles_states_preview_history_reopen_and_accessories() {
    for profile in 0..2 {
        for reversed in [false, true] {
            for handing in [false, true] {
                for leaf in [PoseLeaf::StartSash, PoseLeaf::EndSash] {
                    for state in [
                        OpeningState::Default,
                        OpeningState::CasementPairAngles {
                            start_degrees: 0.,
                            end_degrees: 0.,
                        },
                        OpeningState::CasementPairAngles {
                            start_degrees: 20.,
                            end_degrees: 70.,
                        },
                    ] {
                        let (mut h, id) = casement_pair_fixture(profile, reversed, handing, state);
                        let before = h.app.editor.document.model().clone();
                        let scene = h.app.editor.scene.clone();
                        let camera = h.app.plans.cameras[&h.view];
                        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        let apertures = h
                            .app
                            .plans
                            .drawing
                            .as_ref()
                            .unwrap()
                            .items(context)
                            .unwrap()
                            .to_vec();
                        let other = if leaf == PoseLeaf::StartSash {
                            PoseLeaf::EndSash
                        } else {
                            PoseLeaf::StartSash
                        };
                        let unchanged = casement_pair_geometry(&h, id, other, None).value;
                        let grab = egui::vec2(3., -2.);
                        start_casement_pair(&mut h, id, leaf, grab);
                        let target =
                            h.point(casement_pair_geometry(&h, id, leaf, Some(37.)).point) + grab;
                        h.frame(vec![egui::Event::PointerMoved(target)]);
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.scene, scene);
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                        assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                        assert_eq!(h.app.plans.cameras[&h.view], camera);
                        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                            egui::Shape::LineSegment { stroke, .. } if stroke.color == theme::ACCENT)),
                            "selected symbol preview is painted");
                        let preview = h.output.shapes.clone();
                        h.frame(vec![button(target, false)]);
                        assert!(!h.app.status_error, "{}", h.app.status);
                        // egui pointer positions are f32; short leaves amplify
                        // subpixel rounding in degrees. Compare the actual free
                        // edge to the requested pose at the gesture's pixel precision.
                        let actual = casement_pair_geometry(&h, id, leaf, None);
                        let requested = casement_pair_geometry(&h, id, leaf, Some(37.));
                        assert!(
                            actual.point.distance(requested.point) * camera.pixels_per_metre
                                < 0.001,
                            "profile {profile}, reversed {reversed}, handing {handing}, leaf {leaf:?}, state {state:?}: actual {}, status {}",
                            actual.value,
                            h.app.status
                        );
                        assert_eq!(casement_pair_geometry(&h, id, other, None).value, unchanged);
                        let after = h.app.editor.document.model().clone();
                        let mut expected = before.clone();
                        expected
                            .openings
                            .get_mut(&id)
                            .unwrap()
                            .parameters
                            .open_state = after.openings[&id].parameters.open_state;
                        assert_eq!(after, expected, "only selected pose may change");
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                        h.settle();
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        let drawing = h.app.plans.drawing.as_ref().unwrap();
                        assert_eq!(drawing.items(context).unwrap(), apertures);
                        for line in drawing.provider_lines(context).unwrap().iter().filter(|l| {
                            l.entity == id && l.role == os_geometry::plan::PlanRole::Cut
                        }) {
                            let a = h.point(context.basis.plane_to_world(line.start).unwrap());
                            let b = h.point(context.basis.plane_to_world(line.end).unwrap());
                            assert!(
                                preview.iter().any(
                                    |s| matches!(&s.shape, egui::Shape::LineSegment { points, .. }
                                if (points[0].distance(a)<0.01 && points[1].distance(b)<0.01)
                                || (points[1].distance(a)<0.01 && points[0].distance(b)<0.01))
                                ),
                                "preview matches committed symbol"
                            );
                        }
                        let solids = |m: &Model| {
                            let p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
                            os_geometry::openings::component_solids(
                                &p,
                                &m.resolve_wall(p.host).unwrap().parameters,
                                0.,
                            )
                            .unwrap()
                        };
                        let a = solids(&before);
                        let b = solids(&after);
                        let selected = if leaf == PoseLeaf::StartSash { 0 } else { 1 };
                        // Authored solids follow base solids, in family order.
                        assert_ne!(a[a.len() - 2 + selected], b[b.len() - 2 + selected]);
                        assert_eq!(
                            a[a.len() - 2 + (1 - selected)],
                            b[b.len() - 2 + (1 - selected)]
                        );
                        assert_ne!(h.app.editor.scene, scene, "committed 3D regenerates");
                        h.app.editor.document.undo();
                        assert_eq!(h.app.editor.document.model(), &before);
                        h.app.editor.document.redo();
                        assert_eq!(h.app.editor.document.model(), &after);
                        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
                        let path = dir.path().join("paired-pose.osb");
                        ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
                        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);
                    }
                }
            }
        }
    }
}

#[test]
fn paired_casement_pose_noop_representation_quantization_and_selector_cancellation() {
    for profile in 0..2 {
        for leaf in [PoseLeaf::StartSash, PoseLeaf::EndSash] {
            for state in [
                OpeningState::Default,
                OpeningState::CasementPairAngles {
                    start_degrees: 0.,
                    end_degrees: 0.,
                },
                OpeningState::CasementPairAngles {
                    start_degrees: 20.,
                    end_degrees: 70.,
                },
            ] {
                let (mut h, id) = casement_pair_fixture(profile, false, false, state);
                let before = h.app.editor.document.model().clone();
                let grab = egui::vec2(4., 1.);
                let origin = start_casement_pair(&mut h, id, leaf, grab);
                let target = h.point(casement_pair_geometry(&h, id, leaf, Some(37.)).point) + grab;
                h.frame(vec![egui::Event::PointerMoved(target)]);
                h.frame(vec![egui::Event::PointerMoved(origin)]);
                h.frame(vec![button(origin, false)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                h.settle();
                start_casement_pair(&mut h, id, leaf, egui::Vec2::ZERO);
                let target = h.point(casement_pair_geometry(&h, id, leaf, Some(37.)).point);
                h.time += 1. / 60.;
                let mut raw = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, h.size)),
                    time: Some(h.time),
                    events: vec![egui::Event::PointerMoved(target), button(target, false)],
                    modifiers: egui::Modifiers::SHIFT,
                    ..Default::default()
                };
                raw.viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .native_pixels_per_point = Some(h.scale);
                h.output = h.ctx.run(raw, |ctx| h.app.show(ctx));
                assert_eq!(casement_pair_geometry(&h, id, leaf, None).value, 30.);
                h.settle();
                start_casement_pair(&mut h, id, leaf, egui::Vec2::ZERO);
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                let target = h.point(casement_pair_geometry(&h, id, leaf, Some(60.)).point);
                h.frame(vec![egui::Event::PointerMoved(target)]);
                let other = if leaf == PoseLeaf::StartSash {
                    "End sash"
                } else {
                    "Start sash"
                };
                let selector = h.app.plans.canvas_rect.unwrap().right_top()
                    + egui::vec2(
                        if leaf == PoseLeaf::StartSash {
                            -60.
                        } else {
                            -168.
                        },
                        20.,
                    );
                h.frame(vec![
                    egui::Event::PointerMoved(selector),
                    button(selector, false),
                ]);
                h.text_click(other);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert!(h.app.plans.opening_pose.is_some());
                h.frame(vec![escape()]);
            }
        }
    }
}

#[test]
fn paired_casement_pose_unwrap_opposite_ray_both_directions_and_shift() {
    for profile in 0..2 {
        for operation in 0..2 {
            for reversed in [false, true] {
                for direction in [-1., 1.] {
                    for shift in [false, true] {
                        let leaf = if operation == 0 {
                            PoseLeaf::StartSash
                        } else {
                            PoseLeaf::EndSash
                        };
                        let (mut h, id) = casement_pair_fixture(
                            profile,
                            reversed,
                            true,
                            OpeningState::CasementPairAngles {
                                start_degrees: 45.,
                                end_degrees: 45.,
                            },
                        );
                        let g = casement_pair_geometry(&h, id, leaf, None);
                        let PoseMotion::Angular { pivot, sense } = g.motion else {
                            unreachable!()
                        };
                        let context = h.app.editor.native_plan_context(h.view).unwrap();
                        h.app.plans.cameras.insert(
                            h.view,
                            PlanCamera {
                                center: context.basis.world_to_plane(pivot).unwrap(),
                                pixels_per_metre: 35.,
                            },
                        );
                        h.frame(vec![]);
                        let before = h.app.editor.document.model().clone();
                        let offset = egui::vec2(3., -2.);
                        start_casement_pair(&mut h, id, leaf, offset);
                        // Cross the initial vector's opposite ray, then retrace it.
                        // The endpoint stays clamped until accumulated motion returns.
                        for degrees in [37_f64, 100., 170., 179., 181., 190., 181., 179., 100., 37.]
                        {
                            let angle = (degrees * direction * sense).to_radians();
                            let x = g.point.x - pivot.x;
                            let y = g.point.y - pivot.y;
                            let target = h.point(Point2::new(
                                pivot.x + x * angle.cos() - y * angle.sin(),
                                pivot.y + x * angle.sin() + y * angle.cos(),
                            )) + offset;
                            let expected = 45. + degrees * direction;
                            let expected = if shift {
                                (expected / 15.).round() * 15.
                            } else {
                                expected
                            }
                            .clamp(0., 90.);
                            h.time += 1. / 60.;
                            let mut raw = egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    h.size,
                                )),
                                time: Some(h.time),
                                events: vec![egui::Event::PointerMoved(target)],
                                modifiers: if shift {
                                    egui::Modifiers::SHIFT
                                } else {
                                    egui::Modifiers::NONE
                                },
                                ..Default::default()
                            };
                            raw.viewports
                                .get_mut(&egui::ViewportId::ROOT)
                                .unwrap()
                                .native_pixels_per_point = Some(h.scale);
                            h.output = h.ctx.run(raw, |ctx| h.app.show(ctx));
                            assert!(
                                h.has_text(&format!(
                                    "{} · {expected:.1}°",
                                    if leaf == PoseLeaf::StartSash {
                                        "Start sash"
                                    } else {
                                        "End sash"
                                    }
                                )),
                                "profile {profile}, operation {operation}, reversed {reversed}, direction {direction}, shift {shift}, degrees {degrees}"
                            );
                            assert_eq!(h.app.editor.document.model(), &before);
                            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                        }
                        h.frame(vec![escape()]);
                        assert!(h.app.plans.opening_pose.is_none());
                        assert_eq!(h.app.editor.document.model(), &before);
                        // A fresh gesture releases beyond the opposite ray and commits
                        // the same clamped endpoint that was previewed.
                        h.frame(vec![button(h.point(g.point) + offset, false)]);
                        // The cancelled press remains owned through its release frame.
                        // Render the next frame before acquiring the restored action.
                        h.frame(vec![]);
                        start_casement_pair(&mut h, id, leaf, offset);
                        for degrees in [90_f64, 170., 190.] {
                            let angle = (degrees * direction * sense).to_radians();
                            let x = g.point.x - pivot.x;
                            let y = g.point.y - pivot.y;
                            let target = h.point(Point2::new(
                                pivot.x + x * angle.cos() - y * angle.sin(),
                                pivot.y + x * angle.sin() + y * angle.cos(),
                            )) + offset;
                            h.frame(vec![egui::Event::PointerMoved(target)]);
                            if degrees == 190. {
                                h.frame(vec![button(target, false)]);
                            }
                        }
                        let value = if direction > 0. { 90. } else { 0. };
                        let mut expected = before.clone();
                        expected
                            .openings
                            .get_mut(&id)
                            .unwrap()
                            .parameters
                            .open_state = if operation == 0 {
                            OpeningState::CasementPairAngles {
                                start_degrees: value,
                                end_degrees: 45.,
                            }
                        } else {
                            OpeningState::CasementPairAngles {
                                start_degrees: 45.,
                                end_degrees: value,
                            }
                        };
                        assert_eq!(h.app.editor.document.model(), &expected);
                        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                    }
                }
            }
        }
    }
}

#[test]
fn paired_casement_pose_hit_radius_and_split_canvas_ownership() {
    for profile in 0..2 {
        for leaf in [PoseLeaf::StartSash, PoseLeaf::EndSash] {
            for distance in [9.9, 10.1] {
                let (mut h, id) = casement_pair_fixture(
                    profile,
                    false,
                    false,
                    OpeningState::CasementPairAngles {
                        start_degrees: 0.,
                        end_degrees: 0.,
                    },
                );
                h.app.plans.split = true;
                h.frame(vec![]);
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let camera = h.app.plans.cameras[&h.view];
                let offset = egui::vec2(0., distance);
                start_casement_pair(&mut h, id, leaf, offset);
                let target =
                    h.point(casement_pair_geometry(&h, id, leaf, Some(45.)).point) + offset;
                h.frame(vec![egui::Event::PointerMoved(target)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                h.frame(vec![button(target, false)]);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                if distance < 10. {
                    assert!((casement_pair_geometry(&h, id, leaf, None).value - 45.).abs() < 1e-4);
                    h.settle();
                    assert_ne!(h.app.editor.scene, scene);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                } else {
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
                }
            }
        }
    }
}
