use os_document::Document;
use os_model::{Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
mod common;

fn archive(path: &std::path::Path, value: &Value) {
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};
    let mut zip = ZipWriter::new(std::fs::File::create(path).unwrap());
    let manifest = os_storage::Manifest {
        format: "OpenStructure".into(),
        container_version: 2,
        schema_version: value["schema_version"].as_u64().unwrap() as u32,
        project_id: serde_json::from_value(value["project"]["header"]["id"].clone()).unwrap(),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    zip.start_file("manifest.toml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
        .unwrap();
    zip.start_file("model.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(serde_json::to_string(value).unwrap().as_bytes())
        .unwrap();
    zip.finish().unwrap();
}

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-48-opening-visibility.json")).unwrap()
}

#[test]
fn opening_visibility_frozen_48_migrates_only_headers_and_settings_and_reopens() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 48).unwrap();
    let mut expected = original.clone();
    common::apply_wall_paths(&mut expected);
    expected["schema_version"] = SCHEMA_VERSION.into();
    for (key, data) in expected.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
            }
        }
    }
    common::apply_opening_visibility(&mut expected);
    assert_eq!(value, expected);
    let mut model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let view = *model
        .views
        .iter()
        .find(|(_, v)| v.parameters.plan.is_some())
        .unwrap()
        .0;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opening-visibility.osb");
    archive(&path, &original);
    let original_bytes = std::fs::read(&path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);
    assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
    for (doors, windows) in [(true, true), (false, true), (true, false), (false, false)] {
        let visibility = &mut model
            .views
            .get_mut(&view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .visibility;
        visibility.doors = doors;
        visibility.windows = windows;
        let document = Document::from_model(model.clone()).unwrap();
        ZipJsonStorage.save(&document, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);
    }
}

#[test]
fn opening_visibility_migration_and_required_current_fields_are_strict_atomic() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.osb");
    let original = frozen();
    let view = original["views"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, v)| !v["parameters"]["plan"].is_null())
        .unwrap()
        .0
        .clone();
    for case in 0..12 {
        let mut value = original.clone();
        let plan = &mut value["views"][&view]["parameters"]["plan"];
        match case {
            0 => plan["visibility"]["doors"] = true.into(),
            1 => plan["visibility"]["windows"] = false.into(),
            2 => plan["visibility"]["doors"] = Value::Null,
            3 => {
                plan["visibility"]["doors"] = true.into();
                plan["visibility"]["windows"] = true.into();
            }
            4 => plan["schema_version"] = 4.into(),
            5 => plan["schema_version"] = 2.into(),
            6 => plan["visibility"] = json!([]),
            7 => {
                plan["visibility"].as_object_mut().unwrap().remove("walls");
            }
            8 => value["views"][&view]["header"]["schema_version"] = 47.into(),
            9 => value["project"]["header"]["schema_version"] = 47.into(),
            10 => {
                value.as_object_mut().unwrap().remove("schedules");
            }
            _ => value["schema_version"] = 47.into(),
        }
        let before = value.clone();
        assert!(migrate(&mut value, 48).is_err(), "case {case}");
        assert_eq!(value, before);
        archive(&path, &before);
        let bytes = std::fs::read(&path).unwrap();
        assert!(ZipJsonStorage.open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let mut current = original;
    migrate(&mut current, 48).unwrap();
    for field in ["doors", "windows"] {
        for bad in [None, Some(Value::Null), Some(json!("true")), Some(json!(1))] {
            let mut value = current.clone();
            let visibility = value["views"][&view]["parameters"]["plan"]["visibility"]
                .as_object_mut()
                .unwrap();
            if let Some(bad) = bad {
                visibility.insert(field.into(), bad);
            } else {
                visibility.remove(field);
            }
            let before = value.clone();
            assert!(migrate(&mut value, SCHEMA_VERSION).is_err());
            assert_eq!(value, before);
        }
    }
}
