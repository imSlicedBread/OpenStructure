use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};

#[test]
fn material_patterns_schema69_migration_roundtrip_and_atomic_ambiguity_rejection() {
    let mut old = serde_json::to_value(Model::new("Legacy")).unwrap();
    old.as_object_mut().unwrap().remove("plan_callouts");
    old.as_object_mut().unwrap().remove("section_callouts");
    old.as_object_mut().unwrap().remove("text_notes");
    old.as_object_mut().unwrap().remove("plan_hidden_openings");
    old.as_object_mut().unwrap().remove("group_definitions");
    old.as_object_mut().unwrap().remove("group_instances");
    old.as_object_mut().unwrap().remove("plan_graphics_rules");
    old.as_object_mut().unwrap().remove("sheet_sets");
    old.as_object_mut().unwrap().remove("drawing_issues");
    old.as_object_mut().unwrap().remove("material_patterns");
    old["schema_version"] = 69.into();
    for (name, value) in old.as_object_mut().unwrap() {
        if name == "project" {
            value["header"]["schema_version"] = 69.into();
        } else if let Some(map) = value.as_object_mut() {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                entity["header"]["schema_version"] = 69.into();
            }
        }
    }
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Legacy".into(),
            density_kg_m3: 1000.,
            color: [120; 3],
            cut_pattern: None,
        },
    );
    old["materials"][material.id().to_string()] = serde_json::to_value(&material).unwrap();
    old["materials"][material.id().to_string()]["header"]["schema_version"] = 69.into();
    old["materials"][material.id().to_string()]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("cut_pattern");
    for field in ["patterns", "reference"] {
        let mut bad = old.clone();
        if field == "patterns" {
            bad["material_patterns"] = serde_json::json!({});
        } else {
            bad["materials"][material.id().to_string()]["parameters"]["cut_pattern"] =
                serde_json::Value::Null;
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 69).is_err());
        assert_eq!(bad, before);
    }
    migrate(&mut old, 69).unwrap();
    let model: Model = serde_json::from_value(old).unwrap();
    model.validate().unwrap();
    assert!(model.material_patterns.is_empty());
    assert_eq!(model.materials[&material.id()].parameters.cut_pattern, None);
    let mut doc = Document::from_model(model).unwrap();
    let before = doc.model().clone();
    let pattern = MaterialPattern::new(
        "core.material_pattern",
        MaterialPatternParams {
            name: "Stone".into(),
            angle_degrees: 30.,
            spacing: 0.08,
            origin: [10., 20., 0.5],
        },
    );
    let mut parameters = material.parameters;
    parameters.cut_pattern = Some(pattern.id());
    doc.execute(
        "Pattern + assignment",
        vec![
            Command::AddMaterialPattern(pattern.clone()),
            Command::UpdateMaterial {
                id: material.header.id,
                parameters,
            },
        ],
    )
    .unwrap();
    let after = doc.model().clone();
    assert!(
        doc.execute(
            "Cannot delete assigned pattern",
            vec![Command::RemoveMaterialPattern(pattern.id())]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &after);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hatches.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);
}
