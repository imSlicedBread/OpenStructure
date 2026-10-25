//! Per-view instance exclusions; restore edits are disposable until Apply.
use super::*;
use os_core::ensure;
use std::collections::BTreeSet;

#[cfg(test)]
mod tests;

pub(crate) struct Draft {
    view: Id,
    session: Id,
    revision: u64,
    hidden: BTreeSet<Id>,
    selected: BTreeSet<Id>,
    error: Option<String>,
}

impl DesktopApp {
    fn hidden_openings_view(&self) -> Option<Id> {
        self.plans.active.filter(|view| {
            self.plans.active_sheet.is_none()
                && self.editor.document.model().is_native_floor_plan(*view)
        })
    }

    fn hide_selected_openings(&mut self) -> Result<()> {
        let view = self
            .hidden_openings_view()
            .ok_or_else(|| Error::Invalid("Open a native floor plan".into()))?;
        ensure(
            (1..=256).contains(&self.selected_ids.len()),
            "Select 1–256 visible doors or windows",
        )?;
        ensure(
            !self.editor.plugin_work_pending(),
            "Wait for pending document work",
        )?;
        // Resolve a fresh drawing, not the asynchronously cached canvas snapshot.
        let context = self.editor.native_plan_context(view)?;
        let drawing = self.editor.native_drawing(view)?;
        let visible: BTreeSet<_> = drawing
            .provider_lines(context)?
            .iter()
            .map(|line| line.entity)
            .collect();
        let model = self.editor.document.model();
        let mut kind = None;
        let mut hidden = model
            .plan_hidden_openings
            .get(&view)
            .cloned()
            .unwrap_or_default();
        for id in &self.selected_ids {
            ensure(
                model.plan_hidden_opening_compatible(view, *id) && visible.contains(id),
                format!("Opening {id} must be visible in the active floor plan"),
            )?;
            let resolved = model.resolve_opening(&model.openings[id].parameters)?;
            ensure(
                kind.is_none_or(|kind| kind == resolved.kind),
                "Select doors or windows, not mixed categories",
            )?;
            kind = Some(resolved.kind);
            hidden.insert(*id);
        }
        self.editor.command(
            "Hide openings in plan",
            Command::SetPlanHiddenOpenings { view, hidden },
        )?;
        self.cancel_plan_authoring_for_modal();
        self.select(None);
        Ok(())
    }

    fn begin_hidden_openings(&mut self) {
        let Some(view) = self.hidden_openings_view() else {
            return;
        };
        self.cancel_plan_authoring_for_modal();
        self.plans.hidden_openings = Some(Draft {
            view,
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            hidden: self
                .editor
                .document
                .model()
                .plan_hidden_openings
                .get(&view)
                .cloned()
                .unwrap_or_default(),
            selected: BTreeSet::new(),
            error: None,
        });
    }

    pub(super) fn hidden_opening_controls(&mut self, ui: &mut egui::Ui) {
        let enabled = self.hidden_openings_view().is_some();
        if ui
            .add_enabled(
                enabled && !self.selected_ids.is_empty(),
                egui::Button::new("Hide openings"),
            )
            .clicked()
        {
            let result = self.hide_selected_openings();
            self.report(result, "Selected openings hidden in this plan.");
        }
        if ui
            .add_enabled(enabled, egui::Button::new("Hidden openings…"))
            .clicked()
        {
            self.begin_hidden_openings();
        }
    }

    pub(crate) fn hidden_openings_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.plans.hidden_openings.take() else {
            return;
        };
        if self.hidden_openings_view() != Some(draft.view)
            || self.editor.document.session_id() != draft.session
            || self.editor.document.revision() != draft.revision
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            return;
        }
        let mut close = false;
        egui::Modal::new(egui::Id::new("hidden_openings")).show(ctx, |ui| {
            ui.set_width(360.);
            ui.heading("Hidden openings");
            ui.label(format!("{} openings remain hidden", draft.hidden.len()));
            ui.label(
                "Restore clears this hide flag. Category, phase, range and crop rules still apply.",
            );
            egui::ScrollArea::vertical()
                .max_height(200.)
                .show(ui, |ui| {
                    let model = self.editor.document.model();
                    for id in &draft.hidden {
                        let opening = &model.openings[id];
                        let kind = model.resolve_opening(&opening.parameters).map(|o| o.kind);
                        let mut selected = draft.selected.contains(id);
                        if ui
                            .checkbox(
                                &mut selected,
                                format!(
                                    "{} · {:?}\n{}",
                                    opening.parameters.name,
                                    kind.unwrap(),
                                    id
                                ),
                            )
                            .changed()
                        {
                            if selected {
                                draft.selected.insert(*id);
                            } else {
                                draft.selected.remove(id);
                            }
                        }
                    }
                });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !draft.selected.is_empty(),
                        egui::Button::new("Restore selected"),
                    )
                    .clicked()
                {
                    draft.hidden.retain(|id| !draft.selected.contains(id));
                    draft.selected.clear();
                }
                if ui
                    .add_enabled(!draft.hidden.is_empty(), egui::Button::new("Restore all"))
                    .clicked()
                {
                    draft.hidden.clear();
                    draft.selected.clear();
                }
            });
            if let Some(error) = &draft.error {
                ui.colored_label(theme::ERROR, error);
            }
            ui.horizontal(|ui| {
                if ui.button("Cancel restore").clicked() {
                    close = true;
                }
                if ui.button("Apply restore").clicked() {
                    let result = self.editor.command(
                        "Restore plan openings",
                        Command::SetPlanHiddenOpenings {
                            view: draft.view,
                            hidden: draft.hidden.clone(),
                        },
                    );
                    match result {
                        Ok(()) => {
                            self.status = "Opening hide flags updated.".into();
                            close = true;
                        }
                        Err(error) => draft.error = Some(error.to_string()),
                    }
                }
            });
        });
        if !close {
            self.plans.hidden_openings = Some(draft);
        }
    }
}
