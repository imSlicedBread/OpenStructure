use os_core::Point2;
use os_document::{Command, Document};
use os_model::{DetailLine, DetailLineParams, Model, SCHEMA_VERSION};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen_seventeen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-17-detail-lines.json")).unwrap()
}

#[test]
fn schema_seventeen_migration_is_explicit_and_atomic() {
    let original = frozen_seventeen();
    let mut migrated = original.clone();
    migrate(&mut migrated, 17).unwrap();
    let mut expected = original.clone();
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["detail_lines"] = json!({});
    expected["room_separation_lines"] = json!({});
    expected["wall_joins"] = json!({});
    expected["wall_types"] = json!({});
    expected["wall_type_assignments"] = json!({});
    expected["columns"] = json!({});
    expected["plan_graphics_templates"] = json!({});
    expected["plan_graphics"] = json!({});
    expected["project"]["header"]["schema_version"] = json!(SCHEMA_VERSION);
    for map in ["sites", "buildings", "levels", "views"] {
        for entity in expected[map].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    assert_eq!(migrated, expected);
    let model: Model = serde_json::from_value(migrated).unwrap();
    model.validate().unwrap();
    for shape in [Value::Null, json!([]), json!({})] {
        let mut ambiguous = original.clone();
        ambiguous["detail_lines"] = shape;
        let before = ambiguous.clone();
        assert!(
            migrate(&mut ambiguous, 17)
                .unwrap_err()
                .to_string()
                .contains("ambiguous schema 17")
        );
        assert_eq!(ambiguous, before);
    }
    let mut bad_header = original.clone();
    bad_header["views"]["00000000-0000-4000-8000-000000000005"]["header"]["schema_version"] =
        json!(18);
    let before = bad_header.clone();
    assert!(migrate(&mut bad_header, 17).is_err());
    assert_eq!(bad_header, before);
}

#[test]
fn migrated_archive_with_detail_line_saves_and_reopens_without_identity_loss() {
    let mut value = frozen_seventeen();
    migrate(&mut value, 17).unwrap();
    let mut doc = Document::from_model(serde_json::from_value(value).unwrap()).unwrap();
    let view = *doc.model().views.keys().next().unwrap();
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view,
            start: Point2::new(-1.0, 0.0),
            end: Point2::new(2.0, 0.0),
        },
    );
    let id = line.id();
    doc.execute("Draw detail", vec![Command::AddDetailLine(line)])
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("detail.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), doc.model());
    assert_eq!(reopened.model().detail_lines[&id].id(), id);
    assert_eq!(
        reopened.model().project.header.properties["fixture"],
        "preserve"
    );
}
