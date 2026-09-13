use os_core::{Id, Point2};
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange};
use os_render::{plan::*, snapping::*};
use std::collections::BTreeMap;

fn context() -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: Some(PlanCrop {
            min: Point2::new(2.0, -1.0),
            max: Point2::new(8.0, 1.0),
        }),
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}
fn grid() -> PlanGrid {
    PlanGrid {
        entity: Id::new(),
        name: "A".into(),
        start: Point2::new(0.0, 0.0),
        end: Point2::new(10.0, 0.0),
    }
}
fn query(point: Point2, zoom: f64) -> SnapQuery {
    let camera = PlanCamera {
        center: Point2::new(5.0, 0.0),
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
fn drawing(context: PlanContext, grids: Vec<PlanGrid>) -> os_core::Result<PlanDrawing> {
    PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])?
        .with_grids(grids)?
        .with_snap_segments(vec![])
}

#[test]
fn cropped_grid_axis_snaps_and_picks_use_original_features_and_screen_tolerance() {
    let context = context();
    let grid = grid();
    let drawing = drawing(context, vec![grid.clone()]).unwrap();
    let shown = &drawing.grids(context).unwrap()[0];
    assert_eq!(shown.start, Point2::new(2.0, 0.0));
    assert_eq!(shown.end, Point2::new(8.0, 0.0));
    assert_eq!(shown.entity, grid.entity);
    for zoom in [1.0, 100.0, 10000.0] {
        let mut q = query(Point2::new(3.0, 0.0), zoom);
        q.pointer.y += 5.0;
        q.endpoints = false;
        q.midpoints = false;
        let hit = drawing
            .snap(context, q)
            .unwrap()
            .candidate(context, q)
            .unwrap()
            .unwrap();
        assert_eq!(hit.kind, SnapKind::GridAxis);
        assert_eq!(hit.entity, grid.entity);
        assert!((hit.distance_pixels - 5.0).abs() < 1e-9);
        assert_eq!(
            drawing
                .pick_screen(context, q.camera, q.viewport, q.pointer, 6.0)
                .unwrap(),
            Some(grid.entity)
        );
        q.pointer.y += 2.0;
        assert_eq!(
            drawing
                .pick_screen(context, q.camera, q.viewport, q.pointer, 6.0)
                .unwrap(),
            None
        );
    }
    let mut q = query(Point2::new(2.0, 0.0), 100.0);
    q.midpoints = false;
    q.nearest = false;
    assert!(
        drawing
            .snap(context, q)
            .unwrap()
            .candidate(context, q)
            .unwrap()
            .is_none()
    );
    q.nearest = true;
    assert_eq!(
        drawing
            .snap(context, q)
            .unwrap()
            .candidate(context, q)
            .unwrap()
            .unwrap()
            .kind,
        SnapKind::GridAxis
    );
    let q = query(Point2::new(5.0, 0.02), 100.0);
    assert_eq!(
        drawing
            .snap(context, q)
            .unwrap()
            .candidate(context, q)
            .unwrap()
            .unwrap()
            .kind,
        SnapKind::Midpoint
    );
    let stale = PlanContext {
        model_revision: 1,
        ..context
    };
    assert!(drawing.grids(stale).is_err());
    assert!(drawing.snap(stale, q).is_err());
    assert!(
        drawing
            .pick_screen(stale, q.camera, q.viewport, q.pointer, 6.0)
            .is_err()
    );
}

#[test]
fn hidden_corner_and_parallel_grids_never_supply_snaps() {
    let c = context();
    for (start, end) in [
        (Point2::new(0.0, 2.0), Point2::new(10.0, 2.0)),
        (Point2::new(1.0, 0.0), Point2::new(3.0, 2.0)),
    ] {
        let d = drawing(
            c,
            vec![PlanGrid {
                start,
                end,
                ..grid()
            }],
        )
        .unwrap();
        assert!(d.grids(c).unwrap().is_empty());
        let q = query(Point2::new(2.0, 1.0), 100.0);
        assert!(d.snap(c, q).unwrap().candidate(c, q).unwrap().is_none());
    }
}

#[test]
fn grid_drawing_identity_budget_geometry_and_label_checks() {
    let c = context();
    let g = grid();
    assert!(drawing(c, vec![g.clone(), g.clone()]).is_err());
    assert!(
        PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![g.entity])
            .unwrap()
            .with_grids(vec![g.clone()])
            .is_err()
    );
    for case in 0..4 {
        let mut bad = g.clone();
        match case {
            0 => bad.end = bad.start,
            1 => bad.start.x = f64::INFINITY,
            2 => bad.name = "bad\nname".into(),
            _ => bad.name = "x".repeat(257),
        }
        assert!(drawing(c, vec![bad]).is_err());
    }
    assert!(drawing(c, (0..10001).map(|_| grid()).collect()).is_err());
    assert!(
        drawing(c, vec![g.clone()])
            .unwrap()
            .with_grids(vec![grid()])
            .is_err()
    );
    let a = grid();
    let b = grid();
    let first = drawing(c, vec![a.clone(), b.clone()]).unwrap();
    let second = drawing(c, vec![b, a]).unwrap();
    assert_eq!(first.grids(c).unwrap(), second.grids(c).unwrap());
    let q = query(Point2::new(3.0, 0.0), 100.0);
    assert_eq!(
        first.snap(c, q).unwrap().candidate(c, q).unwrap(),
        second.snap(c, q).unwrap().candidate(c, q).unwrap()
    );
}
