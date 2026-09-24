use os_core::Point2;
use os_document::{Command, Document};
use os_model::{Model, SCHEMA_VERSION, SectionViewSettings, View, ViewKind, ViewParams};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen_fourteen() -> Value {
    let model = Model::new("section migration");
    let mut value = serde_json::to_value(model).unwrap();
    value["schema_version"] = json!(14);
    value.as_object_mut().unwrap().remove("schedules");
    value.as_object_mut().unwrap().remove("detail_lines");
    value
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    value.as_object_mut().unwrap().remove("wall_joins");
    value.as_object_mut().unwrap().remove("wall_types");
    value
        .as_object_mut()
        .unwrap()
        .remove("wall_type_assignments");
    value.as_object_mut().unwrap().remove("columns");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    for collection in [
        "project",
        "sites",
        "buildings",
        "levels",
        "walls",
        "floors",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "dimensions",
        "grids",
        "materials",
        "views",
        "sheets",
    ] {
        if collection == "project" {
            value[collection]["header"]["schema_version"] = json!(14);
            continue;
        }
        for entity in value[collection].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(14);
            if collection == "views" {
                entity["parameters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("section");
            }
        }
    }
    value
}

#[test]
fn schema_fourteen_migrates_to_fifteen_and_preserves_unconfigured_views() {
    let old = frozen_fourteen();
    let mut migrated = old.clone();
    migrate(&mut migrated, 14).unwrap();

    let mut expected = old;
    expected["schedules"] = json!({});
    expected["detail_lines"] = json!({});
    expected["room_separation_lines"] = json!({});
    expected["wall_joins"] = json!({});
    expected["wall_types"] = json!({});
    expected["wall_type_assignments"] = json!({});
    expected["columns"] = json!({});
    expected["plan_graphics_templates"] = json!({});
    expected["plan_graphics"] = json!({});
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["project"]["header"]["schema_version"] = json!(SCHEMA_VERSION);
    for collection in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "floors",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "dimensions",
        "grids",
        "materials",
        "views",
        "sheets",
    ] {
        for entity in expected[collection].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    assert_eq!(migrated, expected);
    let model: Model = serde_json::from_value(migrated).unwrap();
    model.validate().unwrap();
    assert!(
        model
            .views
            .values()
            .all(|view| view.parameters.section.is_none())
    );
}

#[test]
fn schema_fourteen_migration_rejects_bad_headers_atomically() {
    let mut bad = frozen_fourteen();
    bad["views"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["header"]["schema_version"] = json!(13);
    let before = bad.clone();
    assert!(migrate(&mut bad, 14).is_err());
    assert_eq!(bad, before);

    let mut ambiguous = frozen_fourteen();
    let view = ambiguous["views"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    view["parameters"]["section"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 14).is_err());
    assert_eq!(ambiguous, before);
}

#[test]
fn linked_section_definition_is_transactional_and_survives_save_reopen() {
    let mut document = Document::new("Section view").unwrap();
    let level = *document.model().levels.keys().next().unwrap();
    let parameters = ViewParams {
        name: "Section A".into(),
        kind: ViewKind::Section,
        level: Some(level),
        settings_revision: 0,
        plan: None,
        section: Some(SectionViewSettings::new(
            Point2::new(-4.0, 3.0),
            Point2::new(12.0, 3.0),
            -0.5,
            8.0,
        )),
    };
    let section = View::new("core.view", parameters);
    let id = section.id();
    document
        .execute(
            "Create section view",
            vec![Command::AddView(section.clone())],
        )
        .unwrap();
    assert_eq!(document.model().views[&id], section);
    assert!(document.undo());
    assert!(!document.model().views.contains_key(&id));
    assert!(document.redo());
    assert_eq!(document.model().views[&id], section);

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("section.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().views[&id], section);
    assert_eq!(reopened.model().schema_version, SCHEMA_VERSION);
}
