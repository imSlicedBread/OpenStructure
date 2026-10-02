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
fn native_doors_and_windows_validate_station_fit_types_and_overlap_on_arcs() {
    for sweep in [PI / 2., -PI / 2., 1.5 * PI, -1.5 * PI] {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let mut model = Model::new("Arc openings");
            let wall = Wall::new(
                "org.openstructure.walls.wall",
                WallParams {
                    name: "Host".into(),
                    path: arc(sweep),
                    thickness: 0.2,
                    height: 3.,
                    level: *model.levels.keys().next().unwrap(),
                    material: None,
                },
            );
            let host = wall.id();
            model.walls.insert(host, wall);
            let sill = if kind == OpeningKind::Door { 0. } else { 0.7 };
            let ty = OpeningType::new(
                "core.opening_type",
                OpeningTypeParams {
                    name: "Type".into(),
                    family: OpeningFamily::default(),
                    kind,
                    width: 1.,
                    height: 1.8,
                    sill,
                    window_operation: Default::default(),
                    pane_position: Default::default(),
                },
            );
            let type_id = ty.id();
            model.opening_types.insert(type_id, ty);
            for definition in [
                OpeningDefinition::Legacy {
                    kind,
                    width: 1.,
                    height: 1.8,
                    sill,
                },
                OpeningDefinition::Typed { type_id },
            ] {
                let mut p = OpeningParams {
                    open_state: Default::default(),
                    name: "Opening".into(),
                    host,
                    offset: 1.,
                    definition,
                    width_override: None,
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
                    hinge: Default::default(),
                    swing: Default::default(),
                };
                p.validate(&model).unwrap();
                let wire = serde_json::to_value(&p).unwrap();
                assert_eq!(serde_json::from_value::<OpeningParams>(wire).unwrap(), p);
                // End clearance uses arc travel even for major arcs whose
                // endpoint chord is much shorter than the host length.
                p.offset = arc(sweep).length() - 1.002;
                p.validate(&model).unwrap();
                p.offset += 0.0011;
                assert!(p.validate(&model).is_err());
                for offset in [0., -1., f64::NAN, f64::INFINITY] {
                    p.offset = offset;
                    assert!(p.validate(&model).is_err());
                }
                p.offset = 1.;
                let a = Opening::new("core.opening", p.clone());
                let a_id = a.id();
                model.openings.insert(a_id, a);
                p.offset = 2.0011;
                let b = Opening::new("core.opening", p);
                let b_id = b.id();
                model.openings.insert(b_id, b);
                model.validate().unwrap();
                model.openings.get_mut(&b_id).unwrap().parameters.offset = 2.0009;
                assert!(model.validate().is_err());
                model.openings.remove(&a_id);
                model.openings.remove(&b_id);
            }
        }
    }
}

#[test]
fn circular_host_rejects_infeasible_rigid_leaf_and_collapsed_physical_clear_width() {
    let model = Model::new("Tight bend");
    let wall = WallParams {
        name: "Tight".into(),
        path: WallPath::CircularArc {
            center: Point2::default(),
            radius: 0.11,
            start_angle_rad: 0.,
            signed_sweep_rad: PI * 1.5,
        },
        thickness: 0.2,
        height: 3.,
        level: *model.levels.keys().next().unwrap(),
        material: None,
    };
    let mut p = ResolvedOpening {
        open_state: Default::default(),
        name: "Opening".into(),
        host: os_core::Id::new(),
        offset: 0.01,
        kind: OpeningKind::Window,
        width: 0.002,
        height: 1.,
        sill: 0.,
        family: Default::default(),
        window_operation: Default::default(),
        pane_position: Default::default(),
        type_id: None,
        type_name: None,
        hinge: Default::default(),
        swing: Default::default(),
    };
    assert!(
        p.validate_host(&wall)
            .unwrap_err()
            .to_string()
            .contains("physical clear width")
    );
    p.width = 0.1;
    p.validate_host(&wall).unwrap();
    p.family.frame_width = 0.001;
    assert!(
        p.validate_host(&wall)
            .unwrap_err()
            .to_string()
            .contains("arc frame or mullion")
    );
    p.family.frame_width = 0.;
    p.kind = OpeningKind::Door;
    assert!(
        p.validate_host(&wall)
            .unwrap_err()
            .to_string()
            .contains("rigid door leaf")
    );
    let mut straight = wall.clone();
    straight.path = WallPath::Straight {
        start: Point2::default(),
        end: Point2::new(1., 0.),
    };
    p.validate_host(&straight).unwrap();
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
