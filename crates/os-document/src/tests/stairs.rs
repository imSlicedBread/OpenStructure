use super::*;
use os_core::Point2;

fn fixture() -> (Document, Stair, Id, Id, Id, Id) {
    let mut model = Model::new("Stairs");
    let lower = *model.levels.keys().next().unwrap();
    let building = model.levels[&lower].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Level 2".into(),
            elevation: 3.2,
            building,
        },
    );
    let upper_id = upper.id();
    model.levels.insert(upper_id, upper);
    let site = model.sites.values().next().unwrap().id();
    let other_building = Building::new(
        "core.building",
        BuildingParams {
            name: "Other building".into(),
            site,
        },
    );
    let other_building_id = other_building.id();
    model.buildings.insert(other_building_id, other_building);
    let other_level = Level::new(
        "core.level",
        LevelParams {
            name: "Other level".into(),
            elevation: 3.2,
            building: other_building_id,
        },
    );
    let other_level_id = other_level.id();
    model.levels.insert(other_level_id, other_level);
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Concrete".into(),
            density_kg_m3: 2400.0,
            color: [170, 170, 170],
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    let stair = Stair::new(
        "core.stair",
        StairParams {
            name: "Stair 1".into(),
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
    let view = View::new("core.view", ViewParams::floor_plan("Ground", lower));
    let view_id = view.id();
    model.views.insert(view_id, view);
    (
        Document::from_model(model).unwrap(),
        stair,
        lower,
        upper_id,
        view_id,
        other_level_id,
    )
}

#[test]
fn stair_transactions_preserve_identity_regenerate_levels_and_invalidate_plan() {
    let (mut document, stair, lower, upper, view, _) = fixture();
    let original = document.model().clone();
    let id = stair.id();
    document
        .execute("Place stair", vec![Command::AddStair(stair.clone())])
        .unwrap();
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(event.invalidated.contains(&view));
    assert_eq!(document.history_stats().undo_entries, 1);
    assert!(document.undo());
    assert_eq!(document.model(), &original);
    assert!(document.redo());
    assert_eq!(document.model().stairs[&id], stair);

    let mut changed = stair.parameters.clone();
    changed.end = Point2::new(4.0, 0.0);
    changed.width = 1.0;
    document
        .execute(
            "Edit stair",
            vec![Command::UpdateStair {
                id,
                parameters: changed.clone(),
            }],
        )
        .unwrap();
    assert_eq!(document.model().stairs[&id].header, stair.header);
    assert_eq!(document.model().stairs[&id].parameters, changed);
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(event.invalidated.contains(&view));
    assert!(document.undo());
    assert_eq!(document.model().stairs[&id], stair);
    assert!(document.redo());

    let mut upper_parameters = document.model().levels[&upper].parameters.clone();
    upper_parameters.elevation = 3.4;
    document
        .execute(
            "Raise upper level",
            vec![Command::UpdateLevel {
                id: upper,
                parameters: upper_parameters,
            }],
        )
        .unwrap();
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(event.invalidated.contains(&view));

    document
        .execute("Delete stair", vec![Command::RemoveStair(id)])
        .unwrap();
    assert!(!document.model().stairs.contains_key(&id));
    assert!(document.undo());
    assert_eq!(document.model().stairs[&id].parameters, changed);
    assert!(document.model().levels.contains_key(&lower));
}

#[test]
fn invalid_stair_edits_and_level_removal_are_atomic() {
    let (mut document, stair, lower, _upper, _, other_level) = fixture();
    let id = stair.id();
    document
        .execute("Place stair", vec![Command::AddStair(stair.clone())])
        .unwrap();
    document.drain_events();
    let before = document.model().clone();
    let mut changed = stair.parameters.clone();
    changed.end = changed.start;
    let mut same_level = stair.parameters.clone();
    same_level.upper_level = lower;
    let mut too_few = stair.parameters.clone();
    too_few.riser_count = 0;
    let mut invalid_material = stair.parameters.clone();
    invalid_material.material = Some(Id::new());
    let mut wrong_building = stair.parameters.clone();
    wrong_building.upper_level = other_level;
    let before_invalid = document.model().clone();
    let revision = document.revision();
    let entries = document.history_stats().undo_entries;
    for parameters in [
        changed,
        same_level,
        too_few,
        invalid_material,
        wrong_building,
    ] {
        assert!(
            document
                .execute(
                    "Invalid stair",
                    vec![Command::UpdateStair { id, parameters }]
                )
                .is_err()
        );
        assert_eq!(document.model(), &before_invalid);
        assert_eq!(document.revision(), revision);
        assert_eq!(document.history_stats().undo_entries, entries);
        assert!(document.drain_events().is_empty());
    }
    assert!(
        document
            .execute("Remove lower level", vec![Command::RemoveLevel(lower)])
            .is_err()
    );
    assert_eq!(document.model(), &before_invalid);
    assert_eq!(document.revision(), revision);
    assert_eq!(document.history_stats().undo_entries, entries);
    assert_eq!(before.stairs[&id], stair);
}
