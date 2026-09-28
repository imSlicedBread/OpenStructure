use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
mod common;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-42-dimension-anchors.json")).unwrap()
}

#[test]
fn schema_42_dimension_anchors_frozen_migration_and_roundtrip() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 42).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    let mut expected = original.clone();
    common::apply_wall_paths(&mut expected);
    for ty in expected["opening_types"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        ty["parameters"]["window_operation"] = json!("Fixed");
        ty["parameters"]["family"]["version"] = 5.into();
        ty["parameters"]["family"]["side_lite"] = Value::Null;
    }
    for schedule in expected["schedules"].as_object_mut().unwrap().values_mut() {
        schedule["parameters"]["group_by"] = json!([]);
        schedule["parameters"]["phase"] = json!({"mode": "legacy_unphased"});
        schedule["parameters"]["phase"] = json!({"mode": "legacy_unphased"});
    }
    for opening in expected["openings"].as_object_mut().unwrap().values_mut() {
        opening["parameters"]["pane_position_override"] = Value::Null;
        opening["parameters"]["lite_side_override"] = Value::Null;
    }
    for (id, view) in expected["views"].as_object_mut().unwrap() {
        let plan = &mut view["parameters"]["plan"];
        if !plan.is_null() {
            plan["schema_version"] = os_model::PLAN_SETTINGS_VERSION.into();
            plan["target_phase"] = value["views"][id]["parameters"]["plan"]["target_phase"].clone();
            plan["phase_filter"] = "ShowAll".into();
            plan["visibility"]["doors"] = true.into();
            plan["visibility"]["windows"] = true.into();
        }
    }
    expected["schema_version"] = SCHEMA_VERSION.into();
    for (key, data) in expected.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
            }
        }
    }
    for dimension in expected["dimensions"].as_object_mut().unwrap().values_mut() {
        let p = &mut dimension["parameters"];
        for key in ["first", "second"] {
            p[key] = json!({"WallEndpoint": p[key].clone()});
        }
        for r in p["additional"].as_array_mut().unwrap() {
            *r = json!({"WallEndpoint": r.clone()});
        }
    }
    for key in expected.as_object().unwrap().keys() {
        assert_eq!(
            value.get(key),
            expected.get(key),
            "migration changed unexpected root field {key}"
        );
    }
    let mut model: Model = serde_json::from_value(value.clone()).unwrap();
    for d in model.dimensions.values().take(4) {
        d.parameters.validate_creation(&model).unwrap();
    }
    let id = *model.dimensions.keys().next().unwrap();
    let opening = *model.openings.keys().next().unwrap();
    let p = &mut model.dimensions.get_mut(&id).unwrap().parameters;
    p.first = DimensionReference::OpeningJamb {
        opening,
        jamb: DimensionJamb::Start,
    };
    p.second = DimensionReference::OpeningJamb {
        opening,
        jamb: DimensionJamb::End,
    };
    model.dimensions[&id]
        .parameters
        .validate_creation(&model)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jamb-dimensions.osb");
    let doc = Document::from_model(model).unwrap();
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}

#[test]
fn schema_42_ambiguous_or_malformed_references_reject_atomically() {
    for case in 0..12 {
        let mut bad = frozen();
        let dimensions = bad["dimensions"].as_object_mut().unwrap();
        // Fail after earlier dimensions have already been converted in the copy.
        let p = &mut dimensions.values_mut().last().unwrap()["parameters"];
        match case {
            0 => p["first"] = json!({"WallEndpoint":p["first"].clone()}),
            1 => p["first"]["opening"] = p["first"]["wall"].clone(),
            2 => p["first"]["jamb"] = "Start".into(),
            3 => p["first"]["endpoint"] = "Middle".into(),
            4 => p["first"]["wall"] = "not-a-uuid".into(),
            5 => {
                p["first"].as_object_mut().unwrap().remove("endpoint");
            }
            6 => p["second"] = Value::Null,
            7 => {
                p["additional"] = json!([{"OpeningJamb":{"opening":"00000000-0000-4000-8000-000000000008","jamb":"Start"}}])
            }
            8 => p["additional"] = json!({}),
            9 => p["first"]["point"] = json!({"x":0,"y":0}),
            10 => {
                bad["walls"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["header"]["schema_version"] = 41.into()
            }
            _ => {
                bad.as_object_mut().unwrap().remove("dimensions");
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 42).is_err(), "case {case}");
        assert_eq!(bad, before, "case {case}");
    }
}
