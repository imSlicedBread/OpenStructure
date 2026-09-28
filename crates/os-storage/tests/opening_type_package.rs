use os_core::Id;
use os_model::{
    Material, MaterialParams, Model, OpeningKind, OpeningType, OpeningTypeParams,
    WindowPanePosition,
};
use os_storage::{
    MAX_OPENING_TYPE_PACKAGE_BYTES, OPENING_TYPE_PACKAGE_FORMAT, OPENING_TYPE_PACKAGE_VERSION,
    OpeningTypePackage, OpeningTypePackageMaterial, export_opening_type_package,
    parse_opening_type_package, read_opening_type_package, write_opening_type_package,
};
use serde_json::{Value, json};

fn id(value: u8) -> Id {
    serde_json::from_value(json!(format!("00000000-0000-0000-0000-{value:012x}"))).unwrap()
}

fn fixture(kind: OpeningKind) -> (Model, Id) {
    let mut model = Model::new("Source project");
    for (index, name, density, color) in [
        (1, "Oak leaf", 720., [165, 112, 63]),
        (2, "Aluminium frame", 2700., [190, 191, 192]),
        (3, "Unreferenced", 1000., [1, 2, 3]),
    ] {
        let mut material = Material::new(
            "core.material",
            MaterialParams {
                name: name.into(),
                density_kg_m3: density,
                color,
            },
        );
        material.header.id = id(index);
        model.materials.insert(material.id(), material);
    }
    let mut ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            name: format!("Authored {kind:?}"),
            kind,
            width: 1.2,
            height: 2.1,
            sill: if kind == OpeningKind::Door { 0. } else { 0.8 },
            pane_position: WindowPanePosition::Center,
            family: os_model::OpeningFamily {
                frame_width: 0.04,
                panel_material: Some(id(2)),
                frame_material: Some(id(1)),
                ..Default::default()
            },
        },
    );
    ty.header.id = id(4);
    model.opening_types.insert(ty.id(), ty);
    model.validate().unwrap();
    (model, id(4))
}

fn valid_json() -> Value {
    let (model, ty) = fixture(OpeningKind::Door);
    serde_json::from_slice(&export_opening_type_package(&model, ty).unwrap()).unwrap()
}

#[test]
fn two_bay_packages_v1_v2_upgrade_to_v3_strict_and_three_materials() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut model, ty) = fixture(kind);
        let mut v1: Value =
            serde_json::from_slice(&export_opening_type_package(&model, ty).unwrap()).unwrap();
        v1["version"] = 1.into();
        v1["parameters"]["family"]["version"] = 4.into();
        v1["parameters"]["family"]
            .as_object_mut()
            .unwrap()
            .remove("side_lite");
        v1["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("window_operation");
        let imported = parse_opening_type_package(&serde_json::to_vec(&v1).unwrap()).unwrap();
        assert_eq!(imported.parameters, model.opening_types[&ty].parameters);
        v1["parameters"]["family"]["side_lite"] = Value::Null;
        rejected(v1);
        for side in [os_model::LiteSide::Start, os_model::LiteSide::End] {
            model
                .opening_types
                .get_mut(&ty)
                .unwrap()
                .parameters
                .family
                .side_lite = Some(os_model::SideLite {
                side,
                width_fraction: 0.25,
                mullion_width: 0.05,
                material: Some(id(3)),
            });
            let bytes = export_opening_type_package(&model, ty).unwrap();
            let imported = parse_opening_type_package(&bytes).unwrap();
            assert_eq!(imported.materials.len(), 3);
            assert_eq!(imported.parameters, model.opening_types[&ty].parameters);
            let mut value: Value = serde_json::from_slice(&bytes).unwrap();
            value["parameters"]["family"]
                .as_object_mut()
                .unwrap()
                .remove("side_lite");
            rejected(value);
            let mut value: Value = serde_json::from_slice(&bytes).unwrap();
            value["parameters"]["family"]["side_lite"]["unknown"] = true.into();
            rejected(value);
        }
    }
}

#[test]
fn v2_window_type_import_defaults_operation_and_v3_round_trips_it() {
    let (model, ty) = fixture(OpeningKind::Window);
    let mut v2: Value =
        serde_json::from_slice(&export_opening_type_package(&model, ty).unwrap()).unwrap();
    v2["version"] = json!(2);
    v2["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("window_operation");
    let imported = parse_opening_type_package(&serde_json::to_vec(&v2).unwrap()).unwrap();
    assert_eq!(
        imported.parameters.window_operation,
        os_model::WindowOperation::Fixed
    );
    let encoded = serde_json::to_value(&imported.parameters).unwrap();
    assert_eq!(encoded["window_operation"], json!("Fixed"));

    v2["parameters"]["window_operation"] = json!("Sliding");
    rejected(v2);

    let mut v3: Value =
        serde_json::from_slice(&export_opening_type_package(&model, ty).unwrap()).unwrap();
    v3["parameters"]["window_operation"] = json!("Casement");
    assert_eq!(
        parse_opening_type_package(&serde_json::to_vec(&v3).unwrap())
            .unwrap()
            .parameters
            .window_operation,
        os_model::WindowOperation::Casement
    );
}

fn rejected(value: Value) {
    assert!(
        parse_opening_type_package(&serde_json::to_vec(&value).unwrap()).is_err(),
        "accepted {value}"
    );
}

#[test]
fn typed_door_and_window_round_trip_exact_materials_without_mutation() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (model, ty) = fixture(kind);
        let before = model.clone();
        let bytes = export_opening_type_package(&model, ty).unwrap();
        let package = parse_opening_type_package(&bytes).unwrap();
        // A literal also verifies the exact public API needed by the UI.
        assert_eq!(
            package,
            OpeningTypePackage {
                source_type_id: ty,
                parameters: model.opening_types[&ty].parameters.clone(),
                materials: [id(1), id(2)]
                    .map(|source_id| OpeningTypePackageMaterial {
                        source_id,
                        parameters: model.materials[&source_id].parameters.clone(),
                    })
                    .into(),
            }
        );
        assert_eq!(model, before);
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire["format"], OPENING_TYPE_PACKAGE_FORMAT);
        assert_eq!(wire["version"], OPENING_TYPE_PACKAGE_VERSION);
        assert!(wire.get("schema_version").is_none());
    }
}

#[test]
fn material_dependencies_are_exact_deduplicated_and_optional() {
    let (mut model, ty) = fixture(OpeningKind::Window);
    for (panel, frame, expected) in [
        (Some(id(1)), Some(id(1)), vec![id(1)]),
        (None, Some(id(2)), vec![id(2)]),
        (Some(id(1)), None, vec![id(1)]),
        (None, None, vec![]),
    ] {
        let family = &mut model.opening_types.get_mut(&ty).unwrap().parameters.family;
        family.panel_material = panel;
        family.frame_material = frame;
        family.frame_width = 0.; // Disabled frames still carry their material reference.
        let package =
            parse_opening_type_package(&export_opening_type_package(&model, ty).unwrap()).unwrap();
        assert_eq!(
            package
                .materials
                .iter()
                .map(|m| m.source_id)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn bytes_are_deterministic_and_parsing_canonicalizes_material_order() {
    let (model, ty) = fixture(OpeningKind::Door);
    let first = export_opening_type_package(&model, ty).unwrap();
    let mut reordered = model.clone();
    reordered.materials.clear();
    for (id, material) in model.materials.iter().rev() {
        reordered.materials.insert(*id, material.clone());
    }
    reordered.project.parameters.name = "Different project metadata".into();
    assert_eq!(first, export_opening_type_package(&reordered, ty).unwrap());
    assert_eq!(first, export_opening_type_package(&model, ty).unwrap());
    let mut value: Value = serde_json::from_slice(&first).unwrap();
    value["materials"].as_array_mut().unwrap().reverse();
    assert_eq!(
        parse_opening_type_package(&first).unwrap(),
        parse_opening_type_package(&serde_json::to_vec(&value).unwrap()).unwrap()
    );
}

#[test]
fn atomic_path_create_replace_read_and_failed_validation_preserve_destination() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("type.osopening");
    let (mut model, ty) = fixture(OpeningKind::Window);
    write_opening_type_package(&model, ty, &path).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        export_opening_type_package(&model, ty).unwrap()
    );
    model.opening_types.get_mut(&ty).unwrap().parameters.name = "Replacement".into();
    write_opening_type_package(&model, ty, &path).unwrap();
    assert_eq!(
        read_opening_type_package(&path).unwrap().parameters.name,
        "Replacement"
    );
    let before = std::fs::read(&path).unwrap();
    model.opening_types.get_mut(&ty).unwrap().parameters.width = 0.;
    assert!(write_opening_type_package(&model, ty, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    assert!(read_opening_type_package(&directory.path().join("missing")).is_err());
}

#[test]
fn failed_atomic_replacement_cleans_up_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing-directory");
    std::fs::create_dir(&path).unwrap();
    let marker = path.join("keep");
    std::fs::write(&marker, b"preserved").unwrap();
    let (model, ty) = fixture(OpeningKind::Door);
    assert!(write_opening_type_package(&model, ty, &path).is_err());
    assert_eq!(std::fs::read(marker).unwrap(), b"preserved");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn byte_and_path_limits_allow_exact_boundary_and_reject_one_byte_over() {
    let (model, ty) = fixture(OpeningKind::Door);
    let mut bytes = export_opening_type_package(&model, ty).unwrap();
    bytes.resize(MAX_OPENING_TYPE_PACKAGE_BYTES, b' ');
    assert!(parse_opening_type_package(&bytes).is_ok());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bounded");
    std::fs::write(&path, &bytes).unwrap();
    assert!(read_opening_type_package(&path).is_ok());
    bytes.push(b' ');
    assert!(parse_opening_type_package(&bytes).is_err());
    std::fs::write(&path, bytes).unwrap();
    assert!(read_opening_type_package(&path).is_err());
}

#[test]
fn rejects_corrupt_duplicate_escaped_duplicate_and_deep_json() {
    let raw = serde_json::to_string(&valid_json()).unwrap();
    for bytes in [
        Vec::new(),
        b"{broken".to_vec(),
        vec![0xff],
        b"null".to_vec(),
        format!("{raw} {{}}").into_bytes(),
        raw.replacen("\"version\":3", "\"version\":3,\"version\":3", 1)
            .into_bytes(),
        raw.replacen(
            "\"density_kg_m3\":720.0",
            "\"density_kg_m3\":720.0,\"density_kg_m3\":720.0",
            1,
        )
        .into_bytes(),
        raw.replacen("\"x\":0.0", "\"x\":0.0,\"\\u0078\":0.0", 1)
            .into_bytes(),
        format!("{}0{}", "[".repeat(140), "]".repeat(140)).into_bytes(),
    ] {
        assert_ne!(bytes, raw.as_bytes(), "duplicate test must change source");
        assert!(
            parse_opening_type_package(&bytes).is_err(),
            "accepted {}",
            String::from_utf8_lossy(&bytes)
        );
    }
}

#[test]
fn rejects_unknown_fields_at_every_object_layer() {
    for pointer in [
        "",
        "/parameters",
        "/parameters/family",
        "/parameters/family/profile/0",
        "/parameters/family/cut_profile/0",
        "/materials/0",
        "/materials/0/parameters",
    ] {
        let mut value = valid_json();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("surprise".into(), json!(1));
        rejected(value);
    }
}

#[test]
fn rejects_unknown_format_version_missing_fields_and_wrong_shapes() {
    for version in [
        json!(0),
        json!(4),
        json!(-1),
        json!(1.5),
        json!("1"),
        Value::Null,
    ] {
        let mut value = valid_json();
        value["version"] = version;
        rejected(value);
    }
    let mut value = valid_json();
    value["format"] = json!("OpenStructure");
    rejected(value);
    for field in [
        "format",
        "version",
        "source_type_id",
        "parameters",
        "materials",
    ] {
        let mut value = valid_json();
        value.as_object_mut().unwrap().remove(field);
        rejected(value);
    }
    let mut value = valid_json();
    value["materials"] = json!({});
    rejected(value);
}

#[test]
fn rejects_nil_duplicate_missing_and_extra_material_references() {
    for case in 0..8 {
        let mut value = valid_json();
        match case {
            0 => value["source_type_id"] = json!(id(0)),
            1 => value["materials"][0]["source_id"] = json!(id(0)),
            2 => value["parameters"]["family"]["panel_material"] = json!(id(0)),
            3 => value["materials"][1] = value["materials"][0].clone(),
            4 => {
                value["materials"].as_array_mut().unwrap().pop();
            }
            5 => value["parameters"]["family"]["frame_material"] = Value::Null,
            6 => value["materials"][0]["source_id"] = json!(id(9)),
            _ => {
                let extra = value["materials"][0].clone();
                value["materials"].as_array_mut().unwrap().push(extra);
            }
        }
        rejected(value);
    }
}

#[test]
fn rejects_invalid_type_family_and_material_parameters() {
    for (pointer, bad) in [
        ("/parameters/name", json!("  ")),
        ("/parameters/name", json!("bad\nname")),
        ("/parameters/name", json!("x".repeat(257))),
        ("/parameters/width", json!(0)),
        ("/parameters/height", json!(-1)),
        ("/parameters/sill", json!(0.5)),
        ("/parameters/pane_position", json!("LeftFace")),
        ("/parameters/family/version", json!(99)),
        ("/parameters/family/depth", json!(0)),
        ("/parameters/family/frame_width", json!(0.3)),
        ("/parameters/family/profile", json!([])),
        ("/parameters/family/profile/0/x", json!(2)),
        ("/materials/0/parameters/name", json!("")),
        ("/materials/0/parameters/name", json!(" \t ")),
        ("/materials/0/parameters/name", json!("bad\nname")),
        ("/materials/0/parameters/name", json!("x".repeat(257))),
        ("/materials/0/parameters/density_kg_m3", json!(0)),
        ("/materials/0/parameters/density_kg_m3", json!(-1)),
        ("/materials/0/parameters/density_kg_m3", Value::Null),
        ("/materials/0/parameters/color", json!([256, 0, 0])),
    ] {
        let mut value = valid_json();
        *value.pointer_mut(pointer).unwrap() = bad;
        if pointer == "/parameters/family/frame_width" {
            value["parameters"]["width"] = json!(0.4);
        }
        rejected(value);
    }
}

#[test]
fn export_rejects_missing_entities_mismatched_identities_and_nonfinite_values() {
    for case in 0..10 {
        let (mut model, ty) = fixture(OpeningKind::Door);
        match case {
            0 => {
                model.opening_types.clear();
            }
            1 => {
                model.materials.remove(&id(1));
            }
            2 => model.opening_types.get_mut(&ty).unwrap().header.id = id(9),
            3 => model.materials.get_mut(&id(1)).unwrap().header.id = id(9),
            4 => model.opening_types.get_mut(&ty).unwrap().parameters.width = f64::NAN,
            5 => {
                model
                    .materials
                    .get_mut(&id(1))
                    .unwrap()
                    .parameters
                    .density_kg_m3 = f64::INFINITY
            }
            6 => model.materials.get_mut(&id(1)).unwrap().parameters.name = "\n".into(),
            7 => {
                model
                    .opening_types
                    .get_mut(&ty)
                    .unwrap()
                    .parameters
                    .family
                    .profile[0]
                    .x = f64::INFINITY
            }
            8 => model.opening_types.get_mut(&ty).unwrap().header.type_id = "wrong".into(),
            _ => model.materials.get_mut(&id(1)).unwrap().header.type_id = "wrong".into(),
        }
        assert!(
            export_opening_type_package(&model, ty).is_err(),
            "case {case}"
        );
    }
}
