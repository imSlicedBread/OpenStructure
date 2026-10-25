#[path = "../../os-model/tests/support/drafting.rs"]
mod support;
use os_core::Point2;
use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
#[test]
fn drafting_save_reopen_preserves_plan_and_section_annotations() {
    for section in [false, true] {
        let (mut m, view) = support::fixture(section);
        let mut p = TextNoteParams::new(view);
        p.text = "Construction note\nSecond row".into();
        p.leader = Some(Point2::new(-1., -1.));
        let note = TextNote::new("core.text_note", p);
        m.text_notes.insert(note.id(), note);
        let line = DetailLine::new(
            "core.detail_line",
            DetailLineParams {
                view,
                start: Point2::default(),
                end: Point2::new(2., 3.),
            },
        );
        m.detail_lines.insert(line.id(), line);
        let doc = Document::from_model(m.clone()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("draft.osb");
        ZipJsonStorage.save(&doc, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &m);
    }
}
#[test]
fn schema75_migration_is_strict_atomic_and_preserves_plan_line_payloads() {
    let mut old: Value =
        serde_json::from_str(include_str!("fixtures/schema-75-drafting.json")).unwrap();
    let before = old.clone();
    migrate(&mut old, 75).unwrap();
    assert_eq!(old["text_notes"], json!({}));
    let m: Model = serde_json::from_value(old.clone()).unwrap();
    m.validate().unwrap();
    old.as_object_mut().unwrap().remove("text_notes");
    old.as_object_mut().unwrap().remove("plan_hidden_openings");
    old.as_object_mut().unwrap().remove("group_definitions");
    old.as_object_mut().unwrap().remove("group_instances");
    old.as_object_mut()
        .unwrap()
        .remove("plan_element_graphics_overrides");
    old.as_object_mut().unwrap().remove("plan_graphics_rules");
    old["schema_version"] = 75.into();
    for (key, map) in old.as_object_mut().unwrap() {
        if key == "project" {
            map["header"]["schema_version"] = 75.into();
        } else if key != "extensions"
            && let Some(map) = map.as_object_mut()
        {
            for e in map.values_mut().filter(|v| v.get("header").is_some()) {
                e["header"]["schema_version"] = 75.into();
            }
        }
    }
    let migrated_types = old["opening_types"].clone();
    for (id, original) in before["opening_types"].as_object().unwrap() {
        let mut expected = original.clone();
        let family = &mut expected["parameters"]["family"];
        if family["version"] == 9 {
            family["version"] = 10.into();
            for solid in family["extrusions"].as_array_mut().unwrap() {
                solid["bindings"] = json!({"x":null,"y":null,"depth":null,"normal_offset":null});
            }
        }
        assert_eq!(
            migrated_types[id], expected,
            "unexpected opening-family migration for {id}"
        );
    }
    old["opening_types"] = before["opening_types"].clone();
    assert_eq!(
        old, before,
        "schema-75 migration changed data beyond owned version upgrades"
    );
    for field in [Value::Null, json!({}), json!([])] {
        let mut bad = before.clone();
        bad["text_notes"] = field;
        let original = bad.clone();
        assert!(migrate(&mut bad, 75).is_err());
        assert_eq!(bad, original);
    }
    let mut ambiguous = before.clone();
    let owner = ambiguous["detail_lines"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["parameters"]["view"]
        .as_str()
        .unwrap()
        .to_owned();
    ambiguous["views"][&owner]["parameters"]["kind"] = "Section".into();
    let original = ambiguous.clone();
    assert!(
        migrate(&mut ambiguous, 75)
            .unwrap_err()
            .to_string()
            .contains("section detail lines require schema 76")
    );
    assert_eq!(ambiguous, original);
    let mut current = serde_json::to_value(m).unwrap();
    current.as_object_mut().unwrap().remove("text_notes");
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
}
