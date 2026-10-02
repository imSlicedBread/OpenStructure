use super::*;
use os_model::{
    Material, MaterialParams, Model, Opening, OpeningDefinition, OpeningFamily, OpeningKind,
    OpeningParams, OpeningType, OpeningTypeParams, WindowPanePosition,
};

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

#[test]
fn paired_doors_package_replace_explicit_reset_atomic_history_both_dpis() {
    for (size, scale) in PROFILES {
        let directory = tempfile::tempdir().unwrap();
        let path = source_package(
            directory.path(),
            "Single Door",
            OpeningKind::Door,
            0.9,
            false,
        );
        let mut h = Harness::at_size(size, scale);
        let mut target = sample_type("Paired Door", OpeningKind::Door, 0.9);
        target.parameters.family.door_leaves = os_model::DoorLeaves::Paired {
            active_fraction: 0.6,
        };
        let tid = target.id();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(h.app.active_level));
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Pair".into(),
                host: wall.id(),
                offset: 1.,
                definition: OpeningDefinition::Typed { type_id: tid },
                open_state: os_model::OpeningState::DoorPairAngles {
                    active_degrees: 30.,
                    inactive_degrees: 60.,
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let id = opening.id();
        h.app
            .editor
            .document
            .execute(
                "Pair fixture",
                vec![
                    Command::AddWall(wall),
                    Command::AddOpeningType(target),
                    Command::AddOpening(opening),
                ],
            )
            .unwrap();
        h.app.editor.regenerate().unwrap();
        h.app.select(Some(tid));
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        read_button(&mut h, &path);
        h.click("Update selected compatible type");
        h.click("Import type");
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Reset paired poses to Default");
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Import type");
        let after = h.app.editor.document.model().clone();
        assert_eq!(
            after.opening_types[&tid].parameters.family.door_leaves,
            os_model::DoorLeaves::Single
        );
        assert_eq!(
            after.openings[&id].parameters.open_state,
            os_model::OpeningState::Default
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &after);
    }
}

#[test]
fn shared_lengths_package_copy_conflicts_bindings_preview_history_both_dpi() {
    for (size, scale) in PROFILES {
        let mut source = Model::new("Shared parameter source");
        let ty = sample_type("Shared window", OpeningKind::Window, 1.);
        let type_id = ty.id();
        let p = os_model::LengthParameter::new(
            "core.length_parameter",
            os_model::LengthParameterParams {
                name: "Shared size".into(),
                unit: os_model::LengthUnit::Metres,
                value: 1.2,
            },
        );
        let pid = p.id();
        source.length_parameters.insert(pid, p.clone());
        source.opening_types.insert(type_id, ty);
        source.opening_type_length_bindings.insert(
            type_id,
            os_model::OpeningTypeLengthBindings {
                width: Some(pid),
                height: Some(pid),
                sill: Some(pid),
            },
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.osot");
        os_storage::write_opening_type_package(&source, type_id, &path).unwrap();
        let mut h = Harness::at_size(size, scale);
        let mut conflicting = p.clone();
        conflicting.parameters.value = 2.;
        h.app
            .editor
            .document
            .execute("Collision", vec![Command::AddLengthParameter(conflicting)])
            .unwrap();
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        read_button(&mut h, &path);
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Import type");
        let id = h.app.selected.unwrap();
        let model = h.app.editor.document.model();
        let bindings = model.opening_type_length_bindings[&id];
        let imported = bindings.width.unwrap();
        assert_ne!(imported, pid);
        assert_eq!(bindings.height, Some(imported));
        assert_eq!(bindings.sill, Some(imported));
        assert_eq!(
            model.length_parameters[&imported].parameters.name,
            "Shared size (Imported)"
        );
        assert_eq!(model.length_parameters[&pid].parameters.value, 2.);
        assert_eq!(
            model.resolve_opening_type(id).unwrap().parameters.width,
            1.2
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let after = model.clone();
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &after);
    }
}

#[test]
fn two_bay_package_dependencies_collisions_history_and_reopen() {
    for (size, scale) in PROFILES {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let dir = tempfile::tempdir().unwrap();
            let mut source = Model::new("Source");
            let mut ty = sample_type("Two bay", kind, 1.2);
            let mut ids = Vec::new();
            for name in ["Panel", "Frame", "Lite"] {
                let material = Material::new(
                    "core.material",
                    MaterialParams {
                        name: name.into(),
                        density_kg_m3: 2500.,
                        color: [40, 90, 150],
                    },
                );
                ids.push(material.id());
                source.materials.insert(material.id(), material);
            }
            ty.parameters.family.panel_material = Some(ids[0]);
            ty.parameters.family.frame_material = Some(ids[1]);
            ty.parameters.family.side_lite = Some(os_model::SideLite {
                side: os_model::LiteSide::Start,
                width_fraction: 0.25,
                mullion_width: 0.05,
                material: Some(ids[2]),
            });
            let source_id = ty.id();
            source.opening_types.insert(source_id, ty);
            let path = dir.path().join("two-bay.osot");
            os_storage::write_opening_type_package(&source, source_id, &path).unwrap();
            let mut h = Harness::at_size(size, scale);
            let mut collision = source.materials[&ids[2]].clone();
            collision.parameters.color = [255, 0, 0];
            h.app
                .editor
                .document
                .execute(
                    "Conflicts",
                    vec![
                        Command::AddMaterial(collision),
                        Command::AddOpeningType(sample_type("Two bay", kind, 0.8)),
                    ],
                )
                .unwrap();
            let before = h.app.editor.document.model().clone();
            read_button(&mut h, &path);
            assert_eq!(h.app.editor.document.model(), &before);
            h.click("Import type");
            let type_id = h.app.selected.unwrap();
            let after = h.app.editor.document.model().clone();
            let imported = &after.opening_types[&type_id];
            assert_ne!(type_id, source_id);
            let lite = imported.parameters.family.side_lite.as_ref().unwrap();
            assert_ne!(lite.material, Some(ids[2]));
            assert_eq!(
                after.materials[&lite.material.unwrap()].parameters.color,
                [40, 90, 150]
            );
            assert_eq!(after.materials[&ids[2]].parameters.color, [255, 0, 0]);
            assert_eq!(
                os_storage::parse_opening_type_package(
                    &os_storage::export_opening_type_package(&after, type_id).unwrap()
                )
                .unwrap()
                .materials
                .len(),
                3
            );
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &after);
            let native = dir.path().join("two-bay.osb");
            h.app.editor.save(&native).unwrap();
            let mut reopened = Editor::new().unwrap();
            reopened.open(&native).unwrap();
            assert_eq!(reopened.document.model(), &after);
        }
    }
}

fn sample_type(name: &str, kind: OpeningKind, width: f64) -> OpeningType {
    OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            family: OpeningFamily::default(),
            name: name.into(),
            kind,
            width,
            height: if kind == OpeningKind::Door { 2.1 } else { 1.2 },
            sill: if kind == OpeningKind::Door { 0.0 } else { 0.8 },
            pane_position: WindowPanePosition::Center,
        },
    )
}

fn source_package(
    dir: &std::path::Path,
    name: &str,
    kind: OpeningKind,
    width: f64,
    with_materials: bool,
) -> std::path::PathBuf {
    let mut model = Model::new("Package source");
    let mut ty = sample_type(name, kind, width);
    let type_id = ty.id();
    if with_materials {
        let panel = Material::new(
            "core.material",
            MaterialParams {
                name: "Pine panel".into(),
                density_kg_m3: 540.0,
                color: [160, 120, 80],
            },
        );
        let frame = Material::new(
            "core.material",
            MaterialParams {
                name: "Black metal".into(),
                density_kg_m3: 7_850.0,
                color: [25, 25, 25],
            },
        );
        ty.parameters.family.panel_material = Some(panel.id());
        ty.parameters.family.frame_material = Some(frame.id());
        model.materials.insert(panel.id(), panel);
        model.materials.insert(frame.id(), frame);
    }
    model.opening_types.insert(type_id, ty);
    let path = dir.join(format!("{}.osot", name.replace(' ', "-")));
    os_storage::write_opening_type_package(&model, type_id, &path).unwrap();
    path
}

fn read_button(h: &mut Harness, path: &std::path::Path) {
    h.app.begin_import_opening_type_package();
    h.app.opening_type_package_dialog.as_mut().unwrap().path = path.display().to_string();
    h.frame(vec![]);
    h.frame(vec![]);
    h.click("Read and preview");
    h.frame(vec![]);
}

#[test]
fn package_import_is_previewed_atomic_and_round_trips_door_and_window_types_at_both_dpis() {
    for (size, scale) in PROFILES {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let directory = tempfile::tempdir().unwrap();
            let label = if kind == OpeningKind::Door {
                "Packaged Door"
            } else {
                "Packaged Window"
            };
            let path = source_package(directory.path(), label, kind, 1.1, true);
            let mut h = Harness::at_size(size, scale);
            let conflicting = Material::new(
                "core.material",
                MaterialParams {
                    name: "Pine panel".into(),
                    density_kg_m3: 700.0,
                    color: [100, 100, 100],
                },
            );
            h.app
                .editor
                .document
                .execute(
                    "Create type and material conflicts",
                    vec![
                        Command::AddMaterial(conflicting),
                        Command::AddOpeningType(sample_type(label, kind, 0.8)),
                    ],
                )
                .unwrap();
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();

            read_button(&mut h, &path);
            assert!(
                h.visible_text_rect("Pine panel → Pine panel (Imported) (name conflict)")
                    .is_some()
            );
            assert!(
                h.visible_text_rect(
                    "The package name already exists here; a unique imported name was suggested."
                )
                .is_some()
            );
            assert!(h.visible_text_rect("Every affected host and opening component passed preflight. No project changes occur until Import type.").is_some());
            assert_eq!(
                h.app.editor.document.model(),
                &before,
                "preview must not mutate"
            );
            assert_eq!(h.app.editor.document.history_stats(), history);

            h.click("Import type");
            let imported_id = h.app.selected.expect("imported type should be selected");
            let imported = &h.app.editor.document.model().opening_types[&imported_id];
            assert_eq!(imported.parameters.kind, kind);
            assert_eq!(imported.parameters.width, 1.1);
            assert_eq!(imported.parameters.name, format!("{label} (Imported)"));
            assert_ne!(
                imported_id,
                os_storage::read_opening_type_package(&path)
                    .unwrap()
                    .source_type_id
            );
            let panel_id = imported.parameters.family.panel_material.unwrap();
            let frame_id = imported.parameters.family.frame_material.unwrap();
            assert_eq!(
                h.app.editor.document.model().materials[&panel_id]
                    .parameters
                    .name,
                "Pine panel (Imported)"
            );
            assert_eq!(
                h.app.editor.document.model().materials[&panel_id]
                    .parameters
                    .density_kg_m3,
                540.0
            );
            assert_eq!(
                h.app.editor.document.model().materials[&frame_id]
                    .parameters
                    .name,
                "Black metal"
            );
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            let after = h.app.editor.document.model().clone();

            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &after);

            let export = directory.path().join("exported.osot");
            std::fs::write(&export, b"preserve this until consent").unwrap();
            h.app.begin_export_opening_type_package(imported_id);
            h.app.opening_type_package_dialog.as_mut().unwrap().path = export.display().to_string();
            h.frame(vec![]);
            assert!(
                h.visible_text_rect(
                    "This file already exists. Confirm replacement; the write is atomic."
                )
                .is_some()
            );
            h.click("Export package");
            assert_eq!(
                std::fs::read(&export).unwrap(),
                b"preserve this until consent"
            );
            h.click("Replace existing package");
            h.click("Export package");
            let exported = os_storage::read_opening_type_package(&export).unwrap();
            assert_eq!(
                exported.parameters,
                after.opening_types[&imported_id].parameters
            );
            assert_eq!(exported.materials.len(), 2);
            assert_eq!(h.app.editor.document.model(), &after);
            assert_eq!(h.ctx.pixels_per_point(), scale);
        }
    }
}

#[test]
fn package_update_preserves_type_identity_instances_and_pinned_dimensions_in_one_undo() {
    let (size, scale) = PROFILES[1];
    let directory = tempfile::tempdir().unwrap();
    let path = source_package(
        directory.path(),
        "Source Door",
        OpeningKind::Door,
        1.4,
        false,
    );
    let package = os_storage::read_opening_type_package(&path).unwrap();
    let mut source = Model::new("Two bay update source");
    let mut ty = OpeningType::new("core.opening_type", package.parameters);
    ty.header.id = package.source_type_id;
    ty.parameters.family.side_lite = Some(os_model::SideLite {
        side: os_model::LiteSide::End,
        width_fraction: 0.25,
        mullion_width: 0.05,
        material: None,
    });
    source.opening_types.insert(ty.id(), ty);
    os_storage::write_opening_type_package(&source, package.source_type_id, &path).unwrap();
    let mut h = Harness::at_size(size, scale);

    let target = sample_type("Existing Door", OpeningKind::Door, 0.9);
    let target_id = target.id();
    let level = h.app.active_level;
    let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(level));
    let host = wall.id();
    let opening_params = OpeningParams {
        open_state: Default::default(),
        name: "Pinned door".into(),
        host,
        offset: 2.0,
        definition: OpeningDefinition::Typed { type_id: target_id },
        width_override: Some(1.0),
        height_override: Some(1.95),
        sill_override: None,
        pane_position_override: None,
        lite_side_override: None,
        hinge: Default::default(),
        swing: Default::default(),
    };
    let opening = Opening::new("core.opening", opening_params);
    let opening_id = opening.id();
    let inherited = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "Inherited door".into(),
            host,
            offset: 3.5,
            definition: OpeningDefinition::Typed { type_id: target_id },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let inherited_id = inherited.id();
    h.app
        .editor
        .document
        .execute(
            "Set up destination",
            vec![
                Command::AddOpeningType(target),
                Command::AddWall(wall),
                Command::AddOpening(opening),
                Command::AddOpening(inherited),
            ],
        )
        .unwrap();
    h.app.editor.regenerate().unwrap();
    let inherited_mesh_before = h.app.editor.scene[&inherited_id].clone();
    h.app.select(Some(target_id));
    let before = h.app.editor.document.model().clone();
    let history = h.app.editor.document.history_stats();

    read_button(&mut h, &path);
    assert!(
        h.visible_text_rect("Create · 0 affected instance(s) · 0 pinned dimension(s) preserved")
            .is_some()
    );
    h.click("Update selected compatible type");
    assert!(
        h.visible_text_rect("Update · 2 affected instance(s) · 2 pinned dimension(s) preserved")
            .is_some()
    );
    assert_eq!(
        h.app.editor.document.model(),
        &before,
        "update preview must be read-only"
    );
    h.click("Import type");

    let updated = &h.app.editor.document.model().opening_types[&target_id];
    assert_eq!(updated.parameters.name, "Existing Door");
    assert_eq!(updated.parameters.width, 1.4);
    assert!(updated.parameters.family.side_lite.is_some());
    assert_eq!(h.app.selected, Some(target_id));
    assert_eq!(
        h.app.editor.document.model().openings[&opening_id].parameters,
        before.openings[&opening_id].parameters
    );
    assert_eq!(
        h.app.editor.document.model().openings[&inherited_id].parameters,
        before.openings[&inherited_id].parameters
    );
    assert_eq!(
        h.app
            .editor
            .document
            .model()
            .resolve_opening(&h.app.editor.document.model().openings[&inherited_id].parameters)
            .unwrap()
            .width,
        1.4
    );
    assert_ne!(h.app.editor.scene[&inherited_id], inherited_mesh_before);
    assert_eq!(
        h.app.editor.document.history_stats().undo_entries,
        history.undo_entries + 1
    );
    let after = h.app.editor.document.model().clone();
    h.app.history(false);
    assert_eq!(h.app.editor.document.model(), &before);
    h.app.history(true);
    assert_eq!(h.app.editor.document.model(), &after);
    assert_eq!(h.ctx.pixels_per_point(), scale);
}

#[test]
fn package_update_with_host_fit_failure_is_blocked_and_escape_cancels() {
    let directory = tempfile::tempdir().unwrap();
    let path = source_package(
        directory.path(),
        "Too Wide Door",
        OpeningKind::Door,
        1.4,
        false,
    );
    let mut h = Harness::new();
    let target = sample_type("Narrow Door", OpeningKind::Door, 0.9);
    let target_id = target.id();
    let level = h.app.active_level;
    let wall = os_model::Wall::new(
        os_walls::WALL_TYPE,
        os_model::WallParams {
            path: os_model::WallPath::Straight {
                start: default_wall(level).start(),
                end: os_core::Point2::new(1.5, 0.0),
            },
            ..default_wall(level)
        },
    );
    let host = wall.id();
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "Inherited narrow door".into(),
            host,
            offset: 0.3,
            definition: OpeningDefinition::Typed { type_id: target_id },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    h.app
        .editor
        .document
        .execute(
            "Set up short host",
            vec![
                Command::AddOpeningType(target),
                Command::AddWall(wall),
                Command::AddOpening(opening),
            ],
        )
        .unwrap();
    h.app.select(Some(target_id));
    let before = h.app.editor.document.model().clone();
    let history = h.app.editor.document.history_stats();

    read_button(&mut h, &path);
    h.click("Update selected compatible type");
    assert!(h.output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Text(text) => text.galley.job.text.starts_with("Preview blocked:"),
        _ => false,
    }));
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.document.history_stats(), history);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(h.app.opening_type_package_dialog.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.document.history_stats(), history);
}

#[test]
fn package_dialog_cancels_when_document_revision_changes() {
    let mut h = Harness::new();
    h.app.begin_import_opening_type_package();
    h.frame(vec![]);
    h.frame(vec![]);
    h.app
        .editor
        .command("Intervening edit", Command::RenameProject("Changed".into()))
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.opening_type_package_dialog.is_none());
    assert!(h.app.status.contains("document changed"));
    assert_eq!(
        h.app.editor.document.model().project.parameters.name,
        "Changed"
    );
}
