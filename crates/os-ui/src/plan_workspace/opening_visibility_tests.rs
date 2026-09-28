use super::*;

impl Harness {
    fn visibility_text(&self, label: &str) -> Option<egui::Pos2> {
        self.output.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == label => {
                let rect = egui::Rect::from_min_size(t.pos, t.galley.size());
                s.clip_rect.contains_rect(rect).then_some(rect.center())
            }
            _ => None,
        })
    }
    fn visibility_click(&mut self, label: &str) {
        for _ in 0..30 {
            if let Some(pos) = self.visibility_text(label) {
                self.click(pos);
                return;
            }
            let pos = self.visibility_text("Offsets are relative to the level. Rotation is horizontal yaw; scale is 1:N, not screen zoom.")
                .unwrap_or(egui::pos2(self.size.x/2.,self.size.y/2.));
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -150.),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            for _ in 0..8 {
                self.frame(vec![]);
            }
        }
        panic!("visibility control not visible: {label}");
    }
    fn visibility_settings(&mut self) {
        self.app.plan_draft =
            Some(crate::plan_settings::PlanDraft::begin(&self.app.editor, self.view).unwrap());
        self.frame(vec![]);
        self.frame(vec![]);
    }
}

#[test]
fn opening_visibility_desktop_apply_cancel_escape_history_and_hidden_tools() {
    for (size, scale) in PROFILES {
        for kind in KINDS {
            let (mut h, id) = Harness::movable(kind, false, true, size, scale);
            let label = if kind == OpeningKind::Door {
                "Show doors"
            } else {
                "Show windows"
            };
            h.app.select(Some(id));
            let original = h.app.editor.document.model().clone();
            let original_scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats().undo_entries;
            h.visibility_settings();
            h.visibility_click(label);
            assert_eq!(h.app.editor.document.model(), &original);
            h.visibility_click("Cancel plan settings");
            assert_eq!(h.app.editor.document.model(), &original);
            h.visibility_settings();
            h.visibility_click(label);
            h.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(h.app.plan_draft.is_none());
            assert_eq!(h.app.editor.document.model(), &original);
            h.visibility_settings();
            h.visibility_click(label);
            h.visibility_click("Apply plan settings");
            h.settle();
            let saved = h.app.editor.document.model().clone();
            assert!(
                !saved.views[&h.view]
                    .parameters
                    .plan
                    .unwrap()
                    .visibility
                    .shows_opening(kind)
            );
            assert_eq!(saved.openings, original.openings);
            assert_eq!(saved.walls, original.walls);
            assert_eq!(h.app.editor.scene, original_scene);
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history + 1
            );
            assert_eq!(h.app.selected, None);
            assert!(!h.app.plans.drawing.as_ref().unwrap().is_native_line(id));
            assert!(h.app.plans.opening_move.is_none());
            assert!(h.app.plans.opening_flip.is_none());
            assert!(h.app.plans.opening_spacing.is_none());
            h.app.begin_opening_placement(kind);
            assert!(h.app.plans.opening_placement.is_none());
            // Even a stale/manual selection cannot start exact editing or rehost.
            h.app.select(Some(id));
            h.app.begin_opening(None);
            assert!(h.app.opening_draft.is_none());
            h.app.begin_opening_rehost();
            assert!(h.app.plans.opening_rehost.is_none());
            h.app.editor.undo().unwrap();
            assert_eq!(h.app.editor.document.model(), &original);
            h.app.editor.redo().unwrap();
            assert_eq!(h.app.editor.document.model(), &saved);
        }
    }
}
