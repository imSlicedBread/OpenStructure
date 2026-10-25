use os_core::{Id, Point2};
use os_geometry::plan::*;
use os_render::plan::*;
use std::collections::BTreeMap;

fn context() -> PlanContext {
    PlanContext {
        annotation_crop: None,
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

fn graphic(sweep_radians: f64) -> PlanArcLengthDimension {
    PlanArcLengthDimension {
        entity: Id::new(),
        center: Point2::new(0.0, 0.0),
        axis_radius: 2.0,
        radius: 2.5,
        start_radians: 0.4,
        sweep_radians,
        value_m: 2.0 * sweep_radians.abs(),
        orphan_hint: Point2::new(1.0, 2.0),
        diagnostic: None,
    }
}

#[test]
fn arc_length_strokes_and_values_handle_both_directions_and_major_sweeps() {
    for sweep in [
        std::f64::consts::FRAC_PI_2,
        -std::f64::consts::FRAC_PI_2,
        1.5 * std::f64::consts::PI,
        -1.5 * std::f64::consts::PI,
    ] {
        let ctx = context();
        let item = graphic(sweep);
        let lines = item.segments(ctx).unwrap();
        assert_eq!(lines.len(), ARC_LENGTH_SEGMENTS + 2);
        for (i, line) in lines.iter().take(ARC_LENGTH_SEGMENTS).enumerate() {
            let f0 = i as f64 / ARC_LENGTH_SEGMENTS as f64;
            let f1 = (i + 1) as f64 / ARC_LENGTH_SEGMENTS as f64;
            assert_eq!(line.0, item.point(f0));
            assert_eq!(line.1, item.point(f1));
            assert!((line.0.distance(item.center) - item.radius).abs() < 1e-12);
        }
        let start_axis = Point2::new(
            item.center.x + item.axis_radius * item.start_radians.cos(),
            item.center.y + item.axis_radius * item.start_radians.sin(),
        );
        let end_angle = item.start_radians + item.sweep_radians;
        let end_axis = Point2::new(
            item.center.x + item.axis_radius * end_angle.cos(),
            item.center.y + item.axis_radius * end_angle.sin(),
        );
        assert_eq!(lines[ARC_LENGTH_SEGMENTS], (start_axis, item.point(0.0)));
        assert_eq!(lines[ARC_LENGTH_SEGMENTS + 1], (end_axis, item.point(1.0)));
        assert_eq!(item.label(), format!("{:.3} m", item.value_m));
        item.validate().unwrap();
    }
}

#[test]
fn arc_length_crop_paint_geometry_and_screen_pick_share_clipped_strokes() {
    let base = context();
    let camera = PlanCamera::default();
    let size = [900.0, 700.0];
    let item = graphic(-1.5 * std::f64::consts::PI);
    let crop = PlanCrop {
        min: Point2::new(-3.0, -1.0),
        max: Point2::new(1.0, 3.0),
    };
    let ctx = PlanContext {
        crop: Some(crop),
        ..base
    };
    let clipped = item.segments(ctx).unwrap();
    assert!(!clipped.is_empty() && clipped.len() < ARC_LENGTH_SEGMENTS + 2);
    for (a, b) in &clipped {
        for p in [*a, *b] {
            assert!(p.x >= crop.min.x - 1e-10 && p.x <= crop.max.x + 1e-10);
            assert!(p.y >= crop.min.y - 1e-10 && p.y <= crop.max.y + 1e-10);
        }
    }
    let drawing = PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
        .unwrap()
        .with_arc_length_dimensions(vec![item.clone()])
        .unwrap();
    for (a, b) in clipped {
        let middle = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
        assert_eq!(
            drawing
                .pick_dimension_screen(
                    ctx,
                    camera,
                    size,
                    camera.project(middle, size).unwrap(),
                    2.0
                )
                .unwrap(),
            Some(item.entity)
        );
    }

    let outside = camera.project(Point2::new(1.01, 1.0), size).unwrap();
    assert_eq!(
        drawing
            .pick_dimension_screen(ctx, camera, size, outside, 0.1)
            .unwrap(),
        None
    );
    assert!(
        item.label_bounds(ctx, camera, size)
            .unwrap()
            .is_none_or(|bounds| {
                [
                    bounds[0],
                    bounds[1],
                    Point2::new(bounds[0].x, bounds[1].y),
                    Point2::new(bounds[1].x, bounds[0].y),
                ]
                .into_iter()
                .all(|screen| {
                    let world = camera.unproject(screen, size).unwrap();
                    world.x >= crop.min.x - 1e-9
                        && world.x <= crop.max.x + 1e-9
                        && world.y >= crop.min.y - 1e-9
                        && world.y <= crop.max.y + 1e-9
                })
            })
    );
}

#[test]
fn arc_length_graphics_reject_degenerate_or_unbounded_values() {
    let ctx = context();
    let mut item = graphic(std::f64::consts::PI);
    item.radius = 0.0;
    assert!(item.segments(ctx).is_err());
    item = graphic(std::f64::consts::PI);
    item.sweep_radians = std::f64::consts::TAU;
    assert!(item.segments(ctx).is_err());
    item = graphic(std::f64::consts::PI);
    item.value_m = f64::INFINITY;
    assert!(item.validate().is_err());
}
