use os_core::{Id, Point2};
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange};
use os_render::{
    plan::{PlanCamera, PlanContext},
    snapping::*,
};

fn context() -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}

#[test]
fn prepared_dense_scene_preserves_crossings_order_and_revision_rebuilds() {
    let initial = context();
    let owner = Id::new();
    let mut previous: Option<SnapScene> = None;
    for revision in 0..4 {
        let c = PlanContext {
            model_revision: revision,
            ..initial
        };
        let crossing = Point2::new(
            1_000_000.0 + revision as f64 * 13.0,
            -2_000_000.0 + revision as f64 * 7.0,
        );
        let segments: Vec<_> = (0..256)
            .map(|feature| {
                let angle = feature as f64 * std::f64::consts::PI / 256.0 + 0.37;
                let (sin, cos) = angle.sin_cos();
                let length = 2.0 + feature as f64 * 0.13;
                let mut start = Point2::new(crossing.x - cos * length, crossing.y - sin * length);
                let mut end = Point2::new(crossing.x + cos * length, crossing.y + sin * length);
                if feature % 2 == 0 {
                    std::mem::swap(&mut start, &mut end);
                }
                SnapSegment {
                    entity: owner,
                    feature,
                    start,
                    end,
                }
            })
            .collect();
        let scene = SnapScene::new(c, segments.clone()).unwrap();
        let reversed = SnapScene::new(c, segments.into_iter().rev().collect()).unwrap();
        let mut q = query(Point2::default(), 65.0);
        q.camera.center = crossing;
        q.pointer = q.camera.project(crossing, q.viewport).unwrap();
        q.intersections = true;
        let hit = scene.query(c, q).unwrap().candidate(c, q).unwrap().unwrap();
        assert_eq!(hit.kind, SnapKind::Intersection);
        assert!(hit.point.distance(crossing) < 1e-7);
        assert_eq!(hit.entity, owner);
        let other = hit.other.unwrap();
        assert_eq!(other.0, owner);
        assert!(hit.feature < other.1);
        assert_eq!(
            Some(hit),
            reversed.query(c, q).unwrap().candidate(c, q).unwrap()
        );
        if let Some(old) = previous {
            assert!(old.query(c, q).is_err());
        }
        previous = Some(scene);
    }
}

#[test]
fn optional_axis_extensions_keep_semantic_identity_and_crop_limits() {
    let c = context();
    let s = segment();
    let scene = SnapScene::new(c, vec![s]).unwrap();
    for x in [-3.0, 13.0] {
        let mut q = query(Point2::new(x, 0.05), 100.0);
        assert!(
            scene
                .query(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .is_none()
        );
        q.axis_extensions = true;
        let result = scene.query(c, q).unwrap();
        let hit = result.candidate(c, q).unwrap().unwrap();
        assert_eq!(hit.kind, SnapKind::AxisExtension);
        assert_eq!(hit.point, Point2::new(x, 0.0));
        assert_eq!((hit.entity, hit.feature), (s.entity, s.feature));
        q.axis_extensions = false;
        assert!(result.candidate(c, q).is_err());
        q.axis_extensions = true;
        q.exclude_entity = Some(s.entity);
        assert!(
            scene
                .query(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .is_none()
        );
    }
    let cropped = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(-1.0, -1.0),
            max: Point2::new(11.0, 1.0),
        }),
        ..c
    };
    let mut q = query(Point2::new(13.0, 0.05), 100.0);
    q.axis_extensions = true;
    assert!(
        SnapScene::new(cropped, vec![s])
            .unwrap()
            .query(cropped, q)
            .unwrap()
            .candidate(cropped, q)
            .unwrap()
            .is_none()
    );
    q = query(Point2::new(3.0, 0.05), 100.0);
    q.axis_extensions = true;
    assert_eq!(
        scene
            .query(c, q)
            .unwrap()
            .candidate(c, q)
            .unwrap()
            .unwrap()
            .kind,
        SnapKind::Nearest
    );
}

#[test]
fn diagonal_axis_extension_uses_local_arithmetic_at_large_origin() {
    let c = context();
    let origin = Point2::new(1_000_000.0, 2_000_000.0);
    let s = SnapSegment {
        start: origin,
        end: Point2::new(origin.x + 3.0, origin.y + 4.0),
        ..segment()
    };
    let target = Point2::new(origin.x + 6.0, origin.y + 8.0);
    let mut q = query(Point2::default(), 100.0);
    q.camera.center = target;
    q.axis_extensions = true;
    let hit = SnapScene::new(c, vec![s])
        .unwrap()
        .query(c, q)
        .unwrap()
        .candidate(c, q)
        .unwrap()
        .unwrap();
    assert_eq!(hit.kind, SnapKind::AxisExtension);
    assert!(hit.point.distance(target) < 1e-8);
}

#[test]
fn perpendicular_foot_uses_anchor_and_rejects_changed_anchor_or_crop() {
    let c = context();
    let s = segment();
    let scene = SnapScene::new(c, vec![s]).unwrap();
    let mut q = query(Point2::new(3.05, 0.04), 100.0);
    q.perpendicular_from = Some(Point2::new(3.0, 4.0));
    let result = scene.query(c, q).unwrap();
    let hit = result.candidate(c, q).unwrap().unwrap();
    assert_eq!(hit.kind, SnapKind::Perpendicular);
    assert_eq!(hit.point, Point2::new(3.0, 0.0));
    assert_eq!((hit.entity, hit.feature), (s.entity, s.feature));
    assert!(hit.other.is_none());
    q.perpendicular_from = Some(Point2::new(4.0, 4.0));
    assert!(result.candidate(c, q).is_err());
    q.nearest = false;
    q.midpoints = false;
    q.endpoints = false;
    for anchor in [
        Point2::new(-0.01, 2.0),
        Point2::new(10.01, 2.0),
        Point2::new(3.0, 0.0),
    ] {
        q.perpendicular_from = Some(anchor);
        q.pointer = q
            .camera
            .project(Point2::new(anchor.x, 0.0), q.viewport)
            .unwrap();
        assert!(
            scene
                .query(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .is_none()
        );
    }
    q.perpendicular_from = Some(Point2::new(f64::NAN, 1.0));
    assert!(scene.query(c, q).is_err());
    q.perpendicular_from = Some(Point2::new(3.0, 4.0));
    q.pointer = q.camera.project(Point2::new(3.0, 0.0), q.viewport).unwrap();
    let cropped = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(4.0, -1.0),
            max: Point2::new(8.0, 1.0),
        }),
        ..c
    };
    assert!(
        SnapScene::new(cropped, vec![s])
            .unwrap()
            .query(cropped, q)
            .unwrap()
            .candidate(cropped, q)
            .unwrap()
            .is_none()
    );
    q.exclude_entity = Some(s.entity);
    assert!(
        scene
            .query(c, q)
            .unwrap()
            .candidate(c, q)
            .unwrap()
            .is_none()
    );
}

#[test]
fn perpendicular_is_stable_for_diagonal_large_origin_segments() {
    let c = context();
    let origin = Point2::new(1_000_000.0, 2_000_000.0);
    let segment = SnapSegment {
        start: Point2::new(origin.x - 3.0, origin.y - 4.0),
        end: Point2::new(origin.x + 6.0, origin.y + 8.0),
        ..segment()
    };
    let mut q = query(Point2::default(), 100.0);
    q.camera.center = origin;
    q.perpendicular_from = Some(Point2::new(origin.x - 4.0, origin.y + 3.0));
    let hit = SnapScene::new(c, vec![segment])
        .unwrap()
        .query(c, q)
        .unwrap()
        .candidate(c, q)
        .unwrap()
        .unwrap();
    assert_eq!(hit.kind, SnapKind::Perpendicular);
    assert!(hit.point.distance(origin) < 1e-8);
}

#[test]
fn intersections_preserve_both_keys_order_crop_and_exclusion() {
    let c = context();
    let a = SnapSegment {
        entity: Id::new(),
        feature: 4,
        start: Point2::new(-3.0, 0.0),
        end: Point2::new(4.0, 0.0),
    };
    let b = SnapSegment {
        entity: Id::new(),
        feature: 9,
        start: Point2::new(0.0, -2.0),
        end: Point2::new(0.0, 5.0),
    };
    let mut keys = [(a.entity, a.feature), (b.entity, b.feature)];
    keys.sort();
    let mut q = query(Point2::new(0.06, 0.04), 100.0);
    q.intersections = true;
    for segments in [vec![a, b], vec![b, a]] {
        let scene = SnapScene::new(c, segments).unwrap();
        let result = scene.query(c, q).unwrap();
        let hit = result.candidate(c, q).unwrap().unwrap();
        assert_eq!(hit.kind, SnapKind::Intersection);
        assert_eq!(hit.point, Point2::default());
        assert_eq!((hit.entity, hit.feature), keys[0]);
        assert_eq!(hit.other, Some(keys[1]));
        let excluded = SnapQuery {
            exclude_entity: Some(a.entity),
            ..q
        };
        assert!(result.candidate(c, excluded).is_err());
        assert_ne!(
            scene
                .query(c, excluded)
                .unwrap()
                .candidate(c, excluded)
                .unwrap()
                .unwrap()
                .kind,
            SnapKind::Intersection
        );
    }
    let c = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(0.01, -1.0),
            max: Point2::new(2.0, 1.0),
        }),
        ..c
    };
    q.endpoints = false;
    q.midpoints = false;
    q.nearest = false;
    assert!(
        SnapScene::new(c, vec![a, b])
            .unwrap()
            .query(c, q)
            .unwrap()
            .candidate(c, q)
            .unwrap()
            .is_none()
    );
}

#[test]
fn intersection_conditions_do_not_invent_extensions_or_overlap_points() {
    let c = context();
    let mut q = query(Point2::default(), 100.0);
    q.intersections = true;
    q.endpoints = false;
    q.midpoints = false;
    q.nearest = false;
    let a = SnapSegment {
        start: Point2::new(-1.0, 0.0),
        end: Point2::new(1.0, 0.0),
        ..segment()
    };
    for (start, end) in [
        (Point2::new(-0.5, 0.0), Point2::new(0.5, 0.0)),
        (Point2::new(-1.0, 0.01), Point2::new(1.0, 0.01)),
        (Point2::new(0.0, 0.01), Point2::new(0.0, 1.0)),
        (Point2::new(-1.0, -1e-14), Point2::new(1.0, 1e-14)),
    ] {
        let b = SnapSegment {
            start,
            end,
            ..segment()
        };
        assert!(
            SnapScene::new(c, vec![a, b])
                .unwrap()
                .query(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .is_none()
        );
    }
    // Translated diagonal crossing, away from either midpoint, at a large origin.
    let origin = 1_000_000.0;
    let a = SnapSegment {
        start: Point2::new(origin - 2.0, origin - 2.0),
        end: Point2::new(origin + 3.0, origin + 3.0),
        ..a
    };
    let b = SnapSegment {
        start: Point2::new(origin - 2.0, origin + 2.0),
        end: Point2::new(origin + 4.0, origin - 4.0),
        ..segment()
    };
    q.camera.center = Point2::new(origin, origin);
    let hit = SnapScene::new(c, vec![a, b])
        .unwrap()
        .query(c, q)
        .unwrap()
        .candidate(c, q)
        .unwrap()
        .unwrap();
    assert!(hit.point.distance(Point2::new(origin, origin)) < 1e-8);
}

#[test]
fn dense_intersection_queries_fail_explicitly_and_can_be_disabled() {
    let c = context();
    let segments = (0..257)
        .map(|_| SnapSegment {
            start: Point2::new(-1.0, 0.0),
            end: Point2::new(1.0, 0.0),
            ..segment()
        })
        .collect();
    let scene = SnapScene::new(c, segments).unwrap();
    let mut q = query(Point2::default(), 100.0);
    q.intersections = true;
    assert!(
        scene
            .query(c, q)
            .err()
            .unwrap()
            .to_string()
            .contains("256 nearby")
    );
    q.intersections = false;
    assert!(scene.query(c, q).is_ok());
}

#[test]
fn excluded_edit_target_is_not_acquired_and_exclusion_is_part_of_result_identity() {
    let context = context();
    let segment = segment();
    let id = segment.entity;
    let scene = SnapScene::new(context, vec![segment]).unwrap();
    let original = query(Point2::default(), 100.0);
    let result = scene.query(context, original).unwrap();
    assert_eq!(
        result.candidate(context, original).unwrap().unwrap().entity,
        id
    );
    let excluded = SnapQuery {
        exclude_entity: Some(id),
        ..original
    };
    assert!(result.candidate(context, excluded).is_err());
    assert!(
        scene
            .query(context, excluded)
            .unwrap()
            .candidate(context, excluded)
            .unwrap()
            .is_none()
    );
}
fn segment() -> SnapSegment {
    SnapSegment {
        entity: Id::new(),
        feature: 0,
        start: Point2::new(0.0, 0.0),
        end: Point2::new(10.0, 0.0),
    }
}
fn query(point: Point2, zoom: f64) -> SnapQuery {
    let camera = PlanCamera {
        center: Point2::default(),
        pixels_per_metre: zoom,
    };
    SnapQuery {
        camera,
        viewport: [800.0, 600.0],
        pointer: camera.project(point, [800.0, 600.0]).unwrap(),
        radius_pixels: 12.0,
        endpoints: true,
        midpoints: true,
        intersections: false,
        perpendicular_from: None,
        nearest: true,
        axis_extensions: false,
        exclude_entity: None,
    }
}
#[test]
fn snap_kinds_priorities_and_pixel_tolerance_are_explicit() {
    let c = context();
    let s = segment();
    let scene = SnapScene::new(c, vec![s]).unwrap();
    for (p, kind, x) in [
        (Point2::new(0.08, 0.0), SnapKind::Endpoint, 0.0),
        (Point2::new(5.08, 0.0), SnapKind::Midpoint, 5.0),
        (Point2::new(3.0, 0.08), SnapKind::Nearest, 3.0),
    ] {
        let q = query(p, 100.0);
        let hit = scene.query(c, q).unwrap().candidate(c, q).unwrap().unwrap();
        assert_eq!(hit.kind, kind);
        assert_eq!(hit.entity, s.entity);
        assert_eq!(hit.point.x, x);
    }
    for zoom in [0.01, 1.0, 100.0, 10000.0] {
        let mut q = query(Point2::default(), zoom);
        q.midpoints = false;
        q.nearest = false;
        q.pointer.x -= 12.0;
        assert!(
            scene
                .query(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .is_some()
        );
        q.pointer.x -= 0.001;
        assert!(
            scene
                .query(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn crop_does_not_turn_new_clip_corners_into_semantic_endpoints() {
    let c = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(2.0, -1.0),
            max: Point2::new(3.0, 1.0),
        }),
        ..context()
    };
    let scene = SnapScene::new(c, vec![segment()]).unwrap();
    let mut q = query(Point2::new(1.99, 0.0), 100.0);
    let hit = scene.query(c, q).unwrap().candidate(c, q).unwrap().unwrap();
    assert_eq!(hit.kind, SnapKind::Nearest);
    assert_eq!(hit.point, Point2::new(2.0, 0.0));
    q.nearest = false;
    assert!(
        scene
            .query(c, q)
            .unwrap()
            .candidate(c, q)
            .unwrap()
            .is_none()
    );
}
#[test]
fn ties_are_independent_of_provider_order_and_results_reject_navigation_changes() {
    let c = context();
    let a = segment();
    let b = segment();
    let q = query(Point2::default(), 100.0);
    let left = SnapScene::new(c, vec![a, b]).unwrap().query(c, q).unwrap();
    let right = SnapScene::new(c, vec![b, a]).unwrap().query(c, q).unwrap();
    assert_eq!(
        left.candidate(c, q).unwrap(),
        right.candidate(c, q).unwrap()
    );
    assert_eq!(
        left.candidate(c, q).unwrap().unwrap().entity,
        a.entity.min(b.entity)
    );
    let mut changed = q;
    changed.camera.pixels_per_metre *= 2.0;
    assert!(left.candidate(c, changed).is_err());
    changed = q;
    changed.pointer.x += 1.0;
    assert!(left.candidate(c, changed).is_err());
    assert!(
        left.candidate(
            PlanContext {
                model_revision: 1,
                ..c
            },
            q
        )
        .is_err()
    );
    assert!(
        SnapScene::new(c, vec![a])
            .unwrap()
            .query(
                PlanContext {
                    view_id: Id::new(),
                    ..c
                },
                q
            )
            .is_err()
    );
}
#[test]
fn malformed_sources_queries_and_excessive_resources_fail_wholly() {
    let c = context();
    let s = segment();
    for bad in [
        SnapSegment { end: s.start, ..s },
        SnapSegment {
            start: Point2::new(f64::NAN, 0.0),
            ..s
        },
        SnapSegment {
            start: Point2::new(-f64::MAX, 0.0),
            end: Point2::new(f64::MAX, 0.0),
            ..s
        },
    ] {
        assert!(SnapScene::new(c, vec![s, bad]).is_err());
    }
    assert!(SnapScene::new(c, vec![s, s]).is_err());
    assert!(
        SnapScene::new(
            c,
            (0..10001)
                .map(|feature| SnapSegment { feature, ..s })
                .collect()
        )
        .is_err()
    );
    let scene = SnapScene::new(c, vec![s]).unwrap();
    for radius in [0.0, -1.0, 65.0, f64::NAN, f64::INFINITY] {
        assert!(
            scene
                .query(
                    c,
                    SnapQuery {
                        radius_pixels: radius,
                        ..query(Point2::default(), 100.0)
                    }
                )
                .is_err()
        );
    }
    assert!(
        scene
            .query(
                c,
                SnapQuery {
                    viewport: [0.0, 600.0],
                    ..query(Point2::default(), 100.0)
                }
            )
            .is_err()
    );
}
#[test]
fn nearest_points_use_local_arithmetic_at_large_coordinates() {
    let c = context();
    let s = SnapSegment {
        start: Point2::new(1_000_000.0, 1_000_000.0),
        end: Point2::new(1_000_006.0, 1_000_008.0),
        ..segment()
    };
    let scene = SnapScene::new(c, vec![s]).unwrap();
    let camera = PlanCamera {
        center: s.start,
        pixels_per_metre: 100.0,
    };
    let point = Point2::new(1_000_003.04, 1_000_003.97);
    let q = SnapQuery {
        camera,
        pointer: camera.project(point, [800.0, 600.0]).unwrap(),
        endpoints: false,
        midpoints: false,
        intersections: false,
        perpendicular_from: None,
        ..query(Point2::default(), 100.0)
    };
    let hit = scene.query(c, q).unwrap().candidate(c, q).unwrap().unwrap();
    assert!(hit.point.distance(Point2::new(1_000_003.0, 1_000_004.0)) < 1e-8);
    assert!((hit.distance_pixels - 5.0).abs() < 1e-7);
}
