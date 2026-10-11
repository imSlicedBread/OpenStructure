#[path = "../../os-model/tests/support/section_dimensions.rs"]
mod fixture;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

#[test]
fn section_dimension_save_reopen_live_and_orphaned_preserves_identity() {
    let (m, _, opening, _, p) = fixture::fixture();
    let mut doc = Document::from_model(m).unwrap();
    let d = Dimension::new("core.dimension", p);
    let id = d.id();
    doc.execute("Dimension", vec![Command::AddDimension(d)])
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("section.osb");
    for orphan in [false, true] {
        if orphan {
            doc.execute("Delete", vec![Command::RemoveOpening(opening)])
                .unwrap();
        }
        ZipJsonStorage.save(&doc, &path).unwrap();
        let opened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(opened.model(), doc.model());
        assert_eq!(
            opened.model().dimensions[&id]
                .parameters
                .resolve(opened.model())
                .is_err(),
            orphan
        );
    }
}

#[test]
fn frozen_schema68_header_migration_preserves_payload_and_rejects_future_discriminants() {
    let before: Value =
        serde_json::from_str(include_str!("fixtures/schema-68-section-dimensions.json")).unwrap();
    let mut value = before.clone();
    migrate(&mut value, 68).unwrap();
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    assert_eq!(value["extensions"], before["extensions"]);
    for (id, v) in before["views"].as_object().unwrap() {
        assert_eq!(value["views"][id]["parameters"], v["parameters"]);
        assert_eq!(
            value["views"][id]["header"]["properties"],
            v["header"]["properties"]
        );
    }
    serde_json::from_value::<Model>(value)
        .unwrap()
        .validate()
        .unwrap();
    for p in [
        json!({"layout":"SectionHorizontal"}),
        json!({"first":{"SectionWall":{}}}),
        json!({"second":{"SectionOpening":{}}}),
    ] {
        let mut bad = before.clone();
        bad["dimensions"]["future"] = json!({"parameters":p});
        let original = bad.clone();
        assert!(migrate(&mut bad, 68).is_err());
        assert_eq!(bad, original);
    }
}
