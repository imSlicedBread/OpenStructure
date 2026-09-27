//! Array contract exercised with real egui frames at both acceptance profiles.
use super::*;
use os_model::{OpeningTag, OpeningTagLabelPreset, OpeningTagParams};

fn fixture(
    kind: OpeningKind,
    reversed: bool,
    legacy: bool,
    toward_end: bool,
    profile: usize,
) -> (Harness, Id) {
    let (size, scale) = PROFILES[profile];
    let (mut h, id) = Harness::movable(kind, reversed, legacy, size, scale);
    let mut model = h.app.editor.document.model().clone();
    let p = &mut model.openings.get_mut(&id).unwrap().parameters;
    p.offset = if toward_end { 0.5 } else { 5.0 };
    if !legacy {
        p.width_override = Some(1.0);
        p.height_override = Some(1.8);
        if kind == OpeningKind::Window {
            p.sill_override = Some(0.7);
        }
    }
    let tag = OpeningTag::new(
        "core.opening_tag",
        OpeningTagParams {
            view: h.view,
            opening: id,
            position: Point2::new(0.0, 1.0),
            label_preset: OpeningTagLabelPreset::TypeAndDimensions,
        },
    );
    model.opening_tags.insert(tag.id(), tag);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend([h.wall, id]);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();
    h.app.plans.split = true;
    h.frame(vec![]);
    (h, id)
}

#[test]
fn opening_array_properties_tags_preview_history_and_split_geometry() {
    for profile in 0..2 {
        for kind in KINDS {
            for legacy in [false, true] {
                for toward_end in [false, true] {
                    let (mut h, id) = fixture(kind, !toward_end, legacy, toward_end, profile);
                    let before = h.app.editor.document.model().clone();
                    let scene = h.app.editor.scene.clone();
                    let history = h.app.editor.document.history_stats();
                    let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                    h.app.begin_opening_array();
                    let d = h.app.plans.opening_array.as_mut().unwrap();
                    assert!(!d.copy_tag);
                    d.count = 3;
                    d.spacing = 2.0;
                    d.toward_end = toward_end;
                    d.copy_tag = true;
                    let preview = d.preview(&before).unwrap();
                    assert_eq!(preview.openings.len(), 2);
                    assert!(preview.error.is_none(), "{:?}", preview.error);
                    let width = before
                        .resolve_opening(&before.openings[&id].parameters)
                        .unwrap()
                        .width;
                    assert_eq!(preview.gap, 2.0 - width);
                    h.frame(vec![]);
                    h.frame(vec![]);
                    assert!(h.has_text("Clear gap:"));
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats(), history);
                    assert_eq!(h.app.editor.scene, scene);
                    h.text_click("Apply array");
                    h.settle();
                    let after = h.app.editor.document.model().clone();
                    assert_eq!(after.openings.len(), 3, "{}", h.app.status);
                    assert_eq!(after.opening_tags.len(), 3);
                    assert_eq!(after.openings[&id], before.openings[&id]);
                    let source = &before.openings[&id].parameters;
                    let tag = before.opening_tags.values().next().unwrap();
                    let wall = &before.walls[&h.wall].parameters;
                    for opening in after.openings.values().filter(|o| o.id() != id) {
                        let mut expected = source.clone();
                        expected.offset = opening.parameters.offset;
                        assert_eq!(opening.parameters, expected);
                        let delta = opening.parameters.offset - source.offset;
                        let sign = if toward_end { 1.0 } else { -1.0 };
                        assert!([2.0 * sign, 4.0 * sign].contains(&delta));
                        let copy_tag = after
                            .opening_tags
                            .values()
                            .find(|t| t.parameters.opening == opening.id())
                            .unwrap();
                        assert_ne!(copy_tag.id(), tag.id());
                        assert_eq!(
                            copy_tag.parameters.label_preset,
                            tag.parameters.label_preset
                        );
                        assert_eq!(copy_tag.parameters.view, h.view);
                        assert!(
                            (copy_tag.parameters.position.x
                                - tag.parameters.position.x
                                - (wall.end.x - wall.start.x) / wall.length() * delta)
                                .abs()
                                < 1e-10
                        );
                        assert!(
                            (copy_tag.parameters.position.y
                                - tag.parameters.position.y
                                - (wall.end.y - wall.start.y) / wall.length() * delta)
                                .abs()
                                < 1e-10
                        );
                        assert_eq!(
                            h.app.editor.scene[&opening.id()],
                            crate::opening_tools::panel_mesh(&after, opening.id()).unwrap()
                        );
                    }
                    assert_ne!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                    assert_ne!(h.app.editor.scene[&h.wall], scene[&h.wall]);
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        history.undo_entries + 1
                    );
                    h.app.editor.undo().unwrap();
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.scene, scene);
                    h.app.editor.redo().unwrap();
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &after);
                }
            }
        }
    }
}

#[test]
fn opening_array_invalid_batch_is_atomic_and_numeric_limits_are_checked() {
    let (mut h, _) = fixture(OpeningKind::Window, false, false, true, 0);
    h.app.begin_opening_array();
    let before = h.app.editor.document.model().clone();
    let scene = h.app.editor.scene.clone();
    let history = h.app.editor.document.history_stats();
    for (count, spacing) in [
        (1, 2.0),
        (257, 2.0),
        (3, 0.0),
        (3, -1.0),
        (3, f64::NAN),
        (3, f64::INFINITY),
        (3, f64::MAX),
        (3, 1.0),
        (5, 2.0),
    ] {
        let d = h.app.plans.opening_array.as_mut().unwrap();
        d.count = count;
        d.spacing = spacing;
        assert!(h.app.apply_opening_array().is_err(), "{count} {spacing}");
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.editor.scene, scene);
    }
    let d = h.app.plans.opening_array.as_mut().unwrap();
    d.count = 5;
    d.spacing = 2.0;
    let preview = d.preview(&before).unwrap();
    assert_eq!(
        preview.openings.len(),
        4,
        "all stations survive invalid preview"
    );
    assert!(preview.error.unwrap().starts_with("Copy 4:"));
    d.count = 3;
    d.spacing = 1.001;
    assert!(
        d.preview(&before).unwrap().error.is_none(),
        "1 mm gap is accepted"
    );
}

#[test]
fn opening_array_canvas_ownership_escape_pointer_loss_and_stale_document() {
    for profile in 0..2 {
        for cancellation in 0..6 {
            let (mut h, id) = fixture(OpeningKind::Door, false, true, true, profile);
            h.app.begin_opening_array();
            h.frame(vec![]);
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let camera = h.app.plans.cameras[&h.view];
            let rect = h.app.plans.canvas_rect.unwrap();
            let from = rect.left_bottom() + egui::vec2(35.0, -35.0);
            let to = from + egui::vec2(50.0, -30.0);
            h.hover(from);
            h.frame(vec![button(from, true)]);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            if let Some(current) = h.app.plans.cameras.get(&h.view) {
                assert_eq!(*current, camera);
            }
            assert_eq!(h.app.selected, Some(id));
            assert!(h.app.plans.opening_move.is_none());
            match cancellation {
                0 => h.frame(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }]),
                1 => h.frame(vec![egui::Event::PointerGone]),
                2 => {
                    h.app.select(Some(h.wall));
                    h.frame(vec![]);
                }
                3 => {
                    h.app.editor.document = Document::from_model(before.clone()).unwrap();
                    h.frame(vec![]);
                }
                4 => {
                    h.app.plans.active = None;
                    h.frame(vec![]);
                }
                _ => {
                    h.app.plans.drawing = None;
                    h.frame(vec![]);
                }
            }
            assert!(h.app.plans.opening_array.is_none());
            h.frame(vec![button(to, false)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            if let Some(current) = h.app.plans.cameras.get(&h.view) {
                assert_eq!(*current, camera);
            }
        }
    }
}

#[test]
fn opening_array_ui_entry_defaults_and_storage() {
    let (mut h, _) = fixture(OpeningKind::Door, false, false, true, 0);
    h.ribbon_click("Architecture", 30.0..58.0);
    h.text_click("Array along wall");
    assert!(h.app.plans.opening_array.is_some(), "{}", h.app.status);
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(h.has_text("Count (including source)"));
    assert!(h.has_text("Centre-to-centre spacing (m)"));
    assert!(h.has_text("Copy tag in this plan"));
    h.text_click("Apply array");
    h.settle();
    assert_eq!(h.app.editor.document.model().openings.len(), 2);
    assert_eq!(h.app.editor.document.model().opening_tags.len(), 1);
    let path = std::env::temp_dir().join(format!("opening-array-{}.os", Id::new()));
    ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
    let loaded = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(loaded.model(), h.app.editor.document.model());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn opening_array_existing_collision_family_fit_and_tag_limit() {
    let (mut h, id) = fixture(OpeningKind::Window, false, false, true, 0);
    h.app.begin_opening_array();
    let d = h.app.plans.opening_array.as_mut().unwrap();
    d.count = 3;
    d.spacing = 2.0;
    let mut model = h.app.editor.document.model().clone();
    let mut parameters = model.openings[&id].parameters.clone();
    parameters.offset = 4.5;
    let obstacle = Opening::new("core.opening", parameters);
    model.openings.insert(obstacle.id(), obstacle);
    let preview = d.preview(&model).unwrap();
    assert!(preview.error.unwrap().starts_with("Copy 2:"));
    model = h.app.editor.document.model().clone();
    model.walls.get_mut(&h.wall).unwrap().parameters.height = 2.0;
    assert!(d.preview(&model).unwrap().error.is_some(), "head clearance");
    model = h.app.editor.document.model().clone();
    let ty = model.openings[&id].parameters.type_id().unwrap();
    model
        .opening_types
        .get_mut(&ty)
        .unwrap()
        .parameters
        .family
        .frame_width = 2.0;
    assert!(
        d.preview(&model).is_err() || d.preview(&model).unwrap().error.is_some(),
        "family fit"
    );
    model = h.app.editor.document.model().clone();
    d.copy_tag = true;
    let original = model
        .opening_tags
        .values()
        .next()
        .unwrap()
        .parameters
        .clone();
    // Existing orphan labels are legal; the batch must still enforce the global limit.
    for _ in 1..os_model::MAX_OPENING_TAGS {
        let mut parameters = original.clone();
        parameters.opening = Id::new();
        let tag = OpeningTag::new("core.opening_tag", parameters);
        model.opening_tags.insert(tag.id(), tag);
    }
    assert!(d.preview(&model).unwrap().error.unwrap().contains("10000"));
    model.opening_tags.clear();
    assert!(
        d.preview(&model).is_err(),
        "cannot copy a missing source tag"
    );
    d.copy_tag = false;
    d.count = 256;
    d.spacing = 1.1;
    model.walls.get_mut(&h.wall).unwrap().parameters.end = Point2::new(300.0, 200.0);
    assert_eq!(d.preview(&model).unwrap().openings.len(), 255);
}

#[test]
fn opening_array_visibility_selection_provider_and_revision_guards() {
    for reason in ["hidden", "crop", "multiple", "provider", "revision"] {
        let (mut h, _) = fixture(OpeningKind::Door, false, false, true, 0);
        if matches!(reason, "provider" | "revision") {
            h.app.begin_opening_array();
            if reason == "provider" {
                h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
            } else {
                h.app
                    .editor
                    .command("Other edit", Command::RenameProject("Changed".into()))
                    .unwrap();
            }
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            assert!(h.app.apply_opening_array().is_err());
            h.frame(vec![]);
            assert!(h.app.plans.opening_array.is_none());
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
        } else {
            if reason == "multiple" {
                h.app.selected_ids.insert(h.wall);
            } else {
                let mut settings = h.app.editor.document.model().views[&h.view]
                    .parameters
                    .plan
                    .unwrap();
                if reason == "hidden" {
                    settings.visibility.walls = false;
                } else {
                    settings.crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(2.0, 1.0),
                        max: Point2::new(4.0, 3.0),
                    });
                }
                h.app
                    .editor
                    .update_floor_plan(h.view, "Visibility", h.app.active_level, settings)
                    .unwrap();
                h.settle();
            }
            h.app.begin_opening_array();
            assert!(h.app.plans.opening_array.is_none(), "{reason}");
        }
    }
}
