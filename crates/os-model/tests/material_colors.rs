use os_model::MaterialParams;
use serde_json::json;

#[test]
fn material_color_requires_exactly_three_rgb_bytes() {
    for color in [[0, 0, 0], [255, 255, 255], [0, 128, 255]] {
        let params: MaterialParams = serde_json::from_value(json!({
            "name":"Paint", "density_kg_m3":1000., "color":color
        }))
        .unwrap();
        assert_eq!(params.color, color);
        assert_eq!(serde_json::to_value(params).unwrap()["color"], json!(color));
    }
    for color in [
        json!(null),
        json!([]),
        json!([0, 1]),
        json!([0, 1, 2, 3]),
        json!([-1, 1, 2]),
        json!([0, 1, 256]),
        json!([0, 0.5, 2]),
        json!("red"),
    ] {
        assert!(
            serde_json::from_value::<MaterialParams>(json!({
                "name":"Paint", "density_kg_m3":1000., "color":color
            }))
            .is_err()
        );
    }
    assert!(
        serde_json::from_value::<MaterialParams>(json!({
            "name":"Paint", "density_kg_m3":1000.
        }))
        .is_err()
    );
    assert!(
        serde_json::from_str::<MaterialParams>(
            r#"{"name":"Paint","density_kg_m3":1000,"color":[1,2,3],"color":[4,5,6]}"#
        )
        .is_err()
    );
}
