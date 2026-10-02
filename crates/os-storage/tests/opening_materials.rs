mod common;

use os_document::Document;
use os_model::{Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

// Build schema 31 from frozen schema 29 data, without serializing today's family.
fn schema_31() -> Value {
    let mut old: Value =
        serde_json::from_str(include_str!("fixtures/schema-29-opening-dimensions.json")).unwrap();
    old["schema_version"] = json!(31);
    old["opening_tags"] = json!({});
    for (key, value) in old.as_object_mut().unwrap() {
        if key == "project" {
            value["header"]["schema_version"] = json!(31);
        } else if key != "extensions"
            && let Some(map) = value.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = json!(31);
                }
                if key == "openings" {
                    entity["parameters"]["width_override"] = Value::Null;
                    entity["parameters"]["height_override"] = Value::Null;
                }
            }
        }
    }
    old
}

#[test]
fn opening_materials_schema_31_family_3_migration_and_save_reopen() {
    let old = schema_31();
    let mut current = old.clone();
    migrate(&mut current, 31).unwrap();
    let mut model: Model = serde_json::from_value(current.clone()).unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    for ty in model.opening_types.values() {
        assert_eq!(ty.parameters.family.version, 6);
        assert_eq!(ty.parameters.family.panel_material, None);
        assert_eq!(ty.parameters.family.frame_material, None);
    }
    let mut reverse = current;
    reverse["schema_version"] = json!(31);
    for (key, value) in reverse.as_object_mut().unwrap() {
        if key == "project" {
            value["header"]["schema_version"] = json!(31);
        } else if key != "extensions"
            && let Some(map) = value.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = json!(31);
                }
                if key == "opening_types" {
                    let f = entity["parameters"]["family"].as_object_mut().unwrap();
                    f.insert("version".into(), json!(3));
                    f.remove("panel_material");
                    f.remove("frame_material");
                }
            }
        }
    }
    reverse.as_object_mut().unwrap().remove("stairs");
    reverse.as_object_mut().unwrap().remove("roofs");
    common::reverse_schema_38_migration(&mut reverse, &old);
    assert_eq!(
        reverse, old,
        "all pre-existing fields, headers and extension payloads survive"
    );
    let materials = ["Leaf/pane", "Rails"].map(|name| {
        os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: name.into(),
                density_kg_m3: 500.,
                color: [180, 180, 180],
            },
        )
    });
    let ids = materials.each_ref().map(|m| m.id());
    for m in materials {
        model.materials.insert(m.id(), m);
    }
    for ty in model.opening_types.values_mut() {
        ty.parameters.family.panel_material = Some(ids[0]);
        ty.parameters.family.frame_material = Some(ids[1]);
    }
    let doc = Document::from_model(model).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("materials.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}

#[test]
fn opening_materials_malformed_and_ambiguous_content_is_rejected_atomically() {
    let old = schema_31();
    let id = old["opening_types"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    for case in 0..8 {
        let mut bad = old.clone();
        match case {
            0 => bad["opening_types"][&id]["parameters"]["family"]["panel_material"] = Value::Null,
            1 => bad["opening_types"][&id]["parameters"]["family"]["frame_material"] = Value::Null,
            2 => bad["opening_types"][&id]["parameters"]["family"]["version"] = json!(4),
            3 => bad["opening_types"][&id]["parameters"]["family"]["unknown"] = json!(1),
            4 => {
                bad["opening_types"][&id]["parameters"]["family"]
                    .as_object_mut()
                    .unwrap()
                    .remove("profile");
            }
            5 => bad["opening_types"][&id]["parameters"]["family"]["frame_width"] = json!(-1),
            6 => bad["opening_types"][&id]["header"]["schema_version"] = json!(30),
            _ => {
                bad.as_object_mut().unwrap().remove("opening_tags");
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 31).is_err(), "case {case}");
        assert_eq!(bad, before);
    }
    let mut current = old;
    migrate(&mut current, 31).unwrap();
    for field in ["panel_material", "frame_material"] {
        for missing in [false, true] {
            let mut bad = current.clone();
            let f = bad["opening_types"][&id]["parameters"]["family"]
                .as_object_mut()
                .unwrap();
            if missing {
                f.remove(field);
            } else {
                f.insert(field.into(), json!(os_core::Id::new()));
            }
            let before = bad.clone();
            assert!(migrate(&mut bad, SCHEMA_VERSION).is_err());
            assert_eq!(bad, before);
        }
    }
}
