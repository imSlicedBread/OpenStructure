use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
mod common;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-52-dimension-faces.json")).unwrap()
}

#[test]
fn schema_52_face_migration_changes_only_headers_and_roundtrips_live_and_orphan_faces() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 52).unwrap();
    let mut expected = original;
    common::apply_wall_paths(&mut expected);
    expected["schema_version"] = SCHEMA_VERSION.into();
    for (key, data) in expected.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
            }
        }
    }
    common::apply_wall_paths(&mut expected);
    assert_eq!(value, expected);
    let mut model: Model = serde_json::from_value(value).unwrap();
    let id = model
        .dimensions
        .iter()
        .find(|(_, d)| d.parameters.layout == DimensionLayout::Aligned)
        .unwrap()
        .0
        .to_owned();
    let wall = model.dimensions[&id].parameters.first.entity();
    let p = &mut model.dimensions.get_mut(&id).unwrap().parameters;
    p.first = DimensionReference::WallFace {
        wall,
        side: DimensionWallSide::Left,
        station_m: 0.5,
    };
    p.second = DimensionReference::WallFace {
        wall,
        side: DimensionWallSide::Right,
        station_m: 0.5,
    };
    model.dimensions[&id]
        .parameters
        .validate_creation(&model)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("faces.osb");
    for orphan in [false, true] {
        if orphan
            && let DimensionReference::WallFace { station_m, .. } =
                &mut model.dimensions.get_mut(&id).unwrap().parameters.first
        {
            *station_m = 999.0;
        }
        let doc = Document::from_model(model.clone()).unwrap();
        ZipJsonStorage.save(&doc, &path).unwrap();
        let reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        assert_eq!(
            reopened.model().dimensions[&id]
                .parameters
                .resolve(reopened.model())
                .is_err(),
            orphan
        );
    }
}

#[test]
fn schema_52_malformed_headers_and_ambiguous_face_anchors_reject_atomically() {
    for case in 0..8 {
        let mut value = frozen();
        match case {
            0 => value["project"]["header"]["schema_version"] = 53.into(),
            1 => {
                value["walls"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["header"]["schema_version"] = 51.into()
            }
            2 => {
                value.as_object_mut().unwrap().remove("phases");
            }
            3 => value["dimensions"] = json!([]),
            _ => {
                let p = &mut value["dimensions"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"];
                let face = json!({"WallFace":{"wall":p["first"]["WallEndpoint"]["wall"],"side":"Left","station_m":0.5}});
                match case {
                    4 => p["first"] = face,
                    5 => p["second"] = face,
                    6 => p["additional"] = json!([face]),
                    _ => p["first"]["WallFace"] = face["WallFace"].clone(),
                }
            }
        }
        let before = value.clone();
        assert!(migrate(&mut value, 52).is_err(), "case {case}");
        assert_eq!(value, before, "case {case}");
    }
}
