use os_core::{Id, Point2};
use os_model::*;

#[test]
fn elevation_opening_references_follow_effective_sizes_station_and_project_height() {
    use ElevationDimensionFeature as F;
    let mut model = Model::new("Dimensions");
    let level = *model.levels.keys().next().unwrap();
    model.levels.get_mut(&level).unwrap().parameters.elevation = 12.;
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(8., 11.),
                end: Point2::new(2., 3.),
            },
            height: 4.,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let host = wall.id();
    model.walls.insert(host, wall);
    let view = View::new(
        "core.view",
        ViewParams {
            name: "Elevation".into(),
            kind: ViewKind::Elevation,
            level: None,
            settings_revision: 0,
            plan: None,
            section: None,
            facade: None,
            elevation: Some(ElevationViewSettings::new(
                host,
                ElevationSide::Right,
                -1.,
                11.,
                11.,
                17.,
            )),
        },
    );
    let view_id = view.id();
    model.views.insert(view_id, view);
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Window".into(),
            kind: OpeningKind::Window,
            width: 1.2,
            height: 1.5,
            sill: 0.8,
            window_operation: Default::default(),
            family: Default::default(),
            pane_position: Default::default(),
        },
    );
    let type_id = ty.id();
    model.opening_types.insert(type_id, ty);
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Window".into(),
            host,
            offset: 2.,
            definition: OpeningDefinition::Typed { type_id },
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
    let id = opening.id();
    model.openings.insert(id, opening);
    let r = |feature| DimensionReference::ElevationOpening {
        opening: id,
        feature,
    };
    let mut p = DimensionParams {
        layout: DimensionLayout::ElevationHorizontal,
        additional: vec![],
        baseline_spacing_m: 0.25,
        view: view_id,
        first: r(F::Start),
        second: r(F::End),
        offset_m: 0.4,
        orphan_hint: Point2::new(2.6, 14.),
    };
    model.validate().unwrap();
    p.validate_creation(&model).unwrap();
    assert!((p.resolve(&model).unwrap().length_metres - 1.2).abs() < 1e-9);
    assert_eq!(
        r(F::Bottom).resolve_elevation(&model, host).unwrap().y,
        12.8
    );
    assert!(r(F::Start).resolve(&model, level).is_err());
    model
        .opening_types
        .get_mut(&type_id)
        .unwrap()
        .parameters
        .width = 1.8;
    assert!((p.resolve(&model).unwrap().length_metres - 1.8).abs() < 1e-9);
    model
        .openings
        .get_mut(&id)
        .unwrap()
        .parameters
        .width_override = Some(2.);
    assert_eq!(p.resolve(&model).unwrap().length_metres, 2.);
    p.layout = DimensionLayout::ElevationVertical;
    p.first = r(F::Bottom);
    p.second = r(F::Top);
    assert_eq!(p.resolve(&model).unwrap().length_metres, 1.5);
    model
        .openings
        .get_mut(&id)
        .unwrap()
        .parameters
        .height_override = Some(2.);
    model
        .openings
        .get_mut(&id)
        .unwrap()
        .parameters
        .sill_override = Some(1.);
    assert_eq!(p.resolve(&model).unwrap().length_metres, 2.);
    assert_eq!(r(F::Top).resolve_elevation(&model, host).unwrap().y, 15.);
    // Mixed wall and opening anchors measure an axis distance, not diagonal length.
    p.first = DimensionReference::ElevationWall {
        wall: host,
        feature: F::Bottom,
    };
    assert_eq!(p.resolve(&model).unwrap().length_metres, 3.);
    let g = p.resolve(&model).unwrap();
    let (a, b) = p.elevation_line(g);
    assert_eq!(a.x, b.x);
    assert_eq!((b.y - a.y).abs(), 3.);
    let saved = p.clone();
    for case in 0..5 {
        let mut bad = p.clone();
        match case {
            0 => bad.first = r(F::Start),
            1 => bad.layout = DimensionLayout::Aligned,
            2 => bad.additional.push(r(F::Bottom)),
            3 => {
                bad.first = DimensionReference::WallEndpoint {
                    wall: host,
                    endpoint: DimensionEndpoint::Start,
                }
            }
            _ => bad.offset_m = f64::NAN,
        }
        assert!(bad.validate_creation(&model).is_err());
    }
    assert_eq!(
        r(F::Top).resolve_elevation(&model, Id::new()),
        Err(DimensionDiagnostic::MissingWall)
    );
    model.openings.get_mut(&id).unwrap().parameters.host = Id::new();
    assert_eq!(p.resolve(&model), Err(DimensionDiagnostic::WrongHost));
    model.openings.remove(&id);
    p.validate().unwrap();
    assert_eq!(p, saved);
    assert_eq!(p.resolve(&model), Err(DimensionDiagnostic::MissingOpening));
    let wire = serde_json::to_value(&p).unwrap();
    assert!(wire.get("length_metres").is_none());
    assert_eq!(serde_json::from_value::<DimensionParams>(wire).unwrap(), p);
}
