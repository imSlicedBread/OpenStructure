use super::*;
use os_core::{Id, Point2};
use os_document::Command;
use os_model::{Column, ColumnParams, Model};
use serde_json::{Value, json};

fn schema_26() -> Value {
    let mut model = Model::new("Schema 26");
    model.schema_version = 26;
    let mut value = serde_json::to_value(model).unwrap();
    value.as_object_mut().unwrap().remove("columns");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    value["schema_version"] = json!(26);
    for collection in [
        "project",
        "sites",
        "buildings",
        "levels",
        "walls",
        "wall_joins",
        "wall_types",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "room_separation_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        if collection == "project" {
            value[collection]["header"]["schema_version"] = json!(26);
        } else {
            for entity in value[collection].as_object_mut().unwrap().values_mut() {
                entity["header"]["schema_version"] = json!(26);
            }
        }
    }
    let extension: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/extension-envelope-v1.json"
    ))
    .unwrap();
    let extension_id = extension["id"].as_str().unwrap().to_owned();
    let mut extensions = serde_json::Map::new();
    extensions.insert(extension_id, extension);
    value["extensions"] = Value::Object(extensions);
    value["plugin_requirements"] = json!({"org.example.columns":{"version":"1.0.0"}});
    value
}

#[test]
fn schema_26_migration_adds_columns_and_preserves_opaque_extensions() {
    let old = schema_26();
    let mut migrated = old.clone();
    migrate(&mut migrated, 26).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.columns.is_empty());
    assert_eq!(migrated["extensions"], old["extensions"]);
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        SCHEMA_VERSION
    );

    let mut ambiguous = old.clone();
    ambiguous["columns"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 26).is_err());
    assert_eq!(ambiguous, before);

    let mut malformed = old;
    malformed["levels"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["header"]["schema_version"] = json!(25);
    let before = malformed.clone();
    assert!(migrate(&mut malformed, 26).is_err());
    assert_eq!(malformed, before);
}

#[test]
fn migrated_archive_accepts_native_column_and_reopens_identity_and_extension_data() {
    let old = schema_26();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("column.osb");
    let manifest = Manifest {
        format: "OpenStructure".into(),
        container_version: 2,
        schema_version: 26,
        project_id: Id(old["project"]["header"]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    let mut zip = ZipWriter::new(File::create(&path).unwrap());
    let options = SimpleFileOptions::default();
    zip.start_file("manifest.toml", options).unwrap();
    zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
        .unwrap();
    zip.start_file("model.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&old).unwrap()).unwrap();
    zip.finish().unwrap();

    let mut document = ZipJsonStorage.open(&path).unwrap();
    let level = *document.model().levels.keys().next().unwrap();
    let column = Column::new(
        "core.column",
        ColumnParams {
            name: "C1".into(),
            level,
            center: Point2::new(1.0, 2.0),
            width: 0.4,
            depth: 0.6,
            height: 3.0,
            base_offset: 0.1,
            material: None,
        },
    );
    let id = column.id();
    document
        .execute("Place column", vec![Command::AddColumn(column.clone())])
        .unwrap();
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().columns[&id], column);
    assert_eq!(
        serde_json::to_value(&reopened.model().extensions).unwrap(),
        old["extensions"]
    );
    assert!(!reopened.can_undo());
}
