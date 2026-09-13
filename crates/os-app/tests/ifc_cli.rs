use os_storage::{StorageBackend, ZipJsonStorage};
use std::{
    path::Path,
    process::{Command, Output},
};

fn app(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_os-app"))
        .args(args)
        .output()
        .unwrap()
}
fn text(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn exchange_cli_requires_loss_acknowledgement_and_never_clobbers() {
    let dir = tempfile::tempdir().unwrap();
    let native = dir.path().join("original.osb");
    let ifc = dir.path().join("exchange.ifc");
    let imported = dir.path().join("imported.osb");
    assert!(app(&["--smoke", text(&native)]).status.success());
    let refused = app(&["--export-ifc", text(&native), text(&ifc)]);
    assert!(!refused.status.success());
    assert!(!ifc.exists());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--allow-loss"));
    let exported = app(&["--export-ifc", text(&native), text(&ifc), "--allow-loss"]);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let original = std::fs::read(&ifc).unwrap();
    assert!(
        !app(&["--export-ifc", text(&native), text(&ifc), "--allow-loss"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&ifc).unwrap(), original);
    let result = app(&["--import-ifc", text(&ifc), text(&imported)]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let a = ZipJsonStorage.open(&native).unwrap();
    let b = ZipJsonStorage.open(&imported).unwrap();
    assert_eq!(a.model().walls, b.model().walls);
    assert_eq!(a.model().levels, b.model().levels);
    let saved = std::fs::read(&imported).unwrap();
    assert!(
        !app(&["--import-ifc", text(&ifc), text(&imported)])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&imported).unwrap(), saved);
}

#[test]
fn bad_import_and_wrong_extensions_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.ifc");
    let dest = dir.path().join("new.osb");
    std::fs::write(&bad, b"ISO-10303-21; corrupted").unwrap();
    assert!(
        !app(&["--import-ifc", text(&bad), text(&dest)])
            .status
            .success()
    );
    assert!(!dest.exists());
    assert!(
        !app(&["--import-ifc", text(&bad), "wrong.ifc"])
            .status
            .success()
    );
    assert!(
        !app(&["--import-ifc", text(&bad), text(&dest), "--allow-loss"])
            .status
            .success()
    );
}

#[test]
fn auxiliary_files_are_preserved_natively_and_reported_as_ifc_loss() {
    let directory = tempfile::tempdir().unwrap();
    let native = directory.path().join("attachments.osb");
    let output = directory.path().join("exchange.ifc");
    let files = std::collections::BTreeMap::from([(
        "assets/reference.bin".into(),
        b"opaque attachment".to_vec(),
    )]);
    let mut model = os_model::Model::new("Attachments");
    // Remove other native-only data so the attachment itself triggers consent.
    model.views.clear();
    let document = os_document::Document::from_model_and_files(model, files).unwrap();
    ZipJsonStorage.save(&document, &native).unwrap();
    let original = std::fs::read(&native).unwrap();
    let refused = app(&["--export-ifc", text(&native), text(&output)]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("1 native auxiliary file"));
    assert!(!output.exists());
    assert!(
        app(&["--export-ifc", text(&native), text(&output), "--allow-loss"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&native).unwrap(), original);
}
