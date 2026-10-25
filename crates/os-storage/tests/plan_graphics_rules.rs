#[path = "../../os-model/tests/support/plan_graphics_rules.rs"]
mod support;
use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use support::*;

fn schema82() -> Value {
    // Frozen schema-79 payload, advanced through the documented additive maps.
    let mut wire: Value =
        serde_json::from_str(include_str!("fixtures/schema-79-hidden-openings.json")).unwrap();
    wire["plan_hidden_openings"] = json!({});
    wire["group_definitions"] = json!({});
    wire["group_instances"] = json!({});
    wire["plan_element_graphics_overrides"] = json!({});
    versions(&mut wire, 82);
    wire
}
fn versions(wire: &mut Value, version: u32) {
    wire["schema_version"] = version.into();
    for (name, data) in wire.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = version.into();
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = version.into();
            }
        }
    }
}

#[test]
fn rule_schema82_migration_is_exact_strict_and_atomic() {
    let before = schema82();
    let mut wire = before.clone();
    migrate(&mut wire, 82).unwrap();
    assert_eq!(wire["plan_graphics_rules"], json!({}));
    assert_eq!(wire["schema_version"], 83);
    serde_json::from_value::<Model>(wire.clone())
        .unwrap()
        .validate()
        .unwrap();
    wire.as_object_mut().unwrap().remove("plan_graphics_rules");
    versions(&mut wire, 82);
    assert_eq!(
        wire, before,
        "only the declared map/header changes are allowed"
    );
    for version in [79, 80, 81, 82] {
        for premature in [Value::Null, json!({}), json!([]), json!({"preserve":true})] {
            let mut wire = schema82();
            versions(&mut wire, version);
            wire["plan_graphics_rules"] = premature;
            let original = wire.clone();
            assert!(migrate(&mut wire, version).is_err());
            assert_eq!(wire, original);
        }
    }
    let mut wire = schema82();
    wire["project"]["header"]["schema_version"] = 81.into();
    let original = wire.clone();
    assert!(migrate(&mut wire, 82).is_err());
    assert_eq!(wire, original);
    let mut wire = serde_json::to_value(Model::new("Required map")).unwrap();
    wire.as_object_mut().unwrap().remove("plan_graphics_rules");
    assert!(serde_json::from_value::<Model>(wire).is_err());
}

#[test]
fn rule_wire_rejects_duplicates_unknown_fields_nil_ids_and_limits() {
    let (mut model, view, _) = fixture();
    let rule = rule(PlanRuleCategory::Doors);
    model.plan_graphics_rules.insert(view, vec![rule.clone()]);
    let original = serde_json::to_value(&model).unwrap();
    for case in 0..11 {
        let mut bad = original.clone();
        let row = &mut bad["plan_graphics_rules"][view.to_string()][0];
        match case {
            0 => row["matches"] = json!([]),
            1 => row["id"] = json!("00000000-0000-0000-0000-000000000000"),
            2 => row["category"] = json!("External"),
            3 => row["predicates"][0]["kind"] = json!("Script"),
            4 => row["predicates"][0]["path"] = json!("raw.width"),
            5 => row["predicates"] = json!([]),
            6 => row["predicates"] = json!(vec![rule.predicates[0].clone(); 9]),
            7 => row["name"] = json!("x".repeat(257)),
            8 => row["projected"]["weight_mm"] = json!(0.01),
            9 => {
                row["cut"] = Value::Null;
                row["projected"] = Value::Null;
            }
            _ => row["predicates"][0]["op"] = json!("Regex"),
        }
        assert!(serde_json::from_value::<Model>(bad).is_err(), "case {case}");
    }
    let mut bad = original.clone();
    bad["plan_graphics_rules"][view.to_string()] = json!([rule.clone(), rule.clone()]);
    assert!(serde_json::from_value::<Model>(bad).is_err());
    let mut bad = original.clone();
    bad["plan_graphics_rules"][view.to_string()] = json!(
        (0..33)
            .map(|_| support::rule(PlanRuleCategory::Doors))
            .collect::<Vec<_>>()
    );
    assert!(serde_json::from_value::<Model>(bad).is_err());
    let mut bad = original.clone();
    bad["plan_graphics_rules"][view.to_string()] = json!([]);
    assert!(serde_json::from_value::<Model>(bad).is_err());
    let mut empty = model.clone();
    empty.plan_graphics_rules.clear();
    let raw = serde_json::to_string(&empty).unwrap();
    let rows = serde_json::to_string(&vec![rule]).unwrap();
    let duplicate = format!("\"plan_graphics_rules\":{{\"{view}\":{rows},\"{view}\":{rows}}}");
    assert!(
        serde_json::from_str::<Model>(&raw.replace("\"plan_graphics_rules\":{}", &duplicate))
            .is_err()
    );
}

#[test]
fn rule_archive_roundtrip_retains_definitions_and_reevaluates_matches() {
    let (mut model, view, [wall, ..]) = fixture();
    model
        .plan_graphics_rules
        .insert(view, vec![rule(PlanRuleCategory::Walls)]);
    let document = Document::from_model(model.clone()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rules.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), &model);
    assert!(
        reopened
            .model()
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .contains_key(&wall)
    );
    assert!(
        model.estimated_memory_bytes() > {
            let mut empty = model.clone();
            empty.plan_graphics_rules.clear();
            empty.estimated_memory_bytes()
        }
    );
}
