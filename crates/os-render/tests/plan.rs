use os_core::{Id, Point2};
use os_geometry::{Profile, Solid, Transform, Vec3, plan::*};
use os_render::plan::*;
use std::collections::BTreeMap;

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
fn room_tag_bounds_crop_and_pick_share_stable_uuid() {
    let ctx = context();
    let camera = PlanCamera::default();
    let size = [800., 600.];
    let mut tag = PlanRoomTag {
        entity: Id::new(),
        view: ctx.view_id,
        room: Id::new(),
        anchor: Point2::new(0., 0.),
        label: "101 · Office".into(),
        diagnostic: None,
    };
    for label in ["101 · Office".to_string(), "x".repeat(500)] {
        tag.label = label;
        let [a, b] = tag.bounds(ctx, camera, size).unwrap().unwrap();
        assert!((48.0..=320.0).contains(&(b.x - a.x)));
        assert_eq!(b.y - a.y, 24.);
        let drawing = PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
            .unwrap()
            .with_room_tags(vec![tag.clone()])
            .unwrap();
        for point in [a, b, camera.project(tag.anchor, size).unwrap()] {
            assert_eq!(
                drawing
                    .pick_room_tag_screen(ctx, camera, size, point)
                    .unwrap(),
                Some(tag.entity)
            );
        }
        assert_eq!(
            drawing
                .pick_room_tag_screen(ctx, camera, size, Point2::new(b.x + 0.01, b.y))
                .unwrap(),
            None
        );
        // Derive crops around the exact graphic, respecting camera Y inversion.
        let p = camera.unproject(a, size).unwrap();
        let q = camera.unproject(b, size).unwrap();
        let crop = PlanCrop {
            min: Point2::new(p.x.min(q.x) - 0.1, p.y.min(q.y) - 0.1),
            max: Point2::new(p.x.max(q.x) + 0.1, p.y.max(q.y) + 0.1),
        };
        let cropped = PlanContext {
            crop: Some(crop),
            ..ctx
        };
        let drawing = PlanDrawing::from_prisms(cropped, &BTreeMap::new(), vec![])
            .unwrap()
            .with_room_tags(vec![tag.clone()])
            .unwrap();
        assert!(tag.bounds(cropped, camera, size).unwrap().is_some());
        assert_eq!(
            drawing
                .pick_room_tag_screen(
                    cropped,
                    camera,
                    size,
                    camera
                        .project(Point2::new(crop.max.x + 0.01, 0.), size)
                        .unwrap()
                )
                .unwrap(),
            None
        );
        let clipped = PlanContext {
            crop: Some(PlanCrop {
                max: Point2::new(q.x - 0.01, crop.max.y),
                ..crop
            }),
            ..ctx
        };
        let [clipped_a, clipped_b] = tag.bounds(clipped, camera, size).unwrap().unwrap();
        assert_eq!(clipped_a, a);
        assert_eq!(clipped_b.y, b.y);
        assert!((clipped_b.x - (b.x - 0.01 * camera.pixels_per_metre)).abs() < 1e-9);
        let drawing = PlanDrawing::from_prisms(clipped, &BTreeMap::new(), vec![])
            .unwrap()
            .with_room_tags(vec![tag.clone()])
            .unwrap();
        for point in [
            clipped_a,
            clipped_b,
            camera.project(tag.anchor, size).unwrap(),
        ] {
            assert_eq!(
                drawing
                    .pick_room_tag_screen(clipped, camera, size, point)
                    .unwrap(),
                Some(tag.entity)
            );
        }
        assert_eq!(
            drawing
                .pick_room_tag_screen(
                    clipped,
                    camera,
                    size,
                    Point2::new(clipped_b.x + 0.01, clipped_b.y)
                )
                .unwrap(),
            None
        );
        // The original badge overlaps this crop, but its anchor is outside.
        let outside = PlanContext {
            crop: Some(PlanCrop {
                min: Point2::new(0.01, crop.min.y),
                ..crop
            }),
            ..ctx
        };
        assert!(tag.bounds(outside, camera, size).unwrap().is_none());
        let drawing = PlanDrawing::from_prisms(outside, &BTreeMap::new(), vec![])
            .unwrap()
            .with_room_tags(vec![tag.clone()])
            .unwrap();
        assert_eq!(
            drawing
                .pick_room_tag_screen(
                    outside,
                    camera,
                    size,
                    camera.project(Point2::new(0.02, 0.0), size).unwrap()
                )
                .unwrap(),
            None
        );
    }
    tag.diagnostic = Some("Missing room".into());
    tag.label = "Room tag · Missing room".into();
    assert!(
        tag.hit(ctx, camera, size, camera.project(tag.anchor, size).unwrap())
            .unwrap()
    );
    assert!(
        tag.bounds(
            PlanContext {
                view_id: Id::new(),
                ..ctx
            },
            camera,
            size
        )
        .is_err()
    );
    assert!(
        tag.hit(ctx, camera, size, Point2::new(f64::NAN, 0.))
            .is_err()
    );
    assert!(
        PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
            .unwrap()
            .with_room_tags(vec![tag.clone(), tag])
            .is_err()
    );
}

#[test]
fn angular_arc_segments_crop_and_screen_pick_share_uuid() {
    let base = context();
    let camera = PlanCamera::default();
    let size = [800.0, 600.0];
    let graphic = PlanAngularDimension {
        entity: Id::new(),
        center: Point2::new(0.0, 0.0),
        radius: 2.0,
        start_radians: 0.0,
        sweep_radians: std::f64::consts::FRAC_PI_2,
        orphan_hint: Point2::new(1.0, 1.0),
        diagnostic: None,
    };
    let lines = graphic.segments(base).unwrap();
    assert_eq!(lines.len(), 66);
    for (i, (a, b)) in lines[..64].iter().enumerate() {
        assert_eq!(*a, graphic.point(i as f64 / 64.0));
        assert_eq!(*b, graphic.point((i + 1) as f64 / 64.0));
        assert!((a.distance(graphic.center) - 2.0).abs() < 1e-12);
    }
    assert_eq!(graphic.label(), "90.00°");
    for crop in [
        None,
        Some(PlanCrop {
            min: Point2::new(0.5, 0.5),
            max: Point2::new(1.8, 1.8),
        }),
    ] {
        let ctx = PlanContext { crop, ..base };
        let drawing = PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
            .unwrap()
            .with_angular_dimensions(vec![graphic.clone()])
            .unwrap();
        for (a, b) in graphic.segments(ctx).unwrap() {
            if let Some(c) = crop {
                for p in [a, b] {
                    assert!(
                        p.x >= c.min.x - 1e-10
                            && p.x <= c.max.x + 1e-10
                            && p.y >= c.min.y - 1e-10
                            && p.y <= c.max.y + 1e-10
                    );
                }
            }
            let mid = Point2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            assert_eq!(
                drawing
                    .pick_dimension_screen(
                        ctx,
                        camera,
                        size,
                        camera.project(mid, size).unwrap(),
                        2.0
                    )
                    .unwrap(),
                Some(graphic.entity)
            );
        }
        if crop.is_some() {
            let outside = camera.project(Point2::new(1.801, 0.8), size).unwrap();
            assert_eq!(
                drawing
                    .pick_dimension_screen(ctx, camera, size, outside, 10.0)
                    .unwrap(),
                None
            );
        } else {
            let [a, b] = graphic.label_bounds(ctx, camera, size).unwrap().unwrap();
            assert_eq!(
                drawing
                    .pick_dimension_screen(
                        ctx,
                        camera,
                        size,
                        Point2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
                        0.1
                    )
                    .unwrap(),
                Some(graphic.entity)
            );
        }
    }
}
fn room_signature() -> Vec<(Id, bool)> {
    vec![(Id::new(), true), (Id::new(), false), (Id::new(), true)]
}
fn prism(height: f64) -> Solid {
    Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(0.0, 0.0),
                Point2::new(5.0, 0.0),
                Point2::new(5.0, 1.0),
                Point2::new(0.0, 1.0),
            ],
        },
        height,
        transform: Transform {
            translation: Vec3::default(),
            rotation_z: 0.0,
        },
    }
}

#[test]
fn draw_order_and_clipped_picking_share_stable_semantic_ids() {
    let cut = Id::new();
    let projected = Id::new();
    let unavailable = Id::new();
    let context = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(1.0, 0.0),
            max: Point2::new(3.0, 1.0),
        }),
        ..context()
    };
    let drawing = PlanDrawing::from_prisms(
        context,
        &BTreeMap::from([(cut, prism(3.0)), (projected, prism(0.5))]),
        vec![unavailable],
    )
    .unwrap();
    assert_eq!(
        drawing
            .items(context)
            .unwrap()
            .iter()
            .map(|item| item.entity)
            .collect::<Vec<_>>(),
        [projected, cut]
    );
    assert_eq!(
        drawing.pick(context, Point2::new(2.0, 0.5)).unwrap(),
        Some(cut)
    );
    assert_eq!(drawing.pick(context, Point2::new(0.5, 0.5)).unwrap(), None);
    assert_eq!(drawing.unavailable(context).unwrap(), &[unavailable]);
}

#[test]
fn stale_context_and_invalid_candidates_cannot_be_consumed() {
    let context = context();
    let id = Id::new();
    let solids = BTreeMap::from([(id, prism(3.0))]);
    let drawing = PlanDrawing::from_prisms(context, &solids, vec![]).unwrap();
    for changed in [
        PlanContext {
            scale_denominator: 50.0,
            ..context
        },
        PlanContext {
            show_walls: false,
            ..context
        },
        PlanContext {
            show_extensions: false,
            ..context
        },
        PlanContext {
            session_id: Id::new(),
            ..context
        },
        PlanContext {
            model_revision: 1,
            ..context
        },
        PlanContext {
            view_id: Id::new(),
            ..context
        },
        PlanContext {
            settings_revision: 1,
            ..context
        },
        PlanContext {
            range: PlanRange {
                cut: 0.5,
                ..context.range
            },
            ..context
        },
        PlanContext {
            basis: HorizontalBasis {
                rotation: 1.0,
                ..context.basis
            },
            ..context
        },
    ] {
        assert!(drawing.items(changed).is_err());
        assert!(drawing.pick(changed, Point2::new(2.0, 0.5)).is_err());
        assert!(drawing.unavailable(changed).is_err());
    }
    let mut invalid = solids.clone();
    invalid.insert(Id::new(), prism(-1.0));
    assert!(PlanDrawing::from_prisms(context, &invalid, vec![]).is_err());
    assert!(PlanDrawing::from_prisms(context, &solids, vec![id]).is_err());
    assert!(
        PlanDrawing::from_prisms(context, &solids, vec![Id::new(); MAX_PLAN_ELEMENTS]).is_err()
    );
    assert_eq!(drawing.items(context).unwrap().len(), 1);
}

#[test]
fn room_plan_items_are_validated_picked_and_revision_bound() {
    let context = context();
    let room_id = Id::new();
    let room = PlanRoomItem {
        entity: room_id,
        number: "101".into(),
        name: "Living room".into(),
        seed: Point2::new(1.0, 1.0),
        boundary: vec![
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 3.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 3.0),
        ],
        area_m2: 10.0,
        boundary_signature: room_signature(),
        diagnostic: None,
    };
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_rooms(vec![room.clone()])
        .unwrap();
    assert_eq!(drawing.rooms(context).unwrap(), &[room]);
    assert_eq!(
        drawing.pick_room(context, Point2::new(1.0, 1.0)).unwrap(),
        Some(room_id)
    );
    assert_eq!(
        drawing.pick_room(context, Point2::new(6.0, 1.0)).unwrap(),
        None
    );
    assert_eq!(
        drawing.pick_room(context, Point2::new(2.0, 0.0)).unwrap(),
        Some(room_id)
    );
    assert!(
        drawing
            .rooms(PlanContext {
                model_revision: 1,
                ..context
            })
            .is_err()
    );
    assert!(
        drawing
            .pick_room(
                PlanContext {
                    view_id: Id::new(),
                    ..context
                },
                Point2::new(1.0, 1.0)
            )
            .is_err()
    );
}

#[test]
fn unresolved_room_keeps_its_identity_without_a_stale_boundary() {
    let context = context();
    let room_id = Id::new();
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_rooms(vec![PlanRoomItem {
            entity: room_id,
            number: "A".into(),
            name: "Studio".into(),
            seed: Point2::new(0.0, 0.0),
            boundary: vec![],
            area_m2: 0.0,
            boundary_signature: room_signature(),
            diagnostic: Some("Room is not enclosed".into()),
        }])
        .unwrap();
    assert_eq!(drawing.rooms(context).unwrap()[0].entity, room_id);
    assert_eq!(
        drawing.pick_room(context, Point2::new(0.0, 0.0)).unwrap(),
        None
    );
}

#[test]
fn malformed_room_plan_graphics_are_rejected() {
    let context = context();
    let room = |boundary, area_m2, diagnostic| PlanRoomItem {
        entity: Id::new(),
        number: "1".into(),
        name: "Room".into(),
        seed: Point2::new(0.5, 0.5),
        boundary,
        area_m2,
        boundary_signature: room_signature(),
        diagnostic,
    };
    let drawing = || PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap();
    assert!(
        drawing()
            .with_rooms(vec![room(vec![Point2::new(f64::NAN, 0.0); 3], 1.0, None)])
            .is_err()
    );
    assert!(
        drawing()
            .with_rooms(vec![room(
                vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)],
                0.0,
                None
            )])
            .is_err()
    );
    assert!(
        drawing()
            .with_rooms(vec![room(
                vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(1.0, 0.0),
                    Point2::new(1.0, 1.0),
                    Point2::new(0.0, 1.0),
                ],
                2.0,
                None
            )])
            .is_err()
    );
    assert!(
        drawing()
            .with_rooms(vec![room(vec![], 0.0, Some("bad\nlabel".into()))])
            .is_err()
    );
}

#[test]
fn room_graphics_keep_small_areas_at_large_plan_coordinates() {
    let context = context();
    let x = 999_000.0;
    let y = -999_000.0;
    let boundary = vec![
        Point2::new(x, y),
        Point2::new(x + 2.0, y),
        Point2::new(x + 2.0, y + 1.0),
        Point2::new(x, y + 1.0),
    ];
    let room = PlanRoomItem {
        entity: Id::new(),
        number: "1".into(),
        name: "Room".into(),
        seed: Point2::new(x + 1.0, y + 0.5),
        boundary: boundary.clone(),
        area_m2: 2.0,
        boundary_signature: room_signature(),
        diagnostic: None,
    };
    PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_rooms(vec![room])
        .expect("room area is independent of its absolute coordinates");

    let face = PlanRoomFace {
        boundary,
        area_m2: 2.0,
        boundary_signature: room_signature(),
    };
    PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_room_faces(vec![face])
        .expect("room face area is independent of its absolute coordinates");
}

#[test]
fn dimensions_pick_the_annotation_and_orphan_hint_at_logical_pixel_scale() {
    let context = context();
    let dimension = PlanDimensionItem {
        spans: Vec::new(),
        shared_start_witness: false,
        entity: Id::new(),
        witness_start: Point2::new(0.0, 0.0),
        witness_end: Point2::new(4.0, 0.0),
        line_start: Point2::new(0.0, 1.0),
        line_end: Point2::new(4.0, 1.0),
        value_m: Some(4.0),
        orphan_hint: Point2::new(2.0, 1.0),
        diagnostic: None,
    };
    let camera = PlanCamera::default();
    let size = [800.0, 600.0];
    let line_center = camera.project(Point2::new(2.0, 1.0), size).unwrap();
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_dimensions(vec![dimension.clone()])
        .unwrap();
    assert_eq!(
        drawing
            .pick_dimension_screen(context, camera, size, line_center, 6.0)
            .unwrap(),
        Some(dimension.entity)
    );
    assert!(
        drawing
            .dimensions(PlanContext {
                model_revision: 1,
                ..context
            })
            .is_err()
    );

    let cropped_context = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(-0.5, -0.5),
            max: Point2::new(0.5, 1.5),
        }),
        ..context
    };
    let outside = camera.project(Point2::new(3.0, 1.0), size).unwrap();
    assert_eq!(
        PlanDrawing::from_prisms(cropped_context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_dimensions(vec![dimension.clone()])
            .unwrap()
            .pick_dimension_screen(cropped_context, camera, size, outside, 6.0)
            .unwrap(),
        None,
        "a cropped annotation segment cannot be picked outside the crop"
    );
    let near_crop_edge = camera.project(Point2::new(0.55, 1.0), size).unwrap();
    assert_eq!(
        PlanDrawing::from_prisms(cropped_context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_dimensions(vec![dimension.clone()])
            .unwrap()
            .pick_dimension_screen(cropped_context, camera, size, near_crop_edge, 6.0)
            .unwrap(),
        None,
        "the screen-space pick radius must not leak beyond the crop boundary"
    );

    let orphan = PlanDimensionItem {
        spans: Vec::new(),
        shared_start_witness: false,
        entity: Id::new(),
        witness_start: Point2::new(2.0, 1.0),
        witness_end: Point2::new(2.0, 1.0),
        line_start: Point2::new(2.0, 1.0),
        line_end: Point2::new(2.0, 1.0),
        value_m: None,
        orphan_hint: Point2::new(7.0, -3.0),
        diagnostic: Some("A referenced wall no longer exists".into()),
    };
    let hint = camera.project(orphan.orphan_hint, size).unwrap();
    assert_eq!(
        PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_dimensions(vec![orphan.clone()])
            .unwrap()
            .pick_dimension_screen(context, camera, size, hint, 6.0)
            .unwrap(),
        Some(orphan.entity)
    );
}

#[test]
fn chained_dimension_spans_pick_one_identity_and_share_the_join_witness() {
    let context = context();
    let entity = Id::new();
    let first = PlanDimensionItem {
        spans: Vec::new(),
        shared_start_witness: false,
        entity,
        witness_start: Point2::new(0.0, 0.0),
        witness_end: Point2::new(4.0, 0.0),
        line_start: Point2::new(0.0, 1.0),
        line_end: Point2::new(4.0, 1.0),
        value_m: Some(4.0),
        orphan_hint: Point2::new(2.0, 1.0),
        diagnostic: None,
    };
    let second = PlanDimensionItem {
        spans: Vec::new(),
        shared_start_witness: true,
        entity,
        witness_start: Point2::new(4.0, 0.0),
        witness_end: Point2::new(5.0, 0.0),
        line_start: Point2::new(4.0, 1.0),
        line_end: Point2::new(5.0, 1.0),
        value_m: Some(1.0),
        orphan_hint: Point2::new(4.5, 1.0),
        diagnostic: None,
    };
    let mut dimension = first;
    dimension.spans.push(second.clone());
    assert_eq!(
        second.lines().count(),
        2,
        "the shared anchor gets one witness line"
    );
    let camera = PlanCamera::default();
    let viewport = [800.0, 600.0];
    let pointer = camera.project(Point2::new(4.5, 1.0), viewport).unwrap();
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new())
        .unwrap()
        .with_dimensions(vec![dimension])
        .unwrap();
    assert_eq!(
        drawing
            .pick_dimension_screen(context, camera, viewport, pointer, 6.0)
            .unwrap(),
        Some(entity),
        "every chain span selects the single persisted annotation"
    );
}

#[test]
fn room_face_placement_requires_one_strict_interior_face_and_current_context() {
    let context = context();
    let left = PlanRoomFace {
        boundary: vec![
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 2.0),
        ],
        area_m2: 4.0,
        boundary_signature: room_signature(),
    };
    let right = PlanRoomFace {
        boundary: vec![
            Point2::new(2.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 2.0),
            Point2::new(2.0, 2.0),
        ],
        area_m2: 4.0,
        boundary_signature: room_signature(),
    };
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_room_faces(vec![left.clone(), right.clone()])
        .unwrap();
    assert_eq!(
        drawing
            .room_face_at(context, Point2::new(1.0, 1.0))
            .unwrap(),
        Some(&left)
    );
    assert_eq!(
        drawing
            .room_face_at(context, Point2::new(3.0, 1.0))
            .unwrap(),
        Some(&right)
    );
    assert_eq!(
        drawing
            .room_face_at(context, Point2::new(2.0, 1.0))
            .unwrap(),
        None
    );
    assert_eq!(
        drawing
            .room_face_at(context, Point2::new(8.0, 1.0))
            .unwrap(),
        None
    );
    assert!(
        drawing
            .room_face_at(
                PlanContext {
                    model_revision: 1,
                    ..context
                },
                Point2::new(1.0, 1.0)
            )
            .is_err()
    );
    let invalid = PlanRoomFace {
        area_m2: 3.0,
        ..left
    };
    assert!(
        PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_room_faces(vec![invalid])
            .is_err()
    );
}

#[test]
fn screen_plane_round_trip_pan_and_cursor_zoom_are_atomic() {
    for size in [[640.0, 480.0], [1920.0, 1080.0]] {
        for rotation in [0.0, 0.37, std::f64::consts::FRAC_PI_2] {
            let basis = HorizontalBasis {
                origin: Point2::new(1e6, -2e6),
                rotation,
            };
            let world = Point2::new(1e6 + 5.0, -2e6 + 2.0);
            let plane = basis.world_to_plane(world).unwrap();
            let mut camera = PlanCamera {
                center: plane,
                ..Default::default()
            };
            let screen = camera.project(plane, size).unwrap();
            let recovered = basis
                .plane_to_world(camera.unproject(screen, size).unwrap())
                .unwrap();
            assert!(world.distance(recovered) < 1e-8);
            let cursor = Point2::new(175.0, 120.0);
            let anchor = camera.unproject(cursor, size).unwrap();
            camera.zoom_at(cursor, 2.0, size).unwrap();
            assert!(camera.unproject(cursor, size).unwrap().distance(anchor) < 1e-12);
            let prior_screen = camera.project(plane, size).unwrap();
            camera.pan(Point2::new(32.0, -18.0), size).unwrap();
            assert!(
                camera
                    .project(plane, size)
                    .unwrap()
                    .distance(Point2::new(prior_screen.x + 32.0, prior_screen.y - 18.0))
                    < 1e-9
            );
            let before = camera;
            assert!(camera.zoom_at(cursor, f64::NAN, size).is_err());
            assert!(camera.pan(Point2::new(f64::INFINITY, 0.0), size).is_err());
            assert!(camera.zoom_at(cursor, 2.0, [0.0, 480.0]).is_err());
            assert_eq!(camera, before);
        }
    }
}

#[test]
fn concave_floor_fill_picking_and_crop_are_context_bound() {
    let context = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(0.0, 0.0),
            max: Point2::new(2.0, 3.0),
        }),
        ..context()
    };
    let boundary = [
        Point2::new(0.0, 0.0),
        Point2::new(3.0, 0.0),
        Point2::new(3.0, 1.0),
        Point2::new(1.0, 1.0),
        Point2::new(1.0, 3.0),
        Point2::new(0.0, 3.0),
    ]
    .to_vec();
    let floor = PlanFloorItem {
        entity: Id::new(),
        area_m2: 5.0,
        triangles: os_geometry::floors::triangulate_floor(&boundary).unwrap(),
        boundary,
    };
    let floor_id = floor.entity;
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_floors(vec![floor])
        .unwrap();
    assert_eq!(
        drawing.pick_floor(context, Point2::new(0.5, 2.0)).unwrap(),
        Some(floor_id)
    );
    assert_eq!(
        drawing.pick_floor(context, Point2::new(2.0, 2.0)).unwrap(),
        None
    );
    assert_eq!(
        drawing.pick_floor(context, Point2::new(2.5, 0.5)).unwrap(),
        None
    );
    assert!(
        drawing
            .floors(PlanContext {
                model_revision: 1,
                ..context
            })
            .is_err()
    );

    let mut invalid = drawing.floors(context).unwrap()[0].clone();
    invalid.triangles.swap(0, 1);
    assert!(
        PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_floors(vec![invalid])
            .is_err()
    );
}
