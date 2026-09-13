use os_core::Id;
use os_document::{Command, Document};
use os_model::{Model, PlanSettings, SCHEMA_VERSION, ViewKind};
use os_storage::{Manifest, StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use std::{fs::File, io::Write, path::Path};
use zip::{ZipWriter, write::SimpleFileOptions};

const FIXTURE: &str = include_str!("../../../fixtures/schema-2-plans.json");
const PLAN: &str = "55555555-5555-4555-8555-555555555555";

fn archive(path: &Path, value: &Value) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    zip.add_directory("assets/", SimpleFileOptions::default())
        .unwrap();
    zip.add_directory("previews/", SimpleFileOptions::default())
        .unwrap();
    let manifest = Manifest {
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
    zip.start_file("assets/vendor.bin", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&[0, 255, 1, 254]).unwrap();
    zip.finish().unwrap();
}

#[test]
fn frozen_schema_two_views_migrate_without_losing_identity_or_opaque_contents() {
    let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
    let original = value.clone();
    migrate(&mut value, 2).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    assert_eq!(value["extensions"], original["extensions"]);
    assert_eq!(
        value["plugin_requirements"],
        original["plugin_requirements"]
    );
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    for (id, view) in &model.views {
        let old = &original["views"][id.to_string()];
        assert_eq!(
            serde_json::to_value(&view.header.properties).unwrap(),
            old["header"]["properties"]
        );
        assert_eq!(
            serde_json::to_value(&view.header.relationships).unwrap(),
            old["header"]["relationships"]
        );
        assert_eq!(view.parameters.name, old["parameters"]["name"]);
        assert_eq!(
            serde_json::to_value(view.parameters.level).unwrap(),
            old["parameters"]["level"]
        );
        assert_eq!(view.parameters.settings_revision, 0);
        assert_eq!(
            view.parameters.plan,
            (view.parameters.kind == ViewKind::Plan).then(PlanSettings::default)
        );
    }
    let directory = tempfile::tempdir().unwrap();
    let old_path = directory.path().join("original.osb");
    archive(&old_path, &original);
    let bytes = std::fs::read(&old_path).unwrap();
    let document = ZipJsonStorage.open(&old_path).unwrap();
    assert_eq!(document.model(), &model);
    assert_eq!(
        document.auxiliary_files()["assets/vendor.bin"],
        [0, 255, 1, 254]
    );
    let new_path = directory.path().join("upgraded.osb");
    ZipJsonStorage.save(&document, &new_path).unwrap();
    let reopened = ZipJsonStorage.open(&new_path).unwrap();
    assert_eq!(reopened.model(), &model);
    assert_eq!(reopened.auxiliary_files(), document.auxiliary_files());
    assert_eq!(std::fs::read(&old_path).unwrap(), bytes);
}

#[test]
fn ambiguous_or_invalid_old_view_migration_is_atomic() {
    let source: Value = serde_json::from_str(FIXTURE).unwrap();
    for case in [
        "plan field",
        "revision field",
        "header",
        "kind",
        "missing level",
    ] {
        let mut value = source.clone();
        match case {
            "plan field" => value["views"][PLAN]["parameters"]["plan"] = json!({"vendor":true}),
            "revision field" => value["views"][PLAN]["parameters"]["settings_revision"] = json!(7),
            "header" => value["views"][PLAN]["header"]["schema_version"] = json!(3),
            "kind" => value["views"][PLAN]["parameters"]["kind"] = json!("FutureKind"),
            "missing level" => value["views"][PLAN]["parameters"]["level"] = json!(Id::new()),
            _ => unreachable!(),
        }
        let original = value.clone();
        assert!(migrate(&mut value, 2).is_err(), "{case}");
        assert_eq!(value, original, "{case}");
    }
}

#[test]
fn future_malformed_or_ambiguous_current_settings_are_rejected_without_replacing_a_file() {
    let mut source: Value = serde_json::from_str(FIXTURE).unwrap();
    migrate(&mut source, 2).unwrap();
    let directory = tempfile::tempdir().unwrap();
    for (index, field, bad) in [
        (0, "schema_version", json!(2)),
        (1, "range", json!({"top":1,"cut":2,"bottom":0,"depth":-1})),
        (2, "scale_denominator", json!(0)),
        (
            3,
            "basis",
            json!({"origin":{"x":0,"y":0,"z":1},"rotation":0}),
        ),
        (
            4,
            "visibility",
            json!({"walls":true,"extensions":true,"unknown":true}),
        ),
        (5, "crop", json!({"min":{"x":2,"y":0},"max":{"x":1,"y":1}})),
    ] {
        let mut value = source.clone();
        value["views"][PLAN]["parameters"]["plan"][field] = bad;
        let before = value.clone();
        assert!(migrate(&mut value, SCHEMA_VERSION).is_err());
        assert_eq!(value, before);
        let path = directory.path().join(format!("invalid-{index}.osb"));
        archive(&path, &value);
        let bytes = std::fs::read(&path).unwrap();
        assert!(ZipJsonStorage.open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn a_legacy_unassigned_plan_is_preserved_and_requires_explicit_level_assignment() {
    let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
    migrate(&mut value, 2).unwrap();
    let mut document = Document::from_model(serde_json::from_value(value).unwrap()).unwrap();
    let id = document
        .model()
        .views
        .values()
        .find(|view| view.parameters.kind == ViewKind::Plan && view.parameters.level.is_none())
        .unwrap()
        .id();
    let original = document.model().clone();
    let mut parameters = document.model().views[&id].parameters.clone();
    parameters.name = "Assigned plan".into();
    assert!(
        document
            .execute(
                "Invalid",
                vec![Command::UpdateView {
                    id,
                    parameters: parameters.clone()
                }]
            )
            .is_err()
    );
    assert_eq!(document.model(), &original);
    parameters.level = document.model().levels.keys().next().copied();
    document
        .execute("Assign", vec![Command::UpdateView { id, parameters }])
        .unwrap();
    assert_eq!(document.model().views[&id].parameters.settings_revision, 1);
    assert!(document.undo());
    assert_eq!(document.model(), &original);
}
