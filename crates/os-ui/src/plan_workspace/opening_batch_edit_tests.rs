//! Real Properties/dialog/canvas frames at both desktop acceptance profiles.
use super::*;
use opening_batch_edit::{Draft, Field, Mode};

fn fixture(profile: usize, kind: OpeningKind, legacy: bool, path: i32) -> (Harness, Vec<Id>) {
    let (size, scale) = PROFILES[profile];
    let (mut h, id) = Harness::movable(kind, path == 2, false, size, scale);
    let mut model = h.app.editor.document.model().clone();
    if path.abs() == 1 {
        model.walls.get_mut(&h.wall).unwrap().parameters.path = os_model::WallPath::CircularArc {
            center: Point2::default(),
            radius: 4.,
            start_angle_rad: 0.3,
            signed_sweep_rad: path as f64 * 4.7,
        };
    }
    let mut ty = model
        .opening_types
        .values()
        .next()
        .unwrap()
        .parameters
        .clone();
    ty.name = "Other assigned type".into();
    ty.width = 1.1;
    let ty = OpeningType::new("core.opening_type", ty);
    let mut p = model.openings[&id].parameters.clone();
    p.offset = 4.5;
    p.width_override = Some(1.1);
    p.definition = OpeningDefinition::Typed { type_id: ty.id() };
    model.opening_types.insert(ty.id(), ty);
    if legacy {
        let r = model.resolve_opening(&p).unwrap();
        p.definition = OpeningDefinition::Legacy {
            kind,
            width: r.width,
            height: r.height,
            sill: r.sill,
        };
        p.width_override = None;
    }
    let other = Opening::new("core.opening", p);
    let ids = vec![id, other.id()];
    model.openings.insert(other.id(), other);
    let tag = os_model::OpeningTag::new(
        "core.opening_tag",
        os_model::OpeningTagParams {
            opening: id,
            view: h.view,
            position: Point2::new(0., 1.),
            label_preset: os_model::OpeningTagLabelPreset::DimensionsOnly,
        },
    );
    model.opening_tags.insert(tag.id(), tag);
    for id in &ids {
        model
            .openings
            .get_mut(id)
            .unwrap()
            .header
            .properties
            .insert("note".into(), serde_json::json!("preserve"));
    }
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app
        .editor
        .pending_geometry
        .extend(ids.iter().copied().chain([h.wall]));
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select_area(ids.iter().copied().collect());
    h.settle();
    h.app.plans.split = true;
    h.frame(vec![]);
    (h, ids)
}

fn begin(h: &mut Harness) {
    h.text_click("Edit selected instances…");
    assert!(h.app.plans.opening_batch_edit.is_some(), "{}", h.app.status);
}

fn edit(h: &mut Harness, index: usize, mode: Mode, text: &str) {
    let d = h.app.plans.opening_batch_edit.as_mut().unwrap();
    d.fields[index] = Field {
        mode,
        text: text.into(),
    };
    d.preview = d.candidate(h.app.editor.document.model());
    h.frame(vec![]);
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

fn choice_click(h: &mut Harness, row: &str, choice: &str) {
    h.frame(vec![]);
    h.frame(vec![]);
    let y = h
        .output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == row => Some(t.pos.y),
            _ => None,
        })
        .unwrap();
    let point = h
        .output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == choice && (t.pos.y - y).abs() < 4. => {
                let point = egui::Rect::from_min_size(t.pos, t.galley.size()).center();
                assert!(s.clip_rect.contains(point), "choice is clipped: {row}");
                Some(point)
            }
            _ => None,
        })
        .unwrap();
    h.click(point);
    h.frame(vec![]);
}

#[test]
fn opening_batch_edit_properties_mixed_preview_atomic_history_and_geometry() {
    for profile in 0..2 {
        for kind in KINDS {
            for legacy in [false, true] {
                for path in [0, 2, 1, -1] {
                    let (mut h, ids) = fixture(profile, kind, legacy, path);
                    assert!(h.app.selected.is_none(), "group has no primary");
                    assert!(h.has_text("Width: Mixed m · Mixed"));
                    let before = h.app.editor.document.model().clone();
                    let history = h.app.editor.document.history_stats();
                    let scene = h.app.editor.scene.clone();
                    let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                    begin(&mut h);
                    assert!(
                        h.app
                            .plans
                            .opening_batch_edit
                            .as_ref()
                            .unwrap()
                            .preview
                            .as_ref()
                            .unwrap()
                            .commands
                            .is_empty()
                    );
                    edit(&mut h, 0, Mode::Set, "1.3");
                    edit(&mut h, 1, Mode::Set, "1.6");
                    if kind == OpeningKind::Window {
                        edit(&mut h, 2, Mode::Set, "0.65");
                    }
                    if kind == OpeningKind::Door {
                        choice_click(&mut h, "Door hinge", "Start");
                        choice_click(&mut h, "Door swing", "Left");
                        assert_eq!(
                            h.app.plans.opening_batch_edit.as_ref().unwrap().hinge,
                            Some(os_model::DoorHinge::Start),
                            "profile={profile} kind={kind:?} legacy={legacy} path={path}"
                        );
                    }
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    let preview = h
                        .app
                        .plans
                        .opening_batch_edit
                        .as_ref()
                        .unwrap()
                        .preview
                        .as_ref()
                        .unwrap();
                    let items = preview.drawing.items(context).unwrap().to_vec();
                    let lines = preview.drawing.provider_lines(context).unwrap().to_vec();
                    assert_eq!(preview.commands.len(), 2);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.editor.document.history_stats(), history);
                    assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                    let camera = h.app.plans.cameras[&h.view];
                    let rect = h.app.plans.canvas_rect.unwrap();
                    let points: Vec<_> = items[0]
                        .footprint
                        .vertices()
                        .iter()
                        .map(|p| {
                            let p = camera
                                .project(*p, [rect.width() as f64, rect.height() as f64])
                                .unwrap();
                            rect.min + egui::vec2(p.x as f32, p.y as f32)
                        })
                        .collect();
                    assert!(
                        h.output.shapes.iter().any(|s| matches!(&s.shape,
                        egui::Shape::Path(path) if path.closed && path.points == points)),
                        "candidate aperture is painted"
                    );
                    h.drag(
                        rect.right_top() + egui::vec2(-10., 10.),
                        rect.right_top() + egui::vec2(-30., 20.),
                    );
                    assert_eq!(h.app.plans.cameras[&h.view], camera);
                    h.text_click("Apply instances");
                    h.settle();
                    let after = h.app.editor.document.model().clone();
                    let mut expected = before.clone();
                    for id in &ids {
                        let p = &mut expected.openings.get_mut(id).unwrap().parameters;
                        match &mut p.definition {
                            OpeningDefinition::Typed { .. } => {
                                p.width_override = Some(1.3);
                                p.height_override = Some(1.6);
                                if kind == OpeningKind::Window {
                                    p.sill_override = Some(0.65);
                                }
                            }
                            OpeningDefinition::Legacy {
                                width,
                                height,
                                sill,
                                ..
                            } => {
                                *width = 1.3;
                                *height = 1.6;
                                if kind == OpeningKind::Window {
                                    *sill = 0.65;
                                }
                            }
                        }
                        if kind == OpeningKind::Door {
                            p.hinge = os_model::DoorHinge::Start;
                            p.swing = os_model::DoorSwing::Left;
                        }
                        assert_eq!(
                            h.app.editor.scene[id],
                            crate::opening_tools::panel_mesh(&after, *id).unwrap()
                        );
                    }
                    assert_eq!(after, expected);
                    assert_eq!(h.app.selected_ids, ids.iter().copied().collect());
                    assert!(h.app.selected.is_none());
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        history.undo_entries + 1
                    );
                    assert_eq!(
                        h.app.editor.scene[&h.wall],
                        crate::opening_tools::host_mesh(&after, h.wall).unwrap()
                    );
                    let plan = h.app.editor.native_wall_plan(h.view).unwrap();
                    let context = h.app.editor.native_plan_context(h.view).unwrap();
                    assert_eq!(plan.items(context).unwrap(), items);
                    assert!(
                        lines
                            .iter()
                            .all(|l| plan.provider_lines(context).unwrap().contains(l))
                    );
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &after);
                    begin(&mut h);
                    let history = h.app.editor.document.history_stats();
                    h.text_click("Apply instances");
                    h.settle();
                    assert_eq!(h.app.editor.document.history_stats(), history);
                    begin(&mut h);
                    edit(&mut h, 0, Mode::Set, "1.3");
                    h.text_click("Apply instances");
                    h.settle();
                    assert_eq!(h.app.editor.document.history_stats(), history);
                }
            }
        }
    }
}

#[test]
fn opening_batch_edit_keep_inherit_equal_default_and_single_properties() {
    for profile in 0..2 {
        for kind in KINDS {
            let (mut h, ids) = fixture(profile, kind, false, 0);
            let before = h.app.editor.document.model().clone();
            begin(&mut h);
            let value = before
                .resolve_opening(&before.openings[&ids[0]].parameters)
                .unwrap()
                .width
                .to_string();
            edit(&mut h, 0, Mode::Set, &value);
            edit(&mut h, 1, Mode::Set, "1.6");
            if kind == OpeningKind::Window {
                edit(&mut h, 2, Mode::Set, "0.65");
            }
            h.text_click("Apply instances");
            h.settle();
            for id in &ids {
                assert_eq!(
                    h.app.editor.document.model().openings[id]
                        .parameters
                        .width_override,
                    Some(value.parse().unwrap())
                );
            }
            begin(&mut h);
            edit(&mut h, 0, Mode::Set, "garbage");
            edit(&mut h, 0, Mode::Keep, "garbage");
            let history = h.app.editor.document.history_stats();
            h.text_click("Apply instances");
            h.settle();
            assert_eq!(h.app.editor.document.history_stats(), history);
            begin(&mut h);
            edit(&mut h, 0, Mode::Inherit, "");
            edit(&mut h, 1, Mode::Inherit, "");
            if kind == OpeningKind::Window {
                edit(&mut h, 2, Mode::Inherit, "");
            }
            h.text_click("Apply instances");
            h.settle();
            let after = h.app.editor.document.model();
            let mut expected = before.clone();
            for id in &ids {
                expected
                    .openings
                    .get_mut(id)
                    .unwrap()
                    .parameters
                    .width_override = None;
            }
            assert_eq!(after, &expected);
            h.app.select(Some(ids[0]));
            h.frame(vec![]);
            h.ribbon_click("Architecture", 30.0..58.0);
            h.text_click("Edit selected openings…");
            assert!(h.app.plans.opening_batch_edit.is_some());
            h.text_click("Cancel instance edit");
            assert!(h.app.plans.opening_batch_edit.is_none());
            let (mut h, _) = fixture(profile, kind, true, 0);
            begin(&mut h);
            assert!(h.has_text("Legacy selection: Inherit unavailable."));
            for field in 0..if kind == OpeningKind::Window { 3 } else { 2 } {
                edit(&mut h, field, Mode::Inherit, "");
                assert!(h.app.apply_opening_batch_edit().is_err());
                edit(&mut h, field, Mode::Keep, "");
            }
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        }
    }
}

#[test]
fn opening_batch_edit_rejection_escape_and_stale_context() {
    for profile in 0..2 {
        for reason in [
            "overlap",
            "fit",
            "parse",
            "escape",
            "selection",
            "revision",
            "session",
            "view",
            "drawing",
            "provider",
        ] {
            let (mut h, ids) = fixture(profile, OpeningKind::Window, false, 0);
            begin(&mut h);
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            if matches!(reason, "overlap" | "fit" | "parse") {
                edit(
                    &mut h,
                    if reason == "fit" { 1 } else { 0 },
                    Mode::Set,
                    match reason {
                        "overlap" => "3.1",
                        "fit" => "2.9",
                        _ => "NaN",
                    },
                );
                let error = h.app.apply_opening_batch_edit().unwrap_err().to_string();
                assert!(
                    ids.iter().any(|id| error.contains(&id.to_string())),
                    "{error}"
                );
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                continue;
            }
            match reason {
                "escape" => h.frame(vec![key(egui::Key::Escape)]),
                "selection" => h.app.select(Some(ids[0])),
                "revision" => {
                    let mut p = before.openings[&ids[0]].parameters.clone();
                    p.name = "Intervening".into();
                    h.app
                        .editor
                        .document
                        .execute(
                            "Intervening",
                            vec![Command::UpdateOpening {
                                id: ids[0],
                                parameters: p,
                            }],
                        )
                        .unwrap();
                }
                "session" => h.app.editor.document = Document::from_model(before.clone()).unwrap(),
                "view" => h.app.plans.active = None,
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                _ => unreachable!(),
            }
            h.frame(vec![]);
            assert!(h.app.plans.opening_batch_edit.is_none(), "{reason}");
            assert!(h.app.apply_opening_batch_edit().is_err());
        }
    }
}

#[test]
fn opening_batch_edit_real_text_focus_enter_escape_and_no_canvas_leak() {
    for profile in 0..2 {
        let (mut h, _) = fixture(profile, OpeningKind::Window, false, 0);
        let before = h.app.editor.document.model().clone();
        begin(&mut h);
        h.text_click("Set value");
        // Locate the field by its actual widget rectangle after rendering.
        let id = h.ctx.memory(|m| m.focused());
        assert!(id.is_none());
        let label = h
            .output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Width (m)" => Some(t.pos),
                _ => None,
            })
            .unwrap();
        h.click(label + egui::vec2(30., 50.));
        assert!(h.ctx.wants_keyboard_input());
        h.frame(vec![egui::Event::Text("1.25".into())]);
        assert_eq!(
            h.app.plans.opening_batch_edit.as_ref().unwrap().fields[0].text,
            "1.25"
        );
        h.frame(vec![key(egui::Key::Enter)]);
        assert_eq!(h.app.editor.document.model(), &before);
        h.click(label + egui::vec2(30., 50.));
        h.frame(vec![key(egui::Key::Tab)]);
        assert_eq!(h.app.editor.document.model(), &before);
        h.click(label + egui::vec2(30., 50.));
        assert!(h.ctx.wants_keyboard_input());
        h.frame(vec![key(egui::Key::Escape)]);
        assert!(h.app.plans.opening_batch_edit.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
    }
}

#[test]
fn opening_batch_edit_bound_visibility_and_changed_only_256() {
    for profile in 0..2 {
        let (mut h, ids) = fixture(profile, OpeningKind::Window, false, 0);
        let mut model = h.app.editor.document.model().clone();
        let original = model.openings[&ids[0]].parameters.clone();
        model.openings.clear();
        model.walls.get_mut(&h.wall).unwrap().parameters.path = os_model::WallPath::Straight {
            start: Point2::default(),
            end: Point2::new(20., 0.),
        };
        let mut hosts = vec![h.wall];
        for i in 1..32 {
            let mut p = model.walls[&h.wall].parameters.clone();
            p.path = os_model::WallPath::Straight {
                start: Point2::new(0., i as f64 * 3.),
                end: Point2::new(20., i as f64 * 3.),
            };
            let wall = os_model::Wall::new(os_walls::WALL_TYPE, p);
            hosts.push(wall.id());
            model.walls.insert(wall.id(), wall);
        }
        for host in &hosts {
            for i in 0..8 {
                let mut p = original.clone();
                p.host = *host;
                p.offset = 1. + i as f64 * 2.;
                if i == 0 {
                    p.width_override = Some(1.3);
                }
                let opening = Opening::new("core.opening", p);
                model.openings.insert(opening.id(), opening);
            }
        }
        h.app.editor.document = Document::from_model(model.clone()).unwrap();
        h.app
            .editor
            .pending_geometry
            .extend(model.openings.keys().copied().chain(hosts));
        h.app.editor.regenerate().unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.app.select_area(model.openings.keys().copied().collect());
        h.settle();
        begin(&mut h);
        edit(&mut h, 0, Mode::Set, "1.3");
        assert_eq!(
            h.app
                .plans
                .opening_batch_edit
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .commands
                .len(),
            224
        );
        h.text_click("Apply instances");
        h.settle();
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        assert_eq!(h.app.selected_ids.len(), 256);
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &model);
        h.app.selected_ids.insert(Id::new());
        assert!(Draft::begin(&h.app).is_err());
        h.app.select(None);
        assert!(Draft::begin(&h.app).is_err());
        h.app.select_area(model.openings.keys().copied().collect());
        h.app.selected_ids.insert(h.wall);
        assert!(Draft::begin(&h.app).is_err());
        h.app.select_area(model.openings.keys().copied().collect());
        model
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .visibility
            .windows = false;
        h.app.editor.document = Document::from_model(model).unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.settle();
        assert!(Draft::begin(&h.app).is_err());
    }
}

#[test]
fn opening_batch_edit_one_invalid_member_blocks_all_and_names_id() {
    for profile in 0..2 {
        let (mut h, ids) = fixture(profile, OpeningKind::Window, true, 0);
        let mut model = h.app.editor.document.model().clone();
        model.openings.get_mut(&ids[1]).unwrap().parameters.offset = 6.5;
        h.app.editor.document = Document::from_model(model.clone()).unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.settle();
        begin(&mut h);
        edit(&mut h, 0, Mode::Set, "1.6");
        let error = h.app.apply_opening_batch_edit().unwrap_err().to_string();
        assert!(error.contains(&ids[1].to_string()), "{error}");
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
    }
}
