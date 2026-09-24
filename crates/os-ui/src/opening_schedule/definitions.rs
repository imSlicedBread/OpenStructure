use super::*;
use os_document::Command;
use os_model::Schedule;

pub(super) struct DefinitionDraft {
    id: Option<Id>,
    session: Id,
    revision: u64,
    parameters: ScheduleParams,
    error: Option<String>,
}

#[cfg(test)]
mod tests;

impl DesktopApp {
    pub(super) fn refresh_schedule_definition(&mut self, ctx: &egui::Context) {
        let session = self.editor.document.session_id();
        if self.opening_schedule.session != Some(session) {
            self.opening_schedule.selected = None;
            self.opening_schedule.session = Some(session);
        }
        if self
            .opening_schedule
            .selected
            .is_some_and(|id| !self.editor.document.model().schedules.contains_key(&id))
        {
            self.opening_schedule.selected = None;
        }
        let stale = self
            .opening_schedule
            .definition_draft
            .as_ref()
            .is_some_and(|d| d.session != session || d.revision != self.editor.document.revision());
        if stale || !self.opening_schedule.open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.opening_schedule.definition_draft = None;
            if stale {
                self.status = "Schedule configuration cancelled: document changed.".into();
            }
        }
    }

    fn begin_definition(&mut self, category: Option<ScheduleCategory>) {
        let (id, parameters) = if let Some(category) = category {
            let base = match category {
                ScheduleCategory::Door => "Door schedule",
                ScheduleCategory::Window => "Window schedule",
                ScheduleCategory::All => "Opening schedule",
            };
            let names: Vec<_> = self
                .editor
                .document
                .model()
                .schedules
                .values()
                .map(|s| s.parameters.name.to_lowercase())
                .collect();
            let name = (1..)
                .map(|i| format!("{base} {i}"))
                .find(|n| !names.contains(&n.to_lowercase()))
                .unwrap();
            (None, ScheduleParams::new(name, category))
        } else if let Some(schedule) = self
            .opening_schedule
            .selected
            .and_then(|id| self.editor.document.model().schedules.get(&id))
        {
            (Some(schedule.id()), schedule.parameters.clone())
        } else {
            return;
        };
        self.opening_schedule.draft = None;
        self.opening_schedule.definition_draft = Some(DefinitionDraft {
            id,
            parameters,
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            error: None,
        });
    }

    fn apply_definition(&mut self) {
        let Some(mut draft) = self.opening_schedule.definition_draft.take() else {
            return;
        };
        if draft.session != self.editor.document.session_id()
            || draft.revision != self.editor.document.revision()
        {
            self.status = "Schedule configuration cancelled: document changed.".into();
            return;
        }
        let (id, command) = if let Some(id) = draft.id {
            (
                id,
                Command::UpdateSchedule {
                    id,
                    parameters: draft.parameters.clone(),
                },
            )
        } else {
            let schedule = Schedule::new("core.schedule", draft.parameters.clone());
            (schedule.id(), Command::AddSchedule(schedule))
        };
        match self.editor.command("Save schedule definition", command) {
            Ok(()) => {
                self.opening_schedule.selected = Some(id);
                self.status = "Schedule saved.".into();
            }
            Err(error) => {
                draft.error = Some(error.to_string());
                self.opening_schedule.definition_draft = Some(draft);
            }
        }
    }

    pub(super) fn schedule_definition_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("saved_schedule_picker")
                .selected_text(
                    self.opening_schedule
                        .selected
                        .and_then(|id| self.editor.document.model().schedules.get(&id))
                        .map(|s| s.parameters.name.as_str())
                        .unwrap_or("All openings (live)"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.opening_schedule.selected,
                        None,
                        "All openings (live)",
                    );
                    for schedule in self.editor.document.model().schedules.values() {
                        ui.selectable_value(
                            &mut self.opening_schedule.selected,
                            Some(schedule.id()),
                            &schedule.parameters.name,
                        );
                    }
                });
            if ui.button("New Door schedule").clicked() {
                self.begin_definition(Some(ScheduleCategory::Door));
            }
            if ui.button("New Window schedule").clicked() {
                self.begin_definition(Some(ScheduleCategory::Window));
            }
            if ui
                .add_enabled(
                    self.opening_schedule.selected.is_some(),
                    egui::Button::new("Configure schedule"),
                )
                .clicked()
            {
                self.begin_definition(None);
            }
            if ui
                .add_enabled(
                    self.opening_schedule.selected.is_some(),
                    egui::Button::new("Delete schedule"),
                )
                .clicked()
                && let Some(id) = self.opening_schedule.selected
            {
                let result = self
                    .editor
                    .command("Remove schedule", Command::RemoveSchedule(id));
                self.report(result, "Schedule removed. Undo to restore.");
                self.opening_schedule.selected = None;
                self.opening_schedule.definition_draft = None;
            }
        });
        let mut apply = false;
        let mut cancel = false;
        if let Some(draft) = &mut self.opening_schedule.definition_draft {
            ui.label("Schedule name");
            ui.text_edit_singleline(&mut draft.parameters.name);
            ui.horizontal_wrapped(|ui| {
                for category in [
                    ScheduleCategory::Door,
                    ScheduleCategory::Window,
                    ScheduleCategory::All,
                ] {
                    ui.selectable_value(
                        &mut draft.parameters.category,
                        category,
                        format!("{category:?}"),
                    );
                }
                egui::ComboBox::from_id_salt("schedule_sort")
                    .selected_text(format!("Sort: {:?}", draft.parameters.sort))
                    .show_ui(ui, |ui| {
                        for sort in ScheduleSort::ALL {
                            ui.selectable_value(
                                &mut draft.parameters.sort,
                                sort,
                                format!("{sort:?}"),
                            );
                        }
                    });
            });
            ui.label("Columns · check to include, use arrows to reorder");
            ui.horizontal_wrapped(|ui| {
                for column in ScheduleColumn::ALL {
                    let mut enabled = draft.parameters.columns.contains(&column);
                    if ui.checkbox(&mut enabled, column.label()).changed() {
                        if enabled {
                            draft.parameters.columns.push(column);
                        } else {
                            draft.parameters.columns.retain(|c| *c != column);
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                for index in 0..draft.parameters.columns.len() {
                    ui.push_id(index, |ui| {
                        ui.label(draft.parameters.columns[index].label());
                        if ui.add_enabled(index > 0, egui::Button::new("←")).clicked() {
                            draft.parameters.columns.swap(index, index - 1);
                        }
                    });
                }
            });
            if let Some(error) = &draft.error {
                ui.colored_label(crate::theme::ERROR, error);
            }
            ui.horizontal(|ui| {
                apply = ui.button("Save definition").clicked();
                cancel = ui.button("Cancel definition").clicked();
            });
        }
        if cancel {
            self.opening_schedule.definition_draft = None;
        }
        if apply {
            self.apply_definition();
        }
        ui.separator();
    }
}
