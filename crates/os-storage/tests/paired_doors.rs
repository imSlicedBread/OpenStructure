use os_document::{Command, Document};
mod common;
use os_model::*;
use os_storage::{
    StorageBackend, ZipJsonStorage, export_opening_type_package, migrate,
    parse_opening_type_package,
};

fn model() -> Model {
    let mut wire: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-53-straight-walls.json")).unwrap();
    migrate(&mut wire, 53).unwrap();
    serde_json::from_value(wire).unwrap()
}

#[test]
fn paired_doors_schema57_migration_preserves_states_and_rejects_ambiguity() {
    let mut model = model();
    let door = model
        .openings
        .values()
        .find(|o| model.resolve_opening(&o.parameters).unwrap().kind == OpeningKind::Door)
        .unwrap()
        .id();
    model.openings.get_mut(&door).unwrap().parameters.open_state = OpeningState::DoorAngle(32.);
    let mut wire = serde_json::to_value(&model).unwrap();
    common::remove_railings(&mut wire);
    common::remove_curtains(&mut wire);
    for ty in wire["opening_types"].as_object_mut().unwrap().values_mut() {
        ty["parameters"]["family"]["version"] = 5.into();
        ty["parameters"]["family"]
            .as_object_mut()
            .unwrap()
            .remove("door_leaves");
    }
    for (name, data) in wire.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = 57.into();
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = 57.into();
            }
        }
    }
    wire["schema_version"] = 57.into();
    let old = wire.clone();
    migrate(&mut wire, 57).unwrap();
    assert_eq!(serde_json::from_value::<Model>(wire).unwrap(), model);
    for corruption in 0..3 {
        let mut bad = old.clone();
        let family = &mut bad["opening_types"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()["parameters"]["family"];
        match corruption {
            0 => family["door_leaves"] = "Single".into(),
            1 => family["version"] = 6.into(),
            _ => *family = serde_json::Value::Null,
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 57).is_err());
        assert_eq!(bad, before);
    }
}

#[test]
fn paired_doors_package_v5_v4_upgrade_and_save_reopen() {
    let mut model = model();
    let door = model
        .openings
        .values()
        .find(|o| model.resolve_opening(&o.parameters).unwrap().kind == OpeningKind::Door)
        .unwrap()
        .id();
    let resolved = model
        .resolve_opening(&model.openings[&door].parameters)
        .unwrap();
    let mut ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Pair".into(),
            kind: OpeningKind::Door,
            width: resolved.width,
            height: resolved.height,
            sill: 0.,
            pane_position: Default::default(),
            window_operation: Default::default(),
            family: OpeningFamily::default(),
        },
    );
    let length = LengthParameter::new(
        "core.length_parameter",
        LengthParameterParams {
            name: "Pair width".into(),
            value: resolved.width,
            unit: LengthUnit::Metres,
        },
    );
    let binding = OpeningTypeLengthBindings {
        width: Some(length.id()),
        ..Default::default()
    };
    model.length_parameters.insert(length.id(), length);
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Leaf".into(),
            density_kg_m3: 700.,
            color: [120, 90, 60],
        },
    );
    ty.parameters.family.panel_material = Some(material.id());
    model.materials.insert(material.id(), material);
    let tid = ty.id();
    model.opening_types.insert(tid, ty);
    model.opening_type_length_bindings.insert(tid, binding);
    model.openings.get_mut(&door).unwrap().parameters.definition =
        OpeningDefinition::Typed { type_id: tid };
    let bytes = export_opening_type_package(&model, tid).unwrap();
    let current = parse_opening_type_package(&bytes).unwrap();
    let mut old: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    old["version"] = 4.into();
    old["parameters"]["family"]["version"] = 5.into();
    old["parameters"]["family"]
        .as_object_mut()
        .unwrap()
        .remove("door_leaves");
    assert_eq!(
        parse_opening_type_package(&serde_json::to_vec(&old).unwrap()).unwrap(),
        current
    );
    model
        .opening_types
        .get_mut(&tid)
        .unwrap()
        .parameters
        .family
        .door_leaves = DoorLeaves::Paired {
        active_fraction: 0.6,
    };
    model.openings.get_mut(&door).unwrap().parameters.open_state = OpeningState::DoorPairAngles {
        active_degrees: 25.,
        inactive_degrees: 70.,
    };
    model.validate().unwrap();
    let package =
        parse_opening_type_package(&export_opening_type_package(&model, tid).unwrap()).unwrap();
    assert_eq!(package.parameters, model.opening_types[&tid].parameters);
    assert_eq!(package.length_bindings, binding);
    assert_eq!(package.materials.len(), 1);
    let mut doc = Document::from_model(model).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("paired.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    let before = doc.model().clone();
    let stats = doc.history_stats();
    let mut single = before.opening_types[&tid].parameters.clone();
    single.family.door_leaves = DoorLeaves::Single;
    assert!(
        doc.execute(
            "Invalid Single",
            vec![Command::UpdateOpeningType {
                id: tid,
                parameters: single.clone()
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), stats);
    let mut reset = before.openings[&door].parameters.clone();
    reset.open_state = OpeningState::Default;
    doc.execute(
        "Reset and Single",
        vec![
            Command::UpdateOpening {
                id: door,
                parameters: reset,
            },
            Command::UpdateOpeningType {
                id: tid,
                parameters: single,
            },
        ],
    )
    .unwrap();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
}
