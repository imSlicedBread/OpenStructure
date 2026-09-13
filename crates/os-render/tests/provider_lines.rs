use os_core::{Id, Point2};
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange, PlanRole};
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
fn line(entity: Id) -> PlanLine {
    PlanLine {
        entity,
        feature: 7,
        start: Point2::new(0.0, 0.0),
        end: Point2::new(10.0, 0.0),
        role: PlanRole::Cut,
    }
}
fn drawing(
    c: PlanContext,
    ids: Vec<Id>,
    lines: BTreeMap<Id, Vec<PlanLine>>,
) -> os_core::Result<PlanDrawing> {
    PlanDrawing::from_prisms(c, &BTreeMap::new(), ids)?
        .with_provider_lines(lines)?
        .with_snap_segments(vec![])
}
fn query(p: Point2) -> SnapQuery {
    let camera = PlanCamera {
        center: Point2::new(5.0, 0.0),
        pixels_per_metre: 100.0,
    };
    SnapQuery {
        camera,
        viewport: [800.0, 600.0],
        pointer: camera.project(p, [800.0, 600.0]).unwrap(),
        radius_pixels: 6.0,
        endpoints: true,
        midpoints: false,
        intersections: false,
        perpendicular_from: None,
        nearest: false,
        axis_extensions: false,
        exclude_entity: None,
    }
}

#[test]
fn provider_clipping_picking_and_snapping_keep_distinct_semantic_endpoints() {
    let c = context();
    let id = Id::new();
    let absent = Id::new();
    let d = drawing(c, vec![id, absent], [(id, vec![line(id)])].into()).unwrap();
    assert_eq!(d.unavailable(c).unwrap(), &[absent]);
    let shown = &d.provider_lines(c).unwrap()[0];
    assert_eq!(shown.start, Point2::new(2.0, 0.0));
    assert_eq!(shown.end, Point2::new(8.0, 0.0));
    let mut q = query(Point2::new(2.0, 0.0));
    assert!(
        d.snap(c, q).unwrap().candidate(c, q).unwrap().is_none(),
        "crop did not create an endpoint"
    );
    q.nearest = true;
    let hit = d.snap(c, q).unwrap().candidate(c, q).unwrap().unwrap();
    assert_eq!(
        (hit.entity, hit.feature, hit.kind),
        (id, 7, SnapKind::Nearest)
    );
    assert_eq!(hit.point, shown.start);
    assert_eq!(
        d.pick_screen(c, q.camera, q.viewport, q.pointer, 6.0)
            .unwrap(),
        Some(id)
    );
    let outside = query(Point2::new(1.0, 0.0));
    assert_eq!(
        d.pick_screen(c, outside.camera, outside.viewport, outside.pointer, 6.0)
            .unwrap(),
        None
    );
    q.exclude_entity = Some(id);
    assert!(d.snap(c, q).unwrap().candidate(c, q).unwrap().is_none());
    let stale = PlanContext {
        settings_revision: 1,
        ..c
    };
    assert!(d.provider_lines(stale).is_err());
    assert!(
        d.pick_screen(stale, q.camera, q.viewport, q.pointer, 6.0)
            .is_err()
    );
    assert!(d.snap(stale, q).is_err());
}

#[test]
fn provider_draw_order_and_reverse_picking_are_deterministic() {
    let c = context();
    let a = Id::new();
    let b = Id::new();
    let mut low = line(a);
    low.role = PlanRole::Depth;
    let high = line(b);
    let d = drawing(c, vec![a, b], [(a, vec![low]), (b, vec![high])].into()).unwrap();
    assert_eq!(
        d.provider_lines(c)
            .unwrap()
            .iter()
            .map(|l| l.entity)
            .collect::<Vec<_>>(),
        vec![a, b]
    );
    let q = query(Point2::new(5.0, 0.0));
    assert_eq!(
        d.pick_screen(c, q.camera, q.viewport, q.pointer, 6.0)
            .unwrap(),
        Some(b)
    );
}

#[test]
fn invalid_or_excessive_provider_batches_are_not_partial_drawings() {
    let c = context();
    let id = Id::new();
    for lines in [
        vec![line(id), line(id)],
        vec![PlanLine {
            entity: Id::new(),
            ..line(id)
        }],
        vec![PlanLine {
            end: Point2::new(f64::NAN, 0.0),
            ..line(id)
        }],
        vec![PlanLine {
            end: Point2::new(0.0, 0.0),
            ..line(id)
        }],
        (0..257)
            .map(|feature| PlanLine {
                feature,
                ..line(id)
            })
            .collect(),
    ] {
        assert!(drawing(c, vec![id], [(id, lines)].into()).is_err());
    }
    assert!(drawing(c, vec![], [(id, vec![line(id)])].into()).is_err());
    let d = drawing(c, vec![id], [(id, vec![])].into()).unwrap();
    assert!(d.unavailable(c).unwrap().is_empty());
    assert!(d.provider_lines(c).unwrap().is_empty());
    let ids: Vec<_> = (0..40).map(|_| Id::new()).collect();
    let batch = ids
        .iter()
        .map(|id| {
            (
                *id,
                (0..256)
                    .map(|feature| PlanLine {
                        feature,
                        start: Point2::new(100.0, 100.0),
                        end: Point2::new(101.0, 100.0),
                        ..line(*id)
                    })
                    .collect(),
            )
        })
        .collect();
    assert!(
        drawing(c, ids, batch).is_err(),
        "cropped-away input still counts against budget"
    );
}
