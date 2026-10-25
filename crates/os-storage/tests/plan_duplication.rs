#[path = "../../os-document/tests/support/plan_duplication.rs"]
mod support;
use os_document::{Document, PlanDuplicateMode};
use os_storage::{StorageBackend, ZipJsonStorage};

#[test]
fn duplicate_save_reopen_and_delete_preserve_complete_clone_and_sources() {
    let (m, source) = support::fixture();
    let mut doc = Document::from_model(m.clone()).unwrap();
    let draft = doc
        .prepare_plan_duplicate(
            source,
            "Detailed copy".into(),
            PlanDuplicateMode::WithDetailing,
        )
        .unwrap();
    let copy = doc.apply_plan_duplicate(draft).unwrap();
    let tmp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = tmp.path().join("duplicate.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let mut reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), doc.model());
    reopened.delete_plan(copy).unwrap();
    assert_eq!(reopened.model(), &m);
    reopened.undo();
    assert_eq!(reopened.model(), doc.model());
}
