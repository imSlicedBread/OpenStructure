use os_core::{Id, Point2};
use os_model::*;

pub fn fixture() -> (Model, Id, Vec<Id>) {
    let mut m = Model::new("Hidden openings");
    let level = *m.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let view_id = view.id();
    m.views.insert(view_id, view);
    let mut ids = Vec::new();
    for (index, sweep) in [0., 2., -2.].into_iter().enumerate() {
        let path = if index == 0 {
            WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            }
        } else {
            WallPath::CircularArc {
                center: Point2::new(15. * index as f64, 0.),
                radius: 4.,
                start_angle_rad: 0.,
                signed_sweep_rad: sweep,
            }
        };
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Host".into(),
                path,
                thickness: 0.2,
                height: 3.,
                level,
                material: None,
            },
        );
        let host = wall.id();
        m.walls.insert(host, wall);
        for (kind, offset) in [(OpeningKind::Door, 1.), (OpeningKind::Window, 4.)] {
            let opening = Opening::new(
                "core.opening",
                OpeningParams {
                    open_state: Default::default(),
                    name: format!("{kind:?} {index}"),
                    host,
                    offset,
                    definition: OpeningDefinition::Legacy {
                        kind,
                        width: 1.,
                        height: 2.1,
                        sill: 0.,
                    },
                    width_override: None,
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
                    hinge: Default::default(),
                    swing: Default::default(),
                },
            );
            ids.push(opening.id());
            m.openings.insert(opening.id(), opening);
        }
    }
    (m, view_id, ids)
}
