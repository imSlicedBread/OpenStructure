//! Temporary spacing contracts through real headless egui frames.
use super::*;
use crate::opening_tools::spacing::{Dimension, Reference, dimensions};

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn refresh(h: &mut Harness, model: os_model::Model, id: Id) {
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app
        .editor
        .pending_geometry
        .extend(h.app.editor.document.model().openings.keys().copied());
    h.app.editor.pending_geometry.insert(h.wall);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();
    h.app.plans.cameras.insert(
        h.view,
        PlanCamera {
            center: Point2::new(0., 0.),
            pixels_per_metre: 35.,
        },
    );
    h.frame(vec![]);
}

fn fixture(
    profile: usize,
    kind: OpeningKind,
    legacy: bool,
    reversed: bool,
    rotated: bool,
    neighbors: bool,
) -> (Harness, Id) {
    let (size, scale) = PROFILES[profile];
    let (mut h, id) = Harness::movable(kind, reversed, legacy, size, scale);
    let mut model = h.app.editor.document.model().clone();
    if !rotated {
        let wall = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
        *wall.path.straight_start_mut().unwrap() = Point2::new(if reversed { 4. } else { -4. }, 0.);
        *wall.path.straight_end_mut().unwrap() = Point2::new(-wall.start().x, 0.);
    }
    let p = &mut model.openings.get_mut(&id).unwrap().parameters;
    p.offset = 3.2;
    if !legacy {
        p.width_override = Some(1.05);
        p.height_override = Some(1.9);
        if kind == OpeningKind::Window {
            p.sill_override = Some(0.6);
            p.pane_position_override = Some(os_model::WindowPanePosition::RightFace);
        }
        let ty = p.type_id().unwrap();
        let material = os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: "Spacing test material".into(),
                density_kg_m3: 650.,
                color: [100, 90, 80],
            },
        );
        let family = &mut model.opening_types.get_mut(&ty).unwrap().parameters.family;
        family.panel_material = Some(material.id());
        family.frame_material = Some(material.id());
        family.frame_width = 0.025;
        model.materials.insert(material.id(), material);
    }
    if neighbors {
        for offset in [0.6, 6.2] {
            let mut p = model.openings[&id].parameters.clone();
            p.offset = offset;
            let other = Opening::new("core.opening", p);
            model.openings.insert(other.id(), other);
        }
    }
    refresh(&mut h, model, id);
    (h, id)
}

fn controls(h: &Harness, id: Id) -> Vec<Dimension> {
    dimensions(
        &h.app.editor,
        id,
        h.app.plans.drawing.as_ref().unwrap(),
        h.app.editor.native_plan_context(h.view).unwrap(),
        h.app.plans.cameras[&h.view],
        h.app.plans.canvas_rect.unwrap(),
    )
}

fn begin(h: &mut Harness, id: Id, end: bool) -> Dimension {
    let d = controls(h, id)
        .into_iter()
        .find(|d| d.end == end)
        .expect("visible spacing");
    // Acquire from the painted metre value, not an invisible helper control.
    let pos = h
        .direct_door_button(&format!("{:.3} m", d.distance))
        .expect("painted dimension");
    h.click(pos);
    assert!(
        h.app
            .plans
            .opening_spacing
            .as_ref()
            .is_some_and(|d| d.editing)
    );
    h.frame(vec![]);
    d
}

#[test]
fn opening_spacing_exact_preserved_fields_history_all_profiles_and_references() {
    for profile in 0..2 {
        for kind in KINDS {
            for legacy in [false, true] {
                for reversed in [false, true] {
                    for neighbors in [false, true] {
                        for rotated in [false, true] {
                            for end in [false, true] {
                                let (mut h, id) =
                                    fixture(profile, kind, legacy, reversed, rotated, neighbors);
                                assert_eq!(
                                    controls(&h, id).len(),
                                    2,
                                    "profile={profile} kind={kind:?} legacy={legacy} reversed={reversed} neighbors={neighbors} canvas={:?} controls={:?}",
                                    h.app.plans.canvas_rect,
                                    controls(&h, id)
                                );
                                let before = h.app.editor.document.model().clone();
                                let scene = h.app.editor.scene.clone();
                                let history = h.app.editor.document.history_stats();
                                let dimension = begin(&mut h, id, end);
                                assert_eq!(
                                    matches!(dimension.reference, Reference::Jamb(..)),
                                    neighbors
                                );
                                if let Reference::Jamb(other, jamb_end) = dimension.reference {
                                    assert_ne!(other, id);
                                    assert_eq!(jamb_end, !end);
                                }
                                // No-op must preserve history, revision, drawing and scene.
                                let revision = h.app.editor.document.revision();
                                let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                                h.frame(vec![key(egui::Key::Enter)]);
                                assert!(h.app.plans.opening_spacing.is_none());
                                assert_eq!(h.app.editor.document.history_stats(), history);
                                assert_eq!(h.app.editor.document.revision(), revision);
                                assert_eq!(
                                    h.app.plans.drawing.as_ref().unwrap().identity(),
                                    drawing
                                );
                                assert_eq!(h.app.editor.scene, scene);
                                begin(&mut h, id, end);
                                let value = dimension.distance + 0.123456789;
                                h.app.plans.opening_spacing.as_mut().unwrap().value =
                                    value.to_string();
                                h.frame(vec![key(egui::Key::Enter)]);
                                h.settle();
                                let after = h.app.editor.document.model().clone();
                                let mut expected = before.clone();
                                expected.openings.get_mut(&id).unwrap().parameters.offset +=
                                    if end { -0.123456789 } else { 0.123456789 };
                                assert!(
                                    (after.openings[&id].parameters.offset
                                        - expected.openings[&id].parameters.offset)
                                        .abs()
                                        < 1e-12
                                );
                                expected.openings.get_mut(&id).unwrap().parameters.offset =
                                    after.openings[&id].parameters.offset;
                                assert_eq!(after, expected, "only selected offset changes");
                                assert_eq!(
                                    h.app.editor.document.history_stats().undo_entries,
                                    history.undo_entries + 1
                                );
                                assert_ne!(h.app.editor.scene[&id], scene[&id]);
                                assert_ne!(
                                    h.app.plans.drawing.as_ref().unwrap().identity(),
                                    drawing
                                );
                                h.app.editor.undo().unwrap();
                                assert_eq!(h.app.editor.document.model(), &before);
                                assert_eq!(h.app.editor.scene, scene);
                                h.app.editor.redo().unwrap();
                                assert_eq!(h.app.editor.document.model(), &after);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn opening_spacing_invalid_is_draft_only_and_escape_cancels() {
    for profile in 0..2 {
        for end in [false, true] {
            let (mut h, id) = fixture(profile, OpeningKind::Window, false, true, true, true);
            begin(&mut h, id, end);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
            for invalid in [
                "", "word", "NaN", "inf", "-1", "0", "0.0005", "4", "20", "1e308",
            ] {
                h.app.plans.opening_spacing.as_mut().unwrap().value = invalid.into();
                h.frame(vec![key(egui::Key::Enter)]);
                assert!(
                    h.app
                        .plans
                        .opening_spacing
                        .as_ref()
                        .unwrap()
                        .error
                        .is_some(),
                    "{invalid}"
                );
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
            }
            h.frame(vec![escape()]);
            assert!(h.app.plans.opening_spacing.is_none());
            assert_eq!(h.app.editor.document.model(), &before);
        }
    }
}

#[test]
fn opening_spacing_press_cancel_loss_and_same_frame_click() {
    for profile in 0..2 {
        for reason in ["escape", "loss", "drag", "selection", "camera", "multiple"] {
            let (mut h, id) = fixture(profile, OpeningKind::Door, false, false, true, false);
            let pos = controls(&h, id)[0].rect.center();
            let before = h.app.editor.document.model().clone();
            let camera = h.app.plans.cameras[&h.view];
            h.hover(pos);
            h.frame(vec![button(pos, true)]);
            assert!(h.app.plans.opening_spacing_claimed);
            match reason {
                "escape" => h.frame(vec![escape()]),
                "loss" => h.frame(vec![egui::Event::PointerGone]),
                "drag" => h.hover(pos + egui::vec2(70., 40.)),
                "selection" => {
                    h.app.select(Some(h.wall));
                    h.frame(vec![]);
                }
                "multiple" => {
                    h.app.selected_ids.insert(h.wall);
                    h.frame(vec![]);
                }
                "camera" => {
                    h.app
                        .plans
                        .cameras
                        .get_mut(&h.view)
                        .unwrap()
                        .pixels_per_metre = 40.;
                    h.frame(vec![]);
                }
                _ => unreachable!(),
            }
            assert!(h.app.plans.opening_spacing.is_none(), "{reason}");
            h.frame(vec![button(pos, false)]);
            assert!(!h.app.plans.opening_spacing_claimed);
            assert!(h.app.plans.opening_move.is_none());
            assert_eq!(h.app.editor.document.model(), &before);
            if reason != "camera" {
                assert_eq!(h.app.plans.cameras[&h.view], camera);
            }
            if !matches!(reason, "selection" | "multiple") {
                assert_eq!(h.app.selected, Some(id));
            }
        }
        let (mut h, id) = fixture(profile, OpeningKind::Door, false, false, true, false);
        let pos = controls(&h, id)[0].rect.center();
        h.hover(pos);
        h.frame(vec![button(pos, true), button(pos, false)]);
        assert!(h.app.plans.opening_spacing.as_ref().unwrap().editing);
    }
}

#[test]
fn opening_spacing_editor_stale_context_and_changed_reference() {
    for profile in 0..2 {
        for reason in [
            "selection",
            "view",
            "settings",
            "provider",
            "revision",
            "session",
            "drawing",
            "reference",
            "width",
            "pointer",
        ] {
            let (mut h, id) = fixture(profile, OpeningKind::Door, false, false, true, true);
            let d = begin(&mut h, id, false);
            h.app.plans.opening_spacing.as_mut().unwrap().value = "2.123".into();
            match reason {
                "selection" => h.app.select(Some(h.wall)),
                "view" => h.app.plans.active = None,
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "revision" => h
                    .app
                    .editor
                    .command("Other edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap());
                }
                "reference" => {
                    let Reference::Jamb(other, _) = d.reference else {
                        panic!("neighbor");
                    };
                    h.app
                        .editor
                        .command("Remove reference", Command::RemoveOpening(other))
                        .unwrap();
                }
                "settings" => {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    settings.visibility.walls = false;
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Hidden", h.app.active_level, settings)
                        .unwrap();
                }
                "width" => {
                    let mut parameters = h.app.editor.document.model().openings[&id]
                        .parameters
                        .clone();
                    parameters.width_override = Some(1.15);
                    h.app
                        .editor
                        .command(
                            "Changed effective width",
                            Command::UpdateOpening { id, parameters },
                        )
                        .unwrap();
                }
                "pointer" => {}
                _ => unreachable!(),
            }
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.frame(if reason == "pointer" {
                vec![egui::Event::PointerGone]
            } else {
                vec![key(egui::Key::Enter)]
            });
            assert!(h.app.plans.opening_spacing.is_none(), "{reason}");
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
        }
    }
}

#[test]
fn opening_spacing_visibility_phase_range_crop_canvas_and_partial_jambs() {
    for profile in 0..2 {
        for reason in [
            "phase",
            "range",
            "hidden",
            "selected_crop",
            "neighbor_crop",
            "partial_jamb",
            "canvas",
            "level",
        ] {
            let (mut h, id) = fixture(profile, OpeningKind::Window, false, false, false, true);
            let mut model = h.app.editor.document.model().clone();
            let other = model
                .openings
                .iter()
                .find(|(_, o)| o.parameters.offset == 6.2)
                .map(|(id, _)| *id)
                .unwrap();
            match reason {
                "phase" => {
                    let future = os_model::new_phase("Future openings", 2);
                    model.element_lifecycles.insert(
                        other,
                        os_model::ElementLifecycle {
                            created_in: future.id(),
                            demolished_in: None,
                        },
                    );
                    model.phases.insert(future.id(), future);
                }
                "range" => {
                    let p = &mut model.openings.get_mut(&other).unwrap().parameters;
                    p.sill_override = Some(2.6);
                    p.height_override = Some(0.3);
                }
                "hidden" => {
                    model
                        .views
                        .get_mut(&h.view)
                        .unwrap()
                        .parameters
                        .plan
                        .as_mut()
                        .unwrap()
                        .visibility
                        .walls = false
                }
                "selected_crop" | "neighbor_crop" | "partial_jamb" => {
                    model
                        .views
                        .get_mut(&h.view)
                        .unwrap()
                        .parameters
                        .plan
                        .as_mut()
                        .unwrap()
                        .crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(-4.1, -1.),
                        max: Point2::new(
                            match reason {
                                "selected_crop" => -0.5,
                                "neighbor_crop" => 2.1,
                                _ => 2.3,
                            },
                            1.,
                        ),
                    });
                }
                "level" => {
                    let mut parameters = model.levels[&h.app.active_level].parameters.clone();
                    parameters.name = "Other plan level".into();
                    let level = os_model::Level::new("core.level", parameters);
                    model.views.get_mut(&h.view).unwrap().parameters.level = Some(level.id());
                    model.levels.insert(level.id(), level);
                }
                "canvas" => {}
                _ => unreachable!(),
            }
            refresh(&mut h, model, id);
            if reason == "canvas" {
                h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(-30., 0.);
                h.frame(vec![]);
            }
            let d = controls(&h, id);
            match reason {
                "phase" | "range" => {
                    assert_eq!(d.len(), 2, "{reason}");
                    assert_eq!(
                        d.iter().find(|d| d.end).unwrap().reference,
                        Reference::Endpoint(true)
                    );
                }
                "neighbor_crop" => {
                    assert_eq!(d.len(), 1);
                    assert!(!d[0].end, "cropped endpoint cannot replace cropped jamb");
                }
                "partial_jamb" => {
                    assert_eq!(d.len(), 2);
                    assert_eq!(
                        d.iter().find(|d| d.end).unwrap().reference,
                        Reference::Jamb(other, false)
                    );
                }
                _ => assert!(d.is_empty(), "{reason}: {d:?}"),
            }
        }
    }
}

#[test]
fn opening_spacing_tools_and_existing_grips_keep_precedence() {
    for profile in 0..2 {
        for tool in [
            "placement",
            "rehost",
            "array",
            "properties",
            "room",
            "wall",
            "hinge",
            "swing",
            "pane",
            "center",
            "start",
            "end",
        ] {
            let (mut h, id) = fixture(
                profile,
                if tool == "pane" {
                    OpeningKind::Window
                } else {
                    OpeningKind::Door
                },
                false,
                false,
                true,
                false,
            );
            let d = controls(&h, id)[0].clone();
            let camera = h.app.plans.cameras[&h.view];
            match tool {
                "placement" => h.begin(OpeningKind::Window),
                "rehost" => h.app.begin_opening_rehost(),
                "array" => h.app.begin_opening_array(),
                "properties" => h.app.begin_opening(None),
                "room" => h.app.plans.room_placement_active = true,
                "wall" => {
                    h.app.select(Some(h.wall));
                }
                _ => {}
            }
            h.frame(vec![]);
            if matches!(
                tool,
                "hinge" | "swing" | "pane" | "center" | "start" | "end"
            ) {
                let pos = match tool {
                    "hinge" => h.direct_door_button("Hinge").unwrap(),
                    "swing" => h.direct_door_button("Swing").unwrap(),
                    "pane" => h.direct_door_button("Side").unwrap(),
                    _ => {
                        h.use_move_anchor(match tool {
                            "start" => 0.,
                            "end" => 1.,
                            _ => 0.5,
                        });
                        h.opening_point(
                            id,
                            h.app.editor.document.model().openings[&id]
                                .parameters
                                .offset,
                        )
                    }
                };
                for d in controls(&h, id) {
                    assert!(!d.rect.expand(4.).contains(pos));
                }
                h.hover(pos);
                h.frame(vec![button(pos, true)]);
                assert!(h.app.plans.opening_spacing.is_none(), "{tool}");
                if matches!(tool, "hinge" | "swing" | "pane") {
                    assert!(h.app.plans.opening_flip.is_some());
                } else {
                    assert!(h.app.plans.opening_move.is_some());
                }
                h.frame(vec![escape(), button(pos, false)]);
            } else {
                assert!(
                    h.direct_door_button(&format!("{:.3} m", d.distance))
                        .is_none(),
                    "{tool}"
                );
                h.hover(d.rect.center());
                h.frame(vec![button(d.rect.center(), true)]);
                assert!(h.app.plans.opening_spacing.is_none(), "{tool}");
                h.frame(vec![escape(), button(d.rect.center(), false)]);
            }
            if tool != "wall" {
                assert_eq!(h.app.plans.cameras[&h.view].center, camera.center);
            }
        }
    }
}

#[test]
fn opening_spacing_keyboard_draft_exact_metres_ignores_snapping() {
    for profile in 0..2 {
        let (mut h, id) = fixture(profile, OpeningKind::Door, false, false, true, false);
        h.app.plans.snaps.enabled = true;
        begin(&mut h, id, false);
        h.frame(vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            },
            egui::Event::Text("2.123456789".into()),
        ]);
        assert_eq!(
            h.app.plans.opening_spacing.as_ref().unwrap().value,
            "2.123456789"
        );
        h.frame(vec![key(egui::Key::Enter)]);
        assert_eq!(
            h.app.editor.document.model().openings[&id]
                .parameters
                .offset,
            2.123456789
        );
        assert!(h.app.plans.opening_spacing.is_none());
    }
}

#[test]
fn opening_spacing_short_gaps_keep_both_labels_separate() {
    for profile in 0..2 {
        let (mut h, id) = fixture(profile, OpeningKind::Window, true, false, false, true);
        let mut model = h.app.editor.document.model().clone();
        for (&other, opening) in &mut model.openings {
            if other != id {
                opening.parameters.offset = if opening.parameters.offset < 3.2 {
                    1.995
                } else {
                    4.405
                };
            }
        }
        refresh(&mut h, model, id);
        let d = controls(&h, id);
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|d| (d.distance - 0.005).abs() < 1e-12));
        assert!(!d[0].rect.expand(4.).intersects(d[1].rect));
        for d in d {
            h.click(d.rect.center());
            assert!(h.app.plans.opening_spacing.as_ref().unwrap().editing);
            h.frame(vec![escape()]);
        }
    }
}
