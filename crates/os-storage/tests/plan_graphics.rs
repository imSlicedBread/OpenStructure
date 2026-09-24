use os_model::{Model, SCHEMA_VERSION};
use os_storage::migrate;
use serde_json::{Value, json};

fn schema_27_model() -> Value {
    let mut value = serde_json::to_value(Model::new("Legacy graphics")).unwrap();
    value["schema_version"] = json!(27);
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
