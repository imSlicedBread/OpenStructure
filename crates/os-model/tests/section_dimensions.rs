use os_core::{Id, Point2};
use os_model::*;
#[path = "support/section_dimensions.rs"]
mod fixture;

#[test]
fn section_depth_crop_frame_curves_and_wrong_view_fail_closed() {
    let (mut m, host, _, view, p) = fixture::fixture();
    let original = m.clone();
    // Cropping through the aperture retains its full semantic measurement.
    m.views
        .get_mut(&view)
        .unwrap()
        .parameters
        .section
        .as_mut()
        .unwrap()
        .min_station = 3.5;
    assert_eq!(p.resolve(&m).unwrap().length_metres, 2.);
    m.views
        .get_mut(&view)
        .unwrap()
        .parameters
        .section
        .as_mut()
        .unwrap()
        .min_station = 6.;
    assert_eq!(p.resolve(&m), Err(DimensionDiagnostic::OutsideSection));
    m = original.clone();
    let s = m
        .views
        .get_mut(&view)
        .unwrap()
        .parameters
        .section
        .as_mut()
        .unwrap();
    s.start.y = 2.;
    s.end.y = 2.;
    s.mode = SectionDisplayMode::CutAndProjection;
    s.far_depth = 3.;
    assert_eq!(p.resolve(&m).unwrap().length_metres, 2.);
    m.views
        .get_mut(&view)
        .unwrap()
        .parameters
        .section
        .as_mut()
        .unwrap()
        .far_depth = 1.;
    assert_eq!(p.resolve(&m), Err(DimensionDiagnostic::OutsideSection));
    let s = m
        .views
        .get_mut(&view)
        .unwrap()
        .parameters
        .section
        .as_mut()
        .unwrap();
    s.far_depth = 3.;
    s.side = ElevationSide::Right;
    assert_eq!(p.resolve(&m), Err(DimensionDiagnostic::OutsideSection));
    m = original.clone();
    // Reversing the view changes station direction, never the UUID or length.
    let s = m
        .views
        .get_mut(&view)
        .unwrap()
        .parameters
        .section
        .as_mut()
        .unwrap();
    std::mem::swap(&mut s.start, &mut s.end);
    assert_eq!(p.resolve(&m).unwrap().first.x, 5.);
    assert_eq!(p.resolve(&m).unwrap().length_metres, 2.);
    m.walls.get_mut(&host).unwrap().parameters.path = WallPath::CircularArc {
        center: Point2::new(0., 0.),
        radius: 10.,
        start_angle_rad: 0.,
        signed_sweep_rad: 1.,
    };
    assert_eq!(p.resolve(&m), Err(DimensionDiagnostic::InvalidGeometry));
    m = original;
    m.views.get_mut(&view).unwrap().parameters.kind = ViewKind::Elevation;
    assert_eq!(p.resolve(&m), Err(DimensionDiagnostic::MissingSectionView));
    assert!(p.first.resolve_elevation(&m, host).is_err());
}

#[test]
fn section_opening_references_follow_effective_sizes_station_and_project_height() {
    use ElevationDimensionFeature as F;
    let mut model = Model::new("Dimensions");
    let level = *model.levels.keys().next().unwrap();
    model.levels.get_mut(&level).unwrap().parameters.elevation = 12.;
    let wall = Wall::new(
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
    let host = wall.id();
    model.walls.insert(host, wall);
    let view = View::new(
        "core.view",
        ViewParams {
            name: "Elevation".into(),
            kind: ViewKind::Section,
            level: Some(level),
            settings_revision: 0,
            plan: None,
            section: Some(SectionViewSettings::new(
                Point2::new(0., 0.),
                Point2::new(10., 0.),
                11.,
                17.,
            )),
            facade: None,
            elevation: None,
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
    let r = |feature| DimensionReference::SectionOpening {
        opening: id,
        feature,
    };
    let mut p = DimensionParams {
        layout: DimensionLayout::SectionHorizontal,
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
        r(F::Bottom).resolve_section(&model, view_id).unwrap().y,
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
    p.layout = DimensionLayout::SectionVertical;
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
    assert_eq!(r(F::Top).resolve_section(&model, view_id).unwrap().y, 15.);
    // Mixed wall and opening anchors measure an axis distance, not diagonal length.
    p.first = DimensionReference::SectionWall {
        wall: host,
        feature: F::Bottom,
    };
    assert_eq!(p.resolve(&model).unwrap().length_metres, 3.);
    let g = p.resolve(&model).unwrap();
    let (a, b) = p.section_line(g);
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
        r(F::Top).resolve_section(&model, Id::new()),
        Err(DimensionDiagnostic::MissingSectionView)
    );
    model.openings.get_mut(&id).unwrap().parameters.host = Id::new();
    assert_eq!(
        p.resolve(&model),
        Err(DimensionDiagnostic::MissingOpeningHost)
    );
    model.openings.remove(&id);
    p.validate().unwrap();
    assert_eq!(p, saved);
    assert_eq!(p.resolve(&model), Err(DimensionDiagnostic::MissingOpening));
    let wire = serde_json::to_value(&p).unwrap();
    assert!(wire.get("length_metres").is_none());
    assert_eq!(serde_json::from_value::<DimensionParams>(wire).unwrap(), p);
}
