use os_core::{Id, Point2};
use os_model::*;

fn settings(host: Id) -> ElevationViewSettings {
    ElevationViewSettings::new(host, ElevationSide::Left, -0.5, 9.0, -1.0, 5.0)
}

#[test]
fn elevation_crops_are_bounded_strict_and_round_trip() {
    let valid = settings(Id::new());
    valid.validate().unwrap();
    let json = serde_json::to_value(valid).unwrap();
    assert_eq!(
        serde_json::from_value::<ElevationViewSettings>(json.clone()).unwrap(),
        valid
    );
    for key in [
        "schema_version",
        "host",
        "side",
        "min_station",
        "max_station",
        "bottom_elevation",
        "top_elevation",
    ] {
        let mut missing = json.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<ElevationViewSettings>(missing).is_err(),
            "{key}"
        );
    }
    let mut unknown = json.clone();
    unknown["depth"] = 3.into();
    assert!(serde_json::from_value::<ElevationViewSettings>(unknown).is_err());
    for side in ["Front", "left", ""] {
        let mut bad = json.clone();
        bad["side"] = side.into();
        assert!(serde_json::from_value::<ElevationViewSettings>(bad).is_err());
    }
    for case in 0..9 {
        let mut bad = valid;
        match case {
            0 => bad.schema_version = 2,
            1 => bad.host = Id(Default::default()),
            2 => bad.min_station = f64::NAN,
            3 => bad.max_station = f64::INFINITY,
            4 => bad.min_station = bad.max_station,
            5 => bad.top_elevation = bad.bottom_elevation,
            6 => bad.top_elevation = 1e6 + 1.0,
            7 => bad.bottom_elevation = f64::NEG_INFINITY,
            _ => bad.max_station = -1e6 - 1.0,
        }
        assert!(bad.validate().is_err(), "{case}");
    }
}

#[test]
fn elevations_preserve_orphans_and_sheet_placements_but_resolve_only_current_native_straight_hosts()
{
    let mut model = Model::new("Elevations");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(8.0, 0.0),
            },
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let host = wall.id();
    model.walls.insert(host, wall);
    let crop = settings(host);
    let view = View::new(
        "core.view",
        ViewParams {
            drafting: None,
            name: "Left elevation".into(),
            kind: ViewKind::Elevation,
            level: None,
            settings_revision: 0,
            plan: None,
            section: None,
            facade: None,
            elevation: Some(crop),
        },
    );
    let view_id = view.id();
    model.views.insert(view_id, view.clone());
    let mut sheet_params = SheetParams::new("A1", "Elevations");
    sheet_params.viewports.push(SheetViewport {
        id: Id::new(),
        view: view_id,
        model_center_m: Point2::new(4.0, 1.5),
        paper_center_mm: Point2::new(210.0, 148.5),
        width_mm: 200.0,
        height_mm: 100.0,
        scale_denominator: 100.0,
        detail_number: "1".into(),
        title_override: None,
    });
    let sheet = Sheet::new("core.sheet", sheet_params);
    model.sheets.insert(sheet.id(), sheet);
    model.validate().unwrap();
    for path in [
        WallPath::Straight {
            start: Point2::new(10.0, 8.0),
            end: Point2::new(10.0, 0.0),
        },
        WallPath::Straight {
            start: Point2::new(10.0, 0.0),
            end: Point2::new(10.0, 8.0),
        },
    ] {
        model.walls.get_mut(&host).unwrap().parameters.path = path;
        assert_eq!(crop.resolve_host(&model).unwrap().path, path);
        assert_eq!(model.views[&view_id], view);
    }
    for case in 0..3 {
        let mut bad = model.clone();
        match case {
            0 => bad.walls.get_mut(&host).unwrap().header.type_id = "extension.wall".into(),
            1 => bad.walls.get_mut(&host).unwrap().header.schema_version -= 1,
            _ => bad.walls.get_mut(&host).unwrap().header.id = Id::new(),
        }
        assert!(crop.resolve_host(&bad).is_err());
    }
    model.walls.get_mut(&host).unwrap().parameters.path = WallPath::CircularArc {
        center: Point2::new(0.0, 0.0),
        radius: 4.0,
        start_angle_rad: 0.0,
        signed_sweep_rad: 1.0,
    };
    model.validate().unwrap();
    assert!(
        crop.resolve_host(&model)
            .unwrap_err()
            .to_string()
            .contains("straight")
    );
    model.walls.remove(&host);
    model.validate().unwrap();
    assert!(
        crop.resolve_host(&model)
            .unwrap_err()
            .to_string()
            .contains("missing")
    );
    assert_eq!(model.views[&view_id], view);
}

#[test]
fn elevation_kind_and_settings_must_agree() {
    let mut view = ViewParams::floor_plan("Plan", Id::new());
    view.elevation = Some(settings(Id::new()));
    assert!(view.validate().is_err());
    view.kind = ViewKind::Elevation;
    assert!(view.validate().is_err());
    view.plan = None;
    view.level = None;
    view.validate_edit().unwrap();
    view.elevation = None;
    assert!(view.validate().is_err());
}
