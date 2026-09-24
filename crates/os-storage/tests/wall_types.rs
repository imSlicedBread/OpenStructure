use os_model::{Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

#[test]
fn frozen_21_preserves_every_existing_field_and_never_shares_old_walls() {
    let old: Value =
        serde_json::from_str(include_str!("fixtures/schema-21-independent-walls.json")).unwrap();
    let mut value = old.clone();
    migrate(&mut value, 21).unwrap();
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.wall_types.is_empty() && model.wall_type_assignments.is_empty());
    let mut restored = value;
    restored.as_object_mut().unwrap().remove("wall_types");
    restored
        .as_object_mut()
        .unwrap()
        .remove("wall_type_assignments");
    restored.as_object_mut().unwrap().remove("columns");
    restored
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    restored.as_object_mut().unwrap().remove("plan_graphics");
    restored["schema_version"] = json!(21);
    restored["project"]["header"]["schema_version"] = json!(21);
    for (key, data) in restored.as_object_mut().unwrap() {
        if matches!(
            key.as_str(),
            "project" | "extensions" | "plugin_requirements"
        ) {
            continue;
        }
        if let Some(map) = data.as_object_mut() {
            for entity in map.values_mut() {
                entity["header"]["schema_version"] = json!(21);
            }
        }
    }
    assert_eq!(restored, old);
    for wall in model.walls.values() {
        assert_eq!(
            model.resolve_wall(wall.id()).unwrap().parameters,
            wall.parameters
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("migrated.osb");
    ZipJsonStorage
        .save(
            &os_document::Document::from_model(model.clone()).unwrap(),
            &path,
        )
        .unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);
    for field in ["wall_types", "wall_type_assignments"] {
        let mut bad = old.clone();
        bad[field] = json!({});
        let before = bad.clone();
        assert!(migrate(&mut bad, 21).is_err());
        assert_eq!(bad, before);
    }
}
