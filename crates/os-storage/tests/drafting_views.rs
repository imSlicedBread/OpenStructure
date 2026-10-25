use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
#[test]
fn schema77_strict_drafting_migration_preserves_existing_data() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-77-drafting-views.json")).unwrap();
    let mut wire = original.clone();
    migrate(&mut wire, 77).unwrap();
    let model: Model = serde_json::from_value(wire.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(
        model
            .views
            .values()
            .all(|v| v.parameters.drafting.is_none())
    );
    for (id, view) in original["views"].as_object().unwrap() {
        assert_eq!(wire["views"][id]["parameters"], view["parameters"]);
        assert_eq!(wire["views"][id]["header"]["id"], view["header"]["id"]);
    }
    for kind in [false, true] {
        let mut bad = original.clone();
        let view = bad["views"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        if kind {
            view["parameters"]["kind"] = "Drafting".into();
        } else {
            view["parameters"]["drafting"] = serde_json::Value::Null;
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 77).is_err());
        assert_eq!(bad, before);
    }
}
#[test]
fn drafting_view_annotations_crop_scale_name_save_reopen() {
    let mut doc = os_document::Document::new("Drafting").unwrap();
    let mut view = View::new("core.view", ViewParams::drafting("Detail D1"));
    view.parameters.drafting.as_mut().unwrap().scale_denominator = 25.;
    let id = view.id();
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view: id,
            start: os_core::Point2::new(-8., 0.),
            end: os_core::Point2::new(8., 0.),
        },
    );
    let note = TextNote::new("core.text_note", TextNoteParams::new(id));
    doc.execute(
        "Drafting",
        vec![
            os_document::Command::AddView(view),
            os_document::Command::AddDetailLine(line),
            os_document::Command::AddTextNote(note),
        ],
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drafting.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
}
