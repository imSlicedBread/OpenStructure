use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};

#[test]
fn annotation_crop_schema78_strict_migration_and_roundtrip_history() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-78-annotation-crop.json")).unwrap();
    let mut wire = original.clone();
    migrate(&mut wire, 78).unwrap();
    let model: Model = serde_json::from_value(wire.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    for (id, view) in original["views"].as_object().unwrap() {
        assert_eq!(wire["views"][id]["parameters"], view["parameters"]);
    }
    for new_field in [
        serde_json::Value::Null,
        serde_json::json!({"min":{"x":-2.,"y":-2.},"max":{"x":2.,"y":2.}}),
    ] {
        let mut bad = original.clone();
        let view = bad["views"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .find(|v| v["parameters"]["plan"].is_object())
            .unwrap();
        view["parameters"]["plan"]["annotation_crop"] = new_field;
        let before = bad.clone();
        assert!(migrate(&mut bad, 78).is_err());
        assert_eq!(bad, before);
    }
    let mut doc = Document::from_model(model).unwrap();
    let id = *doc
        .model()
        .views
        .iter()
        .find(|(_, v)| v.parameters.plan.is_some())
        .unwrap()
        .0;
    let before = doc.model().clone();
    let mut p = doc.model().views[&id].parameters.clone();
    let s = p.plan.as_mut().unwrap();
    s.crop = Some(PlanViewCrop {
        min: Point2::new(-1., -1.),
        max: Point2::new(1., 1.),
    });
    s.annotation_crop = Some(PlanViewCrop {
        min: Point2::new(-2., -2.),
        max: Point2::new(2., 2.),
    });
    doc.execute(
        "Annotation crop",
        vec![Command::UpdateView { id, parameters: p }],
    )
    .unwrap();
    let changed = doc.model().clone();
    let revision = doc.revision();
    let mut invalid = doc.model().views[&id].parameters.clone();
    invalid.plan.as_mut().unwrap().crop.as_mut().unwrap().max.x = 3.;
    let error = doc
        .execute(
            "Invalid crop",
            vec![Command::UpdateView {
                id,
                parameters: invalid,
            }],
        )
        .unwrap_err();
    assert!(error.to_string().contains("contain the model crop"));
    assert_eq!(doc.model(), &changed);
    assert_eq!(doc.revision(), revision);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &changed);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("annotations.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &changed);
}
