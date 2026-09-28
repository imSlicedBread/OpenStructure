use os_document::{Command, Document};
use os_model::{
    Model, SCHEMA_VERSION,
    WindowPanePosition::{Center, LeftFace, RightFace},
};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
mod common;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-45-window-panes.json")).unwrap()
}

#[test]
fn frozen_45_to_46_preserves_fields_and_pane_pins_across_save_reopen_and_type_edits() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 45).unwrap();
    let mut model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    let mut reversed = value;
    common::remove_opening_visibility(&mut reversed);
    common::reverse_wall_paths(&mut reversed);
    reversed["schema_version"] = 45.into();
    for (key, data) in reversed.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = 45.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = 45.into();
                }
                if key == "openings" {
                    assert_eq!(
                        entity["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("pane_position_override"),
                        Some(Value::Null)
                    );
                }
                if key == "schedules" {
                    assert_eq!(
                        entity["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("phase"),
                        Some(json!({"mode": "legacy_unphased"}))
                    );
                    assert_eq!(
                        entity["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("group_by"),
                        Some(json!([]))
                    );
                }
            }
        }
    }
    assert_eq!(reversed, original);
    let id = model
        .openings
        .values()
        .find(|o| o.parameters.type_id().is_some())
        .unwrap()
        .id();
    let ty = model.openings[&id].parameters.type_id().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for pane in [None, Some(Center), Some(LeftFace), Some(RightFace)] {
        model
            .openings
            .get_mut(&id)
            .unwrap()
            .parameters
            .pane_position_override = pane;
        let doc = Document::from_model(model.clone()).unwrap();
        let path = dir.path().join("panes.osb");
        ZipJsonStorage.save(&doc, &path).unwrap();
        let mut reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        let mut parameters = reopened.model().opening_types[&ty].parameters.clone();
        parameters.pane_position = LeftFace;
        reopened
            .execute(
                "Type pane",
                vec![Command::UpdateOpeningType { id: ty, parameters }],
            )
            .unwrap();
        assert_eq!(
            reopened
                .model()
                .resolve_opening(&reopened.model().openings[&id].parameters)
                .unwrap()
                .pane_position,
            pane.unwrap_or(LeftFace)
        );
    }
}

#[test]
fn pane_migration_rejects_partial_malformed_headers_collections_and_current_missing_field_atomically()
 {
    let original = frozen();
    let id = original["openings"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    for case in 0..8 {
        let mut bad = original.clone();
        match case {
            0 => bad["openings"][id]["parameters"]["pane_position_override"] = Value::Null,
            1 => bad["openings"][id]["parameters"]["pane_position_override"] = json!("LeftFace"),
            2 => bad["openings"][id]["parameters"] = json!([]),
            3 => bad["openings"][id]["header"]["schema_version"] = 46.into(),
            4 => bad["project"]["header"]["schema_version"] = 44.into(),
            5 => bad["walls"] = json!([]),
            6 => {
                bad.as_object_mut().unwrap().remove("phases");
            }
            _ => bad["schema_version"] = 44.into(),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 45).is_err(), "case {case}");
        assert_eq!(bad, before);
    }
    let mut current = original;
    migrate(&mut current, 45).unwrap();
    let id = current["openings"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    for malformed in [None, Some(json!("Left")), Some(json!(1)), Some(json!({}))] {
        let mut bad = current.clone();
        let p = bad["openings"][&id]["parameters"].as_object_mut().unwrap();
        if let Some(value) = malformed {
            p.insert("pane_position_override".into(), value);
        } else {
            p.remove("pane_position_override");
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, SCHEMA_VERSION).is_err());
        assert_eq!(bad, before);
        assert!(serde_json::from_value::<Model>(bad).is_err());
    }
}
