use os_constraints::affected_entities;
use os_core::{Id, Point2};
use os_model::{
    Ceiling, CeilingParams, Material, MaterialParams, Model, Room, RoomParams, View, ViewParams,
};
use std::collections::BTreeSet;

#[test]
fn ceiling_geometry_level_and_material_invalidate_derived_views() {
    let mut model = Model::new("Ceiling dependencies");
    let level = *model.levels.keys().next().unwrap();
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Ceiling paint".into(),
            density_kg_m3: 900.0,
            color: [230, 230, 230],
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Office".into(),
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            level,
            seed: Point2::new(1.0, 1.0),
            boundary_signature: vec![(Id::new(), true), (Id::new(), true), (Id::new(), false)],
        },
    );
    let room_id = room.id();
    model.rooms.insert(room_id, room);
    let ceiling = Ceiling::new(
        "core.ceiling",
        CeilingParams {
            name: "Ceiling".into(),
            level,
            material: Some(material_id),
            boundary_room: Some(room_id),
            boundary: vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 3.0),
                Point2::new(0.0, 3.0),
            ],
            holes: Vec::new(),
            thickness: 0.12,
            elevation_offset: 2.55,
        },
    );
    let ceiling_id = ceiling.id();
    model.ceilings.insert(ceiling_id, ceiling);
    let view = View::new(
        "core.view",
        ViewParams::reflected_ceiling_plan("RCP", level),
    );
    let view_id = view.id();
    model.views.insert(view_id, view);
    model.validate().unwrap();

    for changed in [level, material_id, ceiling_id, room_id] {
        let invalidated = affected_entities(&model, &BTreeSet::from([changed]));
        assert!(invalidated.contains(&ceiling_id), "changed {changed}");
        assert!(invalidated.contains(&view_id), "changed {changed}");
    }
    assert!(affected_entities(&Model::new("Empty"), &BTreeSet::from([Id::new()])).is_empty());
}
