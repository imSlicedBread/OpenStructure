use os_document::{Command, Document};
use os_model::{Model, OpeningKind, OpeningState, WindowOperation};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
mod common;

fn legacy56() -> serde_json::Value {
    let mut wire: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-53-straight-walls.json")).unwrap();
    migrate(&mut wire, 53).unwrap();
    common::remove_door_leaves(&mut wire);
    for opening in wire["openings"].as_object_mut().unwrap().values_mut() {
        opening["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("open_state");
    }
    wire["schema_version"] = 56.into();
    for (name, data) in wire.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = 56.into();
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = 56.into();
            }
        }
    }
    wire
}

#[test]
fn opening_state_schema56_migration_is_strict_and_atomic() {
    let old = legacy56();
    let mut migrated = old.clone();
    migrate(&mut migrated, 56).unwrap();
    assert_eq!(migrated["schema_version"], os_model::SCHEMA_VERSION);
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    for opening in model.openings.values() {
        assert_eq!(opening.parameters.open_state, OpeningState::Default);
        assert_eq!(opening.header.schema_version, os_model::SCHEMA_VERSION);
        let id = opening.id().to_string();
        let mut params = migrated["openings"][&id]["parameters"].clone();
        params.as_object_mut().unwrap().remove("open_state");
        assert_eq!(params, old["openings"][&id]["parameters"]);
    }
    for corruption in 0..3 {
        let mut bad = old.clone();
        let opening = bad["openings"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        match corruption {
            0 => {
                opening["parameters"]["open_state"] = "Default".into();
            }
            1 => {
                opening["parameters"] = serde_json::Value::Null;
            }
            _ => {
                opening["header"]["schema_version"] = 55.into();
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 56).is_err());
        assert_eq!(bad, before);
    }
    let id = model.openings.keys().next().unwrap().to_string();
    migrated["openings"][&id]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("open_state");
    assert!(serde_json::from_value::<Model>(migrated).is_err());
}

#[test]
fn opening_state_save_reopen_all_tags_undo_and_invalid_atomicity() {
    let mut wire = legacy56();
    migrate(&mut wire, 56).unwrap();
    let mut doc = Document::from_model(serde_json::from_value(wire).unwrap()).unwrap();
    let window = doc
        .model()
        .openings
        .values()
        .find(|o| o.parameters.type_id().is_some())
        .unwrap()
        .id();
    let ty = doc.model().openings[&window].parameters.type_id().unwrap();
    let door = doc
        .model()
        .openings
        .values()
        .find(|o| doc.model().resolve_opening(&o.parameters).unwrap().kind == OpeningKind::Door)
        .unwrap()
        .id();
    for (operation, state) in [
        (WindowOperation::Sliding, OpeningState::SlidingFraction(0.5)),
        (WindowOperation::Casement, OpeningState::CasementAngle(45.)),
    ] {
        let before = doc.model().clone();
        let mut type_params = before.opening_types[&ty].parameters.clone();
        type_params.window_operation = operation;
        let mut window_params = before.openings[&window].parameters.clone();
        window_params.open_state = state;
        let mut door_params = before.openings[&door].parameters.clone();
        door_params.open_state = OpeningState::DoorAngle(30.);
        doc.execute(
            "Pose",
            vec![
                Command::UpdateOpeningType {
                    id: ty,
                    parameters: type_params,
                },
                Command::UpdateOpening {
                    id: window,
                    parameters: window_params,
                },
                Command::UpdateOpening {
                    id: door,
                    parameters: door_params,
                },
            ],
        )
        .unwrap();
        let after = doc.model().clone();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("poses.osb");
        ZipJsonStorage.save(&doc, &path).unwrap();
        let reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), &after);
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(doc.redo());
        assert_eq!(doc.model(), &after);
        let stats = doc.history_stats();
        let mut bad = after.openings[&window].parameters.clone();
        bad.open_state = OpeningState::DoorAngle(30.);
        assert!(
            doc.execute(
                "Bad",
                vec![Command::UpdateOpening {
                    id: window,
                    parameters: bad
                }]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &after);
        assert_eq!(doc.history_stats(), stats);
        // Reset before changing operation; incompatible tags never silently coerce.
        let mut reset = after.openings[&window].parameters.clone();
        reset.open_state = OpeningState::Default;
        doc.execute(
            "Reset",
            vec![Command::UpdateOpening {
                id: window,
                parameters: reset,
            }],
        )
        .unwrap();
    }
}
