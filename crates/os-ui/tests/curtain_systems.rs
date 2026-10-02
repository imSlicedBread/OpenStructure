#[path = "../../os-model/tests/support/curtains.rs"]
mod support;
use os_core::Id;
use os_document::Command;
use os_geometry::curtain_systems::curtain_mesh;
use os_model::*;
use os_ui::Editor;

fn editor() -> (Editor, Id) {
    let (model, mut assembly) = support::fixture();
    let mut editor = Editor::new().unwrap();
    assembly.parameters.level = *editor.document.model().levels.keys().next().unwrap();
    let id = assembly.id();
    let mut commands: Vec<_> = model
        .curtain_panel_types
        .into_values()
        .map(Command::AddCurtainPanelType)
        .collect();
    commands.extend(
        model
            .curtain_mullion_types
            .into_values()
            .map(Command::AddCurtainMullionType),
    );
    commands.push(Command::AddCurtainSystem(assembly));
    editor.document.execute("Add curtain", commands).unwrap();
    assert!(editor.scene.is_empty());
    editor.regenerate().unwrap();
    (editor, id)
}

fn assert_current(editor: &Editor, id: Id) {
    let model = editor.document.model();
    assert_eq!(
        editor.scene[&id],
        curtain_mesh(&model.curtain_systems[&id].parameters, model).unwrap()
    );
    assert_eq!(editor.scene.len(), 1);
    assert!(
        model.curtain_systems[&id]
            .parameters
            .child_ids()
            .all(|child| !editor.scene.contains_key(&child))
    );
}

#[test]
fn curtain_scene_add_preview_grid_edit_remove_undo_redo() {
    let (mut editor, id) = editor();
    assert_current(&editor, id);
    let original = editor.scene.clone();
    let model_before = editor.document.model().clone();
    let history = editor.document.history_stats();
    let mut p = model_before.curtain_systems[&id].parameters.clone();
    p.vertical.insert(
        1,
        CurtainGrid {
            id: Id::new(),
            position: 1.,
        },
    );
    p.reconcile().unwrap();
    let command = Command::UpdateCurtainSystem { id, parameters: p };
    let candidate = editor
        .document
        .preview_commands(vec![command.clone()])
        .unwrap();
    let preview = curtain_mesh(&candidate.curtain_systems[&id].parameters, &candidate).unwrap();
    assert_eq!(editor.scene, original);
    assert_eq!(editor.document.model(), &model_before);
    assert_eq!(editor.document.history_stats(), history);
    editor.command("Add grid", command).unwrap();
    assert_eq!(editor.scene[&id], preview);
    assert_current(&editor, id);
    editor.undo().unwrap();
    assert_eq!(editor.scene, original);
    editor.redo().unwrap();
    assert_eq!(editor.scene[&id], preview);
    editor
        .command("Remove curtain", Command::RemoveCurtainSystem(id))
        .unwrap();
    assert!(editor.scene.is_empty());
    editor.undo().unwrap();
    assert_current(&editor, id);
    editor.redo().unwrap();
    assert!(editor.scene.is_empty());
}

#[test]
fn curtain_scene_level_type_material_updates_and_staged_open() {
    let (mut editor, id) = editor();
    let p = editor.document.model().curtain_systems[&id]
        .parameters
        .clone();
    let mut level = editor.document.model().levels[&p.level].parameters.clone();
    level.elevation = 5.;
    editor
        .command(
            "Raise level",
            Command::UpdateLevel {
                id: p.level,
                parameters: level,
            },
        )
        .unwrap();
    assert_current(&editor, id);
    assert!(editor.scene[&id].vertices.iter().all(|v| v.z >= 5.3));
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Glass".into(),
            density_kg_m3: 2500.,
            color: [50, 80, 110],
        },
    );
    let material_id = material.id();
    editor
        .command("Material", Command::AddMaterial(material))
        .unwrap();
    let mut ty = editor.document.model().curtain_panel_types[&p.panel_type]
        .parameters
        .clone();
    ty.material = Some(material_id);
    ty.thickness = 0.04;
    editor
        .command(
            "Panel type",
            Command::UpdateCurtainPanelType {
                id: p.panel_type,
                parameters: ty,
            },
        )
        .unwrap();
    assert_current(&editor, id);
    assert!(
        editor.scene[&id]
            .surfaces
            .iter()
            .any(|s| s.material == Some(material_id))
    );
    let mut mullion = editor.document.model().curtain_mullion_types[&p.mullion_type]
        .parameters
        .clone();
    mullion.width = 0.16;
    editor
        .command(
            "Mullion type",
            Command::UpdateCurtainMullionType {
                id: p.mullion_type,
                parameters: mullion,
            },
        )
        .unwrap();
    assert_current(&editor, id);
    let mut material = editor.document.model().materials[&material_id]
        .parameters
        .clone();
    material.color = [150, 170, 190];
    editor
        .command(
            "Appearance",
            Command::UpdateMaterial {
                id: material_id,
                parameters: material,
            },
        )
        .unwrap();
    assert_current(&editor, id);
    assert!(
        editor.scene[&id]
            .surfaces
            .iter()
            .any(|s| s.color_in(editor.document.model()) == Some([150, 170, 190]))
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("curtain.osb");
    editor.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), editor.document.model());
    assert_eq!(reopened.scene, editor.scene);
    assert!(!reopened.is_dirty());
    let scene = reopened.scene.clone();
    let mut invalid = p.clone();
    invalid.height = 0.;
    assert!(
        reopened
            .command(
                "Invalid",
                Command::UpdateCurtainSystem {
                    id,
                    parameters: invalid
                }
            )
            .is_err()
    );
    assert_eq!(reopened.scene, scene);
    assert!(!reopened.is_dirty());
    assert!(reopened.open(&dir.path().join("missing.osb")).is_err());
    assert_eq!(reopened.scene, scene);
    assert_eq!(reopened.document.model(), editor.document.model());
    assert!(!reopened.is_dirty());
}

#[test]
fn curtain_out_of_envelope_edits_fail_before_model_or_scene_mutation() {
    let (mut editor, id) = editor();
    let original = editor.document.model().clone();
    let scene = editor.scene.clone();
    let dirty = editor.is_dirty();
    let history = editor.document.history_stats();
    let mut p = original.curtain_systems[&id].parameters.clone();
    p.start.y = 1e6;
    p.end.y = 1e6;
    let command = Command::UpdateCurtainSystem { id, parameters: p };
    assert!(
        editor
            .document
            .preview_commands(vec![command.clone()])
            .is_err()
    );
    assert!(editor.command("Out of envelope", command).is_err());
    assert_eq!(editor.document.model(), &original);
    assert_eq!(editor.document.history_stats(), history);
    assert_eq!(editor.scene, scene);
    assert_eq!(editor.is_dirty(), dirty);
    editor.regenerate().unwrap();
    assert_current(&editor, id);
}
