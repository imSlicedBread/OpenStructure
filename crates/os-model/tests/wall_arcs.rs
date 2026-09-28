use os_core::Point2;
use os_model::*;
use std::f64::consts::{PI, TAU};

fn arc(sweep: f64) -> WallPath {
    WallPath::CircularArc {
        center: Point2::new(3.0, -2.0),
        radius: 4.0,
        start_angle_rad: 0.3,
        signed_sweep_rad: sweep,
    }
}

#[test]
fn analytic_stations_tangents_projection_and_three_point_roundtrip() {
    for sweep in [PI / 2.0, -PI / 2.0, PI * 1.5, -PI * 1.5] {
        let p = arc(sweep);
        p.validate(0.2).unwrap();
        assert!((p.length() - 4.0 * sweep.abs()).abs() < 1e-12);
        for f in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let station = p.length() * f;
            let q = p.point(station);
            assert!((p.project(q) - station).abs() < 1e-10, "{sweep} {f}");
            let t = p.tangent(station);
            assert!((t.x.hypot(t.y) - 1.0).abs() < 1e-12);
            let offset = p.offset_point(station, 0.1);
            assert!((offset.distance(q) - 0.1).abs() < 1e-12);
        }
        let rebuilt =
            WallPath::through_three_points(p.start(), p.point(p.length() / 2.0), p.end()).unwrap();
        assert!((rebuilt.length() - p.length()).abs() < 1e-10);
        assert!(
            rebuilt
                .point(rebuilt.length() * 0.25)
                .distance(p.point(p.length() * 0.25))
                < 1e-10
        );
    }
}

#[test]
fn invalid_arc_and_display_budget_reject() {
    for sweep in [0.0, 1e-9, TAU, -TAU, TAU + 0.1, f64::NAN, f64::INFINITY] {
        assert!(arc(sweep).validate(0.2).is_err());
    }
    assert!(arc(PI).validate(8.0).is_err());
    assert!(
        WallPath::through_three_points(
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 0.0)
        )
        .is_err()
    );
    let huge = WallPath::CircularArc {
        center: Point2::default(),
        radius: 1e12,
        start_angle_rad: 0.0,
        signed_sweep_rad: PI,
    };
    assert!(huge.validate(0.2).is_err());
    let p = arc(PI * 1.5);
    let n = p.display_segments(0.1).unwrap();
    assert!(n <= MAX_WALL_ARC_SEGMENTS);
    assert!(4.1 * (1.0 - (PI * 1.5 / n as f64 / 2.0).cos()) <= WALL_DISPLAY_SAGITTA);
}

#[test]
fn resolved_compound_thickness_rechecks_inner_radius() {
    let mut model = Model::new("Arc type");
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Arc".into(),
            path: WallPath::CircularArc {
                center: Point2::default(),
                radius: 0.4,
                start_angle_rad: 0.0,
                signed_sweep_rad: PI,
            },
            thickness: 0.1,
            height: 3.0,
            level: *model.levels.keys().next().unwrap(),
            material: None,
        },
    );
    let id = wall.id();
    model.walls.insert(id, wall);
    model.resolve_wall(id).unwrap();
    let ty = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Thick".into(),
            layers: vec![WallLayer {
                id: os_core::Id::new(),
                name: "Core".into(),
                thickness: 0.9,
                function: LayerFunction::Structure,
                material: None,
            }],
        },
    );
    let type_id = ty.id();
    model.wall_types.insert(type_id, ty);
    model.wall_type_assignments.insert(
        id,
        WallTypeAssignment {
            type_id,
            flipped: false,
        },
    );
    assert!(
        model
            .resolve_wall(id)
            .unwrap_err()
            .to_string()
            .contains("inner radius")
    );
    model
        .wall_types
        .get_mut(&type_id)
        .unwrap()
        .parameters
        .layers[0]
        .thickness = 0.3;
    model.validate().unwrap();
}
