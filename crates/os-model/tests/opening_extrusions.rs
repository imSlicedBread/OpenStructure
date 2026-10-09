use os_core::{Id, Point2};
use os_model::*;

#[test]
fn additive_extrusions_validate_identity_ring_frame_material_and_instance_size() {
    let mut family = OpeningFamily::default();
    family.extrusions.push(OpeningExtrusion::new("Handle"));
    family.validate_for(1., 2., OpeningKind::Door).unwrap();
    let saved = family.clone();
    family.extrusions.push(family.extrusions[0].clone());
    assert!(family.validate().is_err());
    family = saved.clone();
    family.extrusions[0].id =
        serde_json::from_str::<Id>("\"00000000-0000-0000-0000-000000000000\"").unwrap();
    assert!(family.validate().is_err());
    family = saved.clone();
    family.extrusions[0].profile[2] = family.extrusions[0].profile[0];
    assert!(family.validate().is_err());
    family = saved.clone();
    family.extrusions[0].position = Point2::new(2., 0.);
    assert!(family.validate_for(1., 2., OpeningKind::Door).is_err());
    assert!(saved.validate_for(0.001, 2., OpeningKind::Door).is_err());
    family = saved.clone();
    family.door_leaves = DoorLeaves::Paired {
        active_fraction: 0.5,
    };
    assert!(family.validate().is_err());
    family = saved.clone();
    family.extrusions[0].depth = 0.5001;
    assert!(family.validate().is_err());
    family = saved.clone();
    family.extrusions[0].normal_offset = f64::NAN;
    assert!(family.validate().is_err());
    family = saved.clone();
    family.remap_extrusion_ids();
    assert_ne!(family.extrusions[0].id, saved.extrusions[0].id);
    for _ in 0..OpeningFamily::MAX_EXTRUSIONS {
        family.extrusions.push(OpeningExtrusion::new("Extra"));
    }
    assert!(family.validate().is_err());
    for field in ["extrusions", "version"] {
        let mut json = serde_json::to_value(&saved).unwrap();
        json.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<OpeningFamily>(json).is_err());
    }
    let mut json = serde_json::to_value(&saved).unwrap();
    json["extrusions"][0]["profile"][0]["unknown"] = true.into();
    assert!(serde_json::from_value::<OpeningFamily>(json).is_err());
}
