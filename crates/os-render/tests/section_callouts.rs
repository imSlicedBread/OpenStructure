#[path = "../../os-model/tests/support/section_callouts.rs"]
mod fixture;
use os_core::{Id, Point2};
use os_geometry::plan::*;
use os_render::{plan::*, sheet::*};
use std::collections::BTreeMap;

#[test]
fn section_callout_station_bounds_and_live_label_match_sheet_vector_pdf() {
    let (mut model, parent, target, marker) = fixture::fixture();
    fixture::sheet(&mut model, target, "A201", "7");
    let c = PlanContext {
        annotation_crop: None,
        session_id: Id::new(),
        model_revision: 0,
        view_id: parent,
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 25.,
        show_walls: true,
        show_extensions: false,
    };
    let p = &model.section_callouts[&marker].parameters;
    let graphic = PlanCalloutGraphic {
        entity: marker,
        boundary: PlanCrop {
            min: p.boundary.min,
            max: p.boundary.max,
        },
        label: model.section_callout_reference(p).label().into(),
    };
    let drawing = PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![])
        .unwrap()
        .with_section_strokes()
        .with_callouts(vec![graphic.clone()])
        .unwrap();
    assert_eq!(drawing.callouts(c).unwrap(), &[graphic.clone()]);
    let viewport = PaperViewport {
        center_mm: Point2::new(210., 148.5),
        width_mm: 200.,
        height_mm: 160.,
        model_center_m: Point2::default(),
        scale_denominator: 25.,
    };
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.,
            height_mm: 297.,
            number: "A101",
            name: "Sections",
        },
        "Parent",
        viewport,
        c,
        &drawing,
    )
    .unwrap();
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    let corners = graphic.corners();
    for i in 0..4 {
        let points = [
            viewport.model_to_paper(corners[i]).unwrap(),
            viewport.model_to_paper(corners[(i + 1) % 4]).unwrap(),
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
    assert!(
        page.marks()
            .iter()
            .any(|m| matches!(&m.kind,PaperMarkKind::Text { text,.. } if text=="7 / A201"))
    );
    assert!(pdf.contains("<37202F2041323031> Tj"));
    let mut stale = c;
    stale.model_revision += 1;
    assert!(drawing.callouts(stale).is_err());
}
