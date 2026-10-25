use os_model::*;
use os_storage::*;
#[test]
fn typed_parameters_strict_frozen_schema76_and_package9_migration() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-76-parameters.json")).unwrap();
    let mut wire = original.clone();
    migrate(&mut wire, 76).unwrap();
    let model: Model = serde_json::from_value(wire.clone()).unwrap();
    model.validate().unwrap();
    let id = *model.length_parameters.keys().next().unwrap();
    assert_eq!(
        model.length_parameters[&id].parameters.unit,
        LengthUnit::Metres
    );
    assert_eq!(model.length_parameters[&id].parameters.formula, None);
    assert_eq!(
        wire["length_parameters"][id.to_string()]["parameters"]["value"],
        original["length_parameters"][id.to_string()]["parameters"]["value"]
    );
    for field in ["formula", "unit"] {
        let mut bad = original.clone();
        bad["length_parameters"][id.to_string()]["parameters"][field] = if field == "unit" {
            "Scalar".into()
        } else {
            serde_json::Value::Null
        };
        let before = bad.clone();
        assert!(migrate(&mut bad, 76).is_err());
        assert_eq!(bad, before);
    }
    let frozen = include_bytes!("fixtures/package-9-parameters.osot");
    let package = parse_opening_type_package(frozen).unwrap();
    assert_eq!(package.parameters.family.version, 11);
    assert!(
        package
            .length_parameters
            .values()
            .all(|p| p.formula.is_none())
    );
    let mut bad: serde_json::Value = serde_json::from_slice(frozen).unwrap();
    bad["parameters"]["family"]["extrusions"][0]["bindings"] =
        serde_json::json!({"x":null,"y":null,"depth":null,"normal_offset":null});
    assert!(parse_opening_type_package(&serde_json::to_vec(&bad).unwrap()).is_err());
}
#[test]
fn typed_parameters_native_package_roundtrip_exact_transitive_closure() {
    let mut model = Model::new("Formulas");
    let scalar = LengthParameter::new(
        "core.length_parameter",
        LengthParameterParams {
            name: "Multiplier".into(),
            unit: LengthUnit::Scalar,
            value: 2.,
            formula: None,
        },
    );
    let length = LengthParameter::new(
        "core.length_parameter",
        LengthParameterParams {
            name: "Depth".into(),
            unit: LengthUnit::Metres,
            value: 0.01,
            formula: Some(format!("0.01m * @{{{}}}", scalar.id())),
        },
    );
    let (sid, lid) = (scalar.id(), length.id());
    model.length_parameters.insert(sid, scalar);
    model.length_parameters.insert(lid, length);
    let unused = LengthParameter::new(
        "core.length_parameter",
        LengthParameterParams {
            name: "Unrelated".into(),
            unit: LengthUnit::Scalar,
            value: 7.,
            formula: None,
        },
    );
    let uid = unused.id();
    model.length_parameters.insert(uid, unused);
    let mut family = OpeningFamily::default();
    let mut solid = OpeningExtrusion::new("Handle");
    solid.bindings.depth = Some(lid);
    family.extrusions.push(solid);
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Door".into(),
            kind: OpeningKind::Door,
            width: 1.,
            height: 2.,
            sill: 0.,
            pane_position: Default::default(),
            window_operation: Default::default(),
            family,
        },
    );
    let id = ty.id();
    model.opening_types.insert(id, ty);
    model.validate().unwrap();
    let bytes = export_opening_type_package(&model, id).unwrap();
    let package = parse_opening_type_package(&bytes).unwrap();
    assert_eq!(
        package
            .length_parameters
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        [sid, lid].into()
    );
    assert_eq!(
        package.resolved_parameters().unwrap().family.extrusions[0].depth,
        0.02
    );
    let mut bad = package.clone();
    bad.length_parameters
        .insert(uid, model.length_parameters[&uid].parameters.clone());
    assert!(bad.validate().is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("formula.osb");
    let doc = os_document::Document::from_model(model).unwrap();
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    let mut wire = serde_json::to_value(doc.model()).unwrap();
    wire["length_parameters"][lid.to_string()]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("formula");
    assert!(serde_json::from_value::<Model>(wire).is_err());
}
