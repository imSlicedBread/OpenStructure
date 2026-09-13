use os_core::Point2;
use os_document::Command;
use os_model::{Grid, GridParams, Model, SCHEMA_VERSION};
use os_storage::{Manifest, StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use std::{fs, io::Write};
use zip::{ZipWriter, write::SimpleFileOptions};

// Captured from the schema-3 native pointer workflow before implementing schema 4.
const OLD: &str = include_str!("../../../fixtures/schema-3-pointer-walls.json");

#[test]
fn frozen_three_migration_and_grid_save_reopen_preserve_original() {
    let original: Value = serde_json::from_str(OLD).unwrap();
    assert_eq!(original["schema_version"], 3);
    let mut migrated = original.clone();
    migrate(&mut migrated, 3).unwrap();
    assert_eq!(migrated["schema_version"], SCHEMA_VERSION);
    assert_eq!(migrated["grids"], json!({}));
    let mut expected = original.clone();
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["grids"] = json!({});
    expected["project"]["header"]["schema_version"] = json!(SCHEMA_VERSION);
    for collection in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "materials",
        "views",
    ] {
        for entity in expected[collection].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    assert_eq!(migrated, expected); // All other values and identities are unchanged.
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("old.osb");
    let manifest = Manifest {
        format: "OpenStructure".into(),
        container_version: 2,
        schema_version: 3,
        project_id: serde_json::from_value(original["project"]["header"]["id"].clone()).unwrap(),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    let mut zip = ZipWriter::new(fs::File::create(&source).unwrap());
    for name in ["assets/", "previews/"] {
        zip.add_directory(name, SimpleFileOptions::default())
            .unwrap();
    }
    for (name, bytes) in [
        (
            "manifest.toml",
            toml::to_string(&manifest).unwrap().into_bytes(),
        ),
        ("model.json", OLD.as_bytes().to_vec()),
        ("assets/opaque.bin", vec![0, 255, 32]),
    ] {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
    let source_bytes = fs::read(&source).unwrap();
    let mut doc = ZipJsonStorage.open(&source).unwrap();
    assert_eq!(
        doc.model(),
        &serde_json::from_value::<Model>(expected).unwrap()
    );
    let grid = Grid::new(
        "core.grid",
        GridParams {
            name: "A".into(),
            building: *doc.model().buildings.keys().next().unwrap(),
            start: Point2::new(-2.0, 0.0),
            end: Point2::new(8.0, 0.0),
        },
    );
    doc.execute("Grid", vec![Command::AddGrid(grid.clone())])
        .unwrap();
    let saved = dir.path().join("new.osb");
    ZipJsonStorage.save(&doc, &saved).unwrap();
    let reopened = ZipJsonStorage.open(&saved).unwrap();
    assert_eq!(reopened.model(), doc.model());
    assert_eq!(reopened.model().grids[&grid.id()], grid);
    assert_eq!(reopened.auxiliary_files(), doc.auxiliary_files());
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}

#[test]
fn ambiguous_or_corrupt_schema_three_migration_is_atomic() {
    let source: Value = serde_json::from_str(OLD).unwrap();
    for case in 0..4 {
        let mut value = source.clone();
        match case {
            0 => value["grids"] = json!({}),
            1 => value["project"]["header"]["schema_version"] = json!(4),
            2 => value["levels"] = json!([]),
            _ => {
                value["walls"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"]["level"] = json!("00000000-0000-0000-0000-000000000000")
            }
        }
        let before = value.clone();
        assert!(migrate(&mut value, 3).is_err());
        assert_eq!(value, before);
    }
    let mut current = serde_json::to_value(Model::new("Current")).unwrap();
    current.as_object_mut().unwrap().remove("grids");
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
}
