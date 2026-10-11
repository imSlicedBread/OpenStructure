#[path = "../../os-model/tests/support/section_dimensions.rs"]
mod fixture;
use os_core::{Id, Point2};
use os_geometry::plan::*;
use os_render::{plan::*, sheet::*};
use std::collections::BTreeMap;

#[test]
fn section_dimension_witnesses_and_value_share_sheet_and_vector_pdf_coordinates() {
    let (model, wall, opening, view, p) = fixture::fixture();
    let r = p.resolve_section(&model).unwrap();
    let (a, b) = p.section_line(r);
    // A vertical section dimension measures only project elevation. Its
    // semantic anchors can have different stations, so witness distance is
    // deliberately not the displayed value.
    let mut mixed = p.clone();
    mixed.layout = os_model::DimensionLayout::SectionVertical;
    mixed.first = os_model::DimensionReference::SectionWall {
        wall,
        feature: os_model::ElevationDimensionFeature::Bottom,
    };
    mixed.second = os_model::DimensionReference::SectionOpening {
        opening,
        feature: os_model::ElevationDimensionFeature::Bottom,
    };
    mixed.offset_m = 0.5;
    let mixed_resolved = mixed.resolve_section(&model).unwrap();
    assert!(mixed_resolved.first.distance(mixed_resolved.second) > mixed_resolved.length_metres);
    let (mixed_a, mixed_b) = mixed.section_line(mixed_resolved);
    let c = PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: view,
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 100.,
        show_walls: true,
        show_extensions: false,
    };
    let make_graphic =
        |resolved: os_model::ResolvedDimension, line: (Point2, Point2), hint| PlanDimensionItem {
            entity: Id::new(),
            spans: vec![],
            shared_start_witness: false,
            witness_start: resolved.first,
            witness_end: resolved.second,
            line_start: line.0,
            line_end: line.1,
            value_m: Some(resolved.length_metres),
            orphan_hint: hint,
            diagnostic: None,
        };
    let graphic = make_graphic(r, (a, b), p.orphan_hint);
    let mixed_graphic = make_graphic(mixed_resolved, (mixed_a, mixed_b), mixed.orphan_hint);
    let d = PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![])
        .unwrap()
        .with_dimensions(vec![graphic.clone(), mixed_graphic.clone()])
        .unwrap();
    let mut malformed = mixed_graphic.clone();
    malformed.witness_end = malformed.witness_start;
    assert!(
        PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![])
            .unwrap()
            .with_dimensions(vec![malformed])
            .is_err()
    );
    let viewport = PaperViewport {
        center_mm: Point2::new(210., 148.5),
        width_mm: 200.,
        height_mm: 100.,
        model_center_m: Point2::new(5., 2.),
        scale_denominator: 100.,
    };
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.,
            height_mm: 297.,
            number: "A1",
            name: "Section dimensions",
        },
        "Section",
        viewport,
        c,
        &d,
    )
    .unwrap();
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    for expected in [&graphic, &mixed_graphic] {
        for (a, b) in expected.lines().filter(|(a, b)| a.distance(*b) > 1e-6) {
            let points = [
                viewport.model_to_paper(a).unwrap(),
                viewport.model_to_paper(b).unwrap(),
            ];
            assert!(page.marks().iter().any(|m| matches!(&m.kind,PaperMarkKind::Path { points_mm,closed:false,.. } if points_mm.as_slice()==points)));
            let pt = 72. / 25.4;
            assert!(pdf.contains(&format!(
                "{:.6} {:.6} m\n{:.6} {:.6} l",
                points[0].x * pt,
                (297. - points[0].y) * pt,
                points[1].x * pt,
                (297. - points[1].y) * pt
            )));
        }
    }
    assert!(
        page.marks()
            .iter()
            .any(|m| matches!(&m.kind,PaperMarkKind::Text { text,.. } if text=="2.000 m"))
    );
    assert!(page.marks().iter().any(|m| matches!(
        &m.kind,
        PaperMarkKind::Text { text, .. } if text == "1.000 m"
    )));
    assert!(pdf.contains("<322E303030206D> Tj"));
    assert!(pdf.contains("<312E303030206D> Tj"));
    let mut stale = c;
    stale.model_revision += 1;
    assert!(d.dimensions(stale).is_err());
}
