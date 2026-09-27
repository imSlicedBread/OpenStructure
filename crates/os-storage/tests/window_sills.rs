mod common;

use os_document::{Command, Document};
use os_model::{Model, OpeningDefinition, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-28-window-sills.json")).unwrap()
}

#[test]
fn schema_28_window_sills_preserve_identity_geometry_extensions_and_reopen() {
    let original = frozen();
    let mut migrated = original.clone();
    migrate(&mut migrated, 28).unwrap();
    let mut model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert_eq!(migrated["extensions"], original["extensions"]);
    for opening in model.openings.values() {
        assert_eq!(opening.parameters.sill_override, None);
        let expected = match opening.parameters.definition {
            OpeningDefinition::Typed { .. } => 0.8,
            OpeningDefinition::Legacy { .. } => 1.1,
        };
        assert_eq!(
            model.resolve_opening(&opening.parameters).unwrap().sill,
            expected
        );
    }
    // Reversing only the declared additions must restore every original field.
    let mut reversed = migrated;
    reversed.as_object_mut().unwrap().remove("opening_tags");
    reversed["schema_version"] = json!(28);
    for (key, data) in reversed.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = json!(28);
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = json!(28);
                }
                if key == "openings" {
                    for field in ["width_override", "height_override"] {
                        assert_eq!(
                            entity["parameters"].as_object_mut().unwrap().remove(field),
                            Some(Value::Null)
                        );
                    }
                    assert_eq!(
                        entity["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("sill_override"),
                        Some(Value::Null)
                    );
                } else if key == "opening_types" {
                    let family = entity["parameters"]["family"].as_object_mut().unwrap();
                    family.insert("version".into(), json!(3));
                    family.remove("panel_material");
                    family.remove("frame_material");
                }
            }
        }
    }
    reversed.as_object_mut().unwrap().remove("stairs");
    reversed.as_object_mut().unwrap().remove("roofs");
    common::reverse_schema_38_migration(&mut reversed, &original);
    assert_eq!(reversed, original);
    let id = model
        .openings
        .values()
        .find(|o| o.parameters.type_id().is_some())
        .unwrap()
        .id();
    let ty = model.openings[&id].parameters.type_id().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for value in [None, Some(0.8), Some(1.3)] {
        model
            .openings
            .get_mut(&id)
            .unwrap()
            .parameters
            .sill_override = value;
        let doc = Document::from_model(model.clone()).unwrap();
        let path = dir.path().join("sills.osb");
        ZipJsonStorage.save(&doc, &path).unwrap();
        let mut reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        let mut parameters = reopened.model().opening_types[&ty].parameters.clone();
        parameters.sill = 0.9;
        reopened
            .execute(
                "Change default",
                vec![Command::UpdateOpeningType { id: ty, parameters }],
            )
            .unwrap();
        assert_eq!(
            reopened
                .model()
                .resolve_opening(&reopened.model().openings[&id].parameters)
                .unwrap()
                .sill,
            value.unwrap_or(0.9)
        );
    }
}

#[test]
fn schema_28_ambiguity_and_current_missing_sill_are_atomic_errors() {
    let original = frozen();
    let id = original["openings"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    for value in [Value::Null, json!(0.8)] {
        let mut bad = original.clone();
        bad["openings"][id]["parameters"]["sill_override"] = value;
        let before = bad.clone();
        assert!(migrate(&mut bad, 28).is_err());
        assert_eq!(bad, before);
    }
    let mut current = original;
    migrate(&mut current, 28).unwrap();
    let id = current["openings"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    current["openings"][&id]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("sill_override");
    let before = current.clone();
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
    assert_eq!(current, before);
    assert!(serde_json::from_value::<Model>(current).is_err());
}
