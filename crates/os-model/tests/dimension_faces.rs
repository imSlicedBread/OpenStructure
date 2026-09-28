use os_core::{Id, Point2};
use os_model::*;

fn fixture() -> (Model, Id, Id, DimensionParams) {
    let mut model = Model::new("Faces");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Wall".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(1.0, 2.0),
                end: Point2::new(11.0, 2.0),
            },
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let id = wall.id();
    let params = DimensionParams {
        layout: DimensionLayout::Aligned,
        view: view.id(),
        first: face(id, DimensionWallSide::Left, 2.0),
        second: face(id, DimensionWallSide::Right, 2.0),
        additional: vec![],
        baseline_spacing_m: 0.25,
        offset_m: 0.5,
        orphan_hint: Point2::new(3.0, 2.0),
    };
    model.walls.insert(id, wall);
    model.views.insert(view.id(), view);
    (model, id, level, params)
}

fn face(wall: Id, side: DimensionWallSide, station_m: f64) -> DimensionReference {
    DimensionReference::WallFace {
        wall,
        side,
        station_m,
    }
}

#[test]
fn wall_faces_follow_axis_translation_rotation_reversal_and_effective_thickness() {
    let (mut model, id, level, params) = fixture();
    for typed in [false, true] {
        let thickness = if typed { 0.6 } else { 0.2 };
        if typed {
            let ty = WallType::new(
                "core.wall_type",
                WallTypeParams {
                    name: "Compound".into(),
                    layers: [0.2, 0.4]
                        .into_iter()
                        .map(|thickness| WallLayer {
                            id: Id::new(),
                            name: "Layer".into(),
                            thickness,
                            function: LayerFunction::Structure,
                            material: None,
                        })
                        .collect(),
                },
            );
            model.wall_type_assignments.insert(
                id,
                WallTypeAssignment {
                    type_id: ty.id(),
                    flipped: true,
                },
            );
            model.wall_types.insert(ty.id(), ty);
        }
        for (start, unit) in [
            (Point2::new(1.0, 2.0), Point2::new(1.0, 0.0)),
            (Point2::new(11.0, 2.0), Point2::new(-1.0, 0.0)),
            (Point2::new(-3.0, 7.0), Point2::new(0.6, 0.8)),
        ] {
            let p = &mut model.walls.get_mut(&id).unwrap().parameters;
            *p.path.straight_start_mut().unwrap() = start;
            *p.path.straight_end_mut().unwrap() =
                Point2::new(start.x + 10.0 * unit.x, start.y + 10.0 * unit.y);
            for (reference, sign) in [(params.first, 1.0), (params.second, -1.0)] {
                let actual = reference.resolve(&model, level).unwrap();
                let expected = Point2::new(
                    start.x + 2.0 * unit.x - sign * unit.y * thickness / 2.0,
                    start.y + 2.0 * unit.y + sign * unit.x * thickness / 2.0,
                );
                assert!(actual.distance(expected) < 1e-12);
            }
            assert!((params.resolve(&model).unwrap().length_metres - thickness).abs() < 1e-12);
            params.validate_creation(&model).unwrap();
        }
    }
    let assignment = model.wall_type_assignments[&id];
    model
        .wall_types
        .get_mut(&assignment.type_id)
        .unwrap()
        .parameters
        .layers[0]
        .thickness = 0.5;
    assert!((params.resolve(&model).unwrap().length_metres - 0.9).abs() < 1e-12);
    model.wall_type_assignments.remove(&id);
    model.walls.get_mut(&id).unwrap().parameters.thickness = 0.8;
    assert!((params.resolve(&model).unwrap().length_metres - 0.8).abs() < 1e-12);
}

#[test]
fn wall_faces_diagnose_unavailable_geometry_and_reject_angular_use() {
    let (mut model, id, level, mut params) = fixture();
    for station in [-1.0, 11.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            face(id, DimensionWallSide::Left, station).resolve(&model, level),
            Err(DimensionDiagnostic::InvalidGeometry)
        );
    }
    assert_eq!(
        params.first.resolve(&model, Id::new()),
        Err(DimensionDiagnostic::WrongLevel)
    );
    assert_eq!(
        face(Id::new(), DimensionWallSide::Left, 2.0).resolve(&model, level),
        Err(DimensionDiagnostic::MissingWall)
    );
    params.layout = DimensionLayout::Angular;
    assert!(params.validate().is_err());
    params.layout = DimensionLayout::Aligned;
    *model
        .walls
        .get_mut(&id)
        .unwrap()
        .parameters
        .path
        .straight_end_mut()
        .unwrap() = Point2::new(1.5, 2.0);
    params.validate().unwrap(); // Station beyond a shortened wall is preservable.
    assert_eq!(
        params.resolve(&model),
        Err(DimensionDiagnostic::InvalidGeometry)
    );
    *model
        .walls
        .get_mut(&id)
        .unwrap()
        .parameters
        .path
        .straight_end_mut()
        .unwrap() = Point2::new(1.0, 2.0);
    assert_eq!(
        params.resolve(&model),
        Err(DimensionDiagnostic::InvalidGeometry)
    );
    model.walls.remove(&id);
    assert_eq!(
        params.resolve(&model),
        Err(DimensionDiagnostic::MissingWall)
    );
}

#[test]
fn wall_faces_support_chain_baseline_and_strict_wire_syntax() {
    let (model, id, _, mut params) = fixture();
    params.first = face(id, DimensionWallSide::Left, 1.0);
    params.second = face(id, DimensionWallSide::Left, 3.0);
    params.additional = vec![face(id, DimensionWallSide::Left, 6.0)];
    for layout in [DimensionLayout::Chain, DimensionLayout::Baseline] {
        params.layout = layout;
        params.validate_creation(&model).unwrap();
        let points = params.resolve_points(&model).unwrap();
        assert!((points[0].distance(points[2]) - 5.0).abs() < 1e-12);
    }
    let value = serde_json::to_value(params.first).unwrap();
    assert_eq!(
        serde_json::from_value::<DimensionReference>(value.clone()).unwrap(),
        params.first
    );
    for field in ["side", "station_m", "wall"] {
        let mut bad = value.clone();
        bad["WallFace"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<DimensionReference>(bad).is_err());
    }
    let mut bad = value;
    bad["WallFace"]["side"] = "Interior".into();
    assert!(serde_json::from_value::<DimensionReference>(bad).is_err());
}
