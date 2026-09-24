use super::*;

#[test]
fn perpendicular_join_inspector_at_supported_dpi_profiles() {
    for tee in [false, true] {
        for (size, scale) in [
            (egui::vec2(1280., 800.), 1.),
            (egui::vec2(1000., 650.), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            h.app.draft.start = if tee {
                Point2::new(2., 3.)
            } else {
                Point2::new(0., 0.)
            };
            h.app.draft.end = if tee {
                Point2::new(2., 0.)
            } else {
                Point2::new(3., 0.)
            };
            h.app.apply_wall();
            let a = h.app.selected.unwrap();
            h.app.select(None);
            h.app.draft.name = "B".into();
            h.app.draft.start = if tee {
                Point2::new(0., 0.)
            } else {
                Point2::new(3., 0.)
            };
            h.app.draft.end = if tee {
                Point2::new(4., 0.)
            } else {
                Point2::new(3., 4.)
            };
            h.app.apply_wall();
            h.app.select(Some(a));
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let label = if tee {
                "T-join End to B"
            } else {
                "Corner End to B Start"
            };
            h.click("Wall end joins");
            for _ in 0..10 {
                if h.visible_text_rect(label).is_some() {
                    break;
                }
                h.frame(vec![
                    egui::Event::PointerMoved(egui::pos2(160., 230.)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., -45.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                for _ in 0..4 {
                    h.frame(vec![]);
                }
            }
            assert!(h.visible_text_rect(label).is_some(), "{label} at {size:?}");
            h.click(label);
            assert_eq!(
                h.app.editor.document.model().wall_joins.len(),
                1,
                "{}",
                h.app.status
            );
            assert_eq!(h.app.editor.document.model().walls, before.walls);
            h.click("Undo");
            assert_eq!(h.app.editor.document.model(), &before);
            h.click("Redo");
            h.click("Unjoin End");
            assert_eq!(h.app.editor.document.model(), &before);
        }
    }
}

#[test]
fn butt_join_inspector_actions_at_supported_dpi_profiles() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        h.app.draft.start = Point2::new(0., 0.);
        h.app.draft.end = Point2::new(3., 0.);
        h.app.apply_wall();
        let a = h.app.selected.unwrap();
        h.app.select(None);
        h.app.draft.name = "B".into();
        h.app.draft.start = Point2::new(3., 0.);
        h.app.draft.end = Point2::new(7., 0.);
        h.app.apply_wall();
        h.app.select(Some(a));
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        h.click("Wall end joins");
        for _ in 0..8 {
            if h.visible_text_rect("Join End to B Start").is_some() {
                break;
            }
            let pointer = egui::pos2(160.0, 230.0);
            h.frame(vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -45.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            for _ in 0..4 {
                h.frame(vec![]);
            }
        }
        assert!(
            h.visible_text_rect("Join End to B Start").is_some(),
            "compatible join action must be visible at {size:?}"
        );
        h.click("Join End to B Start");
        assert_eq!(
            h.app.editor.document.model().wall_joins.len(),
            1,
            "{}",
            h.app.status
        );
        assert_eq!(h.app.editor.document.model().walls, before.walls);
        h.click("Undo");
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Redo");
        h.click("Unjoin End");
        assert_eq!(h.app.editor.document.model(), &before);
    }
}
