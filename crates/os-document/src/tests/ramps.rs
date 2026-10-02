use super::*;
use os_core::Point2;

fn ramp(lower: Id, upper: Id) -> Ramp {
    Ramp::new(
        "core.ramp",
        RampParams {
            name: "Entry ramp".into(),
            lower_level: lower,
            upper_level: upper,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(6.0, 1.0),
            width: 1.4,
            structural_thickness: 0.18,
            material: None,
        },
    )
}

#[test]
fn ramp_commands_are_atomic_phaseable_and_undoable_with_stable_identity() {
    let mut document = Document::new("Ramp transaction").unwrap();
    let before = document.model().clone();
    let lower = *before.levels.keys().next().unwrap();
    let building = before.levels[&lower].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 1.0,
            building,
        },
    );
    let ramp = ramp(lower, upper.id());
    let id = ramp.id();
    document
        .execute(
            "Place ramp",
            vec![Command::AddLevel(upper), Command::AddRamp(ramp.clone())],
        )
        .unwrap();
    assert!(document.model().is_phaseable_element(id));
    let event = document.drain_events().pop().unwrap();
    assert!(event.changed.contains(&id));
    assert!(event.invalidated.contains(&id));

    let mut edited = ramp.parameters.clone();
    edited.width = 1.8;
    document
        .execute(
            "Edit ramp",
            vec![Command::UpdateRamp {
                id,
                parameters: edited.clone(),
            }],
        )
        .unwrap();
    assert_eq!(document.model().ramps[&id].header, ramp.header);
    assert_eq!(document.model().ramps[&id].parameters, edited);
    assert!(document.undo());
    assert_eq!(document.model().ramps[&id], ramp);
    assert!(document.redo());
    assert_eq!(document.model().ramps[&id].parameters.width, 1.8);

    let committed = document.model().clone();
    let mut invalid = edited;
    invalid.width = 0.0;
    assert!(
        document
            .execute(
                "Invalid ramp edit",
                vec![Command::UpdateRamp {
                    id,
                    parameters: invalid,
                }],
            )
            .is_err()
    );
    assert_eq!(document.model(), &committed);
    document
        .execute("Remove ramp", vec![Command::RemoveRamp(id)])
        .unwrap();
    assert!(!document.model().ramps.contains_key(&id));
    assert!(document.undo());
    assert_eq!(document.model().ramps[&id].parameters.width, 1.8);
    assert!(document.redo());
    assert!(!document.model().ramps.contains_key(&id));
}

#[test]
fn ramp_references_are_validated_and_level_updates_invalidate_geometry() {
    let mut document = Document::new("Ramp dependencies").unwrap();
    let lower = *document.model().levels.keys().next().unwrap();
    let building = document.model().levels[&lower].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 1.0,
            building,
        },
    );
    let upper_id = upper.id();
    let ramp = ramp(lower, upper_id);
    let id = ramp.id();
    document
        .execute(
            "Place ramp",
            vec![Command::AddLevel(upper), Command::AddRamp(ramp)],
        )
        .unwrap();
    document.drain_events();

    let mut parameters = document.model().levels[&upper_id].parameters.clone();
    parameters.elevation = 1.4;
    document
        .execute(
            "Raise upper level",
            vec![Command::UpdateLevel {
                id: upper_id,
                parameters,
            }],
        )
        .unwrap();
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&id));
    assert!(
        document.model().ramps[&id]
            .parameters
            .dimensions_in(document.model())
            .is_ok()
    );

    let before = document.model().clone();
    assert!(
        document
            .execute(
                "Remove referenced level",
                vec![Command::RemoveLevel(upper_id)]
            )
            .is_err()
    );
    assert_eq!(document.model(), &before);
}
