use super::*;
use os_core::Point2;
use os_model::{Casework, CaseworkParams, CaseworkType, CaseworkTypeParams, Model};

fn fixture() -> (Document, CaseworkType, Casework, Casework) {
    let mut model = Model::new("Casework transactions");
    let level = *model.levels.keys().next().unwrap();
    let casework_type = CaseworkType::new(
        "core.casework_type",
        CaseworkTypeParams {
            name: "Base cabinet".into(),
            width: 0.6,
            depth: 0.6,
            height: 0.9,
            material: None,
        },
    );
    let type_id = casework_type.id();
    let first = Casework::new(
        "core.casework",
        CaseworkParams {
            name: "Cabinet A".into(),
            type_id,
            level,
            center: Point2::new(2.0, 3.0),
            yaw: 0.0,
            base_offset: 0.0,
        },
    );
    let second = Casework::new(
        "core.casework",
        CaseworkParams {
            name: "Cabinet B".into(),
            type_id,
            level,
            center: Point2::new(4.0, 3.0),
            yaw: std::f64::consts::FRAC_PI_2,
            base_offset: 0.0,
        },
    );
    let mut document = Document::from_model(model.clone()).unwrap();
    document
        .execute(
            "Place casework",
            vec![
                Command::AddCaseworkType(casework_type.clone()),
                Command::AddCasework(first.clone()),
                Command::AddCasework(second.clone()),
            ],
        )
        .unwrap();
    model = document.model().clone();
    model.validate().unwrap();
    (document, casework_type, first, second)
}

#[test]
fn shared_type_edits_preserve_instance_identity_and_undo_as_one_step() {
    let (mut document, original_type, first, second) = fixture();
    let type_id = original_type.id();
    let first_id = first.id();
    let second_id = second.id();
    let before = document.model().clone();
    let mut updated = original_type.parameters.clone();
    updated.width = 0.75;
    document
        .execute(
            "Edit shared casework type",
            vec![Command::UpdateCaseworkType {
                id: type_id,
                parameters: updated.clone(),
            }],
        )
        .unwrap();
    assert_eq!(
        document.model().casework_types[&type_id].header,
        original_type.header
    );
    assert_eq!(
        document.model().casework_types[&type_id].parameters,
        updated
    );
    assert_eq!(document.model().casework[&first_id], first);
    assert_eq!(document.model().casework[&second_id], second);

    assert!(document.undo());
    assert_eq!(document.model(), &before);
    assert!(document.redo());
    assert_eq!(
        document.model().casework_types[&type_id].parameters.width,
        0.75
    );
}

#[test]
fn invalid_type_update_and_referenced_type_delete_are_atomic() {
    let (mut document, casework_type, _, _) = fixture();
    let type_id = casework_type.id();
    let before = document.model().clone();
    let history = document.history_stats();
    let mut invalid = casework_type.parameters.clone();
    invalid.width = 0.0;
    assert!(
        document
            .execute(
                "Invalid casework type",
                vec![Command::UpdateCaseworkType {
                    id: type_id,
                    parameters: invalid,
                }],
            )
            .is_err()
    );
    assert!(
        document
            .execute(
                "Delete referenced casework type",
                vec![Command::RemoveCaseworkType(type_id)]
            )
            .is_err()
    );
    assert_eq!(document.model(), &before);
    assert_eq!(document.history_stats(), history);

    let instance_id = document.model().casework.keys().next().copied().unwrap();
    document
        .execute(
            "Delete casework",
            vec![Command::RemoveCasework(instance_id)],
        )
        .unwrap();
    assert!(document.undo());
    assert!(document.model().casework.contains_key(&instance_id));
}
