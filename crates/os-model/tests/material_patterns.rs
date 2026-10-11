use os_core::Id;
use os_model::*;

fn pattern() -> MaterialPattern {
    MaterialPattern::new(
        "core.material_pattern",
        MaterialPatternParams {
            name: "Masonry".into(),
            angle_degrees: 45.,
            spacing: 0.1,
            origin: [0.; 3],
        },
    )
}

#[test]
fn pattern_validation_identity_references_and_bounds() {
    let mut model = Model::new("Patterns");
    let pattern = pattern();
    let id = pattern.id();
    model.material_patterns.insert(id, pattern.clone());
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Brick".into(),
            density_kg_m3: 1800.,
            color: [160, 80, 60],
            cut_pattern: Some(id),
        },
    );
    model.materials.insert(material.id(), material.clone());
    model.validate().unwrap();
    for bad in [f64::NAN, f64::INFINITY, -1., 0., 0.0001, 1001.] {
        let mut p = pattern.parameters.clone();
        p.spacing = bad;
        assert!(p.validate().is_err());
    }
    for bad in [f64::NAN, -1., 180.] {
        let mut p = pattern.parameters.clone();
        p.angle_degrees = bad;
        assert!(p.validate().is_err());
    }
    for bad in [f64::INFINITY, 1_000_001.] {
        let mut p = pattern.parameters.clone();
        p.origin[1] = bad;
        assert!(p.validate().is_err());
    }
    model.material_patterns.get_mut(&id).unwrap().header.id = Id::new();
    assert!(model.validate().is_err());
    model.material_patterns.insert(id, pattern);
    model
        .materials
        .get_mut(&material.id())
        .unwrap()
        .parameters
        .cut_pattern = Some(Id::new());
    assert!(model.validate().is_err());
    model
        .materials
        .get_mut(&material.id())
        .unwrap()
        .parameters
        .cut_pattern = None;
    for _ in 0..MAX_MATERIAL_PATTERNS {
        let p = MaterialPattern::new(
            "core.material_pattern",
            model.material_patterns[&id].parameters.clone(),
        );
        model.material_patterns.insert(p.id(), p);
    }
    assert!(model.validate().is_err());
    let old: MaterialParams =
        serde_json::from_str(r#"{"name":"Legacy","density_kg_m3":1000,"color":[1,2,3]}"#).unwrap();
    assert_eq!(old.cut_pattern, None);
}
