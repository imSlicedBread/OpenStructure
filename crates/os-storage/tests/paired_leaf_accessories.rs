mod common;
use os_model::*;
use os_storage::{
    StorageBackend, ZipJsonStorage, export_opening_type_package, migrate,
    parse_opening_type_package,
};
use serde_json::{Value, json};

#[test]
fn schema72_package8_preserve_old_attachments_and_reject_disguised_leaf_semantics() {
    let frozen: Value =
        serde_json::from_str(include_str!("fixtures/schema-72-leaf-accessories.json")).unwrap();
    let bytes = include_bytes!("fixtures/package-8-leaf-accessories.osot");
    for moving in [false, true] {
        let mut source = frozen.clone();
        for ty in source["opening_types"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            if moving {
                ty["parameters"]["window_operation"] = "Casement".into();
                ty["parameters"]["family"]["extrusions"][0]["attachment"] = "MovingSash".into();
            }
        }
        let mut upgraded = source.clone();
        migrate(&mut upgraded, 72).unwrap();
        let model: Model = serde_json::from_value(upgraded.clone()).unwrap();
        model.validate().unwrap();
        let mut expected = source.clone();
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
        assert_eq!(upgraded, expected);
        let mut package: Value = serde_json::from_slice(bytes).unwrap();
        if moving {
            package["parameters"]["window_operation"] = "Casement".into();
            package["parameters"]["family"]["extrusions"][0]["attachment"] = "MovingSash".into();
        }
        let parsed = parse_opening_type_package(&serde_json::to_vec(&package).unwrap()).unwrap();
        assert_eq!(parsed.parameters.family.version, 11);
        assert_eq!(
            parsed.parameters.family.extrusions[0].attachment,
            if moving {
                OpeningAttachment::MovingSash
            } else {
                OpeningAttachment::PrimaryPanel
            }
        );
        for mutation in ["ActiveLeaf", "InactiveLeaf", "family", "paired"] {
            let mutate = |p: &mut Value| match mutation {
                "family" => p["family"]["version"] = 9.into(),
                "paired" => p["family"]["door_leaves"] = json!({"Paired":{"active_fraction":0.6}}),
                attachment => p["family"]["extrusions"][0]["attachment"] = attachment.into(),
            };
            let mut bad = source.clone();
            mutate(
                &mut bad["opening_types"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"],
            );
            let before = bad.clone();
            assert!(migrate(&mut bad, 72).is_err());
            assert_eq!(bad, before);
            let mut bad = package.clone();
            mutate(&mut bad["parameters"]);
            assert!(parse_opening_type_package(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
    }
}

#[test]
fn paired_accessories_roundtrip_exact_ids_pins_and_dependencies() {
    let mut wire: Value =
        serde_json::from_str(include_str!("fixtures/schema-72-leaf-accessories.json")).unwrap();
    migrate(&mut wire, 72).unwrap();
    let mut model: Model = serde_json::from_value(wire).unwrap();
    let mut p = model
        .opening_types
        .values()
        .next()
        .unwrap()
        .parameters
        .clone();
    p.name = "Paired leaf accessories".into();
    p.kind = OpeningKind::Door;
    p.width = 1.8;
    p.sill = 0.;
    p.pane_position = WindowPanePosition::Center;
    p.family.door_leaves = DoorLeaves::Paired {
        active_fraction: 0.6,
    };
    p.family.extrusions[0].attachment = OpeningAttachment::ActiveLeaf;
    p.family.extrusions[1].attachment = OpeningAttachment::InactiveLeaf;
    let material = p.family.panel_material;
    for s in &mut p.family.extrusions {
        s.material = material;
    }
    let ty = OpeningType::new("core.opening_type", p);
    let tid = ty.id();
    model.opening_types.insert(tid, ty);
    let door = model
        .openings
        .values()
        .find(|o| model.resolve_opening(&o.parameters).unwrap().kind == OpeningKind::Door)
        .unwrap()
        .id();
    let placed = &mut model.openings.get_mut(&door).unwrap().parameters;
    placed.definition = OpeningDefinition::Typed { type_id: tid };
    placed.width_override = Some(0.9);
    placed.height_override = Some(2.1);
    placed.open_state = OpeningState::DoorPairAngles {
        active_degrees: 30.,
        inactive_degrees: 90.,
    };
    let bytes = export_opening_type_package(&model, tid).unwrap();
    let wire: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["version"], os_storage::OPENING_TYPE_PACKAGE_VERSION);
    let package = parse_opening_type_package(&bytes).unwrap();
    assert_eq!(package.parameters, model.opening_types[&tid].parameters);
    let mut missing = package.clone();
    missing.materials.retain(|m| Some(m.source_id) != material);
    assert!(missing.validate().is_err());
    let doc = os_document::Document::from_model(model.clone()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("paired.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);
}
