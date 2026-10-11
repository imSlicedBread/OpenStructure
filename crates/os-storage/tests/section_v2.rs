use os_document::Document;
use os_model::{Model, SectionDisplayMode};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
const VIEW: &str = "00000000-0000-4000-8000-000000000168";
fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-67-sections.json")).unwrap()
}

#[test]
fn frozen_schema67_section_migration_preserves_long_extent_identity_metadata_and_reopens() {
    let before = frozen();
    let mut value = before.clone();
    migrate(&mut value, 67).unwrap();
    assert_eq!(
        value["views"][VIEW]["header"]["properties"],
        before["views"][VIEW]["header"]["properties"]
    );
    assert_eq!(value["extensions"], before["extensions"]);
    assert_eq!(
        value["project"]["header"]["properties"],
        before["project"]["header"]["properties"]
    );
    let old = &before["views"][VIEW]["parameters"]["section"];
    let new = &value["views"][VIEW]["parameters"]["section"];
    for field in ["start", "end", "bottom_elevation", "top_elevation"] {
        assert_eq!(new[field], old[field]);
    }
    assert_eq!(new["schema_version"], 2);
    assert_eq!(new["min_station"], 0.);
    assert_eq!(new["max_station"], json!(1800000_f64.hypot(1800000.)));
    assert_eq!(new["mode"], "CutOnly");
    assert_eq!(new["scale_denominator"], 100.);
    let model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let mut doc = Document::from_model(model).unwrap();
    let id = serde_json::from_value(json!(VIEW)).unwrap();
    let mut p = doc.model().views[&id].parameters.clone();
    let s = p.section.as_mut().unwrap();
    s.mode = SectionDisplayMode::CutAndProjection;
    s.min_station = -20.;
    s.max_station = 60.;
    s.far_depth = 12.;
    s.scale_denominator = 50.;
    doc.execute(
        "Edit section",
        vec![os_document::Command::UpdateView { id, parameters: p }],
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("section.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}

#[test]
fn section_migration_rejects_ambiguous_and_malformed_input_atomically() {
    for (field, bad) in [
        ("schema_version", json!(2)),
        ("far_depth", json!(30)),
        ("top_elevation", json!(-4)),
        ("start", json!({"x":0,"y":0,"z":1})),
    ] {
        let mut value = frozen();
        value["views"][VIEW]["parameters"]["section"][field] = bad;
        let before = value.clone();
        assert!(migrate(&mut value, 67).is_err(), "{field}");
        assert_eq!(value, before);
    }
}
