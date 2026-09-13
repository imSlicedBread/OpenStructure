use os_core::Point2;
use os_model::{Grid, GridParams, Model};

#[test]
fn grid_parameters_reject_unsupported_json_fields() {
    let valid = Grid::new(
        "core.grid",
        GridParams {
            name: "A".into(),
            building: *Model::new("Grid").buildings.keys().next().unwrap(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 0.0),
        },
    );
    let value = serde_json::to_value(&valid.parameters).unwrap();
    assert_eq!(
        serde_json::from_value::<GridParams>(value.clone()).unwrap(),
        valid.parameters
    );
    for point in ["start", "end"] {
        let mut bad = value.clone();
        bad[point]["z"] = serde_json::json!(3);
        assert!(serde_json::from_value::<GridParams>(bad).is_err());
    }
    let mut bad = value;
    bad["curve"] = serde_json::json!("arc");
    assert!(serde_json::from_value::<GridParams>(bad).is_err());
}
