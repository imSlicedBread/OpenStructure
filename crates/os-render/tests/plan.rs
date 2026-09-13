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
