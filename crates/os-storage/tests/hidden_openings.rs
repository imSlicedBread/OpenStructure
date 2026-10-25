mod common;
#[path = "../../os-document/tests/support/hidden_openings.rs"]
mod support;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};

#[test]
fn hidden_openings_schema79_frozen_migration_is_atomic_and_preserves_payloads() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-79-hidden-openings.json")).unwrap();
    let mut wire = original.clone();
    migrate(&mut wire, 79).unwrap();
    assert_eq!(wire["schema_version"], SCHEMA_VERSION);
    assert_eq!(wire["plan_hidden_openings"], serde_json::json!({}));
    let mut expected = original.clone();
    common::current_native_headers(&mut expected);
    assert_eq!(wire, expected);
    for value in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!([]),
        serde_json::json!({"bad": ["bad"]}),
    ] {
        let mut bad = original.clone();
        bad["plan_hidden_openings"] = value;
        let before = bad.clone();
        assert!(migrate(&mut bad, 79).is_err());
        assert_eq!(bad, before);
    }
    wire.as_object_mut().unwrap().remove("plan_hidden_openings");
    let before = wire.clone();
    assert!(migrate(&mut wire, SCHEMA_VERSION).is_err());
    assert_eq!(wire, before);
}

#[test]
fn hidden_openings_native_save_reopen_and_history() {
    let (m, view, ids) = support::fixture();
    let mut d = Document::from_model(m.clone()).unwrap();
    d.execute(
        "Hide",
        vec![Command::SetPlanHiddenOpenings {
            view,
            hidden: ids.into_iter().collect(),
        }],
    )
    .unwrap();
    let hidden = d.model().clone();
    d.undo();
    assert_eq!(d.model(), &m);
    d.redo();
    assert_eq!(d.model(), &hidden);
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = dir.path().join("hidden.osb");
    ZipJsonStorage.save(&d, &path).unwrap();
    let mut reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), &hidden);
    reopened
        .execute(
            "Restore",
            vec![Command::SetPlanHiddenOpenings {
                view,
                hidden: Default::default(),
            }],
        )
        .unwrap();
    assert_eq!(reopened.model(), &m);
    reopened.undo();
    assert_eq!(reopened.model(), &hidden);
}
