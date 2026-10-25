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
fn moving_sash_vertices_match_analytic_poses_with_parent_depth_and_stationary_parts() {
    for operation in [WindowOperation::Casement, WindowOperation::Sliding] {
        for direction in [
            Point2::new(1., 0.),
            Point2::new(0., 1.),
            Point2::new(0.6, 0.8),
            Point2::new(-0.6, -0.8),
        ] {
            for width in [1.8, 0.32] {
                for frame in [0., 0.025] {
                    for side in [None, Some(LiteSide::Start), Some(LiteSide::End)] {
                        for pane in [
                            WindowPanePosition::Center,
                            WindowPanePosition::LeftFace,
                            WindowPanePosition::RightFace,
                        ] {
                            let (mut p, mut wall) = fixture(operation);
                            wall.path = WallPath::Straight {
                                start: Point2::new(2., 3.),
                                end: Point2::new(2. + 8. * direction.x, 3. + 8. * direction.y),
                            };
                            p.width = width;
                            p.pane_position = pane;
                            p.family.frame_width = frame;
                            p.family.side_lite = side.map(|side| SideLite {
                                side,
                                width_fraction: 0.12,
                                mullion_width: 0.02,
                                material: Some(Id::new()),
                            });
                            let moving = accessory(OpeningAttachment::MovingSash);
                            let fixed = accessory(OpeningAttachment::OpeningFixed);
                            p.family.extrusions = vec![moving.clone(), fixed.clone()];
                            let baseline = openings::component_solids(&p, &wall, 0.4).unwrap();
                            // Independently derive the attachment bay and effective parent depth.
                            let lite_width = if side.is_some() { width * 0.12 } else { 0. };
                            let clear_width = width
                                - 2. * frame
                                - lite_width
                                - if side.is_some() { 0.02 } else { 0. };
                            let start = p.offset
                                + frame
                                + if side == Some(LiteSide::Start) {
                                    lite_width + 0.02
                                } else {
                                    0.
                                };
                            let clear_height = p.height - 2. * frame;
                            let depth = p.family.depth.min(wall.thickness * 0.2).min(
                                if side.is_some() {
                                    clear_width.min(lite_width)
                                } else {
                                    width
                                } * 0.2,
                            );
                            let center = match pane {
                                WindowPanePosition::Center => 0.,
                                WindowPanePosition::LeftFace => (wall.thickness - depth) / 2.,
                                WindowPanePosition::RightFace => -(wall.thickness - depth) / 2.,
                            };
                            let rail = 0.05_f64.min(clear_width * 0.1).min(clear_height * 0.1);
                            let sash_width = if operation == WindowOperation::Sliding {
                                (clear_width + rail) / 2.
                            } else {
                                clear_width
                            };
                            let mut volume: Option<f64> = None;
                            for fraction in [0., 0.5, 1.] {
                                p.open_state = if operation == WindowOperation::Sliding {
                                    OpeningState::SlidingFraction(fraction)
                                } else {
                                    OpeningState::CasementAngle(fraction * 90.)
                                };
                                let solids = openings::component_solids(&p, &wall, 0.4).unwrap();
                                let mesh = &solids[solids.len() - 2];
                                let expected: Vec<_> = [-moving.depth / 2., moving.depth / 2.]
                                    .into_iter()
                                    .flat_map(|normal| {
                                        moving.profile.iter().map(move |q| (normal, *q))
                                    })
                                    .map(|(normal, q)| {
                                        let u = q.x * sash_width + moving.position.x;
                                        let v = normal + moving.normal_offset
                                            - if operation == WindowOperation::Sliding {
                                                depth * 0.275
                                            } else {
                                                0.
                                            };
                                        let (x, y) = if operation == WindowOperation::Sliding {
                                            (
                                                start + u + fraction * (clear_width - rail) / 2.,
                                                center + v,
                                            )
                                        } else {
                                            let (sin, cos) =
                                                (fraction * 90.).to_radians().sin_cos();
                                            (start + u * cos - v * sin, center + u * sin + v * cos)
                                        };
                                        Vec3::new(
                                            2. + direction.x * x - direction.y * y,
                                            3. + direction.y * x + direction.x * y,
                                            0.4 + p.sill
                                                + frame
                                                + q.y * clear_height
                                                + moving.position.y,
                                        )
                                    })
                                    .collect();
                                assert_vertices(mesh, &expected);
                                assert!(
                                    mesh.surfaces.iter().all(|s| s.layer == Some(moving.id)
                                        && s.material == moving.material)
                                );
                                if let Some(v) = volume {
                                    assert!((mesh.signed_volume() - v).abs() < 1e-10);
                                }
                                volume = Some(mesh.signed_volume());
                                assert_eq!(mesh.triangles, baseline[baseline.len() - 2].triangles);
                                // The five Start sash solids move. End, lite, mullion, frame and
                                // opening-fixed extrusion must retain exactly their closed geometry.
                                assert_eq!(
                                    &solids[5..solids.len() - 2],
                                    &baseline[5..baseline.len() - 2]
                                );
                                assert_eq!(solids.last(), baseline.last());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn moving_sash_keeps_sixteen_exact_distinct_solids_and_plan_surface_identity() {
    let (mut p, wall) = fixture(WindowOperation::Sliding);
    p.open_state = OpeningState::SlidingFraction(0.5);
    p.family.extrusions = (0..OpeningFamily::MAX_EXTRUSIONS)
        .map(|_| accessory(OpeningAttachment::MovingSash))
        .collect();
    assert_eq!(p.family.extrusions.len(), 16);
    let solids = openings::component_solids(&p, &wall, 0.).unwrap();
    assert_eq!(solids.len(), 14 + 16);
    for (solid, mesh) in p.family.extrusions.iter().zip(&solids[14..]) {
        assert!(
            mesh.surfaces.iter().all(
                |surface| surface.layer == Some(solid.id) && surface.material == solid.material
            )
        );
        let bay = openings::sash_bay(&p).unwrap();
        let rail = 0.05_f64.min(bay.width * 0.1).min(bay.height * 0.1);
        let expected_volume =
            (0.61 - 0.23) * (bay.width + rail) / 2. * (0.57 - 0.31) * bay.height * solid.depth;
        assert!((mesh.signed_volume() - expected_volume).abs() < 1e-10);
    }
    let edges = os_geometry::opening_extrusions::plan_edges(
        &p,
        &wall,
        0.,
        os_geometry::plan::PlanRange {
            cut: 1.2,
            ..Default::default()
        },
    )
    .unwrap();
    for solid in &p.family.extrusions {
        let own: Vec<_> = edges
            .iter()
            .filter(|e| e.3.layer == Some(solid.id))
            .collect();
        assert_eq!(own.len(), 4);
        assert!(own.iter().all(|e| e.3.material == solid.material));
    }
    p.family
        .extrusions
        .push(accessory(OpeningAttachment::MovingSash));
    assert!(openings::component_solids(&p, &wall, 0.).is_err());
}
