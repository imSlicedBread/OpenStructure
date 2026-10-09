use super::*;
use crate::section::{VerticalSectionPlane, vertical_section};
use os_model::{DoorLeaves, LiteSide, OpeningFamily, OpeningState, SideLite, WallPath};

fn fixture() -> (ResolvedOpening, WallParams) {
    let id = os_core::Id::new();
    (
        ResolvedOpening {
            name: "Section filling".into(),
            host: id,
            offset: 1.,
            kind: OpeningKind::Window,
            width: 2.,
            height: 1.5,
            sill: 0.8,
            pane_position: WindowPanePosition::Center,
            window_operation: WindowOperation::Fixed,
            type_id: None,
            type_name: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
            open_state: OpeningState::Default,
            family: OpeningFamily {
                frame_width: 0.06,
                panel_material: Some(os_core::Id::new()),
                frame_material: Some(os_core::Id::new()),
                ..Default::default()
            },
        },
        WallParams {
            name: "Host".into(),
            level: id,
            height: 4.,
            thickness: 0.2,
            material: None,
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
        },
    )
}

#[test]
fn authored_extrusions_closed_volume_material_pose_reversal_and_all_projection_inputs() {
    use os_model::{OpeningAttachment, OpeningExtrusion};
    let (mut p, mut wall) = fixture();
    p.family.frame_width = 0.;
    p.width = 1.;
    p.height = 2.;
    p.sill = 0.;
    let mut solid = OpeningExtrusion::new("Handle");
    solid.material = Some(os_core::Id::new());
    p.family.extrusions.push(solid.clone());
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        p.kind = kind;
        for reverse in [false, true] {
            wall.path = if reverse {
                WallPath::Straight {
                    start: Point2::new(8., 0.),
                    end: Point2::new(0., 0.),
                }
            } else {
                WallPath::Straight {
                    start: Point2::new(0., 0.),
                    end: Point2::new(8., 0.),
                }
            };
            for hinge in [DoorHinge::Start, DoorHinge::End] {
                p.hinge = hinge;
                for swing in [DoorSwing::Left, DoorSwing::Right] {
                    p.swing = swing;
                    for angle in [0., 37., 90.] {
                        p.open_state = if kind == OpeningKind::Door {
                            OpeningState::DoorAngle(angle)
                        } else {
                            OpeningState::Default
                        };
                        let meshes = authored_solids(&p, &wall, 0.).unwrap();
                        let mesh = &meshes[0];
                        assert!((mesh.signed_volume() - 0.15 * 0.03 * 2. * 0.02).abs() < 1e-12);
                        assert!(
                            mesh.surfaces
                                .iter()
                                .all(|s| s.material == solid.material && s.layer == Some(solid.id))
                        );
                        let center = component_point(&p, &wall, 0.825);
                        let a = component_point(&p, &wall, 0.);
                        let b = component_point(&p, &wall, 1.);
                        let expected = world(
                            &wall,
                            center.x - (b.y - a.y) * 0.03,
                            center.y + (b.x - a.x) * 0.03,
                        );
                        let average = mesh.vertices.iter().fold(Point2::new(0., 0.), |sum, v| {
                            Point2::new(sum.x + v.x / 8., sum.y + v.y / 8.)
                        });
                        assert!(average.distance(expected) < 1e-10);
                        assert!(
                            mesh.vertices
                                .iter()
                                .all(|v| v.z >= 0.9 - 1e-10 && v.z <= 0.96 + 1e-10)
                        );
                        check(&p, &wall);
                        let edges = crate::opening_extrusions::plan_edges(
                            &p,
                            &wall,
                            0.,
                            crate::plan::PlanRange {
                                cut: 1.2,
                                top: 3.,
                                bottom: 0.,
                                depth: -1.,
                            },
                        )
                        .unwrap();
                        assert_eq!(edges.len(), 4);
                        assert!(edges.iter().all(|e| e.3.material == solid.material));
                        let cut = crate::opening_extrusions::plan_edges(
                            &p,
                            &wall,
                            0.,
                            crate::plan::PlanRange {
                                cut: 0.93,
                                top: 3.,
                                bottom: 0.,
                                depth: -1.,
                            },
                        )
                        .unwrap();
                        assert_eq!(cut.len(), 4);
                        assert!(cut.iter().all(|e| e.2 == crate::plan::PlanRole::Cut));
                    }
                }
            }
        }
    }
    p.kind = OpeningKind::Door;
    p.family.extrusions[0].attachment = OpeningAttachment::OpeningFixed;
    p.open_state = OpeningState::DoorAngle(0.);
    let fixed = authored_solids(&p, &wall, 0.).unwrap();
    p.open_state = OpeningState::DoorAngle(90.);
    assert_eq!(fixed, authored_solids(&p, &wall, 0.).unwrap());
    p.kind = OpeningKind::Window;
    p.open_state = OpeningState::Default;
    p.window_operation = WindowOperation::Sliding;
    assert!(component_solids(&p, &wall, 0.).is_err());
    p.window_operation = WindowOperation::Fixed;
    wall.path = WallPath::CircularArc {
        center: Point2::new(0., 0.),
        radius: 10.,
        start_angle_rad: 0.,
        signed_sweep_rad: 1.,
    };
    assert!(component_solids(&p, &wall, 0.).is_err());
}

#[test]
fn authored_extrusions_primary_attachment_uses_parent_depth_for_narrow_lite_and_overridden_sizes() {
    let (mut p, mut wall) = fixture();
    wall.thickness = 1.;
    p.family.depth = 0.2;
    p.family.frame_width = 0.;
    p.family.side_lite = Some(SideLite {
        side: LiteSide::Start,
        width_fraction: 0.1,
        mullion_width: 0.01,
        material: None,
    });
    let mut solid = os_model::OpeningExtrusion::new("Pane attachment");
    solid.normal_offset = 0.;
    p.family.extrusions.push(solid);
    for width in [1., 1.4, 0.8] {
        p.width = width;
        for position in [WindowPanePosition::LeftFace, WindowPanePosition::RightFace] {
            p.pane_position = position;
            let mesh = authored_solids(&p, &wall, 0.).unwrap().remove(0);
            let actual = mesh
                .vertices
                .iter()
                .map(|v| v.y / mesh.vertices.len() as f64)
                .sum::<f64>();
            let expected = (1. - width * 0.1 * 0.2) / 2.
                * if position == WindowPanePosition::LeftFace {
                    1.
                } else {
                    -1.
                };
            assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
            let mut without = p.clone();
            without.family.extrusions.clear();
            let primary = component_solids(&without, &wall, 0.).unwrap().remove(0);
            let center = primary
                .vertices
                .iter()
                .map(|v| v.y / primary.vertices.len() as f64)
                .sum::<f64>();
            assert!((actual - center).abs() < 1e-10);
        }
    }
}

#[test]
fn authored_extrusions_aggregate_projection_budget_fails_without_partial_geometry() {
    let (mut p, wall) = fixture();
    for i in 0..os_model::OpeningFamily::MAX_EXTRUSIONS {
        let mut s = os_model::OpeningExtrusion::new(format!("Bounded {i}"));
        s.profile = (0..32)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 32.;
                Point2::new(0.5 + 0.4 * a.cos(), 0.5 + 0.4 * a.sin())
            })
            .collect();
        p.family.extrusions.push(s);
    }
    let solids = authored_solids(&p, &wall, 0.).unwrap();
    let meshes: Vec<_> = (0..3)
        .flat_map(|_| solids.iter().cloned().map(|m| (os_core::Id::new(), m)))
        .collect();
    assert!(
        meshes.iter().map(|(_, m)| m.triangles.len()).sum::<usize>()
            > crate::elevation::MAX_ELEVATION_TRIANGLES
    );
    let volume = crate::elevation::FacadeVolume {
        origin: Point2::new(0., 5.),
        axis: Point2::new(1., 0.),
        left_side: true,
        near: 0.,
        far: 10.,
        bounds: crate::elevation::ElevationBounds {
            min_station: 0.,
            max_station: 8.,
            bottom_elevation: -1.,
            top_elevation: 5.,
        },
    };
    assert!(crate::elevation::project_facade(&meshes, volume).is_err());
}

// Check actual world-space solids, their closed topology, positive volume,
// elevation bounds and triangle/material identity with the 3D assembly.
fn check(p: &ResolvedOpening, wall: &WallParams) {
    let solids = component_solids(p, wall, 2.3).unwrap();
    assert_eq!(solids, component_solids(p, wall, 2.3).unwrap());
    assert!(solids.len() <= MAX_OPENING_COMPONENT_SOLIDS);
    let mut assembled = Mesh::default();
    for solid in &solids {
        assert!(solid.signed_volume() > 0.);
        assert!(
            solid
                .vertices
                .iter()
                .all(|v| v.z >= 2.3 + p.sill - 1e-9 && v.z <= 2.3 + p.sill + p.height + 1e-9)
        );
        let center = solid.vertices.iter().fold(Point2::new(0., 0.), |sum, v| {
            Point2::new(
                sum.x + v.x / solid.vertices.len() as f64,
                sum.y + v.y / solid.vertices.len() as f64,
            )
        });
        // Transverse, longitudinal and oblique through each component's interior.
        // Arbitrary stations avoid triangulation-edge coincidences.
        for direction in [
            wall.path.tangent(p.offset),
            Point2::new(0.313, 1.),
            Point2::new(1., 0.271),
        ] {
            let contours = vertical_section(
                solid,
                VerticalSectionPlane {
                    origin: center,
                    direction,
                },
            )
            .unwrap();
            assert!(!contours.is_empty());
        }
        append_mesh(&mut assembled, solid.clone()).unwrap();
    }
    assert_eq!(assembled, component_mesh(p, wall, 2.3).unwrap());
    assert!(
        (assembled.signed_volume() - solids.iter().map(Mesh::signed_volume).sum::<f64>()).abs()
            < 1e-9
    );
}

#[test]
fn section_solids_cover_straight_operations_poses_pairs_frames_lites_and_materials() {
    for reversed in [false, true] {
        let (mut p, mut wall) = fixture();
        let (a, b) = (Point2::new(2., 3.), Point2::new(8.4, 7.8));
        wall.path = WallPath::Straight {
            start: if reversed { b } else { a },
            end: if reversed { a } else { b },
        };
        for lite in [false, true] {
            p.family.side_lite = lite.then(|| SideLite {
                side: LiteSide::End,
                width_fraction: 0.2,
                mullion_width: 0.05,
                material: Some(os_core::Id::new()),
            });
            for (operation, state) in [
                (WindowOperation::Fixed, OpeningState::Default),
                (WindowOperation::Sliding, OpeningState::SlidingFraction(0.)),
                (WindowOperation::Sliding, OpeningState::SlidingFraction(0.6)),
                (WindowOperation::Sliding, OpeningState::SlidingFraction(1.)),
                (WindowOperation::Casement, OpeningState::CasementAngle(0.)),
                (WindowOperation::Casement, OpeningState::CasementAngle(37.)),
                (WindowOperation::Casement, OpeningState::CasementAngle(90.)),
            ] {
                p.kind = OpeningKind::Window;
                p.sill = 0.8;
                p.family.door_leaves = DoorLeaves::Single;
                p.window_operation = operation;
                p.open_state = state;
                check(&p, &wall);
                let solids = component_solids(&p, &wall, 0.).unwrap();
                let materials: std::collections::BTreeSet<_> =
                    solids.iter().map(|s| s.surfaces[0].material).collect();
                assert!(materials.contains(&p.family.panel_material));
                assert!(materials.contains(&p.family.frame_material));
                if lite {
                    assert!(materials.contains(&p.family.side_lite.as_ref().unwrap().material));
                }
                if lite && operation == WindowOperation::Sliding {
                    assert_eq!(solids.len(), 16); // legacy sash + lite + frame composition
                    assert!(solids.len() <= MAX_OPENING_COMPONENT_SOLIDS);
                }
            }
            p.kind = OpeningKind::Door;
            p.sill = 0.;
            p.window_operation = WindowOperation::Fixed;
            for hinge in [DoorHinge::Start, DoorHinge::End] {
                p.hinge = hinge;
                for swing in [DoorSwing::Left, DoorSwing::Right] {
                    p.swing = swing;
                    p.family.door_leaves = DoorLeaves::Single;
                    for angle in [0., 31., 90.] {
                        p.open_state = OpeningState::DoorAngle(angle);
                        check(&p, &wall);
                    }
                    p.family.door_leaves = DoorLeaves::Paired {
                        active_fraction: 0.4,
                    };
                    p.open_state = OpeningState::DoorPairAngles {
                        active_degrees: 23.,
                        inactive_degrees: 67.,
                    };
                    check(&p, &wall);
                }
            }
        }
    }
}

#[test]
fn section_solids_cover_both_arc_directions_and_supported_default_poses() {
    for sweep in [-1.2, 1.2] {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let (mut p, mut wall) = fixture();
            wall.path = WallPath::CircularArc {
                center: Point2::new(3., -2.),
                radius: 8.,
                start_angle_rad: 0.3,
                signed_sweep_rad: sweep,
            };
            p.kind = kind;
            if kind == OpeningKind::Door {
                p.sill = 0.;
            }
            for lite in [false, true] {
                p.family.side_lite = lite.then_some(SideLite {
                    side: LiteSide::Start,
                    width_fraction: 0.2,
                    mullion_width: 0.05,
                    material: p.family.panel_material,
                });
                check(&p, &wall);
            }
        }
    }
}

#[test]
fn section_solids_keep_touching_frames_separate_and_validate_failures() {
    let (p, wall) = fixture();
    let plane = VerticalSectionPlane {
        origin: Point2::new(0., 0.),
        direction: Point2::new(1., 0.),
    };
    let solids = component_solids(&p, &wall, 0.).unwrap();
    assert_eq!(solids.len(), 5);
    assert!(vertical_section(&component_mesh(&p, &wall, 0.).unwrap(), plane).is_err());
    for solid in solids {
        assert_eq!(vertical_section(&solid, plane).unwrap().len(), 1);
        assert!(
            vertical_section(
                &solid,
                VerticalSectionPlane {
                    origin: Point2::new(0., 100.),
                    ..plane
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    assert!(component_solids(&p, &wall, f64::NAN).is_err());
    let mut invalid = p;
    invalid.family.frame_width = invalid.width;
    assert!(component_solids(&invalid, &wall, 0.).is_err());
}

#[test]
fn section_solids_match_analytic_oblique_world_coordinates_and_reversed_equivalent() {
    // An eight-metre 3:4:5 host at (10,-4), elevated 2.3 m. These expectations
    // use its explicit orthonormal basis, never the production path/pose helpers.
    let at = |s: f64, n: f64, z: f64| {
        crate::Vec3::new(10. + 0.8 * s - 0.6 * n, -4. + 0.6 * s + 0.8 * n, z)
    };
    for kind in [OpeningKind::Window, OpeningKind::Door] {
        for reversed in [false, true] {
            let (mut p, mut wall) = fixture();
            p.kind = kind;
            p.offset = if reversed { 5. } else { 1. };
            if kind == OpeningKind::Door && reversed {
                p.hinge = DoorHinge::End;
                p.swing = DoorSwing::Right;
            }
            if kind == OpeningKind::Window {
                p.pane_position = if reversed {
                    WindowPanePosition::RightFace
                } else {
                    WindowPanePosition::LeftFace
                };
            }
            wall.path = if reversed {
                WallPath::Straight {
                    start: Point2::new(16.4, 0.8),
                    end: Point2::new(10., -4.),
                }
            } else {
                WallPath::Straight {
                    start: Point2::new(10., -4.),
                    end: Point2::new(16.4, 0.8),
                }
            };
            // Each tuple is a complete box in the ORIGINAL host basis, with
            // material and expected transverse-section rectangle (if intersected).
            let boxes = if kind == OpeningKind::Window {
                vec![
                    (
                        1.06,
                        2.94,
                        0.075,
                        0.1,
                        3.16,
                        4.54,
                        p.family.panel_material,
                        true,
                    ),
                    (
                        1.,
                        1.06,
                        0.05,
                        0.1,
                        3.1,
                        4.6,
                        p.family.frame_material,
                        false,
                    ),
                    (
                        2.94,
                        3.,
                        0.05,
                        0.1,
                        3.1,
                        4.6,
                        p.family.frame_material,
                        false,
                    ),
                    (
                        1.06,
                        2.94,
                        0.05,
                        0.1,
                        4.54,
                        4.6,
                        p.family.frame_material,
                        true,
                    ),
                    (
                        1.06,
                        2.94,
                        0.05,
                        0.1,
                        3.1,
                        3.16,
                        p.family.frame_material,
                        true,
                    ),
                ]
            } else {
                p.family.frame_width = 0.;
                p.sill = 0.;
                p.open_state = OpeningState::DoorAngle(90.);
                vec![(
                    0.9875,
                    1.0125,
                    -0.1,
                    1.9,
                    2.3,
                    3.8,
                    p.family.panel_material,
                    true,
                )]
            };
            let station = if kind == OpeningKind::Window { 2. } else { 1. };
            let origin = at(station, -1., 0.);
            let plane = VerticalSectionPlane {
                origin: Point2::new(origin.x, origin.y),
                direction: Point2::new(-0.6, 0.8),
            };
            let mut solids = component_solids(&p, &wall, 2.3).unwrap();
            assert_eq!(solids.len(), boxes.len());
            for (s0, s1, n0, n1, z0, z1, material, crosses) in boxes {
                let expected: Vec<_> = [s0, s1]
                    .into_iter()
                    .flat_map(|s| {
                        [n0, n1]
                            .into_iter()
                            .flat_map(move |n| [z0, z1].map(|z| at(s, n, z)))
                    })
                    .collect();
                let matches = |mesh: &Mesh| {
                    mesh.vertices.len() == 8
                        && expected.iter().all(|a| {
                            mesh.vertices.iter().any(|b| {
                                (a.x - b.x).abs() < 1e-8
                                    && (a.y - b.y).abs() < 1e-8
                                    && (a.z - b.z).abs() < 1e-8
                            })
                        })
                };
                let index = solids.iter().position(matches).unwrap_or_else(|| {
                    panic!("missing analytic box {expected:?}, kind={kind:?}, reversed={reversed}")
                });
                let solid = solids.remove(index);
                assert!(solid.surfaces.iter().all(|s| s.material == material));
                let contours = vertical_section(&solid, plane).unwrap();
                if crosses {
                    assert_eq!(contours.len(), 1);
                    let expected = [
                        Point2::new(1. + n0, z0),
                        Point2::new(1. + n1, z0),
                        Point2::new(1. + n1, z1),
                        Point2::new(1. + n0, z1),
                    ];
                    assert_eq!(contours[0].points.len(), 4);
                    for point in expected {
                        assert!(contours[0].points.iter().any(|p| p.distance(point) < 1e-8));
                    }
                } else {
                    assert!(contours.is_empty());
                }
            }
        }
    }
}
