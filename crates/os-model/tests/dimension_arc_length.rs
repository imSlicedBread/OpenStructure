use os_core::{Id, Point2};
use os_model::*;

fn fixture(path: WallPath) -> (Model, Id, Id, DimensionParams) {
    let mut model = Model::new("Arc length dimensions");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Curved wall".into(),
            path,
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    let view_id = view.id();
    let params = DimensionParams {
        layout: DimensionLayout::ArcLength,
        additional: vec![],
        baseline_spacing_m: 0.25,
        view: view_id,
        first: DimensionReference::WallEndpoint {
            wall: wall_id,
            endpoint: DimensionEndpoint::Start,
        },
        second: DimensionReference::WallEndpoint {
            wall: wall_id,
            endpoint: DimensionEndpoint::End,
        },
        offset_m: 0.5,
        orphan_hint: Point2::new(3.0, 3.0),
    };
    model.views.insert(view_id, view);
    model.walls.insert(wall_id, wall);
    (model, wall_id, view_id, params)
}

fn arc(radius: f64, start_angle_rad: f64, signed_sweep_rad: f64) -> WallPath {
    WallPath::CircularArc {
        center: Point2::new(2.5, -1.25),
        radius,
        start_angle_rad,
        signed_sweep_rad,
    }
}

#[test]
fn arc_length_is_analytic_for_clockwise_counterclockwise_minor_and_major_arcs() {
    for (radius, sweep) in [
        (2.0, std::f64::consts::FRAC_PI_2),
        (2.0, -std::f64::consts::FRAC_PI_2),
        (3.0, 1.5 * std::f64::consts::PI),
        (3.0, -1.5 * std::f64::consts::PI),
    ] {
        let (model, _, _, params) = fixture(arc(radius, 0.37, sweep));
        let resolved = params.resolve_arc_length(&model).unwrap();
        let expected = radius * sweep.abs();
        assert!((resolved.length_metres - expected).abs() < 1e-12);
        assert!((params.resolve(&model).unwrap().length_metres - expected).abs() < 1e-12);
        params.validate_creation(&model).unwrap();
    }
}

#[test]
fn arc_length_tracks_host_geometry_and_retains_orphan_intent_until_recovery() {
    let (mut model, wall, _, params) = fixture(arc(2.0, 0.0, -std::f64::consts::FRAC_PI_2));
    assert!(
        (params.resolve_arc_length(&model).unwrap().length_metres - std::f64::consts::PI).abs()
            < 1e-12
    );

    model.walls.get_mut(&wall).unwrap().parameters.path = arc(4.0, 0.7, 1.5 * std::f64::consts::PI);
    let changed = params.resolve_arc_length(&model).unwrap();
    assert!((changed.length_metres - 6.0 * std::f64::consts::PI).abs() < 1e-11);
    assert!(
        (changed.radius - 4.5).abs() < 1e-12,
        "the displayed offset remains metric as the host radius changes"
    );

    let saved_wall = model.walls.remove(&wall).unwrap();
    assert_eq!(
        params.resolve_arc_length(&model),
        Err(DimensionDiagnostic::MissingWall)
    );
    params.validate().unwrap();

    model.walls.insert(wall, saved_wall.clone());
    let level = saved_wall.parameters.level;
    model.walls.get_mut(&wall).unwrap().parameters.level = Id::new();
    assert_eq!(
        params.resolve_arc_length(&model),
        Err(DimensionDiagnostic::WrongLevel)
    );

    model.walls.get_mut(&wall).unwrap().parameters.level = level;
    model.walls.get_mut(&wall).unwrap().parameters.path = WallPath::Straight {
        start: Point2::new(0.0, 0.0),
        end: Point2::new(2.0, 0.0),
    };
    assert_eq!(
        params.resolve_arc_length(&model),
        Err(DimensionDiagnostic::InvalidGeometry)
    );

    model.walls.insert(wall, saved_wall);
    assert!(
        (params.resolve_arc_length(&model).unwrap().length_metres - 6.0 * std::f64::consts::PI)
            .abs()
            < 1e-11
    );
}

#[test]
fn arc_length_placement_preserves_analytic_value_and_rejects_collapsed_offset() {
    let (model, _, _, mut params) = fixture(arc(2.0, 0.0, std::f64::consts::PI));
    params
        .place_arc_length(&model, Point2::new(5.5, -1.25))
        .unwrap();
    assert!((params.offset_m - 1.0).abs() < 1e-12);
    assert!(
        (params.resolve_arc_length(&model).unwrap().length_metres - 2.0 * std::f64::consts::PI)
            .abs()
            < 1e-12
    );
    assert_eq!(params.orphan_hint, Point2::new(5.5, -1.25));

    let saved = params.clone();
    assert!(
        params
            .place_arc_length(&model, Point2::new(2.5, -1.25))
            .is_err()
    );
    assert_eq!(params, saved, "invalid offset placement is atomic");
}

#[test]
fn arc_length_syntax_requires_paired_endpoints_of_one_wall_and_canonical_unused_fields() {
    let (model, wall, _, params) = fixture(arc(2.0, 0.0, std::f64::consts::FRAC_PI_2));
    params.validate_creation(&model).unwrap();

    let mut bad = params.clone();
    bad.second = DimensionReference::WallEndpoint {
        wall,
        endpoint: DimensionEndpoint::Start,
    };
    assert!(bad.validate().is_err());

    let mut bad = params.clone();
    bad.additional.push(DimensionReference::WallEndpoint {
        wall,
        endpoint: DimensionEndpoint::Start,
    });
    assert!(bad.validate().is_err());

    let mut bad = params.clone();
    bad.baseline_spacing_m = 0.5;
    assert!(bad.validate().is_err());

    let mut bad = params.clone();
    bad.offset_m = -2.0;
    assert!(
        bad.validate().is_ok(),
        "persistent orphan intent may retain an invalid current offset"
    );
    assert!(
        bad.validate_creation(&model).is_err(),
        "a nonpositive display radius cannot be created"
    );
}
