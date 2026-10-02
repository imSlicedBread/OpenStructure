use os_core::{Id, Point2};
use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};

#[test]
fn schema_58_curtain_migration_and_populated_round_trip_are_strict() {
    let mut wire: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-58-curtain-base.json")).unwrap();
    let old = wire.clone();

    migrate(&mut wire, 58).unwrap();
    assert_eq!(wire["schema_version"], SCHEMA_VERSION);
    for collection in [
        "curtain_systems",
        "curtain_panel_types",
        "curtain_mullion_types",
        "ramps",
    ] {
        assert_eq!(wire[collection], serde_json::json!({}));
    }
    assert_eq!(wire["project"]["header"]["schema_version"], SCHEMA_VERSION);
    assert_eq!(
        wire["walls"]["00000000-0000-4000-8000-000000000006"]["header"]["schema_version"],
        SCHEMA_VERSION
    );

    let mut model: Model = serde_json::from_value(wire.clone()).unwrap();
    add_populated_curtain(&mut model);
    model.validate().unwrap();
    assert!(
        model.walls.values().any(|wall| {
            wall.header.properties.get("retained") == Some(&serde_json::json!(123))
        })
    );

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("curtain-migration.osb");
    let document = Document::from_model(model).unwrap();
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), document.model());

    let mut ambiguous = old.clone();
    ambiguous["curtain_systems"] = serde_json::json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 58).is_err());
    assert_eq!(ambiguous, before);

    let mut missing_current_field = wire;
    missing_current_field
        .as_object_mut()
        .unwrap()
        .remove("curtain_mullion_types");
    assert!(serde_json::from_value::<Model>(missing_current_field).is_err());
}

fn add_populated_curtain(model: &mut Model) {
    let level = *model.levels.keys().next().expect("fixture has a level");
    let glass = Material::new(
        "core.material",
        MaterialParams {
            name: "Curtain glass".into(),
            density_kg_m3: 2500.0,
            color: [145, 190, 220],
        },
    );
    let aluminum = Material::new(
        "core.material",
        MaterialParams {
            name: "Curtain frame".into(),
            density_kg_m3: 2700.0,
            color: [90, 100, 110],
        },
    );
    let panel_type = CurtainPanelType::new(
        "core.curtain_panel_type",
        CurtainPanelTypeParams {
            name: "Insulating glass".into(),
            kind: CurtainPanelKind::Glazing,
            thickness: 0.024,
            material: Some(glass.id()),
        },
    );
    let mullion_type = CurtainMullionType::new(
        "core.curtain_mullion_type",
        CurtainMullionTypeParams {
            name: "Thermal frame".into(),
            width: 0.06,
            depth: 0.12,
            material: Some(aluminum.id()),
        },
    );
    let mut parameters = CurtainSystemParams {
        name: "South facade".into(),
        level,
        start: Point2::new(1.0, 2.0),
        end: Point2::new(5.0, 3.0),
        base_offset: 0.15,
        height: 3.2,
        normal_flip: true,
        panel_type: panel_type.id(),
        mullion_type: mullion_type.id(),
        vertical: vec![
            CurtainGrid {
                id: Id::new(),
                position: 0.0,
            },
            CurtainGrid {
                id: Id::new(),
                position: 4.123_105_625_617_661,
            },
        ],
        horizontal: vec![
            CurtainGrid {
                id: Id::new(),
                position: 0.0,
            },
            CurtainGrid {
                id: Id::new(),
                position: 3.2,
            },
        ],
        panels: Vec::new(),
        mullions: Vec::new(),
    };
    parameters.reconcile().unwrap();
    let assembly = CurtainSystem::new("core.curtain_system", parameters);
    model.materials.insert(glass.id(), glass);
    model.materials.insert(aluminum.id(), aluminum);
    model
        .curtain_panel_types
        .insert(panel_type.id(), panel_type);
    model
        .curtain_mullion_types
        .insert(mullion_type.id(), mullion_type);
    model.curtain_systems.insert(assembly.id(), assembly);
}
