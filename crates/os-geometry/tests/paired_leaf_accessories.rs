use os_core::{Id, Point2};
use os_geometry::{Vec3, openings};
use os_model::*;

#[test]
fn paired_leaf_accessories_follow_independent_analytic_poses_and_keep_provenance() {
    for direction in [
        Point2::new(1., 0.),
        Point2::new(0., 1.),
        Point2::new(0.6, 0.8),
        Point2::new(-0.6, -0.8),
    ] {
        for hinge in [DoorHinge::Start, DoorHinge::End] {
            for swing in [DoorSwing::Left, DoorSwing::Right] {
                for lite in [None, Some(LiteSide::Start), Some(LiteSide::End)] {
                    for width in [2.4, 0.65] {
                        let wall = WallParams {
                            name: "Host".into(),
                            path: WallPath::Straight {
                                start: Point2::new(2., 3.),
                                end: Point2::new(2. + 8. * direction.x, 3. + 8. * direction.y),
                            },
                            height: 4.,
                            thickness: 0.3,
                            level: Id::new(),
                            material: None,
                        };
                        let mut p = ResolvedOpening {
                            name: "Pair".into(),
                            host: Id::new(),
                            offset: 1.,
                            kind: OpeningKind::Door,
                            width,
                            height: 2.2,
                            sill: 0.,
                            pane_position: WindowPanePosition::Center,
                            window_operation: WindowOperation::Fixed,
                            open_state: OpeningState::Default,
                            type_id: None,
                            type_name: None,
                            hinge,
                            swing,
                            family: OpeningFamily {
                                frame_width: 0.03,
                                door_leaves: DoorLeaves::Paired {
                                    active_fraction: 0.63,
                                },
                                side_lite: lite.map(|side| SideLite {
                                    side,
                                    width_fraction: 0.15,
                                    mullion_width: 0.02,
                                    material: None,
                                }),
                                ..Default::default()
                            },
                        };
                        for attachment in [
                            OpeningAttachment::ActiveLeaf,
                            OpeningAttachment::InactiveLeaf,
                            OpeningAttachment::OpeningFixed,
                        ] {
                            let mut s = OpeningExtrusion::new("Handle");
                            s.attachment = attachment;
                            s.material = Some(Id::new());
                            s.position = Point2::new(0.002, 0.003);
                            s.depth = 0.013;
                            s.normal_offset = 0.071;
                            p.family.extrusions.push(s);
                        }
                        p.open_state = OpeningState::DoorPairAngles {
                            active_degrees: 0.,
                            inactive_degrees: 0.,
                        };
                        let baseline = openings::component_solids(&p, &wall, 0.4).unwrap();
                        for active in [0., 30., 90.] {
                            for inactive in [0., 30., 90.] {
                                p.open_state = OpeningState::DoorPairAngles {
                                    active_degrees: active,
                                    inactive_degrees: inactive,
                                };
                                let meshes = openings::authored_solids(&p, &wall, 0.4).unwrap();
                                let primary_start = 0.03
                                    + if lite == Some(LiteSide::Start) {
                                        width * 0.15 + 0.02
                                    } else {
                                        0.
                                    };
                                let primary_end = width
                                    - 0.03
                                    - if lite == Some(LiteSide::End) {
                                        width * 0.15 + 0.02
                                    } else {
                                        0.
                                    };
                                for (i, mesh) in meshes.iter().enumerate().take(2) {
                                    let s = &p.family.extrusions[i];
                                    let w = (primary_end - primary_start)
                                        * if i == 0 { 0.63 } else { 0.37 };
                                    let start_hinge = (hinge == DoorHinge::Start) == (i == 0);
                                    let h = if start_hinge {
                                        primary_start
                                    } else {
                                        primary_end
                                    };
                                    let angle: f64 = if i == 0 { active } else { inactive };
                                    let (sin, cos) = angle.to_radians().sin_cos();
                                    let dx = if start_hinge { cos } else { -cos };
                                    let side = if swing == DoorSwing::Left { 1. } else { -1. };
                                    let dy = side * sin;
                                    for normal in [-s.depth / 2., s.depth / 2.] {
                                        for q in &s.profile {
                                            let u = q.x * w + s.position.x;
                                            let n = normal + s.normal_offset;
                                            let x = p.offset + h + dx * u - dy * n;
                                            let y = -side * wall.thickness / 2. + dy * u + dx * n;
                                            let want = Vec3::new(
                                                2. + direction.x * x - direction.y * y,
                                                3. + direction.y * x + direction.x * y,
                                                0.4 + q.y * (p.height - 0.03) + s.position.y,
                                            );
                                            assert!(
                                                mesh.vertices
                                                    .iter()
                                                    .any(|v| (v.x - want.x).abs() < 1e-10
                                                        && (v.y - want.y).abs() < 1e-10
                                                        && (v.z - want.z).abs() < 1e-10),
                                                "missing {want:?}"
                                            );
                                        }
                                    }
                                    assert!(mesh.signed_volume() > 0.);
                                    assert!(
                                        mesh.surfaces
                                            .iter()
                                            .all(|v| v.layer == Some(s.id)
                                                && v.material == s.material)
                                    );
                                }
                                let all = openings::component_solids(&p, &wall, 0.4).unwrap();
                                assert_eq!(
                                    &all[2..all.len() - 3],
                                    &baseline[2..baseline.len() - 3]
                                );
                                assert_eq!(all.last(), baseline.last());
                                let old = meshes;
                                p.open_state = OpeningState::DoorPairAngles {
                                    active_degrees: active,
                                    inactive_degrees: (inactive + 30.).min(90.),
                                };
                                let new = openings::authored_solids(&p, &wall, 0.4).unwrap();
                                assert_eq!(old[0], new[0]);
                                assert_eq!(old[2], new[2]);
                            }
                        }
                    }
                }
            }
        }
    }
}
