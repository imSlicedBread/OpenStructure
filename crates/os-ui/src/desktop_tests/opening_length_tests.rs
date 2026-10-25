use super::*;
use os_model::*;

fn sample_type(kind: OpeningKind) -> OpeningType {
    OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: format!("Shared {kind:?}"),
            kind,
            width: 1.,
            height: 1.2,
            sill: if kind == OpeningKind::Door { 0. } else { 0.8 },
            family: Default::default(),
            window_operation: Default::default(),
            pane_position: Default::default(),
        },
    )
}

#[test]
fn typed_formulas_two_types_preview_cancel_apply_geometry_and_history_both_dpi() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        let scalar = LengthParameter::new(
            "core.length_parameter",
            LengthParameterParams {
                name: "Factor".into(),
                unit: LengthUnit::Scalar,
                value: 2.,
                formula: None,
            },
        );
        let sid = scalar.id();
        let width = LengthParameter::new(
            "core.length_parameter",
            LengthParameterParams {
                name: "Width".into(),
                unit: LengthUnit::Metres,
                value: 1.,
                formula: Some(format!("0.5m * @{{{sid}}}")),
            },
        );
        let wid = width.id();
        let depth = LengthParameter::new(
            "core.length_parameter",
            LengthParameterParams {
                name: "Depth".into(),
                unit: LengthUnit::Metres,
                value: 0.02,
                formula: Some(format!("@{{{wid}}} * 0.02")),
            },
        );
        let did = depth.id();
        let mut wp = default_wall(h.app.active_level);
        wp.path = WallPath::Straight {
            start: Point2::default(),
            end: Point2::new(12., 0.),
        };
        let wall = Wall::new(os_walls::WALL_TYPE, wp);
        let host = wall.id();
        let mut commands = vec![
            Command::AddWall(wall),
            Command::AddLengthParameter(scalar),
            Command::AddLengthParameter(width),
            Command::AddLengthParameter(depth),
        ];
        let mut ids = Vec::new();
        for (i, kind) in [OpeningKind::Door, OpeningKind::Window]
            .into_iter()
            .enumerate()
        {
            let mut ty = sample_type(kind);
            let id = ty.id();
            let mut solid = OpeningExtrusion::new("Driven handle");
            solid.bindings = OpeningExtrusionBindings {
                depth: Some(did),
                x: Some(did),
                y: Some(did),
                normal_offset: Some(did),
            };
            ty.parameters.family.extrusions.push(solid);
            commands.push(Command::AddOpeningType(ty));
            commands.push(Command::SetOpeningTypeLengthBindings {
                id,
                bindings: OpeningTypeLengthBindings {
                    width: Some(wid),
                    ..Default::default()
                },
            });
            let opening = Opening::new(
                "core.opening",
                OpeningParams {
                    name: format!("Instance {i}"),
                    host,
                    offset: 1. + i as f64 * 5.,
                    definition: OpeningDefinition::Typed { type_id: id },
                    width_override: (i == 1).then_some(0.9),
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
                    hinge: Default::default(),
                    swing: Default::default(),
                    open_state: Default::default(),
                },
            );
            ids.push(opening.id());
            commands.push(Command::AddOpening(opening));
        }
        h.app.editor.document.execute("Fixture", commands).unwrap();
        h.app.editor.regenerate().unwrap();
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let history = h.app.editor.document.history_stats();
        h.app.begin_length_parameters();
        h.app.length_draft.as_mut().unwrap().selected = Some(wid);
        h.frame(vec![]);
        h.frame(vec![]);
        h.app
            .length_draft
            .as_mut()
            .unwrap()
            .parameters
            .get_mut(&wid)
            .unwrap()
            .parameters
            .formula = Some(format!("0.6m * @{{{sid}}}"));
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(
            h.visible_text_rect("Preview valid: 2 type(s), 2 effective placement(s) change.")
                .is_some()
        );
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.click("Cancel Length edits");
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.begin_length_parameters();
        h.app.length_draft.as_mut().unwrap().selected = Some(wid);
        h.app
            .length_draft
            .as_mut()
            .unwrap()
            .parameters
            .get_mut(&wid)
            .unwrap()
            .parameters
            .formula = Some(format!("0.6m * @{{{sid}}}"));
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Apply Length edits");
        assert!(h.app.length_draft.is_none(), "{}", h.app.status);
        for id in &ids {
            assert_ne!(h.app.editor.scene[id], scene[id]);
        }
        assert_eq!(
            h.app.editor.document.model().openings[&ids[1]]
                .parameters
                .width_override,
            Some(0.9)
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
        h.app.begin_length_parameters();
        h.app.length_draft.as_mut().unwrap().selected = Some(wid);
        h.app
            .length_draft
            .as_mut()
            .unwrap()
            .parameters
            .get_mut(&wid)
            .unwrap()
            .parameters
            .formula = Some("1m + 2".into());
        h.frame(vec![]);
        h.click("Apply Length edits");
        assert_eq!(h.app.editor.document.model(), &after);
        assert!(h.app.length_draft.is_some());
        h.click("Cancel Length edits");
    }
}

#[test]
fn shared_lengths_desktop_manager_bind_preview_apply_undo_cancel_stale_both_dpi() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.click("Manage");
        h.click("Project Length parameters…");
        h.click("New Length");
        let driver = h.app.length_draft.as_ref().unwrap().selected.unwrap();
        assert!(h.app.editor.document.model().length_parameters.is_empty());
        h.click("Apply Length edits");
        assert!(h.app.length_draft.is_none());
        assert!(
            h.app
                .editor
                .document
                .model()
                .length_parameters
                .contains_key(&driver)
        );
        let types = [
            sample_type(OpeningKind::Door),
            sample_type(OpeningKind::Window),
        ];
        let ids = types.each_ref().map(|t| t.id());
        h.app
            .editor
            .document
            .execute(
                "Types",
                types.into_iter().map(Command::AddOpeningType).collect(),
            )
            .unwrap();
        for id in ids {
            h.app.begin_edit_opening_type(id);
            h.app.opening_type_draft.as_mut().unwrap().bindings.width = Some(driver);
            h.frame(vec![]);
            h.frame(vec![]);
            h.click("Apply type");
            assert!(h.app.opening_type_draft.is_none(), "{}", h.app.status);
            assert_eq!(
                h.app.editor.document.model().opening_type_length_bindings[&id].width,
                Some(driver)
            );
        }
        let wall = Wall::new(os_walls::WALL_TYPE, default_wall(h.app.active_level));
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                open_state: Default::default(),
                name: "Shared window".into(),
                host: wall.id(),
                offset: 1.,
                definition: OpeningDefinition::Typed { type_id: ids[1] },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let opening_id = opening.id();
        h.app
            .editor
            .document
            .execute(
                "Placement",
                vec![Command::AddWall(wall), Command::AddOpening(opening)],
            )
            .unwrap();
        h.app.editor.regenerate().unwrap();
        h.app.begin_length_parameters();
        {
            let draft = h.app.length_draft.as_mut().unwrap();
            draft.selected = Some(driver);
            draft.parameters.get_mut(&driver).unwrap().parameters.value = 1.4;
        }
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        let history = h.app.editor.document.history_stats();
        let scene = h.app.editor.scene.clone();
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(
            h.visible_text_rect("Preview valid: 2 type(s), 1 effective placement(s) change.")
                .is_some()
        );
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.revision(), revision);
        h.click("Apply Length edits");
        assert_ne!(h.app.editor.scene[&opening_id], scene[&opening_id]);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        for id in ids {
            assert_eq!(
                h.app
                    .editor
                    .document
                    .model()
                    .resolve_opening_type(id)
                    .unwrap()
                    .parameters
                    .width,
                1.4
            );
        }
        let after = h.app.editor.document.model().clone();
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &after);
        h.app.begin_length_parameters();
        h.app.length_draft.as_mut().unwrap().selected = Some(driver);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Delete Length");
        assert!(
            h.app
                .length_draft
                .as_ref()
                .unwrap()
                .parameters
                .contains_key(&driver)
        );
        h.click("Duplicate Length");
        assert_ne!(
            h.app.length_draft.as_ref().unwrap().selected.unwrap(),
            driver
        );
        h.click("Cancel Length edits");
        assert_eq!(h.app.editor.document.model(), &after);
        h.app.begin_length_parameters();
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(h.app.length_draft.is_none());
        h.app.begin_length_parameters();
        h.app
            .editor
            .document
            .execute(
                "External edit",
                vec![Command::RenameProject("Changed".into())],
            )
            .unwrap();
        h.frame(vec![]);
        assert!(h.app.length_draft.is_none());
    }
}
