use os_document::{Command, Document};
use os_model::*;
use os_storage::{
    StorageBackend, ZipJsonStorage, export_opening_type_package, migrate,
    parse_opening_type_package,
};
use serde_json::{Value, json};

#[test]
fn authored_extrusions_frozen_model_and_package_migration_preserve_old_geometry_and_reject_smuggling()
 {
    let frozen: Value =
        serde_json::from_str(include_str!("fixtures/schema-66-opening-extrusions.json")).unwrap();
    let mut migrated = frozen.clone();
    migrate(&mut migrated, 66).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, os_model::SCHEMA_VERSION);
    assert_eq!(
        migrated["openings"].as_object().unwrap().len(),
        frozen["openings"].as_object().unwrap().len()
    );
    for (key, ty) in frozen["opening_types"].as_object().unwrap() {
        let mut family = ty["parameters"]["family"].clone();
        family["version"] = 7.into();
        family["extrusions"] = json!([]);
        assert_eq!(
            migrated["opening_types"][key]["parameters"]["family"],
            family
        );
        assert_eq!(
            migrated["opening_types"][key]["header"]["id"],
            ty["header"]["id"]
        );
    }
    for payload in [Value::Null, json!([])] {
        let mut bad = frozen.clone();
        bad["opening_types"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()["parameters"]["family"]["extrusions"] = payload;
        let before = bad.clone();
        assert!(migrate(&mut bad, 66).is_err());
        assert_eq!(bad, before);
    }
    let bytes = include_bytes!("fixtures/package-5-opening-extrusions.osot");
    let package = parse_opening_type_package(bytes).unwrap();
    assert_eq!(package.parameters.family.version, 7);
    assert!(package.parameters.family.extrusions.is_empty());
    let ty = *model.opening_types.keys().next().unwrap();
    assert_eq!(
        package,
        parse_opening_type_package(&export_opening_type_package(&model, ty).unwrap()).unwrap()
    );
    let mut bad: Value = serde_json::from_slice(bytes).unwrap();
    bad["parameters"]["family"]["extrusions"] = json!([]);
    assert!(parse_opening_type_package(&serde_json::to_vec(&bad).unwrap()).is_err());
}

#[test]
fn authored_extrusions_archive_package_exact_ids_material_dependencies_and_roundtrip() {
    let mut wire: Value =
        serde_json::from_str(include_str!("fixtures/schema-66-opening-extrusions.json")).unwrap();
    migrate(&mut wire, 66).unwrap();
    let model: Model = serde_json::from_value(wire).unwrap();
    let mut doc = Document::from_model(model).unwrap();
    let ty = *doc.model().opening_types.keys().next().unwrap();
    let mut parameters = doc.model().opening_types[&ty].parameters.clone();
    for i in 0..4 {
        let mat = Material::new(
            "core.material",
            MaterialParams {
                cut_pattern: None,
                name: format!("Solid finish {i}"),
                density_kg_m3: 7800.,
                color: [90, 100, i * 40],
            },
        );
        let mut s = OpeningExtrusion::new(format!("Grille {i}"));
        s.material = Some(mat.id());
        s.normal_offset = i as f64 * 0.03;
        doc.execute("Material", vec![Command::AddMaterial(mat)])
            .unwrap();
        parameters.family.extrusions.push(s);
    }
    doc.execute(
        "Solids",
        vec![Command::UpdateOpeningType { id: ty, parameters }],
    )
    .unwrap();
    let original = doc.model().clone();
    let bytes = export_opening_type_package(&original, ty).unwrap();
    let package = parse_opening_type_package(&bytes).unwrap();
    assert_eq!(package.materials.len(), 6);
    assert_eq!(package.parameters, original.opening_types[&ty].parameters);
    let mut bad = package.clone();
    bad.materials.pop();
    assert!(bad.validate().is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("solids.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &original);
}
