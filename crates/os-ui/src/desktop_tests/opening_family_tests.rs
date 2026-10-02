use super::*;

fn reveal_profile_handles(h: &mut Harness) {
    for _ in 0..12 {
        h.frame(vec![]);
    }
    for _ in 0..8 {
        let circles: Vec<_> = h
            .output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c) if c.radius == 3. || c.radius == 5. => {
                    Some((s.clip_rect, c.center))
                }
                _ => None,
            })
            .collect();
        if circles
            .iter()
            .all(|(clip, center)| clip.shrink(20.).contains(*center))
        {
            break;
        }
        let pointer = circles[0].0.center();
        h.frame(vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -80.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..12 {
            h.frame(vec![]);
        }
    }
}

#[test]
fn paired_doors_egui_author_pose_cancel_apply_history_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.begin_new_opening_type(os_model::OpeningKind::Door);
        h.frame(vec![]);
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        h.click("Paired");
        h.click("Cancel type");
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.begin_new_opening_type(os_model::OpeningKind::Door);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Paired");
        h.click("Apply type");
        let ty = *h
            .app
            .editor
            .document
            .model()
            .opening_types
            .keys()
            .next()
            .unwrap();
        assert_eq!(
            h.app.editor.document.model().opening_types[&ty]
                .parameters
                .family
                .door_leaves,
            os_model::DoorLeaves::Paired {
                active_fraction: 0.5
            }
        );
        let view = h
            .app
            .editor
            .create_floor_plan("Pair plan", h.app.active_level)
            .unwrap();
        h.app.focus_plan(Some(view));
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(h.app.active_level));
        let host = wall.id();
        h.app
            .editor
            .command("Host", Command::AddWall(wall))
            .unwrap();
        h.app.select(Some(host));
        h.app.begin_opening(Some(os_model::OpeningKind::Door));
        assert!(h.app.opening_draft.is_some(), "{}", h.app.status);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Apply opening");
        let id = *h
            .app
            .editor
            .document
            .model()
            .openings
            .keys()
            .next()
            .unwrap();
        for cancel in [true, false] {
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.app.select(Some(id));
            h.app.begin_opening(None);
            h.frame(vec![]);
            h.frame(vec![]);
            for _ in 0..8 {
                if h.visible_text_rect("Active leaf angle (0–90°)").is_some() {
                    break;
                }
                let pointer = egui::pos2(size.x * 0.5, size.y * 0.5);
                h.frame(vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., 160.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
            assert!(
                h.visible_text_rect("Active leaf angle (0–90°)").is_some(),
                "{size:?}: {}",
                h.app.status
            );
            h.click("Override pose");
            h.app
                .opening_draft
                .as_mut()
                .unwrap()
                .set_open_state_for_test("30");
            h.click(if cancel {
                "Cancel opening"
            } else {
                "Apply opening"
            });
            if cancel {
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
            } else {
                let after = h.app.editor.document.model().clone();
                assert_eq!(
                    after.openings[&id].parameters.open_state,
                    os_model::OpeningState::DoorPairAngles {
                        active_degrees: 30.,
                        inactive_degrees: 90.
                    }
                );
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    history.undo_entries + 1
                );
                h.click("Undo");
                assert_eq!(h.app.editor.document.model(), &before);
                h.click("Redo");
                assert_eq!(h.app.editor.document.model(), &after);
            }
        }
    }
}

#[test]
fn two_bay_controls_apply_cancel_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        for kind in [os_model::OpeningKind::Door, os_model::OpeningKind::Window] {
            let mut h = Harness::at_size(size, scale);
            let before = h.app.editor.document.model().clone();
            h.app.begin_new_opening_type(kind);
            h.frame(vec![]);
            h.frame(vec![]);
            h.click("Fixed side lite / two bays");
            h.click("Start");
            assert_eq!(h.app.editor.document.model(), &before);
            h.click("Cancel type");
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.begin_new_opening_type(kind);
            h.frame(vec![]);
            h.frame(vec![]);
            h.click("Fixed side lite / two bays");
            h.click("Apply type");
            let after = h.app.editor.document.model().clone();
            let family = &after
                .opening_types
                .values()
                .next()
                .unwrap()
                .parameters
                .family;
            assert_eq!(
                family.side_lite.as_ref().unwrap().side,
                os_model::LiteSide::End
            );
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &after);
        }
    }
}

fn profile_handles(h: &Harness) -> Vec<egui::Pos2> {
    h.output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) if circle.radius == 3. || circle.radius == 5. => {
                Some(circle.center)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn opening_family_panel_and_frame_material_selectors_work_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        let panel = os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: "Oak panel".into(),
                density_kg_m3: 650.,
                color: [180, 180, 180],
            },
        );
        let frame = os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: "Bronze frame".into(),
                density_kg_m3: 8_500.,
                color: [180, 180, 180],
            },
        );
        let (panel_id, frame_id) = (panel.id(), frame.id());
        h.app
            .editor
            .document
            .execute(
                "Project materials",
                vec![Command::AddMaterial(panel), Command::AddMaterial(frame)],
            )
            .unwrap();
        h.app.begin_new_opening_type(os_model::OpeningKind::Door);
        h.frame(vec![]);
        h.frame(vec![]);

        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Materials…");
        for _ in 0..20 {
            if h.visible_text_rect("Panel material: No material").is_some() {
                break;
            }
            let pointer = egui::pos2(size.x * 0.5, size.y * 0.5);
            h.frame(vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -120.),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        for (selector, material_name) in [
            ("Panel material: No material", "Oak panel"),
            ("Frame material: No material", "Bronze frame"),
        ] {
            assert!(
                h.visible_text_rect(selector).is_some(),
                "{selector}; rendered={:?}",
                h.output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some(t.galley.job.text.to_string()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            );
            h.click(selector);
            h.frame(vec![]);
            assert!(
                h.visible_text_rect(material_name).is_some(),
                "{material_name}"
            );
            h.click(material_name);
        }
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        let material_swatches = h
            .output
            .shapes
            .iter()
            .filter(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => {
                    (rect.rect.width() - 12.).abs() < 0.1
                        && (rect.rect.height() - 12.).abs() < 0.1
                        && rect.fill != egui::Color32::TRANSPARENT
                }
                _ => false,
            })
            .count();
        assert!(
            material_swatches >= 2,
            "both assignments need visible swatches"
        );
        h.click("Apply type");
        let ty = h
            .app
            .editor
            .document
            .model()
            .opening_types
            .values()
            .next()
            .unwrap();
        assert_eq!(ty.parameters.family.panel_material, Some(panel_id));
        assert_eq!(ty.parameters.family.frame_material, Some(frame_id));
        assert_eq!(h.ctx.pixels_per_point(), scale);
    }
}

#[test]
fn opening_family_profile_pointer_editor_apply_cancel_invalid_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.begin_new_opening_type(os_model::OpeningKind::Door);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Apply type");
        let id = *h
            .app
            .editor
            .document
            .model()
            .opening_types
            .keys()
            .next()
            .unwrap();
        let baseline = h.app.editor.document.model().clone();
        let stats = h.app.editor.document.history_stats();
        let handles = |h: &Harness| -> Vec<egui::Pos2> {
            h.output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Circle(c) if c.radius == 3. || c.radius == 5. => Some(c.center),
                    _ => None,
                })
                .collect()
        };
        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![]);
        reveal_profile_handles(&mut h);
        let points = handles(&h);
        assert_eq!(points.len(), 4, "profile handles were not drawn");
        // Real pointer drag of upper-right vertex changes the authored elevation.
        h.drag(points[2], points[2] + egui::vec2(-18., 12.));
        assert_ne!(
            handles(&h)[2],
            points[2],
            "profile pointer drag did not move the vertex handle"
        );
        assert_ne!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .profile_for_test(),
            os_model::OpeningFamily::default().profile
        );
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Insert vertex");
        assert_eq!(handles(&h).len(), 5);
        h.click("Remove vertex");
        assert_eq!(handles(&h).len(), 4);
        h.click("Cancel type");
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.history_stats(), stats);
        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        reveal_profile_handles(&mut h);
        let points = handles(&h);
        h.drag(points[2], points[1]);
        h.click("Apply type");
        assert!(
            h.app.opening_type_draft.is_some(),
            "invalid crossing/duplicate stays in editor"
        );
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.history_stats(), stats);
        h.click("Reset rectangle");
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Add frame");
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .frame_width_for_test(),
            0.05
        );
        reveal_profile_handles(&mut h);
        let points = handles(&h);
        h.drag(points[2], points[2] + egui::vec2(-18., 12.));
        assert_ne!(
            handles(&h)[2],
            points[2],
            "second profile pointer drag did not move the vertex handle"
        );
        assert_ne!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .profile_for_test(),
            os_model::OpeningFamily::default().profile
        );
        h.click("Apply type");
        assert!(h.app.opening_type_draft.is_none());
        let applied = h.app.editor.document.model().clone();
        assert_ne!(
            applied.opening_types[&id].parameters.family,
            baseline.opening_types[&id].parameters.family
        );
        assert_eq!(
            applied.opening_types[&id].parameters.family.frame_width,
            0.05
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            stats.undo_entries + 1
        );
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Redo");
        assert_eq!(h.app.editor.document.model(), &applied);
        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        reveal_profile_handles(&mut h);
        let points = handles(&h);
        h.drag(points[2], points[2] + egui::vec2(-8., 5.));
        h.app
            .editor
            .command("Concurrent", Command::RenameProject("Changed".into()))
            .unwrap();
        let changed = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.opening_type_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &changed);
    }
}

#[test]
fn authored_host_cut_profile_pointer_editor_preview_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.begin_new_opening_type(os_model::OpeningKind::Door);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Apply type");
        let id = *h
            .app
            .editor
            .document
            .model()
            .opening_types
            .keys()
            .next()
            .unwrap();
        let baseline = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();

        h.app.begin_edit_opening_type(id);
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![]);
        reveal_profile_handles(&mut h);
        let points = profile_handles(&h);
        assert_eq!(points.len(), 4);
        h.drag(points[2], points[2] + egui::vec2(-18., 12.));
        assert_ne!(
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .profile_for_test(),
            os_model::OpeningFamily::default().profile
        );

        h.click("Host cut profile");
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(profile_handles(&h).len(), 4);
        h.click("Copy component to cut");
        h.frame(vec![]);
        h.frame(vec![]);
        let draft = h.app.opening_type_draft.as_ref().unwrap();
        assert_eq!(draft.profile_for_test(), draft.cut_profile_for_test());
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.history_stats(), history);

        h.click("Apply type");
        assert!(h.app.opening_type_draft.is_none());
        let applied = h.app.editor.document.model().clone();
        let family = &applied.opening_types[&id].parameters.family;
        assert_eq!(family.host_cut, os_model::OpeningHostCut::Profile);
        assert_ne!(
            family.cut_profile,
            os_model::OpeningFamily::default().cut_profile
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Redo");
        assert_eq!(h.app.editor.document.model(), &applied);
    }
}

#[test]
fn window_type_edit_cancel_stale_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.apply_wall();
        let host = h.app.selected.expect("created wall is selected");
        let ty = os_model::OpeningType::new(
            "core.opening_type",
            os_model::OpeningTypeParams {
                window_operation: Default::default(),
                family: Default::default(),
                name: "Window family".into(),
                kind: os_model::OpeningKind::Window,
                width: 1.0,
                height: 1.2,
                sill: 0.8,
                pane_position: os_model::WindowPanePosition::Center,
            },
        );
        let type_id = ty.id();
        let openings = [1.0, 3.0].map(|offset| {
            os_model::Opening::new(
                "core.opening",
                os_model::OpeningParams {
                    open_state: Default::default(),
                    width_override: None,
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
                    name: "Window instance".into(),
                    host,
                    offset,
                    definition: os_model::OpeningDefinition::Typed { type_id },
                    hinge: Default::default(),
                    swing: Default::default(),
                },
            )
        });
        let opening_ids: Vec<_> = openings.iter().map(|opening| opening.id()).collect();
        h.app
            .editor
            .document
            .execute(
                "Add typed windows",
                std::iter::once(Command::AddOpeningType(ty))
                    .chain(openings.into_iter().map(Command::AddOpening))
                    .collect(),
            )
            .unwrap();
        h.app.editor.regenerate().unwrap();
        let baseline = h.app.editor.document.model().clone();
        let baseline_revision = h.app.editor.document.revision();
        let baseline_history = h.app.editor.document.history_stats();

        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(
            h.visible_text_rect("Pane position").is_some(),
            "draft={}; rendered={:?}",
            h.app.opening_type_draft.is_some(),
            h.output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        );
        h.click("Left face");
        h.click("Cancel type");
        assert_eq!(h.app.editor.document.model(), &baseline);
        assert_eq!(h.app.editor.document.revision(), baseline_revision);
        assert_eq!(h.app.editor.document.history_stats(), baseline_history);

        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        h.click("Right face");
        h.click("Apply type");
        let updated = h.app.editor.document.model().clone();
        assert_eq!(
            updated.opening_types[&type_id].parameters.pane_position,
            os_model::WindowPanePosition::RightFace
        );
        assert_eq!(updated.openings.len(), 2);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            baseline_history.undo_entries + 1
        );
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &baseline);
        h.click("Redo");
        assert_eq!(h.app.editor.document.model(), &updated);

        let fixed_model = h.app.editor.document.model().clone();
        let fixed_scene = h.app.editor.scene.clone();
        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        assert!(h.visible_text_rect("Window operation").is_some());
        h.click("Casement");
        h.click("Cancel type");
        assert_eq!(h.app.editor.document.model(), &fixed_model);
        assert_eq!(h.app.editor.scene, fixed_scene);

        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        h.click("Sliding");
        h.click("Apply type");
        let sliding_model = h.app.editor.document.model().clone();
        assert_eq!(
            sliding_model.opening_types[&type_id]
                .parameters
                .window_operation,
            os_model::WindowOperation::Sliding
        );
        assert_eq!(sliding_model.openings.len(), opening_ids.len());
        assert_eq!(h.app.editor.scene[&host], fixed_scene[&host]);
        for id in opening_ids {
            assert_ne!(h.app.editor.scene[&id], fixed_scene[&id]);
            h.app.editor.scene[&id].validate().unwrap();
            assert!(h.app.editor.scene[&id].signed_volume() > 0.);
        }
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            baseline_history.undo_entries + 2
        );
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &fixed_model);
        assert_eq!(h.app.editor.scene, fixed_scene);
        h.click("Redo");
        assert_eq!(h.app.editor.document.model(), &sliding_model);

        h.app.begin_edit_opening_type(type_id);
        h.frame(vec![]);
        h.click("Center");
        h.app
            .editor
            .command(
                "Concurrent edit",
                Command::RenameProject("Concurrent".into()),
            )
            .unwrap();
        let concurrent = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.opening_type_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &concurrent);
    }
}

#[test]
fn opening_pose_editor_apply_cancel_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        let level = h.app.active_level;
        let view = h.app.editor.create_floor_plan("Pose plan", level).unwrap();
        h.app.focus_plan(Some(view));
        h.app.apply_wall();
        let host = h.app.selected.expect("created wall is selected");

        let window_type = os_model::OpeningType::new(
            "core.opening_type",
            os_model::OpeningTypeParams {
                window_operation: os_model::WindowOperation::Sliding,
                family: Default::default(),
                name: "Slider".into(),
                kind: os_model::OpeningKind::Window,
                width: 1.2,
                height: 1.2,
                sill: 0.8,
                pane_position: Default::default(),
            },
        );
        let window_type_id = window_type.id();
        let door = os_model::Opening::new(
            "core.opening",
            os_model::OpeningParams {
                open_state: Default::default(),
                name: "Door".into(),
                host,
                offset: 1.0,
                definition: os_model::OpeningDefinition::Legacy {
                    kind: os_model::OpeningKind::Door,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.0,
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
        let window = os_model::Opening::new(
            "core.opening",
            os_model::OpeningParams {
                open_state: Default::default(),
                name: "Window".into(),
                host,
                offset: 3.0,
                definition: os_model::OpeningDefinition::Typed {
                    type_id: window_type_id,
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
        let door_id = door.id();
        let window_id = window.id();
        h.app
            .editor
            .document
            .execute(
                "Add pose test openings",
                vec![
                    Command::AddOpeningType(window_type),
                    Command::AddOpening(door),
                    Command::AddOpening(window),
                ],
            )
            .unwrap();
        h.app.editor.regenerate().unwrap();

        for (id, label, value, expected) in [
            (
                door_id,
                "Door angle (0–90°)",
                "45",
                os_model::OpeningState::DoorAngle(45.0),
            ),
            (
                window_id,
                "Sliding fraction (0–1)",
                "0.5",
                os_model::OpeningState::SlidingFraction(0.5),
            ),
        ] {
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            h.app.select(Some(id));
            h.app.begin_opening(None);
            assert!(
                h.app.opening_draft.is_some(),
                "opening edit did not start at {size:?}: {}",
                h.app.status
            );
            h.frame(vec![]);
            h.frame(vec![]);
            assert!(h.visible_text_rect(label).is_some());
            h.click("Override pose");
            h.app
                .opening_draft
                .as_mut()
                .unwrap()
                .set_open_state_for_test(value);
            h.click("Cancel opening");
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats(), history);

            h.app.select(Some(id));
            h.app.begin_opening(None);
            h.frame(vec![]);
            h.frame(vec![]);
            h.click("Override pose");
            h.app
                .opening_draft
                .as_mut()
                .unwrap()
                .set_open_state_for_test(value);
            h.click("Apply opening");
            let after = h.app.editor.document.model().clone();
            assert_eq!(after.openings[&id].parameters.open_state, expected);
            assert_eq!(after.openings[&id].header, before.openings[&id].header);
            assert_eq!(h.app.editor.scene[&host], scene[&host]);
            assert_ne!(h.app.editor.scene[&id], scene[&id]);
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );

            h.click("Undo");
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            h.click("Redo");
            assert_eq!(h.app.editor.document.model(), &after);
        }
    }
}
