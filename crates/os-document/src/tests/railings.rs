use super::*;
use os_core::Point2;
use os_model::{
    Level, LevelParams, Stair, StairParams, StairRailing, StairRailingParams, StairRailingSide,
    StairRailingType, StairRailingTypeParams, View, ViewParams,
};

fn fixture() -> (
    Document,
    Id,
    Id,
    Id,
    Id,
    Stair,
    StairRailing,
    StairRailingType,
) {
    let mut model = Model::new("Hosted railings");
    let lower = model.levels.values().next().unwrap().clone();
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.0,
            building: lower.parameters.building,
        },
    );
    let upper_id = upper.id();
    model.levels.insert(upper_id, upper);
    let view = View::new("core.view", ViewParams::floor_plan("Ground", lower.id()));
    let view_id = view.id();
    model.views.insert(view_id, view);
    let stair = Stair::new(
        "core.stair",
        StairParams {
            name: "Main stair".into(),
            lower_level: lower.id(),
            upper_level: upper_id,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.5, 0.0),
            width: 1.2,
            riser_count: 15,
            structural_thickness: 0.2,
            material: None,
        },
    );
    let stair_id = stair.id();
    let ty = StairRailingType::new(
        "core.railing_type",
        StairRailingTypeParams {
            name: "Standard".into(),
            top_rail_height: 0.95,
            top_rail_width: 0.05,
            top_rail_depth: 0.05,
            post_width: 0.08,
            post_depth: 0.05,
            max_post_spacing: 0.6,
            material: None,
        },
    );
    let type_id = ty.id();
    let railing = StairRailing::new(
        "core.railing",
        StairRailingParams {
            name: "Main stair Left railing".into(),
            stair: stair_id,
            railing_type: type_id,
            side: StairRailingSide::Left,
        },
    );
    let railing_id = railing.id();
    let mut document = Document::from_model(model).unwrap();
    document
        .execute(
            "Place stair railing",
            vec![
                Command::AddStair(stair.clone()),
                Command::AddRailingType(ty.clone()),
                Command::AddRailing(railing.clone()),
            ],
        )
        .unwrap();
    document.drain_events();
    (
        document, stair_id, type_id, railing_id, view_id, stair, railing, ty,
    )
}

#[test]
fn railing_host_and_type_changes_invalidate_one_undoable_drawing() {
    let (mut document, stair_id, type_id, railing_id, view_id, stair, railing, ty) = fixture();
    assert_eq!(document.history_stats().undo_entries, 1);
    assert_eq!(document.model().railings[&railing_id], railing);

    let mut changed_stair = stair.parameters.clone();
    changed_stair.end = Point2::new(5.0, 0.0);
    document
        .execute(
            "Edit host stair",
            vec![Command::UpdateStair {
                id: stair_id,
                parameters: changed_stair.clone(),
            }],
        )
        .unwrap();
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&railing_id));
    assert!(event.invalidated.contains(&view_id));
    assert_eq!(
        document.model().railings[&railing_id].header,
        railing.header
    );
    assert_eq!(document.model().stairs[&stair_id].parameters, changed_stair);

    let mut changed_type = ty.parameters.clone();
    changed_type.top_rail_height = 1.0;
    document
        .execute(
            "Edit shared railing type",
            vec![Command::UpdateRailingType {
                id: type_id,
                parameters: changed_type.clone(),
            }],
        )
        .unwrap();
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&railing_id));
    assert!(event.invalidated.contains(&view_id));
    assert_eq!(
        document.model().railing_types[&type_id].parameters,
        changed_type
    );

    assert!(document.undo());
    assert_eq!(
        document.model().railing_types[&type_id].parameters,
        ty.parameters
    );
    assert_eq!(
        document.model().railings[&railing_id].header,
        railing.header
    );
    assert!(document.redo());
    assert_eq!(
        document.model().railing_types[&type_id].parameters,
        changed_type
    );
}

#[test]
fn railing_tracks_level_changes_and_rejects_duplicate_sides_atomically() {
    let (mut document, stair_id, type_id, railing_id, view_id, stair, railing, _) = fixture();
    let upper_level = stair.parameters.upper_level;
    let mut level = document.model().levels[&upper_level].parameters.clone();
    level.elevation = 3.25;
    document
        .execute(
            "Raise stair level",
            vec![Command::UpdateLevel {
                id: upper_level,
                parameters: level,
            }],
        )
        .unwrap();
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&stair_id));
    assert!(event.invalidated.contains(&railing_id));
    assert!(event.invalidated.contains(&view_id));
    assert_eq!(
        document.model().railings[&railing_id].header,
        railing.header
    );
    assert!(document.undo());
    document.drain_events();

    let duplicate = StairRailing::new(
        "core.railing",
        StairRailingParams {
            name: "Duplicate left railing".into(),
            stair: stair_id,
            railing_type: type_id,
            side: StairRailingSide::Left,
        },
    );
    let before = document.model().clone();
    let revision = document.revision();
    assert!(
        document
            .execute("Duplicate side", vec![Command::AddRailing(duplicate)])
            .is_err()
    );
    assert_eq!(document.model(), &before);
    assert_eq!(document.revision(), revision);
    assert!(document.drain_events().is_empty());
}

#[test]
fn host_and_type_deletion_are_atomic_and_orphans_are_rejected() {
    let (mut document, stair_id, type_id, railing_id, _, _, _, _) = fixture();
    let before = document.model().clone();
    let revision = document.revision();
    assert!(
        document
            .execute("Orphan railing", vec![Command::RemoveStair(stair_id)])
            .is_err()
    );
    assert_eq!(document.model(), &before);
    assert_eq!(document.revision(), revision);
    assert!(document.drain_events().is_empty());
    assert!(
        document
            .execute("Orphan type", vec![Command::RemoveRailingType(type_id)])
            .is_err()
    );
    assert_eq!(document.model(), &before);

    document
        .execute(
            "Delete stair and hosted railing",
            vec![
                Command::RemoveRailing(railing_id),
                Command::RemoveStair(stair_id),
            ],
        )
        .unwrap();
    assert!(!document.model().stairs.contains_key(&stair_id));
    assert!(!document.model().railings.contains_key(&railing_id));
    assert!(document.model().railing_types.contains_key(&type_id));
    assert!(document.undo());
    assert!(document.model().stairs.contains_key(&stair_id));
    assert_eq!(document.model().railings[&railing_id].id(), railing_id);
    assert!(document.redo());
}
