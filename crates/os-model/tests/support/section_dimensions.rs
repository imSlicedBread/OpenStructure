use os_core::{Id, Point2};
use os_model::*;
pub fn fixture() -> (Model, Id, Id, Id, DimensionParams) {
    let mut m = Model::new("Section dimensions");
    let level = *m.levels.keys().next().unwrap();
    let w = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(10., 0.),
            },
            height: 4.,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let host = w.id();
    m.walls.insert(host, w);
    let o = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Window".into(),
            host,
            offset: 3.,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Window,
                width: 2.,
                height: 1.5,
                sill: 1.,
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
            open_state: Default::default(),
        },
    );
    let opening = o.id();
    m.openings.insert(opening, o);
    let mut vp = ViewParams::floor_plan("Section", level);
    vp.kind = ViewKind::Section;
    vp.plan = None;
    vp.section = Some(SectionViewSettings::new(
        Point2::new(0., 0.),
        Point2::new(10., 0.),
        -1.,
        5.,
    ));
    let v = View::new("core.view", vp);
    let view = v.id();
    m.views.insert(view, v);
    let p = DimensionParams {
        view,
        layout: DimensionLayout::SectionHorizontal,
        first: DimensionReference::SectionOpening {
            opening,
            feature: ElevationDimensionFeature::Start,
        },
        second: DimensionReference::SectionOpening {
            opening,
            feature: ElevationDimensionFeature::End,
        },
        additional: vec![],
        baseline_spacing_m: 0.25,
        offset_m: 1.,
        orphan_hint: Point2::new(4., 3.),
    };
    m.validate().unwrap();
    p.validate_creation(&m).unwrap();
    (m, host, opening, view, p)
}
