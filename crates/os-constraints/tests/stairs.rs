use os_constraints::affected_entities;
use os_core::{Id, Point2};
use os_model::{
    Level, LevelParams, Material, MaterialParams, Model, Stair, StairParams, View, ViewParams,
};
use std::collections::BTreeSet;

#[test]
fn stair_levels_and_material_invalidate_its_plan_and_derived_geometry() {
    let mut model = Model::new("Stair dependencies");
    let lower = *model.levels.keys().next().unwrap();
    let building = model.levels[&lower].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.2,
            building,
        },
    );
    let upper_id = upper.id();
    model.levels.insert(upper_id, upper);
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Concrete".into(),
            density_kg_m3: 2400.0,
            color: [160, 160, 160],
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    let stair = Stair::new(
        "core.stair",
        StairParams {
            name: "Main stair".into(),
            lower_level: lower,
            upper_level: upper_id,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(3.6, 0.0),
            width: 0.9,
            riser_count: 18,
            structural_thickness: 0.16,
            material: Some(material_id),
        },
    );
    let stair_id = stair.id();
    model.stairs.insert(stair_id, stair);
    let view = View::new("core.view", ViewParams::floor_plan("Ground", lower));
    let view_id = view.id();
    model.views.insert(view_id, view);
    model.validate().unwrap();

    for changed in [lower, upper_id, material_id, stair_id] {
        let affected = affected_entities(&model, &BTreeSet::from([changed]));
        assert!(affected.contains(&stair_id), "changed {changed}");
        assert!(affected.contains(&view_id), "changed {changed}");
    }
    assert!(affected_entities(&Model::new("Empty"), &BTreeSet::from([Id::new()])).is_empty());
}
