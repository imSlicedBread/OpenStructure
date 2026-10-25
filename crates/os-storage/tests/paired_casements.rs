use os_model::*;
use os_storage::{
    StorageBackend, ZipJsonStorage, export_opening_type_package, migrate,
    parse_opening_type_package,
};
use serde_json::{Value, json};
fn legacy() -> Value {
    let mut v: Value =
        serde_json::from_str(include_str!("fixtures/schema-72-leaf-accessories.json")).unwrap();
    migrate(&mut v, 72).unwrap();
    v
}
#[test]
fn schema_83_and_package_10_upgrade_strictly_and_atomically() {
    let current = legacy();
    let mut old = current.clone();
    old["schema_version"] = 83.into();
    for (key, data) in old.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = 83.into();
        } else if key != "extensions" {
            if let Some(map) = data.as_object_mut() {
                for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                    entity["header"]["schema_version"] = 83.into();
                }
            }
        }
    }
    for ty in old["opening_types"].as_object_mut().unwrap().values_mut() {
        ty["parameters"]["family"]["version"] = 10.into();
    }
    let original = old.clone();
    migrate(&mut old, 83).unwrap();
    assert_eq!(old, current);
    for mutation in 0..4 {
        let mut bad = original.clone();
        if mutation == 0 {
            bad["openings"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap()["parameters"]["open_state"] =
                json!({"CasementPairAngles":{"start_degrees":0.,"end_degrees":0.}});
        } else {
            let p = &mut bad["opening_types"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap()["parameters"];
            match mutation {
                1 => {
                    p["window_operation"] =
                        json!({"PairedCasement":{"start_fraction":0.5,"mullion_width":0.04}})
                }
                2 => p["family"]["version"] = 11.into(),
                _ => p["family"]["extrusions"][0]["attachment"] = "StartSash".into(),
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 83).is_err());
        assert_eq!(bad, before);
    }
    let model: Model = serde_json::from_value(current).unwrap();
    let id = *model.opening_types.keys().next().unwrap();
    let mut package: Value =
        serde_json::from_slice(&export_opening_type_package(&model, id).unwrap()).unwrap();
    package["version"] = 10.into();
    package["parameters"]["family"]["version"] = 10.into();
    let upgraded = parse_opening_type_package(&serde_json::to_vec(&package).unwrap()).unwrap();
    assert_eq!(upgraded.parameters.family.version, 11);
    for mutation in 0..3 {
        let mut bad = package.clone();
        match mutation {
            0 => {
                bad["parameters"]["window_operation"] =
                    json!({"PairedCasement":{"start_fraction":0.5,"mullion_width":0.04}})
            }
            1 => bad["parameters"]["family"]["version"] = 11.into(),
            _ => bad["parameters"]["family"]["extrusions"][0]["attachment"] = "EndSash".into(),
        };
        assert!(parse_opening_type_package(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}
#[test]
fn paired_casements_roundtrip_document_package_materials_and_pins() {
    let mut model: Model = serde_json::from_value(legacy()).unwrap();
    let id = model
        .openings
        .values()
        .find(|o| model.resolve_opening(&o.parameters).unwrap().kind == OpeningKind::Window)
        .unwrap()
        .id();
    let tid = model.openings[&id].parameters.type_id().unwrap();
    let p = &mut model.opening_types.get_mut(&tid).unwrap().parameters;
    p.window_operation = WindowOperation::PairedCasement {
        start_fraction: 0.6,
        mullion_width: 0.04,
    };
    p.family.extrusions.clear();
    for attachment in [OpeningAttachment::StartSash, OpeningAttachment::EndSash] {
        let mut solid = OpeningExtrusion::new("Sash accessory");
        solid.attachment = attachment;
        solid.material = p.family.panel_material;
        p.family.extrusions.push(solid);
    }
    let o = &mut model.openings.get_mut(&id).unwrap().parameters;
    o.open_state = OpeningState::CasementPairAngles {
        start_degrees: 27.,
        end_degrees: 63.,
    };
    model.validate().unwrap();
    let package =
        parse_opening_type_package(&export_opening_type_package(&model, tid).unwrap()).unwrap();
    assert_eq!(package.parameters, model.opening_types[&tid].parameters);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("paired-casement.osb");
    ZipJsonStorage
        .save(
            &os_document::Document::from_model(model.clone()).unwrap(),
            &path,
        )
        .unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);
}
