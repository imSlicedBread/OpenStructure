mod common;
#[path = "../../os-model/tests/support/section_callouts.rs"]
mod support;
use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use support::*;

#[test]
fn callout_save_reopen_preserves_ids_destination_and_unique_detail_numbers() {
    let (mut model, parent, target, marker) = fixture();
    let (source, _) = sheet(&mut model, parent, "A101", "1");
    let (dest, vp) = sheet(&mut model, target, "A201", "A");
    model
        .section_callouts
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
            .section_callout_reference(&model.section_callouts[&marker].parameters)
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

#[test]
fn section_callout_strict_schema74_migration_and_current_wire() {
    let mut old: Value =
        serde_json::from_str(include_str!("fixtures/schema-74-section-callouts.json")).unwrap();
    let before = old.clone();
    migrate(&mut old, 74).unwrap();
    let model: Model = serde_json::from_value(old.clone()).unwrap();
    model.validate().unwrap();
    assert!(model.section_callouts.is_empty());
    old.as_object_mut().unwrap().remove("section_callouts");
    old.as_object_mut().unwrap().remove("text_notes");
    old.as_object_mut().unwrap().remove("plan_hidden_openings");
    old.as_object_mut().unwrap().remove("group_definitions");
    old.as_object_mut().unwrap().remove("group_instances");
    old["schema_version"] = 74.into();
    for (key, map) in old.as_object_mut().unwrap() {
        if key == "project" {
            map["header"]["schema_version"] = 74.into();
        } else if key != "extensions"
            && let Some(map) = map.as_object_mut()
        {
            for e in map.values_mut().filter(|v| v.get("header").is_some()) {
                e["header"]["schema_version"] = 74.into();
            }
        }
    }
    let mut expected = before.clone();
    common::apply_schema_77_defaults(&mut expected);
    common::current_native_headers(&mut old);
    common::current_native_headers(&mut expected);
    assert_eq!(old, expected);
    for field in [Value::Null, json!({}), json!([])] {
        let mut bad = before.clone();
        bad["section_callouts"] = field;
        let original = bad.clone();
        assert!(migrate(&mut bad, 74).is_err());
        assert_eq!(bad, original);
    }
    let mut wire = serde_json::to_value(model).unwrap();
    wire.as_object_mut().unwrap().remove("section_callouts");
    wire.as_object_mut().unwrap().remove("text_notes");
    assert!(migrate(&mut wire, SCHEMA_VERSION).is_err());
}
