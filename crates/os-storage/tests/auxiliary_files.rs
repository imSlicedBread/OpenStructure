use os_document::{Command, Document};
use os_model::Model;
use os_storage::{CONTAINER_VERSION, Manifest, StorageBackend, ZipJsonStorage};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

fn archive(path: &Path, files: &[(&str, &[u8])]) {
    let model = Model::new("Legacy contents");
    let manifest = Manifest {
        format: "OpenStructure".into(),
        container_version: 1,
        schema_version: os_model::SCHEMA_VERSION,
        project_id: model.project.id(),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("manifest.toml", options).unwrap();
    zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
        .unwrap();
    zip.start_file("model.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&model).unwrap()).unwrap();
    for (name, bytes) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn legacy_opaque_contents_survive_edit_history_save_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("legacy.osb");
    let destination = dir.path().join("current.osb");
    archive(
        &source,
        &[
            ("assets/墙.bin", &[0, 255, 128, 1]),
            ("extensions/org.example/data.json", b"{\"unknown\":true}"),
            ("notes.txt", b"keep me"),
        ],
    );
    let original = std::fs::read(&source).unwrap();
    let mut doc = ZipJsonStorage.open(&source).unwrap();
    let files = doc.auxiliary_files().clone();
    let identity = doc.model().project.id();
    doc.execute("Rename", vec![Command::RenameProject("Edited".into())])
        .unwrap();
    assert!(doc.undo());
    assert!(doc.redo());
    assert_eq!(doc.auxiliary_files(), &files);
    ZipJsonStorage.save(&doc, &destination).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let reopened = ZipJsonStorage.open(&destination).unwrap();
    assert_eq!(reopened.model(), doc.model());
    assert_eq!(reopened.model().project.id(), identity);
    for (name, bytes) in files {
        assert_eq!(reopened.auxiliary_files()[&name], bytes);
    }
    assert_eq!(reopened.auxiliary_files()["assets/"], Vec::<u8>::new());
    let mut zip = ZipArchive::new(File::open(destination).unwrap()).unwrap();
    let mut text = String::new();
    zip.by_name("manifest.toml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    let manifest: Manifest = toml::from_str(&text).unwrap();
    assert_eq!(manifest.container_version, CONTAINER_VERSION);
    assert_ne!(
        manifest.container_version, 1,
        "old reader must reject instead of losing files"
    );
}

#[test]
fn new_empty_and_nested_directories_round_trip_without_duplicate_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("directories.osb");
    let files = BTreeMap::from([
        ("assets/".into(), vec![]),
        ("previews/".into(), vec![]),
        ("extensions/empty/".into(), vec![]),
        ("empty.bin".into(), vec![]),
    ]);
    let doc = Document::from_model_and_files(Model::new("Test"), files.clone()).unwrap();
    for _ in 0..3 {
        ZipJsonStorage.save(&doc, &path).unwrap();
        assert_eq!(
            ZipJsonStorage.open(&path).unwrap().auxiliary_files(),
            &files
        );
    }
}

#[test]
fn unsafe_and_ambiguous_names_fail_without_modifying_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unsafe.osb");
    for name in [
        "../escape",
        "/absolute",
        "a/../b",
        "a//b",
        "a\\b",
        "C:drive",
        "NUL.txt",
        "x/COM1.bin",
        "x/trailing.",
        "x/trailing ",
        "Model.json",
        "model.json/nested",
    ] {
        archive(&path, &[(name, b"payload")]);
        let original = std::fs::read(&path).unwrap();
        assert!(ZipJsonStorage.open(&path).is_err(), "accepted {name}");
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    for names in [["data.bin", "DATA.bin"], ["parent", "parent/child"]] {
        archive(&path, &[(names[0], b"one"), (names[1], b"two")]);
        assert!(ZipJsonStorage.open(&path).is_err());
    }
}

#[test]
fn rejected_save_preserves_previous_good_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("good.osb");
    ZipJsonStorage
        .save(&Document::new("Good").unwrap(), &path)
        .unwrap();
    let original = std::fs::read(&path).unwrap();
    for files in [
        BTreeMap::from([("model.json".into(), b"overwrite".to_vec())]),
        BTreeMap::from([("assets/".into(), b"not a directory".to_vec())]),
        BTreeMap::from([("large.bin".into(), vec![0; 16 * 1024 * 1024 + 1])]),
        (0..1021).map(|i| (format!("file{i}"), vec![])).collect(),
    ] {
        let doc = Document::from_model_and_files(Model::new("Bad"), files).unwrap();
        assert!(ZipJsonStorage.save(&doc, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

#[test]
fn oversized_file_and_combined_payload_are_rejected_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.osb");
    let bytes = vec![0; 16 * 1024 * 1024 + 1];
    archive(&path, &[("large.bin", &bytes)]);
    assert!(ZipJsonStorage.open(&path).is_err());
    let chunk = &bytes[..16 * 1024 * 1024];
    archive(
        &path,
        &[
            ("a", chunk),
            ("b", chunk),
            ("c", chunk),
            ("d", chunk),
            ("e", b"x"),
        ],
    );
    assert!(ZipJsonStorage.open(&path).is_err());
}

#[test]
fn corrupted_auxiliary_crc_is_not_silently_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad-crc.osb");
    let marker = b"UNIQUE_AUXILIARY_CONTENT_012345";
    archive(&path, &[("data.bin", marker)]);
    let mut bytes = std::fs::read(&path).unwrap();
    let offset = bytes
        .windows(marker.len())
        .position(|w| w == marker)
        .unwrap();
    bytes[offset] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(ZipJsonStorage.open(&path).is_err());
}

#[test]
fn archived_symlink_is_rejected_not_repackaged_as_an_asset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("link.osb");
    archive(&path, &[]);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let mut zip = ZipWriter::new_append(file).unwrap();
    zip.add_symlink("assets/link", "../../outside", SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    assert!(ZipJsonStorage.open(&path).is_err());
}

#[test]
fn frozen_container_one_fixture_opens_and_upgrades_without_model_changes() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/intersecting-walls.osb");
    let original = std::fs::read(&source).unwrap();
    let doc = ZipJsonStorage.open(&source).unwrap();
    assert_eq!(doc.model().walls.len(), 3);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("upgraded.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    assert_eq!(std::fs::read(&source).unwrap(), original);
}

#[test]
fn deterministic_archive_mutation_fuzz_smoke_is_panic_free() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mutations.osb");
    archive(&path, &[("assets/test.bin", b"payload")]);
    let original = std::fs::read(&path).unwrap();
    let mut state = 0x6d2b79f5_u32;
    for i in 0..128 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let offset = state as usize % original.len();
        let mut bytes = original.clone();
        if i % 2 == 0 {
            bytes.truncate(offset);
        } else {
            bytes[offset] ^= (state >> 24) as u8 | 1;
        }
        std::fs::write(&path, bytes).unwrap();
        if let Ok(document) = ZipJsonStorage.open(&path) {
            document.model().validate().unwrap();
            assert!(
                document
                    .auxiliary_files()
                    .values()
                    .map(Vec::len)
                    .sum::<usize>()
                    <= 64 * 1024 * 1024
            );
        }
    }
}
