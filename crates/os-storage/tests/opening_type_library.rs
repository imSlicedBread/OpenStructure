use os_model::{Model, OpeningKind, OpeningType, OpeningTypeParams, WindowPanePosition};
use os_storage::{
    MAX_OPENING_TYPE_LIBRARY_BYTES, MAX_OPENING_TYPE_LIBRARY_ENTRIES,
    MAX_OPENING_TYPE_LIBRARY_PACKAGES, MAX_OPENING_TYPE_PACKAGE_BYTES, export_opening_type_package,
    parse_opening_type_package, scan_opening_type_library,
};
use std::fs;

fn package_bytes(kind: OpeningKind) -> Vec<u8> {
    let mut model = Model::new("Library test");
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            name: format!("Library {kind:?}"),
            kind,
            width: 1.2,
            height: 2.1,
            sill: if kind == OpeningKind::Door { 0. } else { 0.8 },
            pane_position: WindowPanePosition::Center,
            family: Default::default(),
        },
    );
    let id = ty.id();
    model.opening_types.insert(id, ty);
    export_opening_type_package(&model, id).unwrap()
}

#[test]
fn valid_packages_are_sorted_case_insensitive_extension_and_read_only() {
    let directory = tempfile::tempdir().unwrap();
    let door = package_bytes(OpeningKind::Door);
    let window = package_bytes(OpeningKind::Window);
    let inputs = [("z-window.OsOt", &window), ("a-door.OSOT", &door)];
    for (name, bytes) in inputs {
        fs::write(directory.path().join(name), bytes).unwrap();
    }
    let scan = scan_opening_type_library(directory.path()).unwrap();
    assert_eq!(scan.package_files, 2);
    assert_eq!(scan.bytes_examined, door.len() + window.len());
    assert!(scan.issues.is_empty());
    assert_eq!(scan.entries.len(), 2);
    for (entry, (name, bytes)) in scan.entries.iter().zip(inputs.into_iter().rev()) {
        assert_eq!(entry.path, directory.path().join(name));
        assert_eq!(entry.package, parse_opening_type_package(bytes).unwrap());
        assert_eq!(fs::read(&entry.path).unwrap(), *bytes);
    }
    assert_eq!(scan, scan_opening_type_library(directory.path()).unwrap());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn corrupt_packages_are_sorted_issues_and_valid_packages_remain() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = package_bytes(OpeningKind::Door);
    fs::write(directory.path().join("b.osot"), &bytes).unwrap();
    fs::write(directory.path().join("z.osot"), b"{broken").unwrap();
    let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    unknown["surprise"] = serde_json::json!(true);
    let unknown = serde_json::to_vec(&unknown).unwrap();
    fs::write(directory.path().join("a.osot"), &unknown).unwrap();
    let scan = scan_opening_type_library(directory.path()).unwrap();
    assert_eq!(scan.package_files, 3);
    assert_eq!(scan.bytes_examined, bytes.len() + 7 + unknown.len());
    assert_eq!(scan.entries.len(), 1);
    assert_eq!(scan.entries[0].path, directory.path().join("b.osot"));
    assert_eq!(scan.issues.len(), 2);
    assert_eq!(scan.issues[0].path, directory.path().join("a.osot"));
    assert_eq!(scan.issues[1].path, directory.path().join("z.osot"));
    assert!(scan.issues.iter().all(|issue| !issue.message.is_empty()));
}

#[test]
fn unrelated_files_and_nested_packages_are_ignored() {
    let directory = tempfile::tempdir().unwrap();
    let nested = directory.path().join("nested.osot");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("door.osot"), package_bytes(OpeningKind::Door)).unwrap();
    for name in ["readme.txt", "door.osot.bak", "osot", ".osot"] {
        fs::write(directory.path().join(name), b"unrelated").unwrap();
    }
    let scan = scan_opening_type_library(directory.path()).unwrap();
    assert!(scan.entries.is_empty());
    assert!(scan.issues.is_empty());
    assert_eq!(scan.package_files, 0);
    assert_eq!(scan.bytes_examined, 0);
}

#[test]
fn package_count_cap_includes_invalid_files_and_allows_exact_boundary() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(MAX_OPENING_TYPE_LIBRARY_PACKAGES, 128);
    for index in 0..MAX_OPENING_TYPE_LIBRARY_PACKAGES {
        fs::write(directory.path().join(format!("{index:04}.osot")), b"bad").unwrap();
    }
    let scan = scan_opening_type_library(directory.path()).unwrap();
    assert_eq!(scan.package_files, MAX_OPENING_TYPE_LIBRARY_PACKAGES);
    assert_eq!(scan.issues.len(), MAX_OPENING_TYPE_LIBRARY_PACKAGES);
    fs::write(directory.path().join("extra.osot"), b"bad").unwrap();
    let error = scan_opening_type_library(directory.path()).unwrap_err();
    assert!(error.to_string().contains("128 package files"));
}

#[test]
fn entry_cap_counts_unrelated_files_and_directories() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(MAX_OPENING_TYPE_LIBRARY_ENTRIES, 4096);
    for index in 0..MAX_OPENING_TYPE_LIBRARY_ENTRIES - 1 {
        fs::write(directory.path().join(format!("{index:04}.txt")), b"").unwrap();
    }
    fs::create_dir(directory.path().join("nested")).unwrap();
    assert!(scan_opening_type_library(directory.path()).is_ok());
    fs::write(directory.path().join("extra.txt"), b"").unwrap();
    let error = scan_opening_type_library(directory.path()).unwrap_err();
    assert!(error.to_string().contains("4096 directory entries"));
}

#[test]
fn oversized_package_is_an_issue_without_reading_its_contents() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = package_bytes(OpeningKind::Door);
    fs::write(directory.path().join("valid.osot"), &bytes).unwrap();
    fs::File::create(directory.path().join("oversized.osot"))
        .unwrap()
        .set_len(MAX_OPENING_TYPE_PACKAGE_BYTES as u64 + 1)
        .unwrap();
    let scan = scan_opening_type_library(directory.path()).unwrap();
    assert_eq!(scan.package_files, 2);
    assert_eq!(scan.bytes_examined, bytes.len());
    assert_eq!(scan.entries.len(), 1);
    assert_eq!(scan.issues.len(), 1);
    assert!(scan.issues[0].message.contains("1 MiB"));
}

#[test]
fn cumulative_byte_cap_allows_exact_boundary_and_rejects_one_more_byte() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(MAX_OPENING_TYPE_LIBRARY_BYTES, 32 * 1024 * 1024);
    let mut bytes = package_bytes(OpeningKind::Window);
    bytes.resize(MAX_OPENING_TYPE_PACKAGE_BYTES, b' ');
    let count = MAX_OPENING_TYPE_LIBRARY_BYTES / MAX_OPENING_TYPE_PACKAGE_BYTES;
    for index in 0..count {
        fs::write(directory.path().join(format!("{index:04}.osot")), &bytes).unwrap();
    }
    let scan = scan_opening_type_library(directory.path()).unwrap();
    assert_eq!(scan.bytes_examined, MAX_OPENING_TYPE_LIBRARY_BYTES);
    assert_eq!(scan.entries.len(), count);
    assert!(scan.issues.is_empty());
    fs::write(directory.path().join("extra.osot"), b"x").unwrap();
    let error = scan_opening_type_library(directory.path()).unwrap_err();
    assert!(error.to_string().contains("32 MiB"));
}

#[test]
fn missing_directory_and_file_as_root_are_errors() {
    let directory = tempfile::tempdir().unwrap();
    assert!(scan_opening_type_library(&directory.path().join("missing")).is_err());
    let path = directory.path().join("file.osot");
    fs::write(&path, b"{}").unwrap();
    assert!(scan_opening_type_library(&path).is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("door.osot");
    fs::write(&target, package_bytes(OpeningKind::Door)).unwrap();
    symlink(&target, directory.path().join("linked.osot")).unwrap();
    symlink(outside.path(), directory.path().join("directory.osot")).unwrap();
    symlink(
        outside.path().join("missing"),
        directory.path().join("dangling.osot"),
    )
    .unwrap();
    let scan = scan_opening_type_library(directory.path()).unwrap();
    assert_eq!(scan.package_files, 0);
    assert_eq!(scan.bytes_examined, 0);
    assert!(scan.entries.is_empty());
    assert!(scan.issues.is_empty());
}
