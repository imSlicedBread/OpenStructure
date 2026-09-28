use super::*;

fn lite_fixture(
    kind: OpeningKind,
    default: os_model::LiteSide,
    size: egui::Vec2,
    scale: f32,
) -> (Harness, Id) {
    let (mut h, id) = Harness::movable(kind, false, false, size, scale);
    let mut model = h.app.editor.document.model().clone();
    let type_id = model.openings[&id].parameters.type_id().unwrap();
    model
        .opening_types
        .get_mut(&type_id)
        .unwrap()
        .parameters
        .family
        .side_lite = Some(os_model::SideLite {
        side: default,
        width_fraction: 0.25,
        mullion_width: 0.04,
        material: None,
    });
    model
        .openings
        .get_mut(&id)
        .unwrap()
        .parameters
        .lite_side_override = None;
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend([h.wall, id]);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();

    let model = h.app.editor.document.model();
    let opening = model
        .resolve_opening(&model.openings[&id].parameters)
        .unwrap();
    let center = os_geometry::openings::world(
        &model.resolve_wall(h.wall).unwrap().parameters,
        opening.offset + opening.width / 2.0,
        0.0,
    );
    h.app.plans.cameras.insert(
        h.view,
        PlanCamera {
            center,
            pixels_per_metre: 35.0,
        },
    );
    h.frame(vec![]);
    (h, id)
}

fn click_visible_form_text(h: &mut Harness, label: &str) {
    for _ in 0..30 {
        let rendered = h.output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                Some((text.pos + text.galley.size() * 0.5, shape.clip_rect))
            }
            _ => None,
        });
        let Some((position, clip)) = rendered else {
            panic!("missing form text: {label}");
        };
        if position.y >= clip.top() + 32.0 && position.y <= clip.bottom() - 32.0 {
            h.click(position);
            h.frame(vec![]);
            return;
        }
        let delta = if position.y > clip.bottom() - 32.0 {
            -120.0
        } else {
            120.0
        };
        h.frame(vec![
            egui::Event::PointerMoved(clip.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..8 {
            h.frame(vec![]);
        }
    }
    panic!("form text never became visible: {label}");
}

#[test]
fn direct_plan_flip_lite_is_instance_local_and_undoable() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            for default in [os_model::LiteSide::Start, os_model::LiteSide::End] {
                let (mut h, id) = lite_fixture(kind, default, size, scale);
                let before = h.app.editor.document.model().clone();
                let before_scene = h.app.editor.scene.clone();
                let camera = h.app.plans.cameras[&h.view];
                let history = h.app.editor.document.history_stats().undo_entries;
                let context = h.app.editor.native_plan_context(h.view).unwrap();
                let drawing = h.app.plans.drawing.as_ref().unwrap();
                let identity = drawing.identity();
                let items = drawing.items(context).unwrap().to_vec();
                let lines = drawing.provider_lines(context).unwrap().to_vec();
                let pos = h
                    .direct_door_button("Flip lite")
                    .expect("visible lite control");

                h.hover(pos);
                h.frame(vec![button(pos, true)]);
                assert!(h.app.plans.opening_flip.is_some());
                assert_eq!(h.app.editor.document.model(), &before, "press is transient");
                assert_eq!(
                    h.app.editor.scene, before_scene,
                    "preview does not mutate 3D"
                );
                h.frame(vec![button(pos, false)]);
                h.settle();
                h.move_clean();

                let other = match default {
                    os_model::LiteSide::Start => os_model::LiteSide::End,
                    os_model::LiteSide::End => os_model::LiteSide::Start,
                };
                let mut expected = before.clone();
                expected
                    .openings
                    .get_mut(&id)
                    .unwrap()
                    .parameters
                    .lite_side_override = Some(other);
                assert_eq!(h.app.editor.document.model(), &expected);
                assert_eq!(h.app.selected, Some(id));
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    history + 1
                );
                assert_ne!(h.app.editor.scene[&id], before_scene[&id]);
                assert_eq!(h.app.editor.scene[&h.wall], before_scene[&h.wall]);

                let context = h.app.editor.native_plan_context(h.view).unwrap();
                let drawing = h.app.plans.drawing.as_ref().unwrap();
                assert_ne!(drawing.identity(), identity);
                assert_eq!(drawing.items(context).unwrap(), items);
                assert_ne!(drawing.provider_lines(context).unwrap(), lines);
                assert_eq!(
                    expected.opening_types[&before.openings[&id].parameters.type_id().unwrap()]
                        .parameters
                        .family
                        .side_lite
                        .as_ref()
                        .unwrap()
                        .side,
                    default,
                    "the reusable type remains unchanged"
                );
                assert_eq!(
                    expected
                        .resolve_opening(&expected.openings[&id].parameters)
                        .unwrap()
                        .family
                        .side_lite
                        .unwrap()
                        .side,
                    other
                );

                h.app.history(false);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &before);
                h.app.history(true);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &expected);
            }
        }
    }
}

#[test]
fn opening_editor_lite_side_inherits_pins_and_resets_without_preview_mutation() {
    for (size, scale) in PROFILES {
        let (mut h, id) = lite_fixture(OpeningKind::Door, os_model::LiteSide::Start, size, scale);
        let before = h.app.editor.document.model().clone();

        h.text_click("Edit opening");
        assert!(h.has_text("Lite at start"));
        assert!(h.has_text("Lite at end"));
        assert_eq!(h.app.editor.document.model(), &before);
        click_visible_form_text(&mut h, "Lite at end");
        assert!(h.has_text("Effective lite: End"), "radio updates the draft");
        assert_eq!(h.app.editor.document.model(), &before, "dialog is a draft");
        h.text_click("Apply opening");
        let mut pinned = before.clone();
        pinned
            .openings
            .get_mut(&id)
            .unwrap()
            .parameters
            .lite_side_override = Some(os_model::LiteSide::End);
        assert_eq!(h.app.editor.document.model(), &pinned);

        h.text_click("Edit opening");
        click_visible_form_text(&mut h, "Reset lite to type default");
        assert_eq!(h.app.editor.document.model(), &pinned);
        assert!(
            h.has_text("Effective lite: Start"),
            "reset updates the draft preview"
        );
        h.text_click("Apply opening");
        assert_eq!(h.app.editor.document.model(), &before);

        h.text_click("Edit opening");
        click_visible_form_text(&mut h, "Lite at end");
        h.text_click("Cancel opening");
        assert_eq!(h.app.editor.document.model(), &before);
    }
}
