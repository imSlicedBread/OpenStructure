use super::*;
use os_model::WindowPanePosition::{self, Center, LeftFace, RightFace};

fn fixture(size: egui::Vec2, scale: f32, pane: WindowPanePosition) -> (Harness, Id) {
    let (mut h, id) = Harness::movable(OpeningKind::Window, false, false, size, scale);
    let mut model = h.app.editor.document.model().clone();
    let p = &mut model.openings.get_mut(&id).unwrap().parameters;
    p.width_override = Some(1.);
    p.height_override = Some(1.1);
    p.sill_override = Some(0.7);
    let ty = p.type_id().unwrap();
    model
        .opening_types
        .get_mut(&ty)
        .unwrap()
        .parameters
        .pane_position = pane;
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend([h.wall, id]);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();
    h.use_move_anchor(0.);
    (h, id)
}

#[test]
fn window_side_flip_is_one_instance_update_with_history_both_dpis() {
    for (size, scale) in PROFILES {
        for pane in [LeftFace, RightFace] {
            let (mut h, id) = fixture(size, scale, pane);
            for _ in 0..2 {
                h.frame(vec![]);
                let before = h.app.editor.document.model().clone();
                let stats = h.app.editor.document.history_stats();
                let resolved = before
                    .resolve_opening(&before.openings[&id].parameters)
                    .unwrap();
                let opposite = if resolved.pane_position == LeftFace {
                    RightFace
                } else {
                    LeftFace
                };
                let pos = h
                    .direct_door_button("Side")
                    .expect("off-center window side control");
                h.hover(pos);
                h.frame(vec![button(pos, true)]);
                assert!(h.app.plans.opening_flip.is_some());
                assert_eq!(h.app.editor.document.model(), &before);
                h.frame(vec![button(pos, false)]);
                h.settle();
                let mut expected = before.clone();
                expected
                    .openings
                    .get_mut(&id)
                    .unwrap()
                    .parameters
                    .pane_position_override = Some(opposite);
                assert_eq!(h.app.editor.document.model(), &expected);
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    stats.undo_entries + 1
                );
                h.app.history(false);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &before);
                h.app.history(true);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &expected);
            }
        }
        let (mut h, id) = fixture(size, scale, Center);
        assert!(h.direct_door_button("Side").is_none());
        // Explicit Center suppresses a non-centered type too.
        let mut p = h.app.editor.document.model().openings[&id]
            .parameters
            .clone();
        p.pane_position_override = Some(Center);
        let ty = p.type_id().unwrap();
        h.app
            .editor
            .command("Pin center", Command::UpdateOpening { id, parameters: p })
            .unwrap();
        let mut parameters = h.app.editor.document.model().opening_types[&ty]
            .parameters
            .clone();
        parameters.pane_position = LeftFace;
        h.app
            .editor
            .command(
                "Default left",
                Command::UpdateOpeningType { id: ty, parameters },
            )
            .unwrap();
        h.settle();
        assert!(h.direct_door_button("Side").is_none());
        let (mut h, _) = Harness::movable(OpeningKind::Window, false, true, size, scale);
        h.use_move_anchor(0.);
        assert!(h.direct_door_button("Side").is_none());
    }
}

#[test]
fn window_side_flip_cancel_and_stale_preserve_model_and_history_both_dpis() {
    for (size, scale) in PROFILES {
        for reason in [
            "escape",
            "drag",
            "revision",
            "session",
            "selection",
            "drawing",
            "provider",
            "lost",
            "camera",
            "tool",
        ] {
            let (mut h, _) = fixture(size, scale, LeftFace);
            let pos = h.direct_door_button("Side").unwrap();
            h.hover(pos);
            h.frame(vec![button(pos, true)]);
            assert!(h.app.plans.opening_flip.is_some());
            match reason {
                "escape" => h.frame(vec![escape()]),
                "drag" => h.hover(pos + egui::vec2(25., 20.)),
                "revision" => h
                    .app
                    .editor
                    .command("Other edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "selection" => h.app.select(Some(h.wall)),
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "lost" => h.frame(vec![egui::Event::PointerGone]),
                "camera" => {
                    h.app
                        .plans
                        .cameras
                        .get_mut(&h.view)
                        .unwrap()
                        .pixels_per_metre += 1.
                }
                "tool" => h.app.begin_opening_rehost(),
                _ => unreachable!(),
            }
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.hover(pos);
            h.frame(vec![button(pos, false)]);
            h.frame(vec![]);
            assert!(h.app.plans.opening_flip.is_none(), "{reason}");
            assert_eq!(h.app.editor.document.model(), &before, "{reason}");
            assert_eq!(h.app.editor.document.history_stats(), history, "{reason}");
        }
    }
}
