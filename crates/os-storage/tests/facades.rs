mod common;
use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::Value;
fn headers(value: &mut Value, version: u32) {
    value["schema_version"] = version.into();
    for (key, data) in value.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = version.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = version.into();
            }
        }
    }
}
#[test]
fn schema_65_facade_migration_preserves_identity_metadata_and_rejects_smuggled_fields() {
    let mut old: Value =
        serde_json::from_str(include_str!("fixtures/schema-64-elevation-dimensions.json")).unwrap();
    headers(&mut old, 65);
    old["project"]["header"]["properties"]["opaque_facade"] =
        serde_json::json!({"kind":"Facade","facade":{"schema_version":123}});
    let mut expected = old.clone();
    common::current_native_headers(&mut expected);
    expected["material_patterns"] = serde_json::json!({});
    expected["sheet_sets"] = serde_json::json!({});
    expected["drawing_issues"] = serde_json::json!({});
    expected["plan_callouts"] = serde_json::json!({});
    expected["section_callouts"] = serde_json::json!({});
    expected["text_notes"] = serde_json::json!({});
    expected["plan_hidden_openings"] = serde_json::json!({});
    expected["plan_element_graphics_overrides"] = serde_json::json!({});
    expected["plan_graphics_rules"] = serde_json::json!({});
    expected["group_definitions"] = serde_json::json!({});
    expected["group_instances"] = serde_json::json!({});
    for ty in expected["opening_types"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        ty["parameters"]["family"]["version"] = 10.into();
        ty["parameters"]["family"]["extrusions"] = serde_json::json!([]);
    }
    let mut actual = old.clone();
    migrate(&mut actual, 65).unwrap();
    assert_eq!(actual, expected);
    let model: Model = serde_json::from_value(actual).unwrap();
    model.validate().unwrap();
    for field in [false, true] {
        let mut bad = old.clone();
        let view = bad["views"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        if field {
            view["parameters"]["facade"] = Value::Null;
        } else {
            view["parameters"]["kind"] = "Facade".into();
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 65).is_err());
        assert_eq!(bad, before);
    }
}
#[test]
fn facade_world_definition_round_trips_without_meshes_or_host_links() {
    let mut doc = Document::new("Facade").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let view = View::new(
        "core.view",
        ViewParams {
            drafting: None,
            name: "North".into(),
            kind: ViewKind::Facade,
            level: Some(level),
            settings_revision: 0,
            plan: None,
            section: None,
            elevation: None,
            facade: Some(
                FacadeViewSettings::new(Point2::new(3., 7.), Point2::new(9., 15.), -1., 20.)
                    .unwrap(),
            ),
        },
    );
    let id = view.id();
    doc.execute("Facade", vec![Command::AddView(view)]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("facade.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let loaded = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(loaded.model(), doc.model());
    let json = serde_json::to_value(&loaded.model().views[&id].parameters).unwrap();
    assert!(json["facade"].get("host").is_none());
    assert!(json["facade"].get("meshes").is_none());
}
