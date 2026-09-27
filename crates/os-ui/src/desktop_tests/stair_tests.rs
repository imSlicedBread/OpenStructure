use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{Level, LevelParams, Stair, StairParams};

#[test]
fn stair_mesh_regenerates_for_commit_undo_redo_and_native_reopen() {
    let mut harness = Harness::new();
    let lower = *harness
        .app
        .editor
        .document
        .model()
        .levels
        .keys()
        .next()
        .unwrap();
    let building = harness.app.editor.document.model().levels[&lower]
        .parameters
        .building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.2,
            building,
        },
    );
    let upper_id = upper.id();
    harness
        .app
        .editor
        .command("Add upper level", Command::AddLevel(upper))
        .unwrap();
    let stair = Stair::new(
        "core.stair",
        StairParams {
            name: "Main stair".into(),
            lower_level: lower,
            upper_level: upper_id,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(3.6, 0.0),
            width: 0.9,
            riser_count: 18,
            structural_thickness: 0.16,
            material: None,
        },
    );
    let id = stair.id();
    harness
        .app
        .editor
        .command("Place stair", Command::AddStair(stair.clone()))
        .unwrap();
    let original_volume = harness.app.editor.scene[&id].signed_volume();
    assert!(original_volume > 0.0);

    let mut edited = stair.parameters.clone();
    edited.width = 1.2;
    harness
        .app
        .editor
        .command(
            "Edit stair",
            Command::UpdateStair {
                id,
                parameters: edited,
            },
        )
        .unwrap();
    let edited_volume = harness.app.editor.scene[&id].signed_volume();
    assert!(edited_volume > original_volume);
    harness.app.editor.undo().unwrap();
    assert!((harness.app.editor.scene[&id].signed_volume() - original_volume).abs() < 1e-10);
    harness.app.editor.redo().unwrap();
    assert!((harness.app.editor.scene[&id].signed_volume() - edited_volume).abs() < 1e-10);

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stair.osb");
    harness.app.editor.save(&path).unwrap();
    harness.app.editor.open(&path).unwrap();
    assert!(harness.app.editor.document.model().stairs.contains_key(&id));
    assert!((harness.app.editor.scene[&id].signed_volume() - edited_volume).abs() < 1e-10);
}
