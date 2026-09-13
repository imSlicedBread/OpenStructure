//! Full application flow through the public controller and plugin transport.
use os_core::{Id, Point2};
use os_document::Command;
use os_model::{Level, LevelParams, WallParams};
use os_plugin_api::Request;
use os_ui::Editor;

#[test]
fn history_retention_does_not_clear_dirty_state_or_weaken_failed_open() {
    let mut editor = Editor::new().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.osb");
    editor.save(&path).unwrap();
    let saved_bytes = std::fs::read(&path).unwrap();
    editor
        .document
        .set_history_limits(os_document::HistoryLimits {
            max_entries: 1,
            ..Default::default()
        })
        .unwrap();
    for name in ["First", "Second"] {
        editor
            .command("Rename", Command::RenameProject(name.into()))
            .unwrap();
    }
    assert_eq!(editor.document.history_stats().evicted_entries, 1);
    assert!(editor.is_dirty());
    editor.undo().unwrap();
    assert!(editor.is_dirty());
    assert!(!editor.document.can_undo());
    let before = editor.document.model().clone();
    let stats = editor.document.history_stats();
    let revision = editor.document.revision();
    let bad_path = directory.path().join("missing.osb");
    assert!(editor.open(&bad_path).is_err());
    assert_eq!(editor.document.model(), &before);
    assert_eq!(editor.document.history_stats(), stats);
    assert_eq!(editor.document.revision(), revision);
    assert!(editor.is_dirty());
    assert_eq!(std::fs::read(&path).unwrap(), saved_bytes);
    editor.redo().unwrap();
    editor.save(&path).unwrap();
    assert!(!editor.is_dirty());
    editor.undo().unwrap();
    assert!(editor.is_dirty());
    editor.redo().unwrap();
    assert!(!editor.is_dirty());
    editor.open(&path).unwrap();
    assert_eq!(editor.document.model().project.parameters.name, "Second");
    assert_eq!(editor.document.history_stats().estimated_bytes, 0);
    assert_eq!(editor.document.history_stats().evicted_entries, 0);
    assert!(!editor.is_dirty());
}

#[test]
fn unavailable_extension_data_survives_desktop_save_and_blocks_ifc() {
    let mut editor = Editor::new().unwrap();
    let mut model = editor.document.model().clone();
    let entity: os_model::ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    model.plugin_requirements.insert(
        entity.owner.clone(),
        os_model::PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(entity.id, entity.clone());
    editor.document = os_document::Document::from_model(model).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unavailable.osb");
    editor.save(&path).unwrap();
    editor.open(&path).unwrap();
    editor
        .command("Rename", Command::RenameProject("Still preserved".into()))
        .unwrap();
    editor.save(&path).unwrap();
    editor.open(&path).unwrap();
    assert_eq!(editor.document.model().extensions[&entity.id], entity);
    assert!(editor.scene.is_empty());
    assert!(!editor.is_dirty());
    let error = os_ifc::WallIfc
        .export_report(editor.document.model())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("no mapping for 1 plugin elements")
    );
}

#[test]
fn editor_open_edit_save_retains_auxiliary_contents() {
    use os_storage::{StorageBackend, ZipJsonStorage};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("opaque.osb");
    let files =
        std::collections::BTreeMap::from([("extensions/example/data.bin".into(), vec![0, 255, 1])]);
    let document = os_document::Document::from_model_and_files(
        os_model::Model::new("Original"),
        files.clone(),
    )
    .unwrap();
    ZipJsonStorage.save(&document, &path).unwrap();
    let mut editor = Editor::new().unwrap();
    editor.open(&path).unwrap();
    assert!(!editor.is_dirty());
    editor
        .command("Rename", Command::RenameProject("Changed".into()))
        .unwrap();
    editor.undo().unwrap();
    editor.redo().unwrap();
    editor.save(&path).unwrap();
    assert!(!editor.is_dirty());
    editor.open(&path).unwrap();
    for (name, bytes) in files {
        assert_eq!(editor.document.auxiliary_files()[&name], bytes);
    }
    let original_model = editor.document.model().clone();
    let original_files = editor.document.auxiliary_files().clone();
    let bad = directory.path().join("corrupt.osb");
    std::fs::write(&bad, b"invalid archive").unwrap();
    assert!(editor.open(&bad).is_err());
    assert_eq!(editor.document.model(), &original_model);
    assert_eq!(editor.document.auxiliary_files(), &original_files);
    assert!(!editor.is_dirty());
}

#[test]
fn complete_edit_regenerate_history_save_and_reopen_flow() {
    let mut editor = Editor::new().unwrap();
    assert_eq!(editor.host.manifests().count(), 1);
    let ground = *editor.document.model().levels.keys().next().unwrap();
    let wall = WallParams {
        name: "Test wall".into(),
        start: Point2::new(1.0, 2.0),
        end: Point2::new(4.0, 6.0),
        thickness: 0.2,
        height: 3.0,
        level: ground,
        material: None,
    };
    editor
        .wall_command("Create", Request::CreateWall(wall))
        .unwrap();
    let id = *editor.document.model().walls.keys().next().unwrap();
    let original = editor.document.model().clone();
    let original_mesh = editor.scene[&id].clone();
    assert!((original_mesh.signed_volume() - 3.0).abs() < 1e-9);
    let building = *editor.document.model().buildings.keys().next().unwrap();
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 4.0,
            building,
        },
    );
    let upper_id = upper.id();
    editor
        .command("Add level", Command::AddLevel(upper))
        .unwrap();
    let mut parameters = editor.document.model().walls[&id]
        .parameters
        .with_length(8.0)
        .unwrap();
    parameters.thickness = 0.3;
    parameters.height = 4.0;
    parameters.level = upper_id;
    editor
        .wall_command("Edit", Request::EditWall { id, parameters })
        .unwrap();
    assert!((editor.scene[&id].signed_volume() - 9.6).abs() < 1e-8);
    assert_eq!(editor.scene[&id].vertices[0].z, 4.0);
    let edited = editor.document.model().clone();
    let edited_mesh = editor.scene[&id].clone();
    editor.undo().unwrap();
    editor.undo().unwrap();
    assert_eq!(editor.document.model(), &original);
    assert_eq!(editor.scene[&id], original_mesh);
    editor.redo().unwrap();
    editor.redo().unwrap();
    assert_eq!(editor.document.model(), &edited);
    assert_eq!(editor.scene[&id], edited_mesh);
    let mut level_params = edited.levels[&upper_id].parameters.clone();
    level_params.elevation = 7.0;
    editor
        .command(
            "Elevate",
            Command::UpdateLevel {
                id: upper_id,
                parameters: level_params,
            },
        )
        .unwrap();
    assert_eq!(editor.scene[&id].vertices[0].z, 7.0);
    editor.undo().unwrap();
    assert_eq!(editor.scene[&id], edited_mesh);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.osb");
    editor.save(&path).unwrap();
    assert!(!editor.is_dirty());
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), &edited);
    assert_eq!(reopened.scene[&id], edited_mesh);
    assert!(!reopened.document.can_undo());
    assert!(!reopened.is_dirty());
    reopened.command("Delete", Command::RemoveWall(id)).unwrap();
    assert!(!reopened.scene.contains_key(&id));
    reopened.undo().unwrap();
    assert_eq!(reopened.scene[&id], edited_mesh);
    assert!(!reopened.is_dirty());
}

#[test]
fn failed_edits_and_opens_preserve_the_working_document() {
    let mut editor = Editor::new().unwrap();
    let original = editor.document.model().clone();
    let invalid = WallParams {
        name: "Invalid".into(),
        start: Point2::new(0.0, 0.0),
        end: Point2::new(5.0, 0.0),
        thickness: 0.2,
        height: 3.0,
        level: Id::new(),
        material: None,
    };
    assert!(
        editor
            .wall_command("Invalid", Request::CreateWall(invalid))
            .is_err()
    );
    assert_eq!(editor.document.model(), &original);
    assert!(editor.scene.is_empty());
    let temp = tempfile::NamedTempFile::new().unwrap();
    assert!(editor.open(temp.path()).is_err());
    assert_eq!(editor.document.model(), &original);
    assert!(!editor.document.can_undo());
}

#[test]
fn failed_regeneration_cannot_overwrite_a_reopenable_project() {
    let mut editor = Editor::new().unwrap();
    let level = *editor.document.model().levels.keys().next().unwrap();
    editor
        .wall_command(
            "Create",
            Request::CreateWall(WallParams {
                name: "Valid wall".into(),
                start: Point2::new(0.0, 0.0),
                end: Point2::new(5.0, 0.0),
                thickness: 0.2,
                height: 3.0,
                level,
                material: None,
            }),
        )
        .unwrap();
    let id = *editor.document.model().walls.keys().next().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recoverable.osb");
    editor.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let mut parameters = editor.document.model().walls[&id].parameters.clone();
    // Finite semantic parameters can still exceed a kernel's numerical range.
    parameters.height = 1e200;
    parameters.end.x = 1e200;
    assert!(
        editor
            .wall_command("Unrenderable", Request::EditWall { id, parameters })
            .is_err()
    );
    assert!(!editor.scene.contains_key(&id));
    assert!(editor.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(
        reopened.document.model().walls[&id].parameters.length(),
        5.0
    );
    editor.undo().unwrap();
    editor.save(&path).unwrap();
    assert_eq!(editor.scene[&id], reopened.scene[&id]);
}
