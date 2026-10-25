mod common;
use os_model::*;
use os_storage::{
    StorageBackend, ZipJsonStorage, export_opening_type_package, migrate,
    parse_opening_type_package,
};
use serde_json::{Value, json};

#[test]
fn family7_native_and_package_fixtures_preserve_attachments_and_reject_new_semantics() {
    let frozen: Value =
        serde_json::from_str(include_str!("fixtures/schema-71-sash-accessories.json")).unwrap();
    let mut value = frozen.clone();
    migrate(&mut value, 71).unwrap();
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    let mut expected = frozen.clone();
    expected["schema_version"] = SCHEMA_VERSION.into();
    common::current_native_headers(&mut expected);
    expected["plan_callouts"] = json!({});
    expected["section_callouts"] = json!({});
    expected["text_notes"] = json!({});
    expected["plan_hidden_openings"] = json!({});
    expected["plan_element_graphics_overrides"] = json!({});
    expected["plan_graphics_rules"] = json!({});
    expected["group_definitions"] = json!({});
    expected["group_instances"] = json!({});
    for ty in expected["opening_types"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        ty["parameters"]["family"]["version"] = 9.into();
    }
    common::apply_schema_77_defaults(&mut expected);
    assert_eq!(value, expected);
    for change in ["attachment", "operation", "family"] {
        let mut bad = frozen.clone();
        let p = &mut bad["opening_types"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()["parameters"];
        match change {
            "attachment" => p["family"]["extrusions"][0]["attachment"] = "MovingSash".into(),
            "operation" => p["window_operation"] = "Casement".into(),
            _ => p["family"]["version"] = 8.into(),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 71).is_err());
        assert_eq!(bad, before);
    }
    let bytes = include_bytes!("fixtures/package-7-sash-accessories.osot");
    let package = parse_opening_type_package(bytes).unwrap();
    assert_eq!(package.parameters.family.version, 11);
    assert_eq!(
        package.parameters.family.extrusions[0].attachment,
        OpeningAttachment::PrimaryPanel
    );
    let mut bad: Value = serde_json::from_slice(bytes).unwrap();
    bad["parameters"]["family"]["extrusions"][0]["attachment"] = "MovingSash".into();
    bad["parameters"]["window_operation"] = "Casement".into();
    assert!(parse_opening_type_package(&serde_json::to_vec(&bad).unwrap()).is_err());
}

#[test]
fn moving_accessories_native_and_package_roundtrip_keep_pins_ids_and_exact_material_dependencies() {
    let mut value: Value =
        serde_json::from_str(include_str!("fixtures/schema-71-sash-accessories.json")).unwrap();
    migrate(&mut value, 71).unwrap();
    let mut model: Model = serde_json::from_value(value).unwrap();
    let ty = *model.opening_types.keys().next().unwrap();
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Handle finish".into(),
            density_kg_m3: 7800.,
            color: [40, 60, 80],
            cut_pattern: None,
        },
    );
    let mid = material.id();
    model.materials.insert(mid, material);
    let original_openings = model.openings.clone();
    let p = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
    p.window_operation = WindowOperation::Sliding;
    p.family.extrusions[0].attachment = OpeningAttachment::MovingSash;
    p.family.extrusions[0].material = Some(mid);
    model.validate().unwrap();
    let bytes = export_opening_type_package(&model, ty).unwrap();
    let wire: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        wire["version"],
        json!(os_storage::OPENING_TYPE_PACKAGE_VERSION)
    );
    let package = parse_opening_type_package(&bytes).unwrap();
    assert_eq!(package.parameters, model.opening_types[&ty].parameters);
    assert!(package.materials.iter().any(|m| m.source_id == mid));
    let mut missing = package.clone();
    missing.materials.retain(|m| m.source_id != mid);
    assert!(missing.validate().is_err());
    let mut extra = package.clone();
    let mut unused = package.materials[0].clone();
    unused.source_id = os_core::Id::new();
    extra.materials.push(unused);
    assert!(extra.validate().is_err());
    let doc = os_document::Document::from_model(model.clone()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("moving.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let loaded = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(loaded.model(), &model);
    assert_eq!(loaded.model().openings, original_openings);
}
