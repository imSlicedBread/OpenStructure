use os_core::{Id, Point2};
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange};
use os_render::{
    plan::{PlanCamera, PlanContext, PlanDrawing, PlanRoomSeparationLine},
    snapping::{SnapKind, SnapQuery},
};
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
            min: Point2::new(-1.0, -1.0),
            max: Point2::new(1.0, 1.0),
        }),
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}

#[test]
fn room_separator_crop_paint_pick_and_snap_share_visible_geometry() {
    let context = context();
    let camera = PlanCamera::default();
    let viewport = [800.0, 600.0];
    let line = PlanRoomSeparationLine {
        entity: Id::new(),
        view: context.view_id,
        level: Id::new(),
        start: Point2::new(-3.0, 0.0),
        end: Point2::new(3.0, 0.0),
    };
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_room_separation_lines(line.level, vec![line.clone()])
        .unwrap()
        .with_snap_segments(vec![])
        .unwrap();

    let clipped = drawing.room_separation_lines(context).unwrap();
    assert_eq!(clipped.len(), 1);
    assert_eq!(clipped[0].start, Point2::new(-1.0, 0.0));
    assert_eq!(clipped[0].end, Point2::new(1.0, 0.0));

    let visible_pointer = camera.project(Point2::new(0.04, 0.0), viewport).unwrap();
    assert_eq!(
        drawing
            .pick_room_separation_line_screen(context, camera, viewport, visible_pointer, 6.0)
            .unwrap(),
        Some(line.entity)
    );
    let query = SnapQuery {
        camera,
        viewport,
        pointer: visible_pointer,
        radius_pixels: 12.0,
        endpoints: true,
        midpoints: true,
        intersections: false,
        perpendicular_from: None,
        nearest: true,
        axis_extensions: false,
        exclude_entity: None,
    };
    let snap = drawing
        .snap(context, query)
        .unwrap()
        .candidate(context, query)
        .unwrap()
        .unwrap();
    assert_eq!(snap.entity, line.entity);
    assert_eq!(snap.kind, SnapKind::Midpoint);
    assert_eq!(snap.point, Point2::new(0.0, 0.0));

    let outside = camera.project(Point2::new(1.05, 0.0), viewport).unwrap();
    assert_eq!(
        drawing
            .pick_room_separation_line_screen(context, camera, viewport, outside, 6.0)
            .unwrap(),
        None
    );
    // A nearby pointer just outside the crop may acquire the visible clipped
    // endpoint, but the candidate itself must remain on the visible segment.
    let edge_query = SnapQuery {
        pointer: outside,
        ..query
    };
    let edge_snap = drawing
        .snap(context, edge_query)
        .unwrap()
        .candidate(context, edge_query)
        .unwrap()
        .unwrap();
    assert_eq!(edge_snap.entity, line.entity);
    assert_eq!(edge_snap.point, Point2::new(1.0, 0.0));

    let far_outside = camera.project(Point2::new(1.5, 0.0), viewport).unwrap();
    let no_hit_query = SnapQuery {
        pointer: far_outside,
        ..query
    };
    assert!(
        drawing
            .snap(context, no_hit_query)
            .unwrap()
            .candidate(context, no_hit_query)
            .unwrap()
            .is_none()
    );
}
