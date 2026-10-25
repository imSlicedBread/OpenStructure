use os_core::{Id, Point2};
use os_model::*;
pub fn fixture(section: bool) -> (Model, Id) {
    let mut model = Model::new("Drafting");
    let level = *model.levels.keys().next().unwrap();
    let mut p = ViewParams::floor_plan("Detail", level);
    if section {
        p.kind = ViewKind::Section;
        p.plan = None;
        let mut s =
            SectionViewSettings::new(Point2::new(26., 26.), Point2::new(18., 20.), -5., 10.);
        s.min_station = -5.;
        s.max_station = 15.;
        p.section = Some(s);
    } else {
        p.plan.as_mut().unwrap().basis.rotation = 0.6;
    }
    let view = View::new("core.view", p);
    let id = view.id();
    model.views.insert(id, view);
    (model, id)
}
