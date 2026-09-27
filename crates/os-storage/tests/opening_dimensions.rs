mod common;

use os_document::{Command, Document};
use os_model::{Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-29-opening-dimensions.json")).unwrap()
}

#[test]
fn schema_29_dimensions_preserve_every_prior_field_and_pins_through_reopen() {
    let original = frozen();
    let mut migrated = original.clone();
    migrate(&mut migrated, 29).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    for opening in model.openings.values() {
        assert_eq!(
            (
                opening.parameters.width_override,
                opening.parameters.height_override
            ),
            (None, None)
        );
    }
    let mut reversed = migrated;
    common::remove_phase_fields(&mut reversed);
    common::remove_phase_fields(&mut reversed);
    reversed.as_object_mut().unwrap().remove("opening_tags");
    reversed["schema_version"] = json!(29);
    for (key, value) in reversed.as_object_mut().unwrap() {
        if key == "project" {
            value["header"]["schema_version"] = json!(29);
        } else if key != "extensions"
            && let Some(map) = value.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = json!(29);
                }
                if key == "openings" {
                    for field in ["width_override", "height_override"] {
                        assert_eq!(
                            entity["parameters"].as_object_mut().unwrap().remove(field),
                            Some(Value::Null)
                        );
                    }
                } else if key == "opening_types" {
                    let family = entity["parameters"]["family"].as_object_mut().unwrap();
                    family.insert("version".into(), json!(3));
                    family.remove("panel_material");
                    family.remove("frame_material");
                }
            }
        }
    }
    reversed.as_object_mut().unwrap().remove("stairs");
    reversed.as_object_mut().unwrap().remove("roofs");
    common::reverse_schema_38_migration(&mut reversed, &original);
    assert_eq!(reversed, original);
    let mut model = model;
    let id = model
        .openings
        .values()
        .find(|o| o.parameters.type_id().is_some())
        .unwrap()
        .id();
    let ty = model.openings[&id].parameters.type_id().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (width, height) in [(None, None), (Some(1.), Some(1.2)), (Some(0.8), Some(1.5))] {
        let p = &mut model.openings.get_mut(&id).unwrap().parameters;
        p.width_override = width;
        p.height_override = height;
        let doc = Document::from_model(model.clone()).unwrap();
        let path = dir.path().join("dimensions.osb");
        ZipJsonStorage.save(&doc, &path).unwrap();
        let mut reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        let mut parameters = reopened.model().opening_types[&ty].parameters.clone();
        parameters.width = 1.1;
        parameters.height = 1.4;
        parameters.sill = 0.9;
        reopened
            .execute(
                "Defaults",
                vec![Command::UpdateOpeningType { id: ty, parameters }],
            )
            .unwrap();
        let p = reopened
            .model()
            .resolve_opening(&reopened.model().openings[&id].parameters)
            .unwrap();
        assert_eq!(
            (p.width, p.height, p.sill),
            (width.unwrap_or(1.1), height.unwrap_or(1.4), 0.8)
        );
    }
}

#[test]
fn schema_29_dimensions_reject_ambiguous_and_current_missing_fields_atomically() {
    for field in ["width_override", "height_override"] {
        for value in [Value::Null, json!(1.2)] {
            let mut bad = frozen();
            let id = bad["openings"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone();
            bad["openings"][&id]["parameters"][field] = value;
            let before = bad.clone();
            assert!(migrate(&mut bad, 29).is_err());
            assert_eq!(bad, before);
        }
        let mut current = frozen();
        migrate(&mut current, 29).unwrap();
        let id = current["openings"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        current["openings"][&id]["parameters"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        let before = current.clone();
        assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
        assert_eq!(current, before);
        assert!(serde_json::from_value::<Model>(current).is_err());
    }
    let mut bad = frozen();
    bad["project"]["header"]["schema_version"] = json!(28);
    let before = bad.clone();
    assert!(migrate(&mut bad, 29).is_err());
    assert_eq!(bad, before);
}
