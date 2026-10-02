//! Keyboard and pointer input through real egui frames at both display profiles.
use super::*;

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn enter_width(h: &mut Harness, text: &str) {
    if !h
        .app
        .plans
        .opening_placement
        .as_ref()
        .unwrap()
        .exact_width
        .is_exact()
    {
        h.text_click("Exact width");
    }
    let response = h
        .ctx
        .read_response(egui::Id::new("opening_placement_width"))
        .unwrap();
    assert!(response.enabled());
    h.click(response.rect.center());
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
        egui::Event::Key {
            key: egui::Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        },
        egui::Event::Text(text.into()),
    ]);
    h.frame(vec![]);
    assert!(
        matches!(&h.app.plans.opening_placement.as_ref().unwrap().exact_width,
        placement_width::Intent::Exact(value) if value == text)
    );
}

fn preview(h: &Harness, at: egui::Pos2) -> Result<Option<OpeningPlacementPreview>> {
    let rect = h.app.plans.canvas_rect.unwrap();
    opening_placement_preview(
        h.app.editor.document.model(),
        h.app.plans.drawing.as_ref().unwrap(),
        h.app.editor.native_plan_context(h.view).unwrap(),
        h.app.plans.cameras[&h.view],
        [f64::from(rect.width()), f64::from(rect.height())],
        Point2::new(f64::from(at.x - rect.left()), f64::from(at.y - rect.top())),
        h.app.plans.opening_placement.as_ref().unwrap(),
    )
}

fn commit_and_history(h: &mut Harness, width: f64, pin: Option<f64>) -> Id {
    h.app.plans.snaps.enabled = false;
    h.frame(vec![]);
    let at = h.width_point(5.5);
    let before = h.app.editor.document.model().clone();
    let scene = h.app.editor.scene.clone();
    let history = h.app.editor.document.history_stats();
    h.hover(at);
    let p = preview(h, at).unwrap().unwrap();
    assert!(p.message.is_none(), "{:?}", p.message);
    assert_eq!(p.resolved.width, width);
    assert!((p.parameters.offset + width * 0.5 - 5.5).abs() < 1e-5);
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);
    h.click(at);
    h.settle();
    assert_eq!(
        h.app.editor.document.model().openings.len(),
        before.openings.len() + 1
    );
    let id = h.app.selected.unwrap();
    let after = h.app.editor.document.model().clone();
    let placed = &after.openings[&id].parameters;
    assert_eq!(after.resolve_opening(placed).unwrap().width, width);
    assert_eq!(placed.width_override, pin);
    assert_eq!(
        h.app.editor.document.history_stats().undo_entries,
        history.undo_entries + 1
    );
    assert_ne!(h.app.editor.scene, scene);
    h.app.history(false);
    h.settle();
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    h.app.history(true);
    h.settle();
    assert_eq!(h.app.editor.document.model(), &after);
    id
}

#[test]
fn exact_width_typed_inherit_custom_and_reset_do_not_edit_type() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for action in ["inherit", "exact", "reset"] {
                let (mut h, source) = Harness::movable(kind, false, false, size, scale);
                h.begin(kind);
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let history = h.app.editor.document.history_stats();
                let default = before
                    .resolve_opening(&before.openings[&source].parameters)
                    .unwrap()
                    .width;
                if action != "inherit" {
                    enter_width(&mut h, "1.23456789");
                }
                if action == "reset" {
                    h.text_click("Reset width");
                }
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.editor.document.history_stats(), history);
                let pin = (action == "exact").then_some(1.23456789);
                commit_and_history(&mut h, pin.unwrap_or(default), pin);
                assert_eq!(
                    h.app.editor.document.model().opening_types,
                    before.opening_types
                );
            }
        }
    }
}

#[test]
fn exact_width_copy_preserves_source_pins_and_legacy_until_edited() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for variant in ["inherited", "pinned", "legacy"] {
                for action in ["preserve", "exact", "reset"] {
                    let (mut h, source) =
                        Harness::movable(kind, false, variant == "legacy", size, scale);
                    if variant == "pinned" {
                        let mut model = h.app.editor.document.model().clone();
                        model
                            .openings
                            .get_mut(&source)
                            .unwrap()
                            .parameters
                            .width_override = Some(1.1);
                        h.app.editor.document = Document::from_model(model).unwrap();
                        h.app.editor.pending_geometry.extend([h.wall, source]);
                        h.app.editor.regenerate().unwrap();
                        h.app.plans.poll(&h.app.editor);
                        h.app.focus_plan(Some(h.view));
                        h.app.select(Some(source));
                        h.settle();
                    }
                    let before = h.app.editor.document.model().clone();
                    let source_params = before.openings[&source].parameters.clone();
                    let original = before.resolve_opening(&source_params).unwrap().width;
                    h.app.begin_opening_copy(source);
                    h.frame(vec![]);
                    if action != "preserve" {
                        enter_width(&mut h, "1.3456789");
                    }
                    if action == "reset" {
                        h.text_click("Reset width");
                    }
                    let width = match action {
                        "exact" => 1.3456789,
                        _ => original,
                    };
                    let pin = if variant == "legacy" {
                        None
                    } else {
                        match action {
                            "exact" => Some(width),
                            _ => source_params.width_override,
                        }
                    };
                    let copy = commit_and_history(&mut h, width, pin);
                    let after = h.app.editor.document.model();
                    assert_eq!(after.opening_types, before.opening_types);
                    assert_eq!(after.openings[&source].parameters, source_params);
                    let mut expected = source_params;
                    expected.offset = after.openings[&copy].parameters.offset;
                    match &mut expected.definition {
                        OpeningDefinition::Legacy { width: value, .. } => *value = width,
                        OpeningDefinition::Typed { .. } => expected.width_override = pin,
                    }
                    assert_eq!(after.openings[&copy].parameters, expected);
                }
            }
        }
    }
}

#[test]
fn exact_width_default_type_is_atomic_and_orientation_survives() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for action in ["inherit", "exact", "equal", "reset"] {
                let mut h = Harness::new(size, scale);
                h.begin(kind);
                let default = if kind == OpeningKind::Door { 0.9 } else { 1.2 };
                if action != "inherit" {
                    enter_width(
                        &mut h,
                        &if action == "equal" {
                            default
                        } else {
                            1.23456789
                        }
                        .to_string(),
                    );
                }
                if action == "reset" {
                    h.text_click("Reset width");
                }
                if kind == OpeningKind::Door {
                    h.text_click("Flip hinge");
                }
                let pin = match action {
                    "exact" => Some(1.23456789),
                    "equal" => Some(default),
                    _ => None,
                };
                let id = commit_and_history(&mut h, pin.unwrap_or(default), pin);
                let model = h.app.editor.document.model();
                assert_eq!(model.opening_types.len(), 1);
                let p = &model.openings[&id].parameters;
                assert_eq!(
                    model.opening_types[&p.type_id().unwrap()].parameters.width,
                    default
                );
                if kind == OpeningKind::Door {
                    assert_eq!(p.hinge, os_model::DoorHinge::End);
                }
            }
        }
    }
}

#[test]
fn exact_width_draw_clears_intent_and_invalid_entries_never_commit() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let (mut h, source) = Harness::movable(kind, false, false, size, scale);
            h.app.begin_opening_copy(source);
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            for text in [
                "", ".", "abc", "NaN", "inf", "-1", "0", "0.0009", "1e999", "1m", "1,2", "1/2",
            ] {
                enter_width(&mut h, text);
                assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Text(t) if t.galley.job.text.contains("Width must") || t.galley.job.text.contains("Width: enter"))));
                let at = h.width_point(5.5);
                assert!(preview(&h, at).is_err());
                h.click(at);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.editor.document.history_stats(), history);
            }
            enter_width(&mut h, "0.001");
            assert!(!h.has_text("Cannot place: Width must"));
            enter_width(&mut h, "1.3");
            let overlap = h.width_point(2.0);
            assert!(preview(&h, overlap).unwrap().unwrap().message.is_some());
            h.click(overlap);
            assert_eq!(h.app.editor.document.model(), &before);
            h.text_click("Draw opening width");
            assert!(
                !h.app
                    .plans
                    .opening_placement
                    .as_ref()
                    .unwrap()
                    .exact_width
                    .is_exact()
            );
            assert!(h.has_text("Width from drag"));
            h.app.plans.snaps.enabled = false;
            let from = h.width_point(4.7);
            let to = h.width_point(6.1);
            h.hover(from);
            h.frame(vec![button(from, true)]);
            h.hover(to);
            assert!((preview(&h, to).unwrap().unwrap().resolved.width - 1.4).abs() < 1e-5);
            assert_eq!(h.app.editor.document.model(), &before);
            h.frame(vec![button(to, false)]);
            h.settle();
            let placed = h.app.selected.unwrap();
            assert!(
                (h.app
                    .editor
                    .document
                    .model()
                    .resolve_opening(&h.app.editor.document.model().openings[&placed].parameters)
                    .unwrap()
                    .width
                    - 1.4)
                    .abs()
                    < 1e-5
            );
            h.text_click("Draw opening width");
            assert_eq!(
                preview(&h, h.width_point(5.5))
                    .unwrap()
                    .unwrap()
                    .resolved
                    .width,
                before
                    .resolve_opening(&before.openings[&source].parameters)
                    .unwrap()
                    .width
            );
            let after = h.app.editor.document.model().clone();
            h.frame(vec![escape()]);
            assert!(h.app.plans.opening_placement.is_none());
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
fn exact_width_signed_arc_hosts_use_centerline_width() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for sweep in [1.5 * std::f64::consts::PI, -1.5 * std::f64::consts::PI] {
                let mut h = Harness::new(size, scale);
                let path = os_model::WallPath::CircularArc {
                    center: Point2::default(),
                    radius: 2.0,
                    start_angle_rad: 5.8,
                    signed_sweep_rad: sweep,
                };
                let mut model = h.app.editor.document.model().clone();
                model.walls.get_mut(&h.wall).unwrap().parameters.path = path;
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.editor.pending_geometry.insert(h.wall);
                h.app.editor.regenerate().unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.app.plans.cameras.insert(
                    h.view,
                    PlanCamera {
                        center: path.point(5.5),
                        pixels_per_metre: 35.0,
                    },
                );
                h.settle();
                h.begin(kind);
                enter_width(&mut h, "1.23456789");
                commit_and_history(&mut h, 1.23456789, Some(1.23456789));
            }
        }
    }
}

#[test]
fn exact_width_keyboard_toolbar_and_mode_ownership() {
    for (size, scale) in PROFILES {
        let (mut h, source) = Harness::movable(OpeningKind::Door, false, false, size, scale);
        h.app.begin_opening_copy(source);
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let camera = h.app.plans.cameras[&h.view];
        let selection = h.app.selected;
        let field = egui::Id::new("opening_placement_width");
        enter_width(&mut h, " 1.25e0 ");
        assert!(h.ctx.memory(|m| m.has_focus(field)));
        h.frame(vec![key(egui::Key::Enter)]);
        assert!(!h.ctx.memory(|m| m.has_focus(field)));
        assert_eq!(
            preview(&h, h.width_point(5.5))
                .unwrap()
                .unwrap()
                .resolved
                .width,
            1.25
        );
        enter_width(&mut h, "1.25");
        h.frame(vec![key(egui::Key::Tab)]);
        h.frame(vec![]);
        assert!(!h.ctx.memory(|m| m.has_focus(field)));
        assert!(
            h.ctx.memory(|m| m.focused().is_some()),
            "Tab navigates to another widget"
        );
        // Selection and canvas shortcuts must not consume editing input.
        enter_width(&mut h, "123");
        h.frame(vec![key(egui::Key::Home), key(egui::Key::Delete)]);
        assert!(h.ctx.memory(|m| m.has_focus(field)));
        assert_eq!(h.app.editor.document.model(), &before);
        let long = "1".repeat(80);
        let response = h.ctx.read_response(field).unwrap();
        h.click(response.rect.center());
        h.frame(vec![egui::Event::Paste(long)]);
        assert!(
            matches!(&h.app.plans.opening_placement.as_ref().unwrap().exact_width,
            placement_width::Intent::Exact(text) if text.chars().count() == 64)
        );

        enter_width(&mut h, "1.5");
        h.text_click("Draw opening width");
        assert!(
            !h.app
                .plans
                .opening_placement
                .as_ref()
                .unwrap()
                .exact_width
                .is_exact()
        );
        h.text_click("Exact width");
        assert!(!h.app.plans.opening_placement.as_ref().unwrap().draw_width);
        assert_eq!(
            preview(&h, h.width_point(5.5))
                .unwrap()
                .unwrap()
                .resolved
                .width,
            0.9
        );
        h.text_click("Reset width");
        h.text_click("Exact width");
        h.text_click("Reset width");
        h.text_click("Exact width");
        h.text_click("Exact width");
        assert_eq!(
            preview(&h, h.width_point(5.5))
                .unwrap()
                .unwrap()
                .parameters
                .width_override,
            None
        );
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        assert_eq!(h.app.selected, selection);
        enter_width(&mut h, "1.25");
        h.frame(vec![escape()]);
        assert!(h.app.plans.opening_placement.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
    }
}

#[test]
fn exact_width_type_change_repeat_and_snapped_center() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let (mut h, source) = Harness::movable(kind, true, false, size, scale);
            let mut model = h.app.editor.document.model().clone();
            let original_type = model.openings[&source].parameters.type_id().unwrap();
            let mut ty = OpeningType::new(
                "core.opening_type",
                model.opening_types[&original_type].parameters.clone(),
            );
            ty.parameters.name = "Wider type".into();
            ty.parameters.width = 1.6;
            let wider = ty.id();
            model.opening_types.insert(wider, ty);
            h.app.editor.document = Document::from_model(model).unwrap();
            h.app.editor.pending_geometry.insert(h.wall);
            h.app.editor.regenerate().unwrap();
            h.app.plans.poll(&h.app.editor);
            h.app.focus_plan(Some(h.view));
            h.settle();
            h.begin(kind);
            h.app.change_placement_type(kind, original_type);
            h.frame(vec![]);
            enter_width(&mut h, "1.25");
            h.app.change_placement_type(kind, wider);
            h.frame(vec![]);
            assert_eq!(
                preview(&h, h.width_point(5.5))
                    .unwrap()
                    .unwrap()
                    .resolved
                    .width,
                1.25
            );
            h.text_click("Reset width");
            assert_eq!(
                preview(&h, h.width_point(5.5))
                    .unwrap()
                    .unwrap()
                    .resolved
                    .width,
                1.6
            );
            h.text_click("Exact width");
            assert!(
                matches!(&h.app.plans.opening_placement.as_ref().unwrap().exact_width,
                placement_width::Intent::Exact(text) if text == "1.6")
            );
            enter_width(&mut h, "1.25");
            // Near the host midpoint: exact width must center on its snap station.
            let at = h.width_point(4.04);
            h.hover(at);
            let candidate = preview(&h, at).unwrap().unwrap();
            assert!((candidate.parameters.offset + 0.625 - 4.0).abs() < 1e-5);
            h.click(at);
            h.settle();
            assert!(
                h.app
                    .plans
                    .opening_placement
                    .as_ref()
                    .unwrap()
                    .exact_width
                    .is_exact()
            );
            let first = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let second = h.width_point(6.2);
            h.click(second);
            h.settle();
            let after = h.app.editor.document.model().clone();
            assert_eq!(after.openings.len(), first.openings.len() + 1);
            assert_eq!(
                after.openings[&h.app.selected.unwrap()]
                    .parameters
                    .width_override,
                Some(1.25)
            );
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &first);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after);
        }
    }
}
