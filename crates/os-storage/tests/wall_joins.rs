use os_model::{Model, SCHEMA_VERSION};
use os_storage::migrate;
use serde_json::{Value, json};

#[test]
fn frozen_twenty_preserves_explicit_butt_identity_and_rejects_ambiguous_payloads_atomically() {
    let old: Value =
        serde_json::from_str(include_str!("fixtures/schema-20-butt-join.json")).unwrap();
    let mut value = old.clone();
    migrate(&mut value, 20).unwrap();
    let model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    assert_eq!(model.wall_joins.len(), 1);
    let join = model.wall_joins.values().next().unwrap();
    assert_eq!(
        join.id().to_string(),
        "00000000-0000-4000-8000-000000000009"
    );
    assert_eq!(join.header.properties["intent"], "explicit");
    assert!(matches!(
        join.parameters,
        os_model::WallJoinParams::Butt { .. }
    ));
    for case in 0..4 {
        let mut bad = old.clone();
        let j = bad["wall_joins"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        match case {
            0 => j["parameters"]["kind"] = json!("Corner"),
            1 => j["header"]["type_id"] = json!("core.wall_join"),
            2 => j["header"]["schema_version"] = json!(19),
            _ => j["parameters"]["a"]["endpoint"] = json!("Interior"),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 20).is_err());
        assert_eq!(bad, before);
    }
}

#[test]
fn frozen_nineteen_migrates_without_inventing_joins_and_rejects_ambiguity() {
    let old: Value =
        serde_json::from_str(include_str!("fixtures/schema-19-touching-walls.json")).unwrap();
    let mut value = old.clone();
    migrate(&mut value, 19).unwrap();
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    assert!(model.wall_joins.is_empty());
    assert_eq!(model.walls.len(), 2);
    let mut expected = old.clone();
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["project"]["header"]["schema_version"] = json!(SCHEMA_VERSION);
    for map in ["sites", "buildings", "levels", "walls", "views"] {
        for entity in expected[map].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    expected["wall_joins"] = json!({});
    expected["wall_types"] = json!({});
    expected["wall_type_assignments"] = json!({});
    expected["columns"] = json!({});
    expected["plan_graphics_templates"] = json!({});
    expected["plan_graphics"] = json!({});
    assert_eq!(value, expected);
    for case in 0..5 {
        let mut bad = old.clone();
        match case {
            0 => bad["wall_joins"] = json!({}),
            1 => bad["wall_joins"] = Value::Null,
            2 => bad["project"]["header"]["schema_version"] = json!(18),
            3 => {
                bad.as_object_mut().unwrap().remove("room_separation_lines");
            }
            _ => {
                bad.as_object_mut().unwrap().remove("walls");
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 19).is_err());
        assert_eq!(bad, before);
    }
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
    let before = value.clone();
    assert!(migrate(&mut value, SCHEMA_VERSION).is_err());
    assert_eq!(value, before);
}
