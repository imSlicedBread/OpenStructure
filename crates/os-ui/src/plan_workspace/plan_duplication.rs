use super::*;
use os_document::PlanDuplicateMode;

pub(crate) struct Draft {
    source: Id,
    session: Id,
    revision: u64,
    name: String,
    mode: PlanDuplicateMode,
    error: Option<String>,
}

#[cfg(test)]
mod tests;

impl DesktopApp {
    fn begin_plan_duplicate(&mut self, source: Id) {
        let Ok(name) = self.editor.document.plan_duplicate_name(source) else {
            return;
        };
        self.cancel_plan_authoring_for_modal();
        self.plans.plan_duplicate = Some(Draft {
            source,
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            name,
            mode: PlanDuplicateMode::Plain,
            error: None,
        });
    }

    pub(super) fn cancel_plan_authoring_for_modal(&mut self) {
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.cancel_opening_controls();
        self.plans.area_selection.cancel();
        self.plans.overlap_selection.cancel();
        self.plans.crop = crop::State::default();
        self.plans.callouts = callouts::State::default();
        self.plans.section_callouts = section_callouts::State::default();
        self.plans.text_notes = text_notes::State::default();
        self.plans.detail_line_draft = None;
        self.plans.room_tag_draft = None;
        self.plans.opening_tag_draft = None;
        self.plans.room_separation_line_draft = None;
        self.plans.endpoint_drag = None;
        self.plans.junction_drag = None;
        self.plans.transform = None;
        self.plans.wall_set_move = None;
        self.plans.wall_level_copy = None;
        self.plans.column_placement = None;
        self.plans.casework_placement = None;
        self.plans.floor_sketch = None;
        self.plans.floor_hole_sketch = None;
        self.plans.floor_vertex_drag = None;
        self.plans.curtain_draft = None;
        self.plans.stair_placement = None;
        self.plans.ramp_placement = None;
        self.plans.roof_draft = None;
        self.plans.ceiling_draft = None;
        self.plans.section_placement = None;
        self.plans.section_dimension = None;
        self.plans.facade = None;
        self.plans.elevation.cancel();
        self.plans.room_placement_active = false;
        self.plan_draft = None;
        #[cfg(feature = "external-plugins")]
        self.editor.cancel_plugin_tool();
    }

    pub(super) fn plan_duplicate_controls(&mut self, ui: &mut egui::Ui) {
        let source = self.plans.active.filter(|id| {
            self.plans.active_sheet.is_none()
                && self.editor.document.plan_duplicate_counts(*id).is_ok()
        });
        if ui
            .add_enabled(source.is_some(), egui::Button::new("Duplicate plan…"))
            .clicked()
        {
            self.begin_plan_duplicate(source.unwrap());
        }
        if ui.add_enabled(source.is_some(), egui::Button::new("Delete plan"))
            .on_hover_text("Remove this plan and its owned annotations; referenced or issued plans are protected.")
            .clicked()
        {
            let result = self.editor.document.delete_plan(source.unwrap());
            if result.is_ok() { self.focus_plan(None); }
            self.report(result, "Plan and owned detailing removed.");
        }
    }

    pub(crate) fn plan_duplicate_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.plans.plan_duplicate.take() else {
            return;
        };
        if self.plans.active != Some(draft.source)
            || self.plans.active_sheet.is_some()
            || draft.session != self.editor.document.session_id()
            || draft.revision != self.editor.document.revision()
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            return;
        }
        let Ok(counts) = self.editor.document.plan_duplicate_counts(draft.source) else {
            return;
        };
        let mut close = false;
        egui::Modal::new(egui::Id::new("duplicate_plan")).show(ctx, |ui| {
            ui.set_width(350.);
            ui.heading("Duplicate plan");
            ui.label("Name");
            if ui.text_edit_singleline(&mut draft.name).changed() { draft.error = None; }
            ui.radio_value(&mut draft.mode, PlanDuplicateMode::Plain, "Duplicate");
            ui.radio_value(&mut draft.mode, PlanDuplicateMode::WithDetailing, "Duplicate with detailing");
            if draft.mode == PlanDuplicateMode::Plain {
                ui.label("Settings and graphics binding only. 0 annotations copied.");
            } else {
                ui.label(format!("{} dimensions · {} opening tags · {} room tags\n{} detail lines · {} text notes · {} callouts",
                    counts.dimensions, counts.opening_tags, counts.room_tags,
                    counts.detail_lines, counts.text_notes, counts.callouts));
                ui.label("Includes hidden and off-crop annotations.");
                if counts.callouts > 0 {
                    ui.label("All copied callouts become reference-only markers pointing to their existing targets.");
                }
            }
            ui.label("The new plan is independent and has no sheet placement.");
            let valid = self.editor.document.validate_plan_duplicate_name(&draft.name);
            if let Err(e) = &valid { ui.colored_label(theme::ERROR, e.to_string()); }
            if let Some(e) = &draft.error { ui.colored_label(theme::ERROR, e); }
            ui.horizontal(|ui| {
                if ui.button("Cancel duplication").clicked() { close = true; }
                if ui.add_enabled(valid.is_ok(), egui::Button::new("Apply duplication")).clicked() {
                    let result = self.editor.document.prepare_plan_duplicate(draft.source, draft.name.clone(), draft.mode)
                        .and_then(|prepared| self.editor.document.apply_plan_duplicate(prepared));
                    match result {
                        Ok(id) => {
                            self.focus_plan(Some(id));
                            self.select(None);
                            self.status = "Independent plan created.".into();
                            close = true;
                        }
                        Err(e) => draft.error = Some(e.to_string()),
                    }
                }
            });
        });
        if !close {
            self.plans.plan_duplicate = Some(draft);
        }
    }
}
