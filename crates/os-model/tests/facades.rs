use os_core::{Id, Point2};
use os_model::*;
#[test]
fn facade_settings_validate_world_basis_depth_version_and_strict_fields() {
    let s = FacadeViewSettings::new(Point2::new(2., 3.), Point2::new(8., 11.), -1., 12.).unwrap();
    assert_eq!(s.axis, Point2::new(0.6, 0.8));
    assert_eq!(s.marker_end(), Point2::new(8., 11.));
    assert_eq!(
        serde_json::from_str::<FacadeViewSettings>(&serde_json::to_string(&s).unwrap()).unwrap(),
        s
    );
    for case in 0..7 {
        let mut bad = s;
        match case {
            0 => bad.schema_version = 2,
            1 => bad.axis.x = 2.,
            2 => bad.near_depth = -1.,
            3 => bad.far_depth = bad.near_depth,
            4 => bad.max_station = bad.min_station,
            5 => bad.top_elevation = f64::NAN,
            _ => bad.marker_length = 0.,
        };
        assert!(bad.validate().is_err());
    }
    let mut value = serde_json::to_value(s).unwrap();
    value["origin"]["z"] = 1.into();
    assert!(serde_json::from_value::<FacadeViewSettings>(value).is_err());
    let mut p = ViewParams {
        drafting: None,
        name: "Facade".into(),
        kind: ViewKind::Facade,
        level: Some(Id::new()),
        settings_revision: 0,
        plan: None,
        section: None,
        elevation: None,
        facade: Some(s),
    };
    p.validate_edit().unwrap();
    p.level = None;
    assert!(p.validate_edit().is_err());
    p.level = Some(Id::new());
    p.kind = ViewKind::Elevation;
    assert!(p.validate().is_err());
}
