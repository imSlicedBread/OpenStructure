use os_core::{Id, Point2};
use os_model::*;

pub fn crop() -> PlanViewCrop {
    PlanViewCrop {
        min: Point2::new(-1., -1.),
        max: Point2::new(1., 1.),
    }
}

pub fn fixture() -> (Model, Id, Id, Id) {
    let mut model = Model::new("Callout acceptance");
    let level = *model.levels.keys().next().unwrap();
    let parent = View::new("core.view", ViewParams::floor_plan("Parent", level));
    let target = View::new("core.view", ViewParams::floor_plan("Detail", level));
    let (p, t) = (parent.id(), target.id());
    model.views.insert(p, parent);
    model.views.insert(t, target);
    let marker = PlanCallout::new(
        "core.plan_callout",
        PlanCalloutParams {
            parent: p,
            target: t,
            owns_target: true,
            boundary: crop(),
            destination: None,
        },
    );
    let id = marker.id();
    model.plan_callouts.insert(id, marker);
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
