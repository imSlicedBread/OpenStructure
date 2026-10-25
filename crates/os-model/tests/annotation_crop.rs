use os_core::Point2;
use os_model::*;

fn rect(r: f64) -> PlanViewCrop {
    PlanViewCrop {
        min: Point2::new(-r, -r),
        max: Point2::new(r, r),
    }
}

#[test]
fn annotation_crop_is_optional_floor_plan_only_and_contains_model_crop() {
    let legacy = PlanSettings::default();
    assert!(
        !serde_json::to_value(legacy)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("annotation_crop")
    );
    assert_eq!(
        serde_json::from_value::<PlanSettings>(serde_json::to_value(legacy).unwrap()).unwrap(),
        legacy
    );
    let valid = PlanSettings {
        crop: Some(rect(1.)),
        annotation_crop: Some(rect(2.)),
        ..legacy
    };
    valid.validate().unwrap();
    for mut bad in [valid; 7].into_iter().enumerate() {
        match bad.0 {
            0 => bad.1.crop = None,
            1 => bad.1.crop = Some(rect(3.)),
            2 => bad.1.annotation_crop = Some(rect(0.5)),
            3 => bad.1.annotation_crop.as_mut().unwrap().min.x = f64::NAN,
            4 => bad.1.annotation_crop.as_mut().unwrap().max.y = f64::INFINITY,
            5 => bad.1.annotation_crop.as_mut().unwrap().max.x = -2.,
            _ => {
                bad.1.view_type = PlanViewType::ReflectedCeilingPlan;
                bad.1.range = PlanSettings::reflected_ceiling().range;
            }
        }
        assert!(bad.1.validate().is_err(), "case {}", bad.0);
    }
    let mut unknown = serde_json::to_value(valid).unwrap();
    unknown["annotation_crop"]["min"]["z"] = 0.into();
    assert!(serde_json::from_value::<PlanSettings>(unknown).is_err());
}
