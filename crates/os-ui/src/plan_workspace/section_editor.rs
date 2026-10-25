//! Disposable exact-input section settings and placement review.
use super::*;

impl DesktopApp {
    pub(super) fn validate_section_editor(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.plans.section_pointer_claimed = false;
        } else if self.plans.section_placement.is_some() {
            self.plans.section_pointer_claimed = true;
        }
        if self.plans.section_placement.as_ref().is_some_and(|d| {
            self.plans.active != Some(d.context.view_id)
                || self.editor.document.session_id() != d.session
                || self.editor.document.revision() != d.revision
                || self.plans.active_sheet.is_some()
                || self.editor.native_view_context(d.context.view_id).ok() != Some(d.context)
                || plan_provider_signature(&self.editor) != d.provider_signature
                || ctx.input(|i| {
                    i.key_pressed(egui::Key::Escape)
                        || i.events.iter().any(|e| {
                            matches!(
                                e,
                                egui::Event::PointerGone | egui::Event::WindowFocused(false)
                            )
                        })
                })
        }) {
            self.plans.section_placement = None;
        }
    }

    pub(super) fn begin_section_settings(&mut self, id: Id) -> Result<()> {
        let context = self.editor.native_section_context(id)?;
        let settings = self.editor.document.model().views[&id]
            .parameters
            .section
            .unwrap();
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.crop.cancel();
        self.plans.section_callouts.draft = None;
        self.plans.section_placement = Some(SectionPlacementDraft {
            #[cfg(test)]
            numeric_rects: BTreeMap::new(),
            context,
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            provider_signature: plan_provider_signature(&self.editor),
            first: Some(settings.start),
            bottom_elevation: settings.bottom_elevation,
            top_elevation: settings.top_elevation,
            settings: Some(settings),
            editing: Some(id),
        });
        Ok(())
    }

    pub(super) fn section_settings_toolbar(&mut self, ui: &mut egui::Ui) {
        if self.plans.section_placement.is_none()
            && let Some(id) = self.plans.active
            && self
                .editor
                .document
                .model()
                .views
                .get(&id)
                .is_some_and(|v| v.parameters.section.is_some())
            && ui.button("Section settings").clicked()
        {
            let result = self.begin_section_settings(id);
            self.report(
                result,
                "Edit section settings; Apply commits one transaction.",
            );
        }
        let Some(mut draft) = self.plans.section_placement.take() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        if let Some(s) = &mut draft.settings {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut s.side, os_model::ElevationSide::Left, "View from left");
                ui.selectable_value(
                    &mut s.side,
                    os_model::ElevationSide::Right,
                    "View from right",
                );
                ui.selectable_value(
                    &mut s.mode,
                    os_model::SectionDisplayMode::CutOnly,
                    "Cut only",
                );
                ui.selectable_value(
                    &mut s.mode,
                    os_model::SectionDisplayMode::CutAndProjection,
                    "Cut and background",
                );
            });
            egui::Grid::new("section_exact_fields")
                .num_columns(4)
                .spacing(egui::vec2(8., 4.))
                .show(ui, |ui| {
                    for (index, (label, value)) in [
                        ("Start X", &mut s.start.x),
                        ("Start Y", &mut s.start.y),
                        ("End X", &mut s.end.x),
                        ("End Y", &mut s.end.y),
                        ("Crop start", &mut s.min_station),
                        ("Crop end", &mut s.max_station),
                        ("Bottom", &mut s.bottom_elevation),
                        ("Top", &mut s.top_elevation),
                        ("Depth", &mut s.far_depth),
                        ("Scale 1:", &mut s.scale_denominator),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.label(label);
                        let _response = ui
                            .push_id(label, |ui| {
                                ui.add(egui::DragValue::new(value).speed(0.1).max_decimals(6))
                            })
                            .inner;
                        #[cfg(test)]
                        draft.numeric_rects.insert(label, _response.rect);
                        if index % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            let valid = s.validate();
            if let Err(e) = &valid {
                ui.colored_label(theme::ERROR, e.to_string());
            }
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Apply section"))
                    .clicked();
                cancel = ui.button("Discard section edits").clicked();
            });
        }
        let settings = draft.settings;
        self.plans.section_placement = (!cancel).then_some(draft);
        if apply && let Some(settings) = settings {
            match self.commit_section_placement(settings.start, settings.end) {
                Ok(id) => {
                    self.plans.section_placement = None;
                    self.focus_plan(Some(id));
                    self.report(Ok(()), "Section settings applied in one transaction.");
                }
                Err(e) => self.report(Err(e), ""),
            }
        }
    }
}
