use os_core::{Id, Point2};
use os_model::*;

pub fn crop() -> SectionCalloutBounds {
    SectionCalloutBounds {
        min: Point2::new(-1., -1.),
        max: Point2::new(1., 1.),
    }
}

pub fn fixture() -> (Model, Id, Id, Id) {
    let mut model = Model::new("Callout acceptance");
    let level = *model.levels.keys().next().unwrap();
    let mut p = ViewParams::floor_plan("Parent", level);
    p.kind = ViewKind::Section;
    p.plan = None;
    p.section = Some(SectionViewSettings::new(
        Point2::new(7., 9.),
        Point2::new(-1., 3.),
        -2.,
        8.,
    ));
    let parent = View::new("core.view", p.clone());
    p.name = "Detail".into();
    let target = View::new("core.view", p);
    let (p, t) = (parent.id(), target.id());
    model.views.insert(p, parent);
    model.views.insert(t, target);
    let marker = SectionCallout::new(
        "core.section_callout",
        SectionCalloutParams {
            parent: p,
            target: t,
            owns_target: true,
            boundary: crop(),
            destination: None,
        },
    );
    let id = marker.id();
    model.section_callouts.insert(id, marker);
    model.validate().unwrap();
    (model, p, t, id)
}

pub fn sheet(model: &mut Model, view: Id, number: &str, detail: &str) -> (Id, Id) {
    let viewport = Id::new();
    let mut p = SheetParams::new(number, "Callout drawing");
    p.viewports.push(SheetViewport {
        id: viewport,
        view,
        detail_number: detail.into(),
        model_center_m: Point2::default(),
        paper_center_mm: Point2::new(210., 130.),
        width_mm: 120.,
        height_mm: 100.,
        scale_denominator: 50.,
        title_override: None,
    });
    let sheet = Sheet::new("core.sheet", p);
    let id = sheet.id();
    model.sheets.insert(id, sheet);
    model.validate().unwrap();
    (id, viewport)
}
