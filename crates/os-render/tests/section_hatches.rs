use os_core::{Id, Point2};
use os_geometry::{SurfaceIdentity, plan::*, section::VerticalSectionPlane};
use os_model::MaterialPatternParams;
use os_render::{plan::*, sheet::*};
use std::collections::BTreeMap;

fn context() -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 1,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 100.,
        show_walls: true,
        show_extensions: true,
    }
}
fn plane() -> VerticalSectionPlane {
    VerticalSectionPlane {
        origin: Point2::default(),
        direction: Point2::new(1., 0.),
    }
}
fn pattern() -> MaterialPatternParams {
    MaterialPatternParams {
        name: "Hatch".into(),
        angle_degrees: 0.,
        spacing: 0.25,
        origin: [0., 0., 0.125],
    }
}
fn rect(material: Option<Id>, x: f64, y: f64, w: f64, h: f64) -> PlanSectionFill {
    let [a, b, c, d] = [
        Point2::new(x, y),
        Point2::new(x + w, y),
        Point2::new(x + w, y + h),
        Point2::new(x, y + h),
    ];
    PlanSectionFill {
        entity: Id::new(),
        component: 0,
        surface: SurfaceIdentity {
            material,
            layer: None,
        },
        triangles: vec![[a, b, c], [a, c, d]],
    }
}
fn drawing(ctx: PlanContext, fills: Vec<PlanSectionFill>) -> PlanDrawing {
    PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![])
        .unwrap()
        .with_section_fills(fills)
        .unwrap()
        .with_section_strokes()
}

#[test]
fn cut_hatches_merge_triangle_seams_preserve_holes_components_and_masking() {
    let ctx = context();
    let material = Id::new();
    let mut region = rect(Some(material), 0., 0., 3., 1.);
    // A real central void: use two disjoint strips with one component identity.
    region
        .triangles
        .extend(rect(Some(material), 0., 2., 3., 1.).triangles);
    region
        .triangles
        .extend(rect(Some(material), 0., 1., 1., 1.).triangles);
    region
        .triangles
        .extend(rect(Some(material), 2., 1., 1., 1.).triangles);
    let d = drawing(ctx, vec![region, rect(None, 2.5, 0., 0.5, 3.)])
        .with_section_hatches(BTreeMap::from([(material, pattern())]), plane())
        .unwrap();
    let lines = d.section_hatches(ctx).unwrap();
    assert_eq!(
        lines.len(),
        16,
        "one span per uninterrupted region, no tessellation strokes"
    );
    for line in lines {
        assert!(line.end.x <= 2.5 && line.start.x >= 0.);
        assert_eq!(line.start.y, line.end.y);
        assert!(((line.start.y - 0.125) / 0.25).fract().abs() < 1e-12);
        if (1.0..2.0).contains(&line.start.y) {
            assert!(line.end.x <= 1. || line.start.x >= 2., "void remains clear");
        }
    }
    assert!(
        d.section_hatches(PlanContext {
            model_revision: 2,
            ..ctx
        })
        .is_err()
    );
    assert!(
        d.with_section_fills(vec![])
            .unwrap()
            .section_hatches(ctx)
            .unwrap()
            .is_empty()
    );
    assert!(
        drawing(ctx, vec![rect(None, 0., 0., 1., 1.)])
            .section_hatches(ctx)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cut_hatches_rotated_plane_shared_origin_components_crop_and_scale() {
    let ctx = context();
    let material = Id::new();
    let p = MaterialPatternParams {
        angle_degrees: 90.,
        origin: [12., 23., 0.],
        ..pattern()
    };
    let plane = VerticalSectionPlane {
        origin: Point2::new(10., 20.),
        direction: Point2::new(3., 4.),
    };
    // Projected origin station is 3.6 m for the rotated section.
    let mut first = rect(Some(material), 0., 0., 2., 1.);
    first.component = 7;
    let mut second = rect(Some(material), 2., 0., 2., 1.);
    second.component = 8;
    let fills = vec![first, second];
    let d = drawing(ctx, fills.clone())
        .with_section_hatches(BTreeMap::from([(material, p.clone())]), plane)
        .unwrap();
    for h in d.section_hatches(ctx).unwrap() {
        let n = (h.start.x - 3.6) / 0.25;
        assert!((n - n.round()).abs() < 1e-12);
    }
    let crop = PlanCrop {
        min: Point2::new(1., 0.2),
        max: Point2::new(3., 0.8),
    };
    let cropped = PlanContext {
        crop: Some(crop),
        scale_denominator: 50.,
        ..ctx
    };
    let d2 = drawing(cropped, fills)
        .with_section_hatches(BTreeMap::from([(material, p)]), plane)
        .unwrap();
    assert!(!d2.section_hatches(cropped).unwrap().is_empty());
    for h in d2.section_hatches(cropped).unwrap() {
        for pt in [h.start, h.end] {
            assert!(pt.x >= crop.min.x - 1e-12 && pt.x <= crop.max.x + 1e-12);
            assert!(pt.y >= crop.min.y - 1e-12 && pt.y <= crop.max.y + 1e-12);
        }
        assert!(
            d.section_hatches(ctx)
                .unwrap()
                .iter()
                .any(|old| (old.start.x - h.start.x).abs() < 1e-12)
        );
    }
}

#[test]
fn cut_hatches_reject_dense_work_without_partial_output() {
    let ctx = context();
    let material = Id::new();
    let d = drawing(ctx, vec![rect(Some(material), 0., 0., 100., 10000.)]);
    let p = MaterialPatternParams {
        spacing: 0.001,
        ..pattern()
    };
    assert!(d.section_hatches(ctx).unwrap().is_empty());
    assert!(
        d.with_section_hatches(BTreeMap::from([(material, p)]), plane())
            .err()
            .unwrap()
            .to_string()
            .contains("budget")
    );
    let p = MaterialPatternParams {
        angle_degrees: f64::NAN,
        ..pattern()
    };
    assert!(
        drawing(ctx, vec![])
            .with_section_hatches(BTreeMap::from([(material, p)]), plane())
            .is_err()
    );
}

#[test]
fn cut_hatches_sheet_viewports_and_pdf_use_exact_shared_vectors_and_paper_weight() {
    let ctx = context();
    let material = Id::new();
    let d = drawing(ctx, vec![rect(Some(material), 0., 0., 1., 1.)])
        .with_section_hatches(BTreeMap::from([(material, pattern())]), plane())
        .unwrap();
    let paper = PaperSheetInfo {
        width_mm: 420.,
        height_mm: 297.,
        number: "A01",
        name: "Hatches",
    };
    for scale in [50., 100.] {
        let viewport = PaperViewport {
            center_mm: Point2::new(100., 100.),
            width_mm: 80.,
            height_mm: 80.,
            model_center_m: Point2::new(0.5, 0.5),
            scale_denominator: scale,
        };
        let page = compose_view_sheet(paper, "Section", viewport, ctx, &d).unwrap();
        let hatches: Vec<_> = page
            .marks()
            .iter()
            .filter_map(|m| match &m.kind {
                PaperMarkKind::Path {
                    points_mm,
                    stroke: Some(s),
                    fill: None,
                    closed: false,
                } if m.clip == Some(viewport.paper_rect().unwrap())
                    && s.width_mm == SECTION_HATCH_WEIGHT_MM =>
                {
                    Some(points_mm)
                }
                _ => None,
            })
            .collect();
        assert_eq!(hatches.len(), d.section_hatches(ctx).unwrap().len());
        for (paper, model) in hatches.into_iter().zip(d.section_hatches(ctx).unwrap()) {
            assert_eq!(
                *paper,
                vec![
                    viewport.model_to_paper(model.start).unwrap(),
                    viewport.model_to_paper(model.end).unwrap()
                ]
            );
        }
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert_eq!(pdf.matches("0.283465 w").count(), 4);
        assert!(pdf.find("h\nf\n").unwrap() < pdf.find("0.283465 w").unwrap());
    }
}
