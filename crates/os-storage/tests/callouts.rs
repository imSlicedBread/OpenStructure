mod common;
#[path = "../../os-model/tests/support/callouts.rs"]
mod support;
use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use support::*;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-73-callouts.json")).unwrap()
}

#[test]
fn callout_frozen_schema73_migration_preserves_data_and_assigns_ordered_detail_numbers() {
    let old = frozen();
    assert_eq!(old["schema_version"], 73);
    assert!(old.get("plan_callouts").is_none());
    let mut upgraded = old.clone();
    migrate(&mut upgraded, 73).unwrap();
    let model: Model = serde_json::from_value(upgraded.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.plan_callouts.is_empty());
    let numbers: Vec<_> = model
        .sheets
        .values()
        .next()
        .unwrap()
        .parameters
        .viewports
        .iter()
        .map(|v| v.detail_number.as_str())
        .collect();
    assert_eq!(numbers, ["1", "2"]);
    // Normalize only documented migration fields; every other byte-level JSON value survives.
    upgraded.as_object_mut().unwrap().remove("plan_callouts");
    upgraded.as_object_mut().unwrap().remove("section_callouts");
    upgraded.as_object_mut().unwrap().remove("text_notes");
    upgraded
        .as_object_mut()
        .unwrap()
        .remove("plan_hidden_openings");
    upgraded
        .as_object_mut()
        .unwrap()
        .remove("group_definitions");
    upgraded.as_object_mut().unwrap().remove("group_instances");
    upgraded["schema_version"] = 73.into();
    for (key, map) in upgraded.as_object_mut().unwrap() {
        if key == "project" {
            map["header"]["schema_version"] = 73.into();
        } else if key != "extensions"
            && let Some(map) = map.as_object_mut()
        {
            for e in map.values_mut().filter(|v| v.get("header").is_some()) {
                e["header"]["schema_version"] = 73.into();
            }
        }
    }
    for s in upgraded["sheets"].as_object_mut().unwrap().values_mut() {
        for v in s["parameters"]["viewports"].as_array_mut().unwrap() {
            v.as_object_mut().unwrap().remove("detail_number");
        }
    }
    let mut expected = old;
    common::apply_schema_77_defaults(&mut expected);
    common::current_native_headers(&mut upgraded);
    common::current_native_headers(&mut expected);
    assert_eq!(upgraded, expected);
}

#[test]
fn callout_legacy_new_fields_are_rejected_atomically_even_when_empty_or_null() {
    for field in ["plan_callouts", "detail_number"] {
        for value in [Value::Null, json!({}), json!("7")] {
            let mut old = frozen();
            if field == "plan_callouts" {
                old[field] = value;
            } else {
                old["sheets"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"]["viewports"][1][field] = value;
            }
            let before = old.clone();
            assert!(migrate(&mut old, 73).is_err(), "{field}");
            assert_eq!(old, before, "no partially upgraded caller data");
        }
    }
}

#[test]
fn callout_save_reopen_preserves_ids_destination_and_unique_detail_numbers() {
    let (mut model, parent, target, marker) = fixture();
    let (source, _) = sheet(&mut model, parent, "A101", "1");
    let (dest, vp) = sheet(&mut model, target, "A201", "A");
    model
        .plan_callouts
        .get_mut(&marker)
        .unwrap()
        .parameters
        .destination = Some(vp);
    let document = Document::from_model(model.clone()).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("callouts.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let restored = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(restored.model(), &model);
    assert_eq!(
        restored
            .model()
            .callout_reference(&model.plan_callouts[&marker].parameters)
            .label(),
        "A / A201"
    );
    restored
        .model()
        .validate_callout_issue_references(&[source, dest])
        .unwrap();
    for number in ["A", " a ", "", "\n"] {
        let mut broken = model.clone();
        let p = &mut broken.sheets.get_mut(&dest).unwrap().parameters;
        let mut duplicate = p.viewports[0].clone();
        duplicate.id = os_core::Id::new();
        duplicate.detail_number = number.into();
        p.viewports.push(duplicate);
        assert!(
            Document::from_model(broken).is_err(),
            "duplicate/invalid detail {number:?}"
        );
    }
    // Numbers are sheet-local, not project-global.
    model.sheets.get_mut(&source).unwrap().parameters.viewports[0].detail_number = "A".into();
    model.validate().unwrap();
}
