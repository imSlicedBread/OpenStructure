mod common;

use os_model::{Model, SCHEMA_VERSION};
use os_storage::migrate;
use serde_json::{Value, json};

fn schema_27_model() -> Value {
    let mut value = serde_json::to_value(Model::new("Legacy graphics")).unwrap();
    common::remove_phase_fields(&mut value);
    value.as_object_mut().unwrap().remove("stairs");
    value.as_object_mut().unwrap().remove("roofs");
    value["schema_version"] = json!(27);
    value.as_object_mut().unwrap().remove("opening_tags");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    for entity in value.as_object_mut().unwrap().values_mut() {
        if let Some(header) = entity.get_mut("header") {
            header["schema_version"] = json!(27);
        }
        if let Some(entities) = entity.as_object_mut() {
            for nested in entities.values_mut() {
                if let Some(header) = nested.get_mut("header") {
                    header["schema_version"] = json!(27);
                }
            }
        }
    }
    value
}

fn schema_81_model() -> Value {
    let mut value = serde_json::to_value(Model::new("Schema 81 graphics")).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_element_graphics_overrides");
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    value["schema_version"] = json!(81);
    for (name, data) in value.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = json!(81);
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map
                .values_mut()
                .filter(|entity| entity.get("header").is_some())
            {
                entity["header"]["schema_version"] = json!(81);
            }
        }
    }
    value
}

#[test]
fn schema_27_adds_empty_graphics_maps_and_rejects_ambiguous_payload_atomically() {
    let mut migrated = schema_27_model();
    migrate(&mut migrated, 27).unwrap();
    assert_eq!(migrated["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(migrated["plan_graphics_templates"], json!({}));
    assert_eq!(migrated["plan_graphics"], json!({}));
    let model: Model = serde_json::from_value(migrated).unwrap();
    model.validate().unwrap();

    let mut ambiguous = schema_27_model();
    ambiguous["plan_graphics"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 27).is_err());
    assert_eq!(ambiguous, before);
}

#[test]
fn schema_81_adds_element_overrides_and_rejects_unversioned_override_data() {
    let mut migrated = schema_81_model();
    migrate(&mut migrated, 81).unwrap();
    assert_eq!(migrated["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(migrated["plan_element_graphics_overrides"], json!({}));
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        json!(SCHEMA_VERSION)
    );
    let model: Model = serde_json::from_value(migrated).unwrap();
    model.validate().unwrap();

    let mut ambiguous = schema_81_model();
    ambiguous["plan_element_graphics_overrides"] = json!({"unknown-view": {"unknown-element": {
        "cut": null,
        "projected": null
    }}});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 81).is_err());
    assert_eq!(ambiguous, before);

    let mut malformed_current = serde_json::to_value(Model::new("Missing map")).unwrap();
    malformed_current
        .as_object_mut()
        .unwrap()
        .remove("plan_element_graphics_overrides");
    assert!(serde_json::from_value::<Model>(malformed_current).is_err());
}
