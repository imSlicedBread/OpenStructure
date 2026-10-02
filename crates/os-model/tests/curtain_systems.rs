#[path = "support/curtains.rs"]
mod support;
use os_core::Id;
use os_model::*;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn curtain_components_have_disjoint_clear_envelopes_and_finite_quantities() {
    let (mut model, assembly) = support::fixture();
    let components = assembly.parameters.resolve(&model).unwrap();
    assert_eq!(components.len(), 13);
    for (i, a) in components.iter().enumerate() {
        assert!(a.volume > 0. && a.volume.is_finite());
        for b in components.iter().skip(i + 1) {
            assert!((0..3).any(|axis| a.max[axis] <= b.min[axis] || b.max[axis] <= a.min[axis]));
        }
    }
    let left = components
        .iter()
        .find(|c| c.id == assembly.parameters.mullions[0].id)
        .unwrap();
    assert_eq!(left.min[0], 0.);
    assert_eq!(left.max[0], 0.05);
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Overflow".into(),
            density_kg_m3: f64::MAX,
            color: [0; 3],
        },
    );
    for ty in model.curtain_mullion_types.values_mut() {
        ty.parameters.material = Some(material.id());
    }
    model.materials.insert(material.id(), material);
    let mut p = assembly.parameters;
    p.height = 100.;
    p.horizontal.last_mut().unwrap().position = 100.;
    assert!(p.resolve(&model).is_err());
}

#[test]
fn curtain_child_references_are_valid_and_collisions_are_rejected() {
    let (mut model, assembly) = support::fixture();
    let child = assembly.parameters.panels[0].id;
    model.curtain_systems.insert(assembly.id(), assembly);
    model.plugin_requirements.insert(
        "org.example.test".into(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    let ext = ExtensionEntity {
        envelope_version: 1,
        id: Id::new(),
        owner: "org.example.test".into(),
        type_id: "org.example.test.reference".into(),
        name: "Reference".into(),
        payload_schema_version: 1,
        relationships: BTreeMap::new(),
        depends_on: BTreeSet::from([child]),
        payload: serde_json::json!({}),
    };
    model.extensions.insert(ext.id, ext.clone());
    model.validate().unwrap();
    model.extensions.clear();
    let mut collision = ext;
    collision.id = child;
    model.extensions.insert(child, collision);
    assert!(model.validate().is_err());
}

#[test]
fn curtain_reconcile_rejects_duplicate_topology_without_mutating() {
    let (_, assembly) = support::fixture();
    let mut p = assembly.parameters;
    let mut duplicate = p.panels[0].clone();
    duplicate.id = Id::new();
    p.panels.push(duplicate);
    let before = p.clone();
    assert!(p.reconcile().is_err());
    assert_eq!(p, before);
}

#[test]
fn curtain_model_rejects_world_corner_escape_and_uses_reflected_normal() {
    let (mut model, mut assembly) = support::fixture();
    let id = assembly.id();
    assembly.parameters.start = os_core::Point2::new(1e6, 0.);
    assembly.parameters.end = os_core::Point2::new(1e6, 4.);
    for flip in [false, true] {
        assembly.parameters.normal_flip = flip;
        model.curtain_systems.insert(id, assembly.clone());
        assert!(model.validate().is_err());
    }
    assembly.parameters.start.x -= 0.2;
    assembly.parameters.end.x -= 0.2;
    model.curtain_systems.insert(id, assembly.clone());
    model.validate().unwrap();
    let p = &mut assembly.parameters;
    p.normal_flip = false;
    let front = p.world_point([2., 0.1, 1.], 7.);
    p.normal_flip = true;
    let back = p.world_point([2., 0.1, 1.], 7.);
    assert!((front[0] - (p.start.x - 0.1)).abs() < 1e-8);
    assert!((back[0] - (p.start.x + 0.1)).abs() < 1e-8);
    assert!((front[1] - 2.).abs() < 1e-8);
    assert!((front[2] - 8.3).abs() < 1e-8);
}
