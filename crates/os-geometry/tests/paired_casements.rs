use os_core::{Id, Point2};
use os_geometry::{Mesh, Vec3, openings};
use os_model::*;

fn fixture(operation: WindowOperation) -> (ResolvedOpening, WallParams) {
    let id = Id::new();
    let p = ResolvedOpening {
        name: "Accessory sash".into(),
        host: id,
        offset: 1.,
        kind: OpeningKind::Window,
        width: 1.8,
        height: 1.3,
        sill: 0.7,
        pane_position: WindowPanePosition::Center,
        window_operation: operation,
        open_state: OpeningState::Default,
        type_id: None,
        type_name: None,
        hinge: DoorHinge::Start,
        swing: DoorSwing::Left,
        family: OpeningFamily {
            frame_width: 0.025,
            depth: 0.09,
            panel_material: Some(Id::new()),
            frame_material: Some(Id::new()),
            ..Default::default()
        },
    };
    let wall = WallParams {
        name: "Host".into(),
        path: WallPath::Straight {
            start: Point2::new(2., 3.),
            end: Point2::new(10., 3.),
        },
        height: 4.,
        thickness: 0.3,
        level: id,
        material: None,
    };
    (p, wall)
}

fn accessory(attachment: OpeningAttachment) -> OpeningExtrusion {
    let mut s = OpeningExtrusion::new("Bronze latch");
    s.attachment = attachment;
    s.profile = vec![
        Point2::new(0.23, 0.31),
        Point2::new(0.61, 0.31),
        Point2::new(0.61, 0.57),
        Point2::new(0.23, 0.57),
    ];
    s.position = Point2::new(0.003, 0.004);
    s.depth = 0.012;
    s.normal_offset = 0.083;
    s.material = Some(Id::new());
    s
}

fn assert_vertices(mesh: &Mesh, expected: &[Vec3]) {
    mesh.validate().unwrap();
    assert_eq!(mesh.vertices.len(), expected.len());
    for want in expected {
        assert!(
            mesh.vertices.iter().any(|v| (v.x - want.x).abs() < 1e-10
                && (v.y - want.y).abs() < 1e-10
                && (v.z - want.z).abs() < 1e-10),
            "missing {want:?}: {:?}",
            mesh.vertices
        );
    }
    assert!(mesh.signed_volume() > 0.);
}

#[test]
fn paired_sashes_analytic_geometry_materials_accessories_handles_and_mullion() {
    for fraction in [0.5, 0.7] {
        for direction in [
            Point2::new(1., 0.),
            Point2::new(0., 1.),
            Point2::new(-0.6, -0.8),
        ] {
            for side in [None, Some(LiteSide::Start), Some(LiteSide::End)] {
                for pane in [
                    WindowPanePosition::Center,
                    WindowPanePosition::LeftFace,
                    WindowPanePosition::RightFace,
                ] {
                    let (mut p, mut wall) = fixture(WindowOperation::PairedCasement {
                        start_fraction: fraction,
                        mullion_width: 0.04,
                    });
                    wall.path = WallPath::Straight {
                        start: Point2::new(2., 3.),
                        end: Point2::new(2. + 8. * direction.x, 3. + 8. * direction.y),
                    };
                    p.pane_position = pane;
                    p.family.side_lite = side.map(|side| SideLite {
                        side,
                        width_fraction: 0.15,
                        mullion_width: 0.03,
                        material: None,
                    });
                    p.family.extrusions = vec![
                        accessory(OpeningAttachment::StartSash),
                        accessory(OpeningAttachment::EndSash),
                    ];
                    let clear = p.width
                        - 2. * p.family.frame_width
                        - if side.is_some() {
                            p.width * 0.15 + 0.03
                        } else {
                            0.
                        };
                    let a = p.offset
                        + p.family.frame_width
                        + if side == Some(LiteSide::Start) {
                            p.width * 0.15 + 0.03
                        } else {
                            0.
                        };
                    let widths = [(clear - 0.04) * fraction, (clear - 0.04) * (1. - fraction)];
                    let baseline = openings::component_solids(&p, &wall, 0.4).unwrap();
                    for angles in [[0., 0.], [27., 63.], [90., 0.], [0., 90.]] {
                        p.open_state = OpeningState::CasementPairAngles {
                            start_degrees: angles[0],
                            end_degrees: angles[1],
                        };
                        let solids = openings::component_solids(&p, &wall, 0.4).unwrap();
                        assert_eq!(solids[0], baseline[0], "fixed center mullion");
                        for s in &solids {
                            s.validate().unwrap();
                            assert!(s.signed_volume() > 0.);
                        }
                        let center = match pane {
                            WindowPanePosition::Center => 0.,
                            WindowPanePosition::LeftFace => {
                                (wall.thickness - openings::depth(&p, &wall)) / 2.
                            }
                            WindowPanePosition::RightFace => {
                                -(wall.thickness - openings::depth(&p, &wall)) / 2.
                            }
                        };
                        for index in 0..2 {
                            let leaf = if index == 0 {
                                openings::PoseLeaf::StartSash
                            } else {
                                openings::PoseLeaf::EndSash
                            };
                            let g = openings::pose_handle_for_leaf(&p, &wall, leaf).unwrap();
                            let pivot = if index == 0 { a } else { a + clear };
                            let (sin, cos) = (angles[index] * if index == 0 { 1. } else { -1. })
                                .to_radians()
                                .sin_cos();
                            let transform = |x: f64, y: f64, z: f64| {
                                let u = x - pivot;
                                let v = y - center;
                                let x = pivot + u * cos - v * sin;
                                let y = center + u * sin + v * cos;
                                Vec3::new(
                                    2. + direction.x * x - direction.y * y,
                                    3. + direction.y * x + direction.x * y,
                                    z,
                                )
                            };
                            let free = pivot + if index == 0 { widths[0] } else { -widths[1] };
                            let want = transform(free, center, 0.);
                            assert!(g.point.distance(Point2::new(want.x, want.y)) < 1e-10);
                            assert_eq!(g.value, angles[index]);
                            let accessory = &p.family.extrusions[index];
                            let start = if index == 0 { a } else { a + widths[0] + 0.04 };
                            let expected: Vec<_> = [-accessory.depth / 2., accessory.depth / 2.]
                                .into_iter()
                                .flat_map(|normal| {
                                    accessory.profile.iter().map(move |q| (normal, *q))
                                })
                                .map(|(n, q)| {
                                    transform(
                                        start + q.x * widths[index] + accessory.position.x,
                                        center + n + accessory.normal_offset,
                                        0.4 + p.sill
                                            + p.family.frame_width
                                            + q.y * (p.height - 2. * p.family.frame_width)
                                            + accessory.position.y,
                                    )
                                })
                                .collect();
                            let mesh = &solids[solids.len() - 2 + index];
                            assert_vertices(mesh, &expected);
                            assert!(
                                mesh.surfaces
                                    .iter()
                                    .all(|s| s.material == accessory.material
                                        && s.layer == Some(accessory.id))
                            );
                            if angles[index] == 0. {
                                assert_eq!(mesh, &baseline[baseline.len() - 2 + index]);
                            }
                            let symbol = openings::paired_casement_plan_lines(&p, &wall).unwrap();
                            let local_free = Point2::new(
                                pivot + (free - pivot) * cos,
                                center + (free - pivot) * sin,
                            );
                            assert!(symbol.iter().any(|(a, b)| a.distance(local_free) < 1e-10
                                || b.distance(local_free) < 1e-10));
                        }
                    }
                }
            }
        }
    }
}
