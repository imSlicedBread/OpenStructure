use super::*;
use std::f64::consts::PI;

fn preview_at(h: &Harness, screen: egui::Pos2) -> OpeningPlacementPreview {
    let rect = h.app.plans.canvas_rect.unwrap();
    let context = h.app.editor.native_plan_context(h.view).unwrap();
    opening_placement_preview(
        h.app.editor.document.model(),
        h.app.plans.drawing.as_ref().unwrap(),
        context,
        h.app.plans.cameras[&h.view],
        [f64::from(rect.width()), f64::from(rect.height())],
        Point2::new(
            f64::from(screen.x - rect.left()),
            f64::from(screen.y - rect.top()),
        ),
        h.app.plans.opening_placement.as_ref().unwrap(),
    )
    .unwrap()
    .expect("pointer is over a visible host")
}

fn add_window_options(h: &mut Harness, id: Id) -> (Id, Id) {
    let mut model = h.app.editor.document.model().clone();
    let type_id = model.openings[&id].parameters.type_id().unwrap();
    let ty = &mut model.opening_types.get_mut(&type_id).unwrap().parameters;
    ty.pane_position = os_model::WindowPanePosition::LeftFace;
    ty.family.side_lite = Some(os_model::SideLite {
        side: os_model::LiteSide::Start,
        width_fraction: 0.25,
        mullion_width: 0.04,
        material: None,
    });
    let mut plain = os_model::OpeningType::new("core.opening_type", ty.clone());
    plain.parameters.name = "Centered plain window".into();
    plain.parameters.pane_position = os_model::WindowPanePosition::Center;
    plain.parameters.family.side_lite = None;
    let plain_id = plain.id();
    model.opening_types.insert(plain_id, plain);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend([h.wall, id]);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();
    (type_id, plain_id)
}

#[test]
fn door_orientation_is_preview_only_persists_across_repeats_and_commits_once() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let before = h.app.editor.document.model().clone();
        let before_scene = h.app.editor.scene.clone();
        let history = h.app.editor.document.history_stats();
        h.begin(OpeningKind::Door);

        // Toolbar input belongs to the orientation controls, not the canvas.
        h.text_click("Flip hinge");
        h.text_click("Flip hinge");
        h.text_click("Flip swing");
        h.text_click("Flip swing");
        assert!(h.app.editor.document.model().openings.is_empty());
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, before_scene);
        assert_eq!(h.app.editor.document.history_stats(), history);

        let first = h.point(Point2::new(-2.0, 0.0));
        let neutral = preview_at(&h, first);
        let mut expected = h
            .app
            .plans
            .opening_placement
            .as_ref()
            .unwrap()
            .base_parameters(&before, h.wall)
            .unwrap();
        expected.offset = neutral.parameters.offset;
        assert_eq!(
            neutral.parameters, expected,
            "two flips restore exact defaults"
        );

        h.text_click("Flip hinge");
        h.text_click("Flip swing");
        h.hover(first);
        let preview = preview_at(&h, first);
        assert_eq!(preview.resolved.hinge, os_model::DoorHinge::End);
        assert_eq!(preview.resolved.swing, os_model::DoorSwing::Right);
        assert!(preview.message.is_none());
        assert!(h.has_text("Hinge: End · Swing: Right"));
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, before_scene);
        assert_eq!(h.app.editor.document.history_stats(), history);

        h.click(first);
        h.settle();
        let first_id = h.app.selected.unwrap();
        let placed = &h.app.editor.document.model().openings[&first_id].parameters;
        assert_eq!(placed.hinge, os_model::DoorHinge::End);
        assert_eq!(placed.swing, os_model::DoorSwing::Right);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);

        let after_first = h.app.editor.document.model().clone();
        let scene_after_first = h.app.editor.scene.clone();
        let overlapping = preview_at(&h, first);
        assert!(overlapping.message.is_some());
        h.click(first);
        assert_eq!(h.app.editor.document.model(), &after_first);
        assert_eq!(h.app.editor.scene, scene_after_first);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        h.assert_active(OpeningKind::Door);

        // Repeat placement keeps the transient choices and one click is one edit.
        let second = h.point(Point2::new(2.0, 0.0));
        h.hover(second);
        assert!(h.has_text("Hinge: End · Swing: Right"));
        h.click(second);
        h.settle();
        let second_id = h.app.selected.unwrap();
        assert_ne!(first_id, second_id);
        let second_params = &h.app.editor.document.model().openings[&second_id].parameters;
        assert_eq!(second_params.hinge, os_model::DoorHinge::End);
        assert_eq!(second_params.swing, os_model::DoorSwing::Right);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 2);

        let after = h.app.editor.document.model().clone();
        h.frame(vec![escape()]);
        assert!(h.app.plans.opening_placement.is_none());
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &after_first);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &after);
    }
}

#[test]
fn drawn_width_placement_commits_the_same_preview_orientation() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let mut h = if kind == OpeningKind::Door {
                Harness::new(size, scale)
            } else {
                let (mut h, id) = Harness::movable(kind, false, false, size, scale);
                add_window_options(&mut h, id);
                h
            };
            h.begin_width(kind);
            if kind == OpeningKind::Window {
                let type_id = h
                    .app
                    .editor
                    .document
                    .model()
                    .opening_types
                    .values()
                    .find(|ty| {
                        ty.parameters.kind == kind
                            && ty.parameters.pane_position == os_model::WindowPanePosition::LeftFace
                    })
                    .unwrap()
                    .id();
                h.app.change_placement_type(kind, type_id);
                h.frame(vec![]);
            }
            match kind {
                OpeningKind::Door => {
                    h.text_click("Flip hinge");
                    h.text_click("Flip swing");
                }
                OpeningKind::Window => {
                    h.text_click("Flip side");
                    h.text_click("Flip lite");
                }
            }
            h.app.plans.snaps.enabled = false;
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            let from = h.width_point(4.1);
            let to = h.width_point(5.5);
            h.hover(from);
            h.frame(vec![button(from, true)]);
            h.hover(to);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats(), history);
            let preview = preview_at(&h, to);
            assert!(preview.message.is_none());
            assert!((preview.resolved.width - 1.4).abs() < 1e-4);
            h.frame(vec![button(to, false)]);
            h.settle();

            let placed_id = h.app.selected.unwrap();
            let placed = &h.app.editor.document.model().openings[&placed_id].parameters;
            assert_eq!(placed.host, h.wall);
            assert!((placed.offset - 4.1).abs() < 1e-4);
            match kind {
                OpeningKind::Door => {
                    assert_eq!(placed.hinge, os_model::DoorHinge::End);
                    assert_eq!(placed.swing, os_model::DoorSwing::Right);
                }
                OpeningKind::Window => {
                    assert_eq!(
                        placed.pane_position_override,
                        Some(os_model::WindowPanePosition::RightFace)
                    );
                    assert_eq!(placed.lite_side_override, Some(os_model::LiteSide::End));
                }
            }
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            assert_ne!(h.app.editor.scene, scene);
            let after = h.app.editor.document.model().clone();
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after);
        }
    }
}

#[test]
fn copied_orientation_is_relative_preserves_source_and_double_toggle_keeps_nullable_pins() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for legacy in [false, true] {
                let (mut h, source_id, target_wall) =
                    Harness::rehostable(kind, legacy, false, false, size, scale);
                let mut model = h.app.editor.document.model().clone();
                if !legacy {
                    let type_id = model.openings[&source_id].parameters.type_id().unwrap();
                    let ty = &mut model.opening_types.get_mut(&type_id).unwrap().parameters;
                    if kind == OpeningKind::Window {
                        ty.pane_position = os_model::WindowPanePosition::LeftFace;
                    }
                    ty.family.side_lite = Some(os_model::SideLite {
                        side: os_model::LiteSide::Start,
                        width_fraction: 0.25,
                        mullion_width: 0.04,
                        material: None,
                    });
                }
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.editor.pending_geometry.insert(source_id);
                h.app.editor.regenerate().unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.app.select(Some(source_id));
                h.settle();
                h.app.plans.cameras.get_mut(&h.view).unwrap().center =
                    Point2::new(0.0, if scale > 1.0 { 0.65 } else { 0.5 });
                h.frame(vec![]);

                let before = h.app.editor.document.model().clone();
                let before_scene = h.app.editor.scene.clone();
                let source = before.openings[&source_id].parameters.clone();
                let history = h.app.editor.document.history_stats();
                h.app.begin_opening_copy(source_id);
                h.frame(vec![]);
                let labels: &[&str] = match (kind, legacy) {
                    (OpeningKind::Door, _) => &["Flip hinge", "Flip swing"],
                    (OpeningKind::Window, false) => &["Flip side", "Flip lite"],
                    (OpeningKind::Window, true) => &[],
                };
                if kind == OpeningKind::Window && legacy {
                    assert!(!h.has_text("Flip side"));
                    assert!(!h.has_text("Flip lite"));
                }
                for &label in labels {
                    assert!(h.has_text(label), "copy offers {label}");
                    h.text_click(label);
                    h.text_click(label);
                }
                if kind == OpeningKind::Door && !legacy {
                    h.text_click("Flip lite");
                    h.text_click("Flip lite");
                }

                let target = h.point(Point2::new(-2.0, 2.0));
                h.hover(target);
                let candidate = preview_at(&h, target);
                assert!(candidate.message.is_none());
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, before_scene);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(
                    h.app.editor.document.model().openings[&source_id].parameters,
                    source,
                    "copy preview never changes source"
                );

                let mut expected_restored = source.clone();
                expected_restored.host = target_wall;
                expected_restored.offset = candidate.parameters.offset;
                assert_eq!(candidate.parameters, expected_restored);
                assert_eq!(source.pane_position_override, None);
                assert_eq!(source.lite_side_override, None);
                assert_eq!(h.app.editor.document.model(), &before);

                // One toggle changes only the requested orientation fields.
                for &label in labels {
                    h.text_click(label);
                }
                if kind == OpeningKind::Door && !legacy {
                    h.text_click("Flip lite");
                }
                let candidate = preview_at(&h, target);
                let mut expected = source.clone();
                expected.host = target_wall;
                expected.offset = candidate.parameters.offset;
                if kind == OpeningKind::Door {
                    expected.hinge = match source.hinge {
                        os_model::DoorHinge::Start => os_model::DoorHinge::End,
                        os_model::DoorHinge::End => os_model::DoorHinge::Start,
                    };
                    expected.swing = match source.swing {
                        os_model::DoorSwing::Left => os_model::DoorSwing::Right,
                        os_model::DoorSwing::Right => os_model::DoorSwing::Left,
                    };
                    if !legacy {
                        expected.lite_side_override = Some(os_model::LiteSide::End);
                    }
                } else if !legacy {
                    expected.pane_position_override = Some(os_model::WindowPanePosition::RightFace);
                    expected.lite_side_override = Some(os_model::LiteSide::End);
                }
                assert_eq!(candidate.parameters, expected);
                h.click(target);
                h.settle();
                let copy_id = h.app.selected.unwrap();
                assert_ne!(copy_id, source_id);
                assert_eq!(
                    h.app.editor.document.model().openings[&copy_id].parameters,
                    expected
                );
                assert_eq!(
                    h.app.editor.document.model().openings[&source_id].parameters,
                    source
                );
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    history.undo_entries + 1
                );
                let after = h.app.editor.document.model().clone();
                h.app.history(false);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &before);
                h.app.history(true);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &after);
            }
        }
    }
}

#[test]
fn window_orientation_availability_tracks_type_and_signed_arc_hosts() {
    for (size, scale) in PROFILES {
        // A selected type can be changed while the tool is active. Ineligible
        // pane/lite intent must be forgotten rather than resurfacing later.
        let (mut h, id) = Harness::movable(OpeningKind::Window, false, false, size, scale);
        let (type_id, plain_id) = add_window_options(&mut h, id);
        h.begin(OpeningKind::Window);
        h.app.change_placement_type(OpeningKind::Window, type_id);
        h.frame(vec![]);
        h.text_click("Flip side");
        h.text_click("Flip lite");
        h.app.change_placement_type(OpeningKind::Window, plain_id);
        h.frame(vec![]);
        assert!(!h.has_text("Flip side"));
        assert!(!h.has_text("Flip lite"));
        h.app.change_placement_type(OpeningKind::Window, type_id);
        let path = h.app.editor.document.model().walls[&h.wall].parameters.path;
        let target = h.point(path.point(5.8));
        h.hover(target);
        let candidate = preview_at(&h, target);
        assert_eq!(
            candidate.resolved.pane_position,
            os_model::WindowPanePosition::LeftFace
        );
        assert_eq!(
            candidate.resolved.family.side_lite.as_ref().unwrap().side,
            os_model::LiteSide::Start
        );

        // Preview/commit orientation uses the same analytic station and panel
        // geometry for wrapped major arcs in both directions.
        for sweep in [1.5 * PI, -1.5 * PI] {
            let mut arc = Harness::new(size, scale);
            let path = os_model::WallPath::CircularArc {
                center: Point2::default(),
                radius: 2.0,
                start_angle_rad: if sweep.is_sign_positive() { 5.8 } else { -5.8 },
                signed_sweep_rad: sweep,
            };
            let mut model = arc.app.editor.document.model().clone();
            model.walls.get_mut(&arc.wall).unwrap().parameters.path = path;
            arc.app.editor.document = Document::from_model(model).unwrap();
            arc.app.editor.pending_geometry.insert(arc.wall);
            arc.app.editor.regenerate().unwrap();
            arc.app.plans.poll(&arc.app.editor);
            arc.app.focus_plan(Some(arc.view));
            arc.app.plans.cameras.insert(
                arc.view,
                PlanCamera {
                    center: path.point(path.length() * 0.5),
                    pixels_per_metre: 35.0,
                },
            );
            arc.settle();
            arc.begin(OpeningKind::Door);
            arc.text_click("Flip hinge");
            arc.text_click("Flip swing");
            let at = arc.point(path.point(path.length() * 0.5));
            arc.hover(at);
            let before = arc.app.editor.document.model().clone();
            let scene = arc.app.editor.scene.clone();
            let history = arc.app.editor.document.history_stats();
            let preview = preview_at(&arc, at);
            assert!(preview.message.is_none());
            assert_eq!(preview.resolved.hinge, os_model::DoorHinge::End);
            assert_eq!(preview.resolved.swing, os_model::DoorSwing::Right);
            assert_eq!(arc.app.editor.document.model(), &before);
            assert_eq!(arc.app.editor.scene, scene);
            assert_eq!(arc.app.editor.document.history_stats(), history);
            arc.click(at);
            arc.settle();
            let model = arc.app.editor.document.model();
            let opening = model.openings.values().next().unwrap();
            assert_eq!(opening.parameters.hinge, os_model::DoorHinge::End);
            assert_eq!(opening.parameters.swing, os_model::DoorSwing::Right);
            let native = os_geometry::walls::NativeWall::from_model(model, arc.wall).unwrap();
            assert!(native.mesh().unwrap().signed_volume() > 0.0);
            assert!(arc.app.editor.scene.contains_key(&opening.id()));
        }
    }
}
