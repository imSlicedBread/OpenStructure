use os_core::Point2;
use os_geometry::{Profile, Solid, Transform, Vec3, plan::*};

fn prism(base: f64, height: f64) -> Solid {
    Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(0.0, -0.1),
                Point2::new(5.0, -0.1),
                Point2::new(5.0, 0.1),
                Point2::new(0.0, 0.1),
            ],
        },
        height,
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, base),
            rotation_z: 0.0,
        },
    }
}
fn role(base: f64, height: f64) -> Option<PlanRole> {
    rectangular_plan(
        &prism(base, height),
        PlanRange::default(),
        HorizontalBasis::default(),
        None,
    )
    .unwrap()
    .map(|footprint| footprint.role)
}

#[test]
fn horizontal_range_distinguishes_cut_projection_depth_and_hidden_boundaries() {
    assert_eq!(role(0.0, 3.0), Some(PlanRole::Cut));
    assert_eq!(role(0.0, 1.2), Some(PlanRole::Projected)); // top touches cut
    assert_eq!(role(1.2, 1.0), Some(PlanRole::Cut)); // base at cut
    assert_eq!(role(1.3, 1.0), None); // entirely overhead
    assert_eq!(role(2.5, 1.0), None); // touches view top
    assert_eq!(role(-2.0, 2.0), Some(PlanRole::Depth)); // top at bottom
    assert_eq!(role(-2.0, 1.0), None); // top at depth
    assert_eq!(role(-2.0, 1.5), Some(PlanRole::Depth));
    assert_eq!(role(-2.0, 2.5), Some(PlanRole::Projected));
    assert_eq!(role(-2.0, 10.0), Some(PlanRole::Cut));
    assert_eq!(
        role(0.0, 1.2 + PLAN_TOLERANCE / 2.0),
        Some(PlanRole::Projected)
    );
    assert_eq!(role(0.0, 1.2 + PLAN_TOLERANCE * 2.0), Some(PlanRole::Cut));
}

#[test]
fn level_relative_range_moves_with_the_same_prism_and_rotated_basis() {
    let mut solid = prism(4.2, 3.0);
    solid.transform.translation.x = 1_000_000.0;
    solid.transform.translation.y = -2_000_000.0;
    solid.transform.rotation_z = 0.713;
    let basis = HorizontalBasis {
        origin: Point2::new(1_000_000.0, -2_000_000.0),
        rotation: 0.713,
    };
    let footprint = rectangular_plan(
        &solid,
        PlanRange::default().at_level(4.2).unwrap(),
        basis,
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(footprint.role, PlanRole::Cut);
    assert!((footprint.area() - 1.0).abs() < 1e-8);
    for (actual, expected) in footprint.vertices().iter().zip(&solid.profile.vertices) {
        assert!(actual.distance(*expected) < 1e-8);
        let world = basis.plane_to_world(*actual).unwrap();
        assert!(basis.world_to_plane(world).unwrap().distance(*actual) < 1e-8);
    }
    assert!(footprint.contains(Point2::new(2.0, 0.0)));
    assert!(!footprint.contains(Point2::new(2.0, 0.11)));
    assert!(!footprint.contains(Point2::new(f64::NAN, 0.0)));
}

#[test]
fn crop_geometry_area_and_picking_agree_and_touching_is_empty() {
    let crop = PlanCrop {
        min: Point2::new(1.0, -0.05),
        max: Point2::new(3.0, 1.0),
    };
    let footprint = rectangular_plan(
        &prism(0.0, 3.0),
        PlanRange::default(),
        HorizontalBasis::default(),
        Some(crop),
    )
    .unwrap()
    .unwrap();
    assert!((footprint.area() - 0.3).abs() < 1e-12);
    assert!(footprint.contains(Point2::new(2.0, 0.0)));
    assert!(!footprint.contains(Point2::new(0.5, 0.0)));
    assert!(!footprint.contains(Point2::new(2.0, -0.075)));
    for min_x in [5.0, 6.0] {
        let crop = PlanCrop {
            min: Point2::new(min_x, -1.0),
            max: Point2::new(7.0, 1.0),
        };
        assert!(
            rectangular_plan(
                &prism(0.0, 3.0),
                PlanRange::default(),
                HorizontalBasis::default(),
                Some(crop)
            )
            .unwrap()
            .is_none()
        );
    }
}

#[test]
fn rotated_rectangle_crop_is_bounded_deterministic_and_independently_measured() {
    let solid = Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(-1.0, -1.0),
                Point2::new(1.0, -1.0),
                Point2::new(1.0, 1.0),
                Point2::new(-1.0, 1.0),
            ],
        },
        height: 3.0,
        transform: Transform {
            rotation_z: std::f64::consts::FRAC_PI_4,
            ..Default::default()
        },
    };
    let crop = Some(PlanCrop {
        min: Point2::new(-1.0, -1.0),
        max: Point2::new(1.0, 1.0),
    });
    let first = rectangular_plan(
        &solid,
        PlanRange::default(),
        HorizontalBasis::default(),
        crop,
    )
    .unwrap()
    .unwrap();
    assert_eq!(first.vertices().len(), 8);
    // Square area minus four right corner triangles of leg 2-sqrt(2).
    let expected = 4.0 - 2.0 * (2.0 - 2_f64.sqrt()).powi(2);
    assert!((first.area() - expected).abs() < 1e-12);
    assert!(!first.contains(Point2::new(0.99, 0.99)));
    for _ in 0..10 {
        assert_eq!(
            rectangular_plan(
                &solid,
                PlanRange::default(),
                HorizontalBasis::default(),
                crop
            )
            .unwrap()
            .unwrap(),
            first
        );
    }
}

#[test]
fn corner_aligned_crops_do_not_create_duplicate_vertices_or_false_overflow() {
    let mut solid = prism(0.0, 3.0);
    for rotation in [0.0, 0.37, std::f64::consts::FRAC_PI_4] {
        solid.transform.rotation_z = rotation;
        let full = rectangular_plan(
            &solid,
            PlanRange::default(),
            HorizontalBasis::default(),
            None,
        )
        .unwrap()
        .unwrap();
        for corner in full.vertices() {
            for dx in [-1.0, 1.0] {
                for dy in [-1.0, 1.0] {
                    let other = Point2::new(corner.x + dx, corner.y + dy);
                    let crop = PlanCrop {
                        min: Point2::new(corner.x.min(other.x), corner.y.min(other.y)),
                        max: Point2::new(corner.x.max(other.x), corner.y.max(other.y)),
                    };
                    if let Some(clipped) = rectangular_plan(
                        &solid,
                        PlanRange::default(),
                        HorizontalBasis::default(),
                        Some(crop),
                    )
                    .unwrap()
                    {
                        assert!(clipped.vertices().len() <= 8);
                        assert!(clipped.area() > 0.0 && clipped.area() <= full.area() + 1e-9);
                        assert!(
                            clipped
                                .vertices()
                                .windows(2)
                                .all(|pair| pair[0].distance(pair[1]) > PLAN_TOLERANCE)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_or_unsupported_geometry_and_context_never_return_a_partial_plan() {
    let valid = prism(0.0, 3.0);
    let basis = HorizontalBasis::default();
    assert!(
        rectangular_plan(
            &valid,
            PlanRange::default(),
            HorizontalBasis {
                origin: Point2::new(f64::MAX, f64::MAX),
                ..basis
            },
            None
        )
        .is_err()
    );
    for range in [
        PlanRange {
            cut: 3.0,
            ..Default::default()
        },
        PlanRange {
            depth: 1.0,
            ..Default::default()
        },
        PlanRange {
            top: f64::INFINITY,
            ..Default::default()
        },
    ] {
        assert!(rectangular_plan(&valid, range, basis, None).is_err());
    }
    assert!(PlanRange::default().at_level(f64::MAX).is_err());
    assert!(
        basis
            .world_to_plane(Point2::new(f64::INFINITY, 0.0))
            .is_err()
    );
    assert!(
        rectangular_plan(
            &valid,
            PlanRange::default(),
            HorizontalBasis {
                rotation: f64::NAN,
                ..basis
            },
            None
        )
        .is_err()
    );
    let mut invalid = prism(50.0, 3.0); // invalid hidden geometry still fails
    invalid.profile.vertices.pop();
    assert!(rectangular_plan(&invalid, PlanRange::default(), basis, None).is_err());
    let crop = PlanCrop {
        min: Point2::new(2.0, 0.0),
        max: Point2::new(1.0, 1.0),
    };
    assert!(rectangular_plan(&valid, PlanRange::default(), basis, Some(crop)).is_err());
}
