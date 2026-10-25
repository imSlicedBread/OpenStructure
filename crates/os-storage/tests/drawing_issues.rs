use os_core::Id;
use os_model::*;
use os_storage::{drawing_issue::IssueBundle, migrate};
use std::io::{Cursor, Write};
use zip::{ZipWriter, write::SimpleFileOptions};

fn bundle() -> IssueBundle {
    let pdf = b"%PDF-1.4\nfixture\n%%EOF\n".to_vec();
    IssueBundle::new(
        IssueManifest {
            format_version: ISSUE_BUNDLE_VERSION,
            state: IssueState::Issued,
            issue: Id::new(),
            project: Id::new(),
            set: Id::new(),
            sequence: 1,
            revision: "A".into(),
            description: "Test issue".into(),
            date: "2026-10-11".into(),
            recipients: vec!["Team".into()],
            supersedes: None,
            schema_version: SCHEMA_VERSION,
            native_api_version: 52,
            application_version: "0.1.0".into(),
            source_session: Id::new(),
            source_revision: 1,
            source_sha256: sha256(b"model"),
            providers: vec![],
            provider_versions: vec![("native".into(), "0.1.0".into(), 52)],
            sheets: vec![IssueSheet {
                id: Id::new(),
                number: "A101".into(),
                name: "First".into(),
                views: vec![(Id::new(), 0)],
            }],
            warnings: vec![IssueWarning {
                message: "Font warning".into(),
                acknowledgment: "Accepted".into(),
            }],
            pdf_sha256: sha256(&pdf),
        },
        pdf,
    )
    .unwrap()
}

#[test]
fn drawing_issue_bundle_hashes_tampering_truncation_and_unknown_entries() {
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let b = bundle();
    let bytes = b.bytes().unwrap();
    let reopened = IssueBundle::parse(&bytes).unwrap();
    assert_eq!(reopened.pdf(), b.pdf());
    assert_eq!(reopened.manifest(), b.manifest());
    assert_eq!(reopened.bytes().unwrap(), bytes);
    let mut tampered = bytes.clone();
    let at = tampered.windows(7).position(|w| w == b"fixture").unwrap();
    tampered[at] = b'X';
    assert!(IssueBundle::parse(&tampered).is_err());
    let mut tampered = bytes.clone();
    let at = tampered.windows(4).position(|w| w == b"A101").unwrap();
    tampered[at] = b'B';
    assert!(IssueBundle::parse(&tampered).is_err());
    for length in [0, 32, bytes.len() / 2, bytes.len() - 1] {
        assert!(IssueBundle::parse(&bytes[..length]).is_err());
    }
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for name in ["manifest.json", "drawings.pdf", "unknown.txt"] {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(b"bad").unwrap();
    }
    assert!(IssueBundle::parse(&zip.finish().unwrap().into_inner()).is_err());
}

#[test]
fn drawing_issue_bundle_rejects_duplicate_directory_entries_and_symlinks() {
    let b = bundle();
    let mut bytes = b.bytes().unwrap();
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    let end = bytes.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
    let field = |i| u16::from_le_bytes([bytes[central + i], bytes[central + i + 1]]) as usize;
    let entry = bytes[central..central + 46 + field(28) + field(30) + field(32)].to_vec();
    let size = u32::from_le_bytes(bytes[end + 12..end + 16].try_into().unwrap());
    bytes.splice(end..end, entry.iter().copied());
    let new_end = end + entry.len();
    bytes[new_end + 8..new_end + 10].copy_from_slice(&4u16.to_le_bytes());
    bytes[new_end + 10..new_end + 12].copy_from_slice(&4u16.to_le_bytes());
    bytes[new_end + 12..new_end + 16].copy_from_slice(&(size + entry.len() as u32).to_le_bytes());
    assert!(
        IssueBundle::parse(&bytes)
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("manifest.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&b.manifest().bytes().unwrap()).unwrap();
    zip.add_symlink("drawings.pdf", "outside.pdf", SimpleFileOptions::default())
        .unwrap();
    zip.start_file("integrity.sha256", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"invalid").unwrap();
    assert!(
        IssueBundle::parse(&zip.finish().unwrap().into_inner())
            .unwrap_err()
            .to_string()
            .contains("regular")
    );
}

#[test]
fn drawing_issue_schema70_migration_is_explicit_atomic_and_preserves_native_data() {
    let m = Model::new("Schema 70 project");
    let mut old = serde_json::to_value(&m).unwrap();
    old.as_object_mut().unwrap().remove("plan_callouts");
    old.as_object_mut().unwrap().remove("section_callouts");
    old.as_object_mut().unwrap().remove("text_notes");
    old.as_object_mut().unwrap().remove("plan_hidden_openings");
    old.as_object_mut().unwrap().remove("group_definitions");
    old.as_object_mut().unwrap().remove("group_instances");
    old.as_object_mut().unwrap().remove("plan_graphics_rules");
    old["schema_version"] = 70.into();
    for (name, data) in old.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = 70.into();
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                entity["header"]["schema_version"] = 70.into();
            }
        }
    }
    let mut migrated = old.clone();
    migrate(&mut migrated, 70).unwrap();
    let restored: Model = serde_json::from_value(migrated).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, m);
    for name in ["sheet_sets", "drawing_issues", "plan_callouts"] {
        let mut ambiguous = old.clone();
        ambiguous[name] = serde_json::json!({});
        let before = ambiguous.clone();
        assert!(migrate(&mut ambiguous, 70).is_err());
        assert_eq!(ambiguous, before);
    }
}
