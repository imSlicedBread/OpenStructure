use os_core::{Id, Point2};
use os_geometry::{SurfaceIdentity, plan::*, *};
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

fn column(center: Point2, base: f64, height: f64) -> Solid {
    Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(-0.2, -0.3),
                Point2::new(0.2, -0.3),
                Point2::new(0.2, 0.3),
                Point2::new(-0.2, 0.3),
            ],
        },
        height,
        transform: Transform {
            translation: Vec3::new(center.x, center.y, base),
            rotation_z: 0.0,
        },
    }
}

#[test]
fn column_plan_uses_vertical_cut_projection_crop_basis_and_interior_pick() {
    let id = Id::new();
    let solid = column(Point2::new(2.0, 3.0), 0.15, 3.2);
    let surface = SurfaceIdentity {
        layer: None,
        material: None,
    };
    let ctx = context();
    let drawing = PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
        .unwrap()
        .with_columns(&[(id, solid.clone(), surface)])
        .unwrap();
    let item = &drawing.columns(ctx).unwrap()[0];
    assert_eq!(item.entity, id);
    assert_eq!(item.footprint.role, PlanRole::Cut);
    assert_eq!(item.footprint.vertices().len(), 4);
    assert_eq!(
        drawing.pick_column(ctx, Point2::new(2.0, 3.0)).unwrap(),
        Some(id)
    );
    assert_eq!(
        drawing.pick_column(ctx, Point2::new(4.0, 3.0)).unwrap(),
        None
    );

    let projected_context = PlanContext {
        range: PlanRange {
            top: 2.5,
            cut: 1.2,
            bottom: 0.0,
            depth: -1.0,
        },
        ..ctx
    };
    // Projection follows the current native plan policy: geometry below the
    // cut and above the view bottom is projected; overhead inference is later.
    let projected = column(Point2::new(0.0, 0.0), 0.0, 1.0);
    let drawing = PlanDrawing::from_prisms(projected_context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_columns(&[(id, projected, surface)])
        .unwrap();
    assert_eq!(
        drawing.columns(projected_context).unwrap()[0]
            .footprint
            .role,
        PlanRole::Projected
    );

    let rotated_context = PlanContext {
        basis: HorizontalBasis {
            origin: Point2::new(1.0, 1.0),
            rotation: std::f64::consts::FRAC_PI_2,
        },
        crop: Some(PlanCrop {
            min: Point2::new(1.6, -1.4),
            max: Point2::new(2.4, -0.6),
        }),
        ..ctx
    };
    let drawing = PlanDrawing::from_prisms(rotated_context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_columns(&[(id, solid, surface)])
        .unwrap();
    let item = &drawing.columns(rotated_context).unwrap()[0];
    assert!(item.footprint.vertices().iter().all(|p| {
        p.x >= rotated_context.crop.unwrap().min.x - 1e-9
            && p.x <= rotated_context.crop.unwrap().max.x + 1e-9
            && p.y >= rotated_context.crop.unwrap().min.y - 1e-9
            && p.y <= rotated_context.crop.unwrap().max.y + 1e-9
    }));
    let (x, y) = item
        .footprint
        .vertices()
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
    let centroid = Point2::new(
        x / item.footprint.vertices().len() as f64,
        y / item.footprint.vertices().len() as f64,
    );
    let expected = rotated_context
        .basis
        .world_to_plane(Point2::new(2.0, 3.0))
        .unwrap();
    assert!(centroid.distance(expected) < 1e-9);
    assert_eq!(
        drawing.pick_column(rotated_context, centroid).unwrap(),
        Some(id)
    );

    let clipped_context = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(1.8, -1.3),
            max: Point2::new(2.1, -0.7),
        }),
        ..rotated_context
    };
    let clipped = PlanDrawing::from_prisms(clipped_context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_columns(&[(id, column(Point2::new(2.0, 3.0), 0.15, 3.2), surface)])
        .unwrap();
    let clipped_item = &clipped.columns(clipped_context).unwrap()[0];
    assert!(
        clipped_item
            .footprint
            .vertices()
            .iter()
            .all(|p| p.x <= 2.1 + 1e-9 && p.y >= -1.3 - 1e-9 && p.y <= -0.7 + 1e-9)
    );
    assert!(
        clipped_item
            .footprint
            .vertices()
            .iter()
            .any(|p| (p.x - 2.1).abs() < 1e-9)
    );
}

#[test]
fn columns_above_and_below_view_depth_are_hidden_and_duplicate_ids_rejected() {
    let ctx = context();
    let id = Id::new();
    let surface = SurfaceIdentity::default();
    let above = column(Point2::new(0.0, 0.0), 4.0, 1.0);
    let below = column(Point2::new(0.0, 0.0), -2.0, 0.5);
    let drawing = PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
        .unwrap()
        .with_columns(&[(id, above, surface), (Id::new(), below, surface)])
        .unwrap();
    assert!(drawing.columns(ctx).unwrap().is_empty());
    assert!(
        PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
            .unwrap()
            .with_columns(&[
                (id, column(Point2::new(0.0, 0.0), 0.0, 3.0), surface),
                (id, column(Point2::new(2.0, 0.0), 0.0, 3.0), surface)
            ])
            .is_err()
    );
}
