use os_core::{Id, Point2};
use os_model::*;

fn fixture(kind: OpeningKind, typed: bool) -> (Model, Id, Id, DimensionParams) {
    let mut model = Model::new("Jambs");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            start: Point2::new(1.0, 2.0),
            end: Point2::new(11.0, 2.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            family: Default::default(),
            name: "Type".into(),
            kind,
            width: 1.2,
            height: 2.0,
            sill: 0.0,
            pane_position: WindowPanePosition::Center,
        },
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Opening".into(),
            host: wall.id(),
            offset: 2.0,
            definition: if typed {
                OpeningDefinition::Typed { type_id: ty.id() }
            } else {
                OpeningDefinition::Legacy {
                    kind,
                    width: 1.2,
                    height: 2.0,
                    sill: 0.0,
                }
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
        },
    );
    let params = DimensionParams {
        layout: DimensionLayout::Aligned,
        view: view.id(),
        first: DimensionReference::OpeningJamb {
            opening: opening.id(),
            jamb: DimensionJamb::Start,
        },
        second: DimensionReference::OpeningJamb {
            opening: opening.id(),
            jamb: DimensionJamb::End,
        },
        additional: vec![],
        baseline_spacing_m: 0.25,
        offset_m: 1.0,
        orphan_hint: Point2::new(3.0, 3.0),
    };
    let (host, id) = (wall.id(), opening.id());
    model.views.insert(view.id(), view);
    model.walls.insert(host, wall);
    model.opening_types.insert(ty.id(), ty);
    model.openings.insert(id, opening);
    model.validate().unwrap();
    (model, host, id, params)
}

fn points(params: &DimensionParams, model: &Model, a: Point2, b: Point2) {
    let resolved = params.resolve(model).unwrap();
    assert!(resolved.first.distance(a) < 1e-12);
    assert!(resolved.second.distance(b) < 1e-12);
    assert!((resolved.length_metres - a.distance(b)).abs() < 1e-12);
    params.validate_creation(model).unwrap();
}

#[test]
fn dimension_jambs_follow_effective_width_current_host_and_void_direction() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        for typed in [false, true] {
            let (mut model, host, opening, params) = fixture(kind, typed);
            let saved = params.clone();
            points(
                &params,
                &model,
                Point2::new(3.0, 2.0),
                Point2::new(4.2, 2.0),
            );
            let p = &mut model.openings.get_mut(&opening).unwrap().parameters;
            if kind == OpeningKind::Door {
                p.hinge = DoorHinge::End;
                p.swing = DoorSwing::Right;
            }
            if let OpeningDefinition::Typed { type_id } = p.definition {
                let t = &mut model.opening_types.get_mut(&type_id).unwrap().parameters;
                t.width = 1.6;
                t.family.frame_width = 0.04;
                if kind == OpeningKind::Window {
                    t.pane_position = WindowPanePosition::LeftFace;
                }
                points(
                    &params,
                    &model,
                    Point2::new(3.0, 2.0),
                    Point2::new(4.6, 2.0),
                );
                model
                    .openings
                    .get_mut(&opening)
                    .unwrap()
                    .parameters
                    .width_override = Some(1.2);
                model
                    .opening_types
                    .get_mut(&type_id)
                    .unwrap()
                    .parameters
                    .width = 1.8;
            }
            points(
                &params,
                &model,
                Point2::new(3.0, 2.0),
                Point2::new(4.2, 2.0),
            );
            let wall = &mut model.walls.get_mut(&host).unwrap().parameters;
            std::mem::swap(&mut wall.start, &mut wall.end);
            points(
                &params,
                &model,
                Point2::new(9.0, 2.0),
                Point2::new(7.8, 2.0),
            );
            let mut new_host = model.walls[&host].clone();
            new_host.header.id = Id::new();
            new_host.parameters.start = Point2::new(5.0, 10.0);
            new_host.parameters.end = Point2::new(5.0, 0.0);
            let new_id = new_host.id();
            model.walls.insert(new_id, new_host);
            model.openings.get_mut(&opening).unwrap().parameters.host = new_id;
            model.openings.get_mut(&opening).unwrap().parameters.offset = 3.0;
            points(
                &params,
                &model,
                Point2::new(5.0, 7.0),
                Point2::new(5.0, 5.8),
            );
            assert_eq!(params, saved, "resolution never stores geometry");
        }
    }
}

#[test]
fn dimension_jamb_diagnostics_chain_validation_and_strict_angular_syntax() {
    let (mut model, host, opening, params) = fixture(OpeningKind::Door, true);
    let mut chain = params.clone();
    chain.layout = DimensionLayout::Chain;
    chain.additional.push(DimensionReference::WallEndpoint {
        wall: host,
        endpoint: DimensionEndpoint::End,
    });
    chain.validate_creation(&model).unwrap();
    let mut other = model.walls[&host].clone();
    other.header.id = Id::new();
    other.parameters.start.y += 1.0;
    other.parameters.end.y += 1.0;
    let other_id = other.id();
    model.walls.insert(other_id, other);
    model.openings.get_mut(&opening).unwrap().parameters.host = other_id;
    assert_eq!(
        chain.resolve_points(&model),
        Err(DimensionDiagnostic::OffAxis(3))
    );
    assert!(chain.validate_creation(&model).is_err());
    model.openings.get_mut(&opening).unwrap().parameters.host = host;
    let original_wall = model.walls[&host].clone();
    model.walls.get_mut(&host).unwrap().parameters.level = Id::new();
    assert_eq!(params.resolve(&model), Err(DimensionDiagnostic::WrongLevel));
    assert_eq!(
        chain.resolve_points(&model),
        Err(DimensionDiagnostic::WrongLevelAt(1))
    );
    model.walls.remove(&host);
    assert_eq!(
        params.resolve(&model),
        Err(DimensionDiagnostic::MissingOpeningHost)
    );
    assert_eq!(
        chain.resolve_points(&model),
        Err(DimensionDiagnostic::MissingOpeningHostAt(1))
    );
    model.walls.insert(host, original_wall);
    let removed = model.openings.remove(&opening).unwrap();
    assert_eq!(
        params.resolve(&model),
        Err(DimensionDiagnostic::MissingOpening)
    );
    assert_eq!(
        chain.resolve_points(&model),
        Err(DimensionDiagnostic::MissingOpeningAt(1))
    );
    params.validate().unwrap();
    model.openings.insert(opening, removed);
    chain.validate_creation(&model).unwrap();
    let mut angular = params.clone();
    angular.layout = DimensionLayout::Angular;
    assert!(angular.validate().is_err());
    assert!(
        angular
            .place_angular(&model, Point2::new(3.0, 3.0))
            .is_err()
    );
    let mut duplicate = params.clone();
    duplicate.second = duplicate.first;
    assert!(duplicate.validate_creation(&model).is_err());
    for json in [
        serde_json::json!({"wall":host,"endpoint":"Start"}),
        serde_json::json!({"OpeningJamb":{"opening":opening,"jamb":"Start","point":{"x":0,"y":0}}}),
        serde_json::json!({"WallEndpoint":{"wall":host,"endpoint":"Start"},"OpeningJamb":{"opening":opening,"jamb":"End"}}),
    ] {
        assert!(serde_json::from_value::<DimensionReference>(json).is_err());
    }
}
