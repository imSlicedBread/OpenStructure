use os_core::Point2;
use os_model::{Model, Wall, WallParams};
use os_storage::migrate;
use serde_json::Value;

fn schema_43_fixture() -> Value {
    let mut model = Model::new("Legacy phases");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Existing wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    model.walls.insert(wall.id(), wall);
    let mut value = serde_json::to_value(model).unwrap();
    value.as_object_mut().unwrap().remove("phases");
    value.as_object_mut().unwrap().remove("element_lifecycles");
    fn set_entity_headers(value: &mut Value) {
        if let Some(object) = value.as_object_mut() {
            if let Some(header) = object.get_mut("header").and_then(Value::as_object_mut) {
                header.insert("schema_version".into(), 43.into());
            }
            for child in object.values_mut() {
                set_entity_headers(child);
            }
        } else if let Some(array) = value.as_array_mut() {
            for child in array {
                set_entity_headers(child);
            }
        }
    }
    set_entity_headers(&mut value);
    value["schema_version"] = 43.into();
    value
}

#[test]
fn schema_43_migration_assigns_legacy_elements_to_existing_and_round_trips() {
    let mut value = schema_43_fixture();
    let mut reopened_legacy = value.clone();
    let wall_id = value["walls"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    migrate(&mut value, 43).unwrap();
    migrate(&mut reopened_legacy, 43).unwrap();
    assert_eq!(
        value, reopened_legacy,
        "migration identities are stable across opens"
    );
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, os_model::SCHEMA_VERSION);
    assert_eq!(model.phases.len(), 2);
    let existing = model.ordered_phases()[0].id();
    let wall_id: os_core::Id = serde_json::from_value(Value::String(wall_id)).unwrap();
    assert_eq!(model.element_lifecycles[&wall_id].created_in, existing);
    assert_eq!(
        model.phase_status(wall_id, existing).unwrap(),
        os_model::PhaseStatus::Existing
    );
    assert_eq!(
        serde_json::from_value::<Model>(serde_json::to_value(&model).unwrap()).unwrap(),
        model
    );
}

#[test]
fn ambiguous_schema_43_phase_fields_reject_without_mutating_input() {
    let mut value = schema_43_fixture();
    value["phases"] = serde_json::json!({});
    let before = value.clone();
    assert!(migrate(&mut value, 43).is_err());
    assert_eq!(value, before);
}
