//! Disposable standalone view settings; all ownership guards remain document-owned.
use super::*;
pub(super) struct Draft {
    id: Id,
    session: Id,
    revision: u64,
    parameters: os_model::ViewParams,
}
impl DesktopApp {
    pub(super) fn active_drafting_view(&self) -> Option<Id> {
        self.plans.active.filter(|id| {
            self.editor
                .document
                .model()
                .views
                .get(id)
                .is_some_and(|v| v.parameters.kind == os_model::ViewKind::Drafting)
        })
    }
    pub(super) fn drafting_view_toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.active_drafting_view() else {
            return;
        };
        ui.label("Drafting XY · annotations only");
        if ui.button("Drafting settings").clicked() {
            self.plans.detail_line_draft = None;
            self.plans.text_notes.draft = None;
            self.plans.drafting_view = Some(Draft {
                id,
                session: self.editor.document.session_id(),
                revision: self.editor.document.revision(),
                parameters: self.editor.document.model().views[&id].parameters.clone(),
            });
        }
        if ui.button("Delete drafting view").clicked() {
            let result = self
                .editor
                .command("Delete drafting view", Command::RemoveView(id));
            if result.is_ok() {
                self.focus_plan(None);
            }
            self.report(result, "Drafting view removed.");
        }
    }
    pub(super) fn drafting_view_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.plans.drafting_view.take() else {
            return;
        };
        if self.plans.active != Some(draft.id)
            || draft.session != self.editor.document.session_id()
            || draft.revision != self.editor.document.revision()
            || ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::WindowFocused(false)))
            })
        {
            return;
        }
        let mut close = false;
        egui::Modal::new(egui::Id::new("drafting_settings")).show(ctx, |ui| {
            ui.set_width(370.);
            ui.heading("Drafting view settings");
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut draft.parameters.name).char_limit(256));
            let settings = draft.parameters.drafting.as_mut().unwrap();
            ui.label("Rectangular crop · drafting XY metres");
            for (label, value) in [
                ("Minimum X", &mut settings.crop.min.x),
                ("Minimum Y", &mut settings.crop.min.y),
                ("Maximum X", &mut settings.crop.max.x),
                ("Maximum Y", &mut settings.crop.max.y),
                ("Scale 1:", &mut settings.scale_denominator),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.1));
                });
            }
            let valid = draft.parameters.validate_edit();
            if let Err(error) = &valid {
                ui.colored_label(theme::ERROR, error.to_string());
            }
            ui.horizontal(|ui| {
                if ui.button("Cancel drafting settings").clicked() {
                    close = true;
                }
                if ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Apply drafting settings"))
                    .clicked()
                {
                    let result = self.editor.command(
                        "Drafting view settings",
                        Command::UpdateView {
                            id: draft.id,
                            parameters: draft.parameters.clone(),
                        },
                    );
                    close = result.is_ok();
                    self.report(result, "Drafting settings saved.");
                }
            });
        });
        if !close {
            self.plans.drafting_view = Some(draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::sheet_tests::raw_input;
    use super::*;
    struct Harness {
        app: DesktopApp,
        ctx: egui::Context,
        size: egui::Vec2,
        dpi: f32,
        time: f64,
    }
    impl Harness {
        fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            self.time += 0.1;
            self.ctx
                .run(raw_input(self.size, self.dpi, self.time, events), |ctx| {
                    self.app.show(ctx)
                })
        }
        fn click_label(&mut self, label: &str) {
            self.frame(vec![]);
            let out = self.frame(vec![]);
            let p = out
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == label => {
                        Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"));
            self.frame(vec![egui::Event::PointerMoved(p)]);
            for pressed in [true, false] {
                self.frame(vec![egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }]);
            }
        }
    }
    #[test]
    fn drafting_settings_real_egui_create_cancel_apply_stale_history_and_delete_both_dpis() {
        for (size, dpi) in [
            (egui::vec2(1280., 800.), 1.),
            (egui::vec2(1000., 650.), 1.5),
        ] {
            let mut h = Harness {
                app: DesktopApp::new().unwrap(),
                ctx: egui::Context::default(),
                size,
                dpi,
                time: 0.,
            };
            theme::apply(&h.ctx);
            h.click_label("New drafting view");
            let id = h.app.active_drafting_view().unwrap();
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.click_label("Drafting settings");
            h.app.plans.drafting_view.as_mut().unwrap().parameters.name = "Canceled".into();
            h.click_label("Cancel drafting settings");
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            h.click_label("Drafting settings");
            let p = &mut h.app.plans.drafting_view.as_mut().unwrap().parameters;
            p.name = "Renamed detail".into();
            let s = p.drafting.as_mut().unwrap();
            s.crop.min = Point2::new(-3., -4.);
            s.scale_denominator = 25.;
            assert_eq!(h.app.editor.document.model(), &before);
            h.click_label("Apply drafting settings");
            let after = h.app.editor.document.model().clone();
            assert_eq!(after.views[&id].parameters.name, "Renamed detail");
            assert_eq!(
                after.views[&id]
                    .parameters
                    .drafting
                    .unwrap()
                    .scale_denominator,
                25.
            );
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &after);
            h.click_label("Drafting settings");
            h.app
                .editor
                .command("External", Command::RenameProject("External".into()))
                .unwrap();
            let external = h.app.editor.document.model().clone();
            h.frame(vec![]);
            assert!(h.app.plans.drafting_view.is_none());
            assert_eq!(h.app.editor.document.model(), &external);
            h.click_label("Drafting settings");
            h.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(h.app.plans.drafting_view.is_none());
            assert_eq!(h.app.editor.document.model(), &external);
            h.click_label("Delete drafting view");
            assert!(!h.app.editor.document.model().views.contains_key(&id));
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &external);
        }
    }
}
