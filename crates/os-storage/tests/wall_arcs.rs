use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::Value;
mod common;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-53-straight-walls.json")).unwrap()
}

#[test]
fn frozen_53_migration_preserves_every_value_and_roundtrips_straights_and_arcs() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 53).unwrap();
    let mut reverse = value.clone();
    common::reverse_wall_paths(&mut reverse);
    reverse["schema_version"] = 53.into();
    for (key, data) in reverse.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = 53.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = 53.into();
            }
        }
    }
    assert_eq!(reverse, original);
    let mut model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("arc.osb");
    for add_arc in [false, true] {
        if add_arc {
            let wall = Wall::new(
                "org.openstructure.walls.wall",
                WallParams {
                    name: "Arc".into(),
                    path: WallPath::CircularArc {
                        center: os_core::Point2::default(),
                        radius: 5.0,
                        start_angle_rad: 0.2,
                        signed_sweep_rad: -4.0,
                    },
                    thickness: 0.2,
                    height: 3.0,
                    level: *model.levels.keys().next().unwrap(),
                    material: None,
                },
            );
            model.walls.insert(wall.id(), wall);
        }
        let doc = Document::from_model(model.clone()).unwrap();
        ZipJsonStorage.save(&doc, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);
    }
}

#[test]
fn ambiguous_53_and_current_redundant_endpoints_reject_atomically() {
    for case in 0..4 {
        let mut value = frozen();
        let wall = value["walls"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        match case {
            0 => wall["parameters"]["path"] = serde_json::json!({"kind":"Straight"}),
            1 => {
                wall["parameters"].as_object_mut().unwrap().remove("end");
            }
            2 => wall["header"]["schema_version"] = 52.into(),
            _ => {
                wall["parameters"]["path"] = serde_json::json!({"kind":"CircularArc","center":{"x":0,"y":0},"radius":2,"start_angle_rad":0,"signed_sweep_rad":1});
            }
        }
        let before = value.clone();
        assert!(migrate(&mut value, 53).is_err());
        assert_eq!(value, before);
    }
    let mut value = frozen();
    migrate(&mut value, 53).unwrap();
    value["walls"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["parameters"]["start"] = serde_json::json!({"x":0,"y":0});
    assert!(serde_json::from_value::<Model>(value).is_err());
}
