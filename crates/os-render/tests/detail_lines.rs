use os_core::{Id, Point2};
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange};
use os_render::plan::{
    DETAIL_LINE_WEIGHT_MM, PlanCamera, PlanContext, PlanDetailLine, PlanDrawing,
};
use os_render::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};
use os_render::snapping::{SnapKind, SnapQuery};
use std::collections::BTreeMap;

fn context(crop: Option<PlanCrop>) -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop,
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}

fn drawing(context: PlanContext, id: Id) -> PlanDrawing {
    PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_detail_lines(vec![PlanDetailLine {
            entity: id,
            view: context.view_id,
            start: Point2::new(-2.0, 0.0),
            end: Point2::new(2.0, 0.0),
        }])
        .unwrap()
        .with_snap_segments(vec![])
        .unwrap()
}

#[test]
fn crop_pick_and_endpoint_snapping_use_visible_detail_segment() {
    let crop = PlanCrop {
        min: Point2::new(-1.0, -1.0),
        max: Point2::new(1.0, 1.0),
    };
    let context = context(Some(crop));
    let id = Id::new();
    let drawing = drawing(context, id);
    let line = &drawing.detail_lines(context).unwrap()[0];
    assert_eq!(
        (line.start, line.end),
        (Point2::new(-1.0, 0.0), Point2::new(1.0, 0.0))
    );
    let camera = PlanCamera::default();
    let viewport = [800.0, 600.0];
    let mid = camera.project(Point2::new(0.0, 0.0), viewport).unwrap();
    assert_eq!(
        drawing
            .pick_detail_line_screen(context, camera, viewport, mid, 6.0)
            .unwrap(),
        Some(id)
    );
    let outside = camera.project(Point2::new(1.2, 0.0), viewport).unwrap();
    assert_eq!(
        drawing
            .pick_detail_line_screen(context, camera, viewport, outside, 6.0)
            .unwrap(),
        None
    );
    let uncropped = self::context(None);
    let whole = self::drawing(uncropped, id);
    let query = SnapQuery {
        camera,
        viewport,
        pointer: camera.project(Point2::new(-2.0, 0.0), viewport).unwrap(),
        radius_pixels: 12.0,
        endpoints: true,
        midpoints: false,
        intersections: false,
        perpendicular_from: None,
        nearest: false,
        axis_extensions: false,
        exclude_entity: None,
    };
    let snapped = whole
        .snap(uncropped, query)
        .unwrap()
        .candidate(uncropped, query)
        .unwrap()
        .unwrap();
    assert_eq!(
        (snapped.entity, snapped.kind, snapped.point),
        (id, SnapKind::Endpoint, Point2::new(-2.0, 0.0))
    );
    let excluded = SnapQuery {
        exclude_entity: Some(id),
        ..query
    };
    assert!(
        whole
            .snap(uncropped, excluded)
            .unwrap()
            .candidate(uncropped, excluded)
            .unwrap()
            .is_none()
    );
    let stale = PlanContext {
        model_revision: 1,
        ..context
    };
    assert!(drawing.detail_lines(stale).is_err());
}

#[test]
fn sheet_and_pdf_keep_detail_line_as_vector_stroke_at_documented_weight() {
    let context = context(None);
    let id = Id::new();
    let drawing = drawing(context, id);
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.0,
            height_mm: 297.0,
            number: "A101",
            name: "Details",
        },
        "Ground detail",
        PaperViewport {
            center_mm: Point2::new(210.0, 125.0),
            width_mm: 390.0,
            height_mm: 230.0,
            model_center_m: Point2::new(0.0, 0.0),
            scale_denominator: 100.0,
        },
        context,
        &drawing,
    )
    .unwrap();
    let segment = page
        .marks()
        .iter()
        .find_map(|mark| match &mark.kind {
            PaperMarkKind::Path {
                points_mm,
                closed: false,
                stroke: Some(stroke),
                ..
            } if points_mm.len() == 2
                && points_mm[0] == Point2::new(190.0, 125.0)
                && points_mm[1] == Point2::new(230.0, 125.0) =>
            {
                Some(stroke)
            }
            _ => None,
        })
        .expect("detail segment on sheet");
    assert_eq!(segment.width_mm, DETAIL_LINE_WEIGHT_MM);
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    assert!(
        pdf.contains("<41313031> Tj"),
        "sheet number is searchable text"
    );
    assert!(
        pdf.contains("0.708661 w"),
        "0.25 mm detail stroke is preserved in PDF points"
    );
}
