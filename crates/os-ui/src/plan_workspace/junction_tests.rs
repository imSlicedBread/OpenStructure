//! Real egui pointer frames at both supported desktop profiles.
use super::*;
use os_model::{WallAnchor, WallEndpoint, WallJoin, WallJoinParams};

fn fixture(
    size: egui::Vec2,
    scale: f32,
    reverse_a: bool,
    reverse_b: bool,
) -> (Harness, [WallAnchor; 2]) {
    let mut h = Harness::new(size, scale);
    let mut model = h.app.editor.document.model().clone();
    let a = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
    *a.path.straight_start_mut().unwrap() = Point2::new(-1.0, 0.0);
    *a.path.straight_end_mut().unwrap() = Point2::new(0.0, 0.0);
    if reverse_a {
        a.path = os_model::WallPath::Straight {
            start: a.end(),
            end: a.start(),
        };
    }
    let mut b = a.clone();
    *b.path.straight_start_mut().unwrap() = Point2::new(0.0, 0.0);
    *b.path.straight_end_mut().unwrap() = Point2::new(1.0, 0.0);
    if reverse_b {
        b.path = os_model::WallPath::Straight {
            start: b.end(),
            end: b.start(),
        };
    }
    let b = os_model::Wall::new(os_walls::WALL_TYPE, b);
    let anchors = [
        WallAnchor {
            wall: h.wall,
            endpoint: if reverse_a {
                WallEndpoint::Start
            } else {
                WallEndpoint::End
            },
        },
        WallAnchor {
            wall: b.id(),
            endpoint: if reverse_b {
                WallEndpoint::End
            } else {
                WallEndpoint::Start
            },
        },
    ];
    model.walls.insert(b.id(), b);
    let join = WallJoin::new(
        "core.wall_join",
        WallJoinParams::Butt {
            a: anchors[0],
            b: anchors[1],
        },
    );
    model.wall_joins.insert(join.id(), join);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.plans.snaps.enabled = false;
    h.settle();
    (h, anchors)
}

#[allow(clippy::too_many_arguments)]
fn perpendicular_fixture(
    size: egui::Vec2,
    scale: f32,
    kind: &str,
    reverse_a: bool,
    reverse_b: bool,
    select_b: bool,
    owner_b: bool,
) -> (Harness, [WallAnchor; 2], Id) {
    let (mut h, anchors) = fixture(size, scale, reverse_a, reverse_b);
    let mut model = h.app.editor.document.model().clone();
    let join = *model.wall_joins.keys().next().unwrap();
    let b = &mut model.walls.get_mut(&anchors[1].wall).unwrap().parameters;
    *b.path.straight_start_mut().unwrap() = Point2::new(0.0, if reverse_b { 1.0 } else { 0.0 });
    *b.path.straight_end_mut().unwrap() = Point2::new(0.0, if reverse_b { 0.0 } else { 1.0 });
    model.wall_joins.get_mut(&join).unwrap().parameters = if kind == "corner" {
        WallJoinParams::Corner {
            a: anchors[0],
            b: anchors[1],
            owner: anchors[usize::from(owner_b)].wall,
        }
    } else {
        let host = &mut model.walls.get_mut(&anchors[0].wall).unwrap().parameters;
        *host.path.straight_start_mut().unwrap() =
            Point2::new(if reverse_a { 1.0 } else { -1.0 }, 0.0);
        *host.path.straight_end_mut().unwrap() =
            Point2::new(if reverse_a { -1.0 } else { 1.0 }, 0.0);
        WallJoinParams::Tee {
            host: anchors[0].wall,
            station: 1.0,
            branch: anchors[1],
        }
    };
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(anchors[usize::from(select_b)].wall));
    h.settle();
    (h, anchors, join)
}

fn world_opening(model: &os_model::Model, opening: Id) -> Point2 {
    let p = &model.openings[&opening].parameters;
    os_model::wall_point(&model.walls[&p.host].parameters, p.offset, 0.0)
}

fn interaction_fixture(size: egui::Vec2, scale: f32, kind: &str) -> Harness {
    if let Some(base) = kind.strip_suffix(" chain") {
        let mut h = interaction_fixture(size, scale, base);
        let mut model = h.app.editor.document.model().clone();
        let parameters = model.wall_joins.values().next().unwrap().parameters.clone();
        let branch = *parameters.anchors().last().unwrap();
        let wall = &model.walls[&branch.wall].parameters;
        let start = branch.endpoint.opposite().point(wall);
        let node = branch.endpoint.point(wall);
        let end = Point2::new(start.x + start.x - node.x, start.y + start.y - node.y);
        let peer = graph_wall(
            &mut model,
            branch.wall,
            (start.x, start.y),
            (end.x, end.y),
            false,
        );
        graph_join(
            &mut model,
            WallJoinParams::Butt {
                a: WallAnchor {
                    endpoint: branch.endpoint.opposite(),
                    ..branch
                },
                b: peer[0],
            },
        );
        let selected = h.app.selected.unwrap();
        install_graph(&mut h, model, selected);
        return h;
    }
    if kind == "butt" {
        fixture(size, scale, false, false).0
    } else {
        perpendicular_fixture(
            size,
            scale,
            if kind == "corner" { "corner" } else { "tee" },
            false,
            false,
            kind == "tee branch",
            kind == "corner",
        )
        .0
    }
}

#[test]
fn junction_corner_both_owners_directions_and_selected_roles_preserve_identity_and_openings() {
    for (size, scale) in PROFILES {
        for reverse_a in [false, true] {
            for reverse_b in [false, true] {
                for select_b in [false, true] {
                    for owner_b in [false, true] {
                        let (mut h, anchors, _) = perpendicular_fixture(
                            size, scale, "corner", reverse_a, reverse_b, select_b, owner_b,
                        );
                        let selected = anchors[usize::from(select_b)];
                        let peer = anchors[usize::from(!select_b)];
                        let selected_opening = add_opening(&mut h, selected.wall);
                        let peer_opening = add_opening(&mut h, peer.wall);
                        let before = h.app.editor.document.model().clone();
                        let scene = h.app.editor.scene.clone();
                        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                        let camera = h.app.plans.cameras[&h.view];
                        h.press(h.point(Point2::new(0.0, 0.0)));
                        assert!(h.app.plans.junction_drag.is_some(), "{}", h.app.status);
                        let target = Point2::new(0.1, 0.1);
                        let p = h.point(target);
                        h.frame(vec![egui::Event::PointerMoved(p)]);
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert!(!h.app.editor.document.can_undo());
                        assert_eq!(h.app.editor.scene, scene);
                        assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                        assert_eq!(h.app.plans.cameras[&h.view], camera);
                        h.release(p);
                        assert!(!h.app.status_error, "{}", h.app.status);
                        let after = h.app.editor.document.model().clone();
                        let node = selected
                            .endpoint
                            .point(&after.walls[&selected.wall].parameters);
                        let expected_node = if select_b {
                            Point2::new(0.0, 0.1)
                        } else {
                            Point2::new(0.1, 0.0)
                        };
                        assert!(node.distance(expected_node) < 1e-6);
                        assert_eq!(
                            node,
                            peer.endpoint.point(&after.walls[&peer.wall].parameters)
                        );
                        let mut expected = before.clone();
                        let wall = &mut expected.walls.get_mut(&selected.wall).unwrap().parameters;
                        match selected.endpoint {
                            WallEndpoint::Start => *wall.path.straight_start_mut().unwrap() = node,
                            WallEndpoint::End => *wall.path.straight_end_mut().unwrap() = node,
                        }
                        let wall = &mut expected.walls.get_mut(&peer.wall).unwrap().parameters;
                        *wall.path.straight_start_mut().unwrap() =
                            Point2::new(wall.start().x + node.x, wall.start().y + node.y);
                        *wall.path.straight_end_mut().unwrap() =
                            Point2::new(wall.end().x + node.x, wall.end().y + node.y);
                        expected
                            .openings
                            .get_mut(&selected_opening)
                            .unwrap()
                            .parameters
                            .offset = after.openings[&selected_opening].parameters.offset;
                        assert_eq!(after, expected);
                        assert!(
                            world_opening(&after, selected_opening)
                                .distance(world_opening(&before, selected_opening))
                                < 1e-12
                        );
                        let old = world_opening(&before, peer_opening);
                        assert!(
                            world_opening(&after, peer_opening)
                                .distance(Point2::new(old.x + node.x, old.y + node.y))
                                < 1e-12
                        );
                        assert_eq!(h.app.editor.document.revision(), 1);
                        assert_ne!(h.app.editor.scene, scene);
                        h.app.history(false);
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert!(!h.app.editor.document.can_undo());
                        h.app.history(true);
                        assert_eq!(h.app.editor.document.model(), &after);
                    }
                }
            }
        }
    }
}

#[test]
fn junction_tee_host_station_and_branch_grips_reversed_axes_exact_contact_and_atomic_history() {
    for (size, scale) in PROFILES {
        for reverse_a in [false, true] {
            for reverse_b in [false, true] {
                for select_b in [false, true] {
                    let (mut h, anchors, join) = perpendicular_fixture(
                        size, scale, "tee", reverse_a, reverse_b, select_b, false,
                    );
                    let host = anchors[0].wall;
                    let branch = anchors[1];
                    let host_opening = add_opening(&mut h, host);
                    let branch_opening = add_opening(&mut h, branch.wall);
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                    let camera = h.app.plans.cameras[&h.view];
                    h.press(h.point(Point2::new(0.0, 0.0)));
                    let draft = h.app.plans.junction_drag.as_ref().expect("Tee grip");
                    assert_eq!(
                        draft.grip,
                        if select_b {
                            junctions::Grip::Endpoint(if reverse_b {
                                WallEdit::ResizeEnd
                            } else {
                                WallEdit::ResizeStart
                            })
                        } else {
                            junctions::Grip::TeeStation(join)
                        }
                    );
                    assert!(h.app.wall_gesture.is_none());
                    let p = h.point(Point2::new(-0.1, 0.15));
                    h.frame(vec![egui::Event::PointerMoved(p)]);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.revision(), 0);
                    assert!(!h.app.editor.document.can_undo());
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                    h.release(p);
                    assert!(!h.app.status_error, "{}", h.app.status);
                    let after = h.app.editor.document.model().clone();
                    let WallJoinParams::Tee { station, .. } = after.wall_joins[&join].parameters
                    else {
                        panic!("Tee")
                    };
                    assert!((station - if reverse_a { 1.1 } else { 0.9 }).abs() < 1e-6);
                    let node = os_model::wall_point(&before.walls[&host].parameters, station, 0.0);
                    assert_eq!(
                        node,
                        branch.endpoint.point(&after.walls[&branch.wall].parameters)
                    );
                    let mut expected = before.clone();
                    expected.wall_joins.get_mut(&join).unwrap().parameters = WallJoinParams::Tee {
                        host,
                        station,
                        branch,
                    };
                    let wall = &mut expected.walls.get_mut(&branch.wall).unwrap().parameters;
                    wall.path.straight_start_mut().unwrap().x += node.x;
                    wall.path.straight_end_mut().unwrap().x += node.x;
                    assert_eq!(after, expected);
                    assert_eq!(
                        world_opening(&after, host_opening),
                        world_opening(&before, host_opening)
                    );
                    let old = world_opening(&before, branch_opening);
                    assert!(
                        world_opening(&after, branch_opening)
                            .distance(Point2::new(old.x + node.x, old.y))
                            < 1e-12
                    );
                    assert_eq!(h.app.editor.document.revision(), 1);
                    assert_ne!(h.app.editor.scene, scene);
                    h.app.history(false);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert!(!h.app.editor.document.can_undo());
                    h.app.history(true);
                    assert_eq!(h.app.editor.document.model(), &after);
                }
            }
        }
    }
}

#[test]
fn junction_drag_both_dpis_reversed_axes_preview_identity_and_atomic_history() {
    for (size, scale) in PROFILES {
        for reverse_a in [false, true] {
            for reverse_b in [false, true] {
                let (mut h, anchors) = fixture(size, scale, reverse_a, reverse_b);
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                let camera = h.app.plans.cameras[&h.view];
                h.press(h.point(Point2::new(0.0, 0.0)));
                assert!(h.app.plans.junction_drag.is_some(), "{}", h.app.status);
                assert!(h.app.wall_gesture.is_none());
                let destination = h.point(Point2::new(0.3, 0.2));
                h.frame(vec![egui::Event::PointerMoved(destination)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.revision(), 0);
                assert!(!h.app.editor.document.can_undo());
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                let preview_lines = h.output.shapes.iter().filter(|s| matches!(&s.shape,
                    egui::Shape::LineSegment { stroke, .. } if stroke.width == 2.0 && stroke.color == theme::ACCENT)).count();
                assert!(preview_lines >= 2);
                h.release(destination);
                assert!(!h.app.status_error, "{}", h.app.status);
                assert!(h.app.plans.junction_drag.is_none());
                h.clean();
                let after = h.app.editor.document.model().clone();
                let node = anchors[0]
                    .endpoint
                    .point(&after.walls[&anchors[0].wall].parameters);
                assert_eq!(
                    node,
                    anchors[1]
                        .endpoint
                        .point(&after.walls[&anchors[1].wall].parameters)
                );
                assert!((node.x - 0.3).abs() < 1e-6);
                assert_eq!(node.y, 0.0);
                for anchor in anchors {
                    let wall = &after.walls[&anchor.wall];
                    let old = &before.walls[&anchor.wall];
                    let mut expected = old.clone();
                    match anchor.endpoint {
                        WallEndpoint::Start => {
                            *expected.parameters.path.straight_start_mut().unwrap() = node
                        }
                        WallEndpoint::End => {
                            *expected.parameters.path.straight_end_mut().unwrap() = node
                        }
                    }
                    assert_eq!(wall, &expected);
                    assert_eq!(
                        anchor.endpoint.opposite().point(&wall.parameters),
                        anchor.endpoint.opposite().point(&old.parameters)
                    );
                }
                assert_eq!(after.wall_joins, before.wall_joins);
                assert_eq!(after.wall_type_assignments, before.wall_type_assignments);
                assert_eq!(h.app.editor.document.revision(), 1);
                assert_ne!(h.app.editor.scene, scene);
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &before);
                assert!(!h.app.editor.document.can_undo());
                h.app.history(true);
                assert_eq!(h.app.editor.document.model(), &after);
            }
        }
    }
}

fn add_opening(h: &mut Harness, host: Id) -> Id {
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Fixed world window".into(),
            host,
            offset: 0.4,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Window,
                width: 0.3,
                height: 1.0,
                sill: 1.0,
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
    let mut model = h.app.editor.document.model().clone();
    model.openings.insert(id, opening);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.settle();
    id
}

#[test]
fn junction_drag_preserves_opening_world_location_and_rejects_clearance() {
    for (size, scale) in PROFILES {
        for reverse in [false, true] {
            let (mut h, anchors) = fixture(size, scale, reverse, reverse);
            let host = if reverse {
                anchors[0].wall
            } else {
                anchors[1].wall
            };
            let opening = add_opening(&mut h, host);
            let before = h.app.editor.document.model().clone();
            let world = os_model::wall_point(
                &before.walls[&host].parameters,
                before.openings[&opening].parameters.offset,
                0.0,
            );
            h.press(h.point(Point2::new(0.0, 0.0)));
            let p = h.point(Point2::new(if reverse { -0.2 } else { 0.2 }, 0.15));
            h.frame(vec![egui::Event::PointerMoved(p)]);
            assert_eq!(h.app.editor.document.model(), &before);
            h.release(p);
            assert!(!h.app.status_error, "{}", h.app.status);
            let after = h.app.editor.document.model();
            let new = &after.openings[&opening];
            let new_world =
                os_model::wall_point(&after.walls[&host].parameters, new.parameters.offset, 0.0);
            assert!(new_world.distance(world) < 1e-12);
            let mut expected = before.openings[&opening].clone();
            expected.parameters.offset = new.parameters.offset;
            assert_eq!(*new, expected);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.settle();
            h.press(h.point(Point2::new(0.0, 0.0)));
            let p = h.point(Point2::new(if reverse { -0.5 } else { 0.5 }, 0.15));
            h.frame(vec![egui::Event::PointerMoved(p)]);
            h.release(p);
            assert!(h.app.status_error);
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
        }
    }
}

#[test]
fn junction_drag_cancel_stale_outside_and_short_wall_retain_pointer_until_release() {
    for (size, scale) in PROFILES {
        for kind in [
            "butt",
            "corner",
            "tee host",
            "tee branch",
            "butt chain",
            "corner chain",
            "tee host chain",
            "tee branch chain",
        ] {
            for reason in [
                "escape",
                "outside",
                "gone",
                "session",
                "revision",
                "settings",
                "view",
                "selection",
                "provider",
                "drawing",
                "short",
                "click",
            ] {
                let mut h = interaction_fixture(size, scale, kind);
                let before = h.app.editor.document.model().clone();
                let camera = h.app.plans.cameras[&h.view];
                let origin = h.point(Point2::new(0.0, 0.0));
                h.press(origin);
                assert!(h.app.plans.junction_drag.is_some());
                let mut p = h.point(Point2::new(0.25, 0.1));
                if reason != "click" {
                    h.frame(vec![egui::Event::PointerMoved(p)]);
                }
                match reason {
                    "escape" => h.frame(vec![escape()]),
                    "outside" => {
                        h.frame(vec![egui::Event::PointerMoved(
                            h.app.plans.canvas_rect.unwrap().right_bottom()
                                + egui::vec2(20.0, 20.0),
                        )]);
                    }
                    "gone" => h.frame(vec![egui::Event::PointerGone]),
                    "session" => {
                        h.app.editor.document = Document::from_model(before.clone()).unwrap()
                    }
                    "revision" => h
                        .app
                        .editor
                        .command("Rename", Command::RenameProject("Changed".into()))
                        .unwrap(),
                    "settings" => {
                        let mut settings = before.views[&h.view].parameters.plan.unwrap();
                        settings.scale_denominator = 50.0;
                        h.app
                            .editor
                            .update_floor_plan(h.view, "Changed", h.app.active_level, settings)
                            .unwrap();
                    }
                    "view" => h.app.focus_plan(None),
                    "selection" => h.app.select(None),
                    "provider" => {
                        h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                    }
                    "drawing" => {
                        h.app.plans.drawing = Some(
                            PlanDrawing::from_prisms(
                                h.app.plans.desired.unwrap(),
                                &BTreeMap::new(),
                                vec![],
                            )
                            .unwrap(),
                        );
                    }
                    "short" => {
                        p = h.point(Point2::new(
                            if kind.starts_with("corner") {
                                -0.9999
                            } else {
                                0.9999
                            },
                            0.0,
                        ))
                    }
                    "click" => p = origin,
                    _ => unreachable!(),
                }
                let expected = h.app.editor.document.model().clone();
                if !["click", "short"].contains(&reason) {
                    h.frame(vec![egui::Event::PointerMoved(p)]);
                    assert!(h.app.plans.junction_drag.is_none(), "{reason}");
                    assert!(h.app.plans.endpoint_pointer_claimed, "{reason}");
                    if let Some(current) = h.app.plans.cameras.get(&h.view) {
                        assert_eq!(*current, camera, "{reason}");
                    }
                }
                h.release(p);
                h.clean();
                assert!(h.app.plans.junction_drag.is_none());
                assert_eq!(h.app.editor.document.model(), &expected, "{reason}");
                assert_eq!(
                    h.app.editor.document.can_undo(),
                    reason == "revision" || reason == "settings",
                    "{reason}"
                );
            }
        }
    }
}

#[test]
fn junction_drag_hidden_cropped_hit_radius_and_pan_precedence() {
    for (size, scale) in PROFILES {
        for kind in [
            "butt",
            "corner",
            "tee host",
            "tee branch",
            "corner chain",
            "tee host chain",
        ] {
            for case in ["hidden", "cropped", "hit", "miss", "body"] {
                let mut h = interaction_fixture(size, scale, kind);
                if case == "hidden" || case == "cropped" {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    if case == "hidden" {
                        settings.visibility.walls = false;
                    } else {
                        settings.crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(0.2, -1.0),
                            max: Point2::new(2.0, 1.0),
                        });
                    }
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Visibility", h.app.active_level, settings)
                        .unwrap();
                    h.settle();
                }
                let camera = h.app.plans.cameras[&h.view];
                let p = if case == "body" {
                    h.point(Point2::new(-0.5, 0.0))
                } else {
                    h.point(Point2::new(0.0, 0.0))
                        + egui::vec2(
                            0.0,
                            if case == "hit" {
                                10.0
                            } else if case == "miss" {
                                10.1
                            } else {
                                0.0
                            },
                        )
                };
                h.press(p);
                assert_eq!(h.app.plans.junction_drag.is_some(), case == "hit", "{case}");
                assert!(h.app.wall_gesture.is_none());
                let end = p + egui::vec2(35.0, 25.0);
                h.frame(vec![egui::Event::PointerMoved(end)]);
                if case == "hit" {
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                }
                if case == "body" {
                    assert_ne!(h.app.plans.cameras[&h.view], camera);
                }
                h.frame(vec![escape()]);
                h.release(end);
            }
        }
    }
}

#[test]
fn junction_drag_rejects_competing_third_wall_atomically() {
    let (mut h, _) = fixture(PROFILES[0].0, 1.0, false, false);
    let mut model = h.app.editor.document.model().clone();
    let mut params = model.walls[&h.wall].parameters.clone();
    *params.path.straight_start_mut().unwrap() = Point2::new(0.5, 0.0);
    *params.path.straight_end_mut().unwrap() = Point2::new(0.5, 1.0);
    let wall = os_model::Wall::new(os_walls::WALL_TYPE, params);
    model.walls.insert(wall.id(), wall);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.settle();
    let before = h.app.editor.document.model().clone();
    // Snap the node exactly onto the third wall's endpoint to test model validation.
    h.app.plans.snaps = SnapOptions {
        enabled: true,
        endpoints: true,
        midpoints: false,
        nearest: false,
        perpendicular: false,
        intersections: false,
        axis_extensions: false,
    };
    h.press(h.point(Point2::new(0.0, 0.0)));
    let p = h.point(Point2::new(0.5, 0.0));
    h.frame(vec![egui::Event::PointerMoved(p)]);
    h.release(p);
    assert!(h.app.status_error, "{}", h.app.status);
    assert_eq!(h.app.editor.document.model(), &before);
    assert!(!h.app.editor.document.can_undo());
}

#[test]
fn junction_drag_chain_acquires_and_far_endpoint_never_acquires_single_wall_resize() {
    for kind in ["chain", "far endpoint"] {
        let (mut h, anchors) = fixture(PROFILES[0].0, 1.0, false, false);
        let mut model = h.app.editor.document.model().clone();
        if kind == "chain" {
            let mut params = model.walls[&h.wall].parameters.clone();
            *params.path.straight_start_mut().unwrap() = Point2::new(1.0, 0.0);
            *params.path.straight_end_mut().unwrap() = Point2::new(2.0, 0.0);
            let third = os_model::Wall::new(os_walls::WALL_TYPE, params);
            let join = WallJoin::new(
                "core.wall_join",
                WallJoinParams::Butt {
                    a: WallAnchor {
                        wall: anchors[1].wall,
                        endpoint: WallEndpoint::End,
                    },
                    b: WallAnchor {
                        wall: third.id(),
                        endpoint: WallEndpoint::Start,
                    },
                },
            );
            model.walls.insert(third.id(), third);
            model.wall_joins.insert(join.id(), join);
        }
        h.app.editor.document = Document::from_model(model).unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.settle();
        let before = h.app.editor.document.model().clone();
        let p = h.point(Point2::new(
            if kind == "far endpoint" { -1.0 } else { 0.0 },
            0.0,
        ));
        h.press(p);
        assert_eq!(
            h.app.plans.junction_drag.is_some(),
            kind == "chain",
            "{kind}"
        );
        assert!(h.app.wall_gesture.is_none(), "{kind}");
        let end = p + egui::vec2(30.0, 20.0);
        h.frame(vec![egui::Event::PointerMoved(end)]);
        assert_eq!(h.app.editor.document.model(), &before);
        h.release(end);
        if kind == "chain" {
            assert!(!h.app.status_error, "{}", h.app.status);
            assert_ne!(h.app.editor.document.model(), &before);
            h.app.history(false);
        } else {
            assert!(h.app.status_error, "{kind}");
        }
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(!h.app.editor.plugin_work_pending());
    }
}

#[test]
fn junction_drag_snap_excludes_both_walls_and_stays_on_axis() {
    for (size, scale) in PROFILES {
        for kind in ["butt", "corner", "tee host", "tee branch"] {
            for case in ["self", "shared node", "grid", "off axis"] {
                let mut h = interaction_fixture(size, scale, kind);
                if ["grid", "off axis"].contains(&case) {
                    let grid = os_model::Grid::new(
                        "core.grid",
                        os_model::GridParams {
                            name: "A".into(),
                            building: h.app.editor.document.model().levels[&h.app.active_level]
                                .parameters
                                .building,
                            start: Point2::new(0.25, if case == "grid" { 0.0 } else { 0.02 }),
                            end: Point2::new(0.25, 1.0),
                        },
                    );
                    h.app
                        .editor
                        .command("Grid", Command::AddGrid(grid))
                        .unwrap();
                    h.settle();
                }
                h.app.plans.snaps = SnapOptions {
                    enabled: true,
                    endpoints: true,
                    midpoints: true,
                    nearest: false,
                    perpendicular: false,
                    intersections: false,
                    axis_extensions: false,
                };
                h.press(h.point(Point2::new(0.0, 0.0)));
                let x = match case {
                    "self" => -0.49,
                    "shared node" => 0.06,
                    _ => 0.24,
                };
                let p = h.point(Point2::new(x, 0.2));
                h.frame(vec![egui::Event::PointerMoved(p)]);
                h.release(p);
                assert!(!h.app.status_error, "{}", h.app.status);
                let model = h.app.editor.document.model();
                let anchor = model
                    .wall_joins
                    .values()
                    .next()
                    .unwrap()
                    .parameters
                    .anchors()[0];
                let node = anchor.endpoint.point(&model.walls[&anchor.wall].parameters);
                assert!(
                    (node.x - if case == "grid" { 0.25 } else { x }).abs() < 1e-6,
                    "{case}: {node:?}"
                );
                assert_eq!(node.y, 0.0);
            }
        }
    }
}

#[test]
fn junction_perpendicular_invalid_clearance_third_wall_and_connected_graphs_are_atomic() {
    for (size, scale) in PROFILES {
        for kind in ["corner", "tee host", "tee branch"] {
            for case in ["clearance", "third", "chain", "far endpoint"] {
                let mut h = interaction_fixture(size, scale, kind);
                let mut model = h.app.editor.document.model().clone();
                let parameters = model.wall_joins.values().next().unwrap().parameters.clone();
                if case == "clearance" {
                    add_opening(&mut h, parameters.members()[0]);
                } else if case == "third" || case == "chain" {
                    let branch = *parameters.anchors().last().unwrap();
                    let mut params = model.walls[&branch.wall].parameters.clone();
                    *params.path.straight_start_mut().unwrap() = Point2::new(
                        if case == "third" { 0.5 } else { 0.0 },
                        if case == "third" { 0.0 } else { 1.0 },
                    );
                    *params.path.straight_end_mut().unwrap() =
                        Point2::new(params.start().x, params.start().y + 1.0);
                    let wall = os_model::Wall::new(os_walls::WALL_TYPE, params);
                    if case == "chain" {
                        let join = WallJoin::new(
                            "core.wall_join",
                            WallJoinParams::Butt {
                                a: WallAnchor {
                                    wall: branch.wall,
                                    endpoint: branch.endpoint.opposite(),
                                },
                                b: WallAnchor {
                                    wall: wall.id(),
                                    endpoint: WallEndpoint::Start,
                                },
                            },
                        );
                        model.wall_joins.insert(join.id(), join);
                    }
                    model.walls.insert(wall.id(), wall);
                    h.app.editor.document = Document::from_model(model).unwrap();
                    h.app.editor.regenerate().unwrap();
                    h.app.plans.poll(&h.app.editor);
                    h.app.focus_plan(Some(h.view));
                    h.settle();
                }
                let before = h.app.editor.document.model().clone();
                let origin = if case == "far endpoint" {
                    if kind == "tee branch" {
                        Point2::new(0.0, 1.0)
                    } else {
                        Point2::new(-1.0, 0.0)
                    }
                } else {
                    Point2::new(0.0, 0.0)
                };
                h.press(h.point(origin));
                assert_eq!(
                    h.app.plans.junction_drag.is_some(),
                    case != "far endpoint",
                    "{kind} {case}"
                );
                assert!(h.app.wall_gesture.is_none());
                let p = h.point(Point2::new(
                    if case == "clearance" { -0.2 } else { 0.5 },
                    0.1,
                ));
                h.frame(vec![egui::Event::PointerMoved(p)]);
                assert_eq!(h.app.editor.document.model(), &before);
                h.release(p);
                if case == "chain" {
                    assert!(!h.app.status_error, "{kind}: {}", h.app.status);
                    assert_ne!(h.app.editor.document.model(), &before);
                    h.app.history(false);
                } else {
                    assert!(h.app.status_error, "{kind} {case}: {}", h.app.status);
                }
                assert_eq!(h.app.editor.document.model(), &before);
                assert!(!h.app.editor.document.can_undo());
                assert!(!h.app.editor.plugin_work_pending());
            }
        }
    }
}

#[test]
fn junction_tee_endpoints_win_overlap_and_missing_peer_is_unavailable() {
    for (size, scale) in PROFILES {
        let (mut h, _, join) =
            perpendicular_fixture(size, scale, "tee", false, false, false, false);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let rect = h.app.plans.canvas_rect.unwrap();
        let mut camera = h.app.plans.cameras[&h.view];
        // Shrink the plan until the endpoint and interior station hit circles overlap.
        camera.pixels_per_metre = 8.0;
        h.app.plans.cameras.insert(h.view, camera);
        h.frame(vec![]);
        let origin = h.point(Point2::new(0.0, 0.0));
        let stations = junctions::station_handles(
            h.app.editor.document.model(),
            h.wall,
            context,
            camera,
            rect,
        );
        assert_eq!(
            junctions::hit_station(&stations, origin),
            Some(junctions::Grip::TeeStation(join))
        );
        h.press(origin);
        assert!(h.app.plans.junction_drag.is_none());
        assert!(h.app.wall_gesture.is_none());
        assert!(h.app.status_error);
        h.release(origin);

        let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap();
        assert!(
            junctions::visible_availability(
                &h.app.editor,
                &drawing,
                context,
                h.wall,
                junctions::Grip::TeeStation(join)
            )
            .is_err()
        );
        assert!(
            junctions::JunctionDrag::begin(
                &h.app.editor,
                &drawing,
                context,
                h.wall,
                junctions::Grip::TeeStation(join),
                origin
            )
            .is_err()
        );
        assert!(!h.app.editor.plugin_work_pending());
    }
}

#[test]
#[cfg(feature = "external-plugins")]
#[ignore = "requires explicitly installed independent Wall guest via OPENSTRUCTURE_WALL_TEST_PLUGIN"]
fn junction_drag_installed_provider_never_submits_partial_job() {
    let directory =
        std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN").expect("installed Wall directory");
    for (size, scale) in PROFILES {
        for kind in [
            "butt",
            "corner",
            "tee host",
            "tee branch",
            "corner chain",
            "tee host chain",
        ] {
            let mut h = interaction_fixture(size, scale, kind);
            let selected = h.app.selected.unwrap();
            let grip = fixture_grip(&h);
            h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
            h.app
                .editor
                .host
                .load_wasm_directory(
                    Path::new(&directory),
                    [
                        Permission::ModelRead,
                        Permission::ModelWrite,
                        Permission::UiTool,
                    ]
                    .into(),
                )
                .unwrap();
            h.settle();
            let before = h.app.editor.document.model().clone();
            assert!(
                junctions::availability(&h.app.editor, selected, grip)
                    .unwrap_err()
                    .to_string()
                    .contains("installed Wall provider")
            );
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            assert!(
                junctions::JunctionDrag::begin(
                    &h.app.editor,
                    h.app.plans.drawing.as_ref().unwrap(),
                    context,
                    selected,
                    grip,
                    h.point(Point2::new(0.0, 0.0))
                )
                .is_err()
            );
            assert!(!h.app.editor.plugin_work_pending());
            assert!(h.app.wall_gesture.is_none());
            assert_eq!(h.app.editor.document.model(), &before);
        }
    }
}

fn fixture_grip(h: &Harness) -> junctions::Grip {
    let selected = h.app.selected.unwrap();
    let join = h
        .app
        .editor
        .document
        .model()
        .wall_joins
        .values()
        .find(|join| join.parameters.members().contains(&selected))
        .unwrap();
    if matches!(join.parameters, WallJoinParams::Tee { host, .. } if host == selected) {
        junctions::Grip::TeeStation(join.id())
    } else {
        let anchor = join
            .parameters
            .anchors()
            .into_iter()
            .find(|a| a.wall == selected)
            .unwrap();
        junctions::Grip::Endpoint(match anchor.endpoint {
            WallEndpoint::Start => WallEdit::ResizeStart,
            WallEndpoint::End => WallEdit::ResizeEnd,
        })
    }
}

fn graph_wall(
    model: &mut os_model::Model,
    template: Id,
    start: (f64, f64),
    end: (f64, f64),
    reverse: bool,
) -> [WallAnchor; 2] {
    let mut parameters = model.walls[&template].parameters.clone();
    *parameters.path.straight_start_mut().unwrap() = Point2::new(start.0, start.1);
    *parameters.path.straight_end_mut().unwrap() = Point2::new(end.0, end.1);
    if reverse {
        parameters.path = os_model::WallPath::Straight {
            start: parameters.end(),
            end: parameters.start(),
        };
    }
    let wall = os_model::Wall::new(os_walls::WALL_TYPE, parameters);
    let id = wall.id();
    model.walls.insert(id, wall);
    [
        WallAnchor {
            wall: id,
            endpoint: if reverse {
                WallEndpoint::End
            } else {
                WallEndpoint::Start
            },
        },
        WallAnchor {
            wall: id,
            endpoint: if reverse {
                WallEndpoint::Start
            } else {
                WallEndpoint::End
            },
        },
    ]
}

fn graph_join(model: &mut os_model::Model, parameters: WallJoinParams) -> Id {
    let join = WallJoin::new("core.wall_join", parameters);
    let id = join.id();
    model.wall_joins.insert(id, join);
    id
}

fn install_graph(h: &mut Harness, model: os_model::Model, selected: Id) {
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(selected));
    h.app.plans.snaps.enabled = false;
    h.settle();
}

fn graph_drag(h: &mut Harness, origin: Point2, target: Point2, valid: bool) -> os_model::Model {
    let before = h.app.editor.document.model().clone();
    let scene = h.app.editor.scene.clone();
    let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
    let camera = h.app.plans.cameras[&h.view];
    h.press(h.point(origin));
    assert!(h.app.plans.junction_drag.is_some(), "{}", h.app.status);
    let pointer = h.point(target);
    h.frame(vec![egui::Event::PointerMoved(pointer)]);
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
    assert_eq!(h.app.plans.cameras[&h.view], camera);
    assert!(!h.app.editor.document.can_undo());
    h.release(pointer);
    assert_eq!(!h.app.status_error, valid, "{}", h.app.status);
    let after = h.app.editor.document.model().clone();
    if valid {
        assert_ne!(after, before);
        for join in after.wall_joins.values() {
            join.parameters.validate(&after).unwrap();
        }
        let mut metadata = after.clone();
        for (id, wall) in &mut metadata.walls {
            *wall.parameters.path.straight_start_mut().unwrap() =
                before.walls[id].parameters.start();
            *wall.parameters.path.straight_end_mut().unwrap() = before.walls[id].parameters.end();
        }
        for (id, opening) in &mut metadata.openings {
            opening.parameters.offset = before.openings[id].parameters.offset;
        }
        for (id, join) in &mut metadata.wall_joins {
            join.parameters = before.wall_joins[id].parameters.clone();
        }
        assert_eq!(
            metadata, before,
            "Only geometry, stations and opening offsets may change"
        );
        assert_eq!(h.app.editor.document.revision(), 1);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(!h.app.editor.document.can_undo());
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &after);
    } else {
        assert_eq!(after, before);
        assert!(!h.app.editor.document.can_undo());
        assert_eq!(h.app.editor.document.revision(), 0);
    }
    after
}

#[test]
fn junction_graph_butt_chain_start_opening_and_hosted_tee_station() {
    for (size, scale) in PROFILES {
        let (mut h, anchors) = fixture(size, scale, true, false);
        let mut model = h.app.editor.document.model().clone();
        let third = graph_wall(&mut model, h.wall, (-1.0, 0.0), (-2.0, 0.0), false);
        graph_join(
            &mut model,
            WallJoinParams::Butt {
                a: WallAnchor {
                    wall: h.wall,
                    endpoint: WallEndpoint::End,
                },
                b: third[0],
            },
        );
        // Host the Tee on the other resized-start member, clear of its opening.
        model
            .walls
            .get_mut(&anchors[1].wall)
            .unwrap()
            .parameters
            .path
            .straight_end_mut()
            .unwrap()
            .x = 2.0;
        let branch = graph_wall(&mut model, h.wall, (1.3, 0.0), (1.3, 1.0), true);
        let tee = graph_join(
            &mut model,
            WallJoinParams::Tee {
                host: anchors[1].wall,
                station: 1.3,
                branch: branch[0],
            },
        );
        let selected = h.wall;
        install_graph(&mut h, model, selected);
        let opening = add_opening(&mut h, anchors[1].wall);
        let before = h.app.editor.document.model().clone();
        let after = graph_drag(&mut h, Point2::new(0.0, 0.0), Point2::new(0.1, 0.0), true);
        assert!(world_opening(&after, opening).distance(world_opening(&before, opening)) < 1e-12);
        assert_eq!(after.walls[&third[0].wall], before.walls[&third[0].wall]);
        assert_eq!(after.walls[&branch[0].wall], before.walls[&branch[0].wall]);
        let WallJoinParams::Tee { station, .. } = after.wall_joins[&tee].parameters else {
            panic!()
        };
        assert!((station - 1.2).abs() < 1e-6);
    }
}

#[test]
fn junction_graph_four_corner_loop_and_invalid_cycle_are_atomic() {
    for (size, scale) in PROFILES {
        for reverse in [false, true] {
            for owner_peer in [false, true] {
                for valid in [true, false] {
                    let (mut h, anchors, _) = perpendicular_fixture(
                        size, scale, "corner", reverse, !reverse, false, owner_peer,
                    );
                    let mut model = h.app.editor.document.model().clone();
                    let top = graph_wall(&mut model, h.wall, (0.0, 1.0), (-1.0, 1.0), reverse);
                    let left = graph_wall(&mut model, h.wall, (-1.0, 1.0), (-1.0, 0.0), !reverse);
                    for (a, b) in [
                        (
                            WallAnchor {
                                endpoint: anchors[1].endpoint.opposite(),
                                ..anchors[1]
                            },
                            top[0],
                        ),
                        (top[1], left[0]),
                        (
                            left[1],
                            WallAnchor {
                                endpoint: anchors[0].endpoint.opposite(),
                                ..anchors[0]
                            },
                        ),
                    ] {
                        graph_join(
                            &mut model,
                            WallJoinParams::Corner {
                                a,
                                b,
                                owner: if owner_peer { b.wall } else { a.wall },
                            },
                        );
                    }
                    let outside = graph_wall(&mut model, h.wall, (2.0, -1.0), (3.0, -1.0), false);
                    let selected = h.wall;
                    install_graph(&mut h, model, selected);
                    let opening = add_opening(&mut h, anchors[1].wall);
                    let before = h.app.editor.document.model().clone();
                    let after = graph_drag(
                        &mut h,
                        Point2::new(0.0, 0.0),
                        Point2::new(if valid { 0.25 } else { -1.2 }, 0.0),
                        valid,
                    );
                    assert_eq!(
                        after.walls[&outside[0].wall],
                        before.walls[&outside[0].wall]
                    );
                    if valid {
                        assert_eq!(after.walls[&left[0].wall], before.walls[&left[0].wall]);
                        assert!(
                            (top[0]
                                .endpoint
                                .point(&after.walls[&top[0].wall].parameters)
                                .x
                                - 0.25)
                                .abs()
                                < 1e-6
                        );
                        assert_eq!(after.openings[&opening], before.openings[&opening]);
                        assert!(
                            (world_opening(&after, opening).x
                                - world_opening(&before, opening).x
                                - 0.25)
                                .abs()
                                < 1e-6
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn junction_graph_tee_path_reconnects_to_fixed_host() {
    for (size, scale) in PROFILES {
        for reverse in [false, true] {
            for select_branch in [false, true] {
                let (mut h, anchors, _) = perpendicular_fixture(
                    size,
                    scale,
                    "tee",
                    reverse,
                    !reverse,
                    select_branch,
                    false,
                );
                let mut model = h.app.editor.document.model().clone();
                let host = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
                if reverse {
                    host.path.straight_start_mut().unwrap().x = 2.0;
                } else {
                    host.path.straight_end_mut().unwrap().x = 2.0;
                }
                // Changing reversed host start also changes the original station.
                for join in model.wall_joins.values_mut() {
                    if let WallJoinParams::Tee { station, .. } = &mut join.parameters {
                        *station = if reverse { 2.0 } else { 1.0 };
                    }
                }
                let top = graph_wall(&mut model, h.wall, (0.0, 1.0), (1.0, 1.0), reverse);
                let right = graph_wall(&mut model, h.wall, (1.0, 1.0), (1.0, 0.0), !reverse);
                let a = WallAnchor {
                    endpoint: anchors[1].endpoint.opposite(),
                    ..anchors[1]
                };
                graph_join(
                    &mut model,
                    WallJoinParams::Corner {
                        a,
                        b: top[0],
                        owner: a.wall,
                    },
                );
                graph_join(
                    &mut model,
                    WallJoinParams::Corner {
                        a: top[1],
                        b: right[0],
                        owner: right[0].wall,
                    },
                );
                graph_join(
                    &mut model,
                    WallJoinParams::Tee {
                        host: h.wall,
                        station: if reverse { 1.0 } else { 2.0 },
                        branch: right[1],
                    },
                );
                let selected = anchors[usize::from(select_branch)].wall;
                install_graph(&mut h, model, selected);
                let before = h.app.editor.document.model().clone();
                let after = graph_drag(&mut h, Point2::new(0.0, 0.0), Point2::new(0.2, 0.0), true);
                assert_eq!(after.walls[&h.wall], before.walls[&h.wall]);
                assert_eq!(after.walls[&right[0].wall], before.walls[&right[0].wall]);
                assert!(
                    (top[0]
                        .endpoint
                        .point(&after.walls[&top[0].wall].parameters)
                        .x
                        - 0.2)
                        .abs()
                        < 1e-6
                );
            }
        }
    }
}

#[test]
fn junction_graph_snap_excludes_downstream_walls_and_ranks_only_legal_points() {
    for (size, scale) in PROFILES {
        for with_grids in [false, true] {
            let mut h = interaction_fixture(size, scale, "butt chain");
            let mut model = h.app.editor.document.model().clone();
            if with_grids {
                for (name, x, y) in [("off axis", 0.98, 0.01), ("legal", 0.94, 0.0)] {
                    let grid = os_model::Grid::new(
                        "core.grid",
                        os_model::GridParams {
                            name: name.into(),
                            building: model.levels[&h.app.active_level].parameters.building,
                            start: Point2::new(x, y),
                            end: Point2::new(x, 1.0),
                        },
                    );
                    model.grids.insert(grid.id(), grid);
                }
            }
            let selected = h.wall;
            install_graph(&mut h, model, selected);
            h.app.plans.snaps = SnapOptions {
                enabled: true,
                endpoints: true,
                midpoints: false,
                nearest: false,
                perpendicular: false,
                intersections: false,
                axis_extensions: false,
            };
            let after = graph_drag(&mut h, Point2::new(0.0, 0.0), Point2::new(0.98, 0.0), true);
            let node = after.walls[&h.wall].parameters.end();
            assert!(
                (node.x - if with_grids { 0.94 } else { 0.98 }).abs() < 1e-6,
                "{node:?}"
            );
        }
    }
}

#[test]
fn junction_missing_provider_never_acquires_or_submits_work() {
    for (size, scale) in PROFILES {
        for kind in [
            "butt",
            "corner",
            "tee host",
            "tee branch",
            "corner chain",
            "tee host chain",
        ] {
            let mut h = interaction_fixture(size, scale, kind);
            let grip = fixture_grip(&h);
            let selected = h.app.selected.unwrap();
            let before = h.app.editor.document.model().clone();
            h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            assert!(junctions::availability(&h.app.editor, selected, grip).is_err());
            assert!(
                junctions::JunctionDrag::begin(
                    &h.app.editor,
                    h.app.plans.drawing.as_ref().unwrap(),
                    context,
                    selected,
                    grip,
                    h.point(Point2::new(0.0, 0.0))
                )
                .is_err()
            );
            assert!(!h.app.editor.plugin_work_pending());
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
        }
    }
}
