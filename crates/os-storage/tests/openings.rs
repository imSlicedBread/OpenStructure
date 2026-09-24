use os_model::{Model, SCHEMA_VERSION};
use os_storage::migrate;
use serde_json::{Value, json};

// A schema-3 fixture supplies fixed identities and wall metadata; the old 3→4
// addition is reconstructed explicitly, independently of the migration under test.
fn schema_four() -> Value {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../fixtures/schema-3-pointer-walls.json"
    ))
    .unwrap();
    value["schema_version"] = json!(4);
    value["grids"] = json!({});
    value["project"]["header"]["schema_version"] = json!(4);
    for key in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "materials",
        "views",
    ] {
        for entity in value[key].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(4);
        }
    }
    value
}
#[test]
fn four_to_current_preserves_all_existing_fields_and_rejects_ambiguous_input_atomically() {
    let original = schema_four();
    let mut value = original.clone();
    migrate(&mut value, 4).unwrap();
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.openings.is_empty());
    assert!(model.opening_types.is_empty());
    value.as_object_mut().unwrap().remove("openings");
    value.as_object_mut().unwrap().remove("rooms");
    value.as_object_mut().unwrap().remove("dimensions");
    value.as_object_mut().unwrap().remove("opening_types");
    value.as_object_mut().unwrap().remove("floors");
    value.as_object_mut().unwrap().remove("room_tags");
    value.as_object_mut().unwrap().remove("sheets");
    value.as_object_mut().unwrap().remove("schedules");
    value.as_object_mut().unwrap().remove("detail_lines");
    value
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    value.as_object_mut().unwrap().remove("wall_joins");
    value.as_object_mut().unwrap().remove("wall_types");
    value
        .as_object_mut()
        .unwrap()
        .remove("wall_type_assignments");
    value.as_object_mut().unwrap().remove("columns");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    value["schema_version"] = json!(4);
    value["project"]["header"]["schema_version"] = json!(4);
    for key in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "grids",
        "materials",
        "views",
    ] {
        for entity in value[key].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(4);
        }
    }
    assert_eq!(value, original);
    for case in 0..3 {
        let mut bad = original.clone();
        match case {
            0 => bad["openings"] = json!({}),
            1 => bad["project"]["header"]["schema_version"] = json!(5),
            _ => bad["grids"] = json!([]),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 4).is_err());
        assert_eq!(bad, before);
    }
    let mut current = serde_json::to_value(Model::new("Missing map")).unwrap();
    current.as_object_mut().unwrap().remove("openings");
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
}
