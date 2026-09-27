use os_core::Point2;
use os_document::{Command, Document};
use os_model::{ElementLifecycle, Model, Wall, WallParams};

#[test]
fn new_elements_use_latest_phase_and_lifecycle_edits_are_atomic_and_undoable() {
    let mut document = Document::new("Phase test").unwrap();
    let existing = document.model().ordered_phases()[0].id();
    let new_construction = document.model().ordered_phases()[1].id();
    let level = *document.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Partition".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    document
        .execute("Add phased wall", vec![Command::AddWall(wall)])
        .unwrap();
    assert_eq!(
        document.model().element_lifecycles[&wall_id].created_in,
        new_construction
    );
    assert_eq!(
        document
            .model()
            .phase_status(wall_id, new_construction)
            .unwrap(),
        os_model::PhaseStatus::New
    );

    let revision = document.revision();
    assert!(
        document
            .execute("Remove first phase", vec![Command::RemovePhase(existing)])
            .is_err()
    );
    assert_eq!(document.revision(), revision);
    assert!(document.model().phases.contains_key(&existing));

    document
        .execute(
            "Demolish wall",
            vec![Command::SetElementLifecycle {
                element: wall_id,
                lifecycle: ElementLifecycle {
                    created_in: new_construction,
                    demolished_in: Some(new_construction),
                },
            }],
        )
        .unwrap();
    assert_eq!(
        document
            .model()
            .phase_status(wall_id, new_construction)
            .unwrap(),
        os_model::PhaseStatus::Temporary
    );
    assert!(document.undo());
    assert_eq!(
        document
            .model()
            .phase_status(wall_id, new_construction)
            .unwrap(),
        os_model::PhaseStatus::New
    );
    assert!(document.redo());
    assert_eq!(
        document
            .model()
            .phase_status(wall_id, new_construction)
            .unwrap(),
        os_model::PhaseStatus::Temporary
    );
}

#[test]
fn editing_implicit_existing_elements_does_not_reclassify_them_as_new() {
    let mut model = Model::new("Legacy phases");
    let existing = model.ordered_phases()[0].id();
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Legacy wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    model.walls.insert(wall_id, wall.clone());
    let mut document = Document::from_model(model).unwrap();

    let mut parameters = wall.parameters.clone();
    parameters.name = "Edited legacy wall".into();
    document
        .execute(
            "Edit legacy wall",
            vec![Command::UpdateWall {
                id: wall_id,
                parameters,
            }],
        )
        .unwrap();
    assert!(!document.model().element_lifecycles.contains_key(&wall_id));
    assert_eq!(
        document.model().phase_status(wall_id, existing).unwrap(),
        os_model::PhaseStatus::Existing
    );

    let mut replacement = wall;
    replacement.parameters.name = "Identity-preserving replacement".into();
    document
        .execute(
            "Replace legacy wall in one transaction",
            vec![Command::RemoveWall(wall_id), Command::AddWall(replacement)],
        )
        .unwrap();
    assert!(!document.model().element_lifecycles.contains_key(&wall_id));
    assert_eq!(
        document.model().phase_status(wall_id, existing).unwrap(),
        os_model::PhaseStatus::Existing
    );
}
