use os_core::Id;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{
    StorageBackend, ZipJsonStorage, export_opening_type_package, migrate,
    parse_opening_type_package,
};

#[test]
fn shared_lengths_frozen_legacy_migration_save_reopen_and_package_v5() {
    let mut wire: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-53-straight-walls.json")).unwrap();
    migrate(&mut wire, 53).unwrap();
    let model: Model = serde_json::from_value(wire).unwrap();
    assert!(model.length_parameters.is_empty());
    assert!(model.opening_type_length_bindings.is_empty());
    let mut doc = Document::from_model(model).unwrap();
    let ty = *doc.model().opening_types.keys().next().unwrap();
    let width = doc
        .model()
        .resolve_opening_type(ty)
        .unwrap()
        .parameters
        .width;
    let p = LengthParameter::new(
        "core.length_parameter",
        LengthParameterParams {
            name: "Width".into(),
            unit: LengthUnit::Metres,
            value: width,
        },
    );
    let id = p.id();
    doc.execute(
        "Bind",
        vec![
            Command::AddLengthParameter(p),
            Command::SetOpeningTypeLengthBindings {
                id: ty,
                bindings: OpeningTypeLengthBindings {
                    width: Some(id),
                    ..Default::default()
                },
            },
        ],
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lengths.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    let bytes = export_opening_type_package(doc.model(), ty).unwrap();
    let package = parse_opening_type_package(&bytes).unwrap();
    assert_eq!(package.length_bindings.width, Some(id));
    assert_eq!(package.length_parameters.len(), 1);
    assert_eq!(package.resolved_parameters().unwrap().width, width);
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["version"], 5);
    wire["length_parameters"][Id::new().to_string()] =
        serde_json::to_value(&doc.model().length_parameters[&id].parameters).unwrap();
    assert!(parse_opening_type_package(&serde_json::to_vec(&wire).unwrap()).is_err());
}

#[test]
fn shared_lengths_schema55_requires_unambiguous_fields_and_strict_current_wire() {
    let mut wire = serde_json::to_value(Model::new("Legacy")).unwrap();
    wire["schema_version"] = 55.into();
    // Ambiguous legacy input must not silently discard newly authored data.
    assert!(migrate(&mut wire, 55).is_err());
    let mut current = serde_json::to_value(Model::new("Current")).unwrap();
    current.as_object_mut().unwrap().remove("length_parameters");
    assert!(serde_json::from_value::<Model>(current).is_err());
}
