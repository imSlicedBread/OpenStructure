//! Local, revision-bound phase and lifecycle drafts. No plan filtering is applied.
use super::*;
use os_model::{ElementLifecycle, MAX_PHASES, Phase};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Context {
    session: Id,
    revision: u64,
}
impl Context {
    fn new(document: &Document) -> Self {
        Self {
            session: document.session_id(),
            revision: document.revision(),
        }
    }
    fn check(self, document: &Document) -> Result<()> {
        os_core::ensure(
            self == Self::new(document),
            "Document changed; cancel and reopen phase editing",
        )
    }
}

pub(super) struct PhaseDraft {
    context: Context,
    phases: Vec<Phase>,
    selected: usize,
    error: Option<String>,
}

#[derive(Clone, Copy)]
enum PhaseAction {
    Add,
    Up,
    Down,
    Delete,
}

impl PhaseDraft {
    fn edit(&mut self, document: &Document, action: PhaseAction) -> Result<()> {
        self.context.check(document)?;
        let index = self.selected;
        match action {
            PhaseAction::Add => {
                os_core::ensure(
                    self.phases.len() < MAX_PHASES,
                    "Project requires 1 to 64 phases",
                )?;
                let mut suffix = self.phases.len() + 1;
                let name = loop {
                    let name = format!("Phase {suffix}");
                    if !self
                        .phases
                        .iter()
                        .any(|p| p.parameters.name.eq_ignore_ascii_case(&name))
                    {
                        break name;
                    }
                    suffix += 1;
                };
                self.phases
                    .push(os_model::new_phase(name, self.phases.len() as u32));
                self.selected = self.phases.len() - 1;
            }
            PhaseAction::Up | PhaseAction::Down => {
                let other = if matches!(action, PhaseAction::Up) {
                    index.checked_sub(1)
                } else {
                    index.checked_add(1)
                };
                os_core::ensure(
                    index > 0 && other.is_some_and(|i| i > 0 && i < self.phases.len()),
                    "The first phase is pinned; move only adjacent non-first phases",
                )?;
                let other = other.unwrap();
                self.phases.swap(index, other);
                self.selected = other;
            }
            PhaseAction::Delete => {
                os_core::ensure(
                    index > 0 && index < self.phases.len(),
                    "The first phase is pinned and cannot be deleted",
                )?;
                let id = self.phases[index].id();
                os_core::ensure(
                    !referenced(document.model(), id),
                    "Phase is referenced by an element lifecycle or plan and cannot be deleted",
                )?;
                self.phases.remove(index);
                self.selected = index.min(self.phases.len() - 1);
            }
        }
        for (order, phase) in self.phases.iter_mut().enumerate() {
            phase.parameters.order = order as u32;
        }
        Ok(())
    }

    fn commands(&self, document: &Document) -> Result<Vec<Command>> {
        self.context.check(document)?;
        let model = document.model();
        os_core::ensure(
            self.phases.first().map(Phase::id) == model.existing_phase(),
            "The first phase is pinned",
        )?;
        let mut commands = Vec::new();
        for phase in model.phases.values() {
            if !self.phases.iter().any(|p| p.id() == phase.id()) {
                os_core::ensure(
                    !referenced(model, phase.id()),
                    "Phase is referenced by an element lifecycle or plan and cannot be deleted",
                )?;
                commands.push(Command::RemovePhase(phase.id()));
            }
        }
        for phase in &self.phases {
            match model.phases.get(&phase.id()) {
                None => commands.push(Command::AddPhase(phase.clone())),
                Some(old) if old.parameters != phase.parameters => {
                    commands.push(Command::UpdatePhase {
                        id: phase.id(),
                        parameters: phase.parameters.clone(),
                    })
                }
                _ => {}
            }
        }
        Ok(commands)
    }
}

fn referenced(model: &Model, phase: Id) -> bool {
    model
        .element_lifecycles
        .values()
        .any(|l| l.created_in == phase || l.demolished_in == Some(phase))
        || model.views.values().any(|view| {
            view.parameters
                .plan
                .is_some_and(|plan| plan.target_phase == Some(phase))
        })
}

pub(super) struct LifecycleDraft {
    context: Context,
    element: Id,
    lifecycle: ElementLifecycle,
    error: Option<String>,
}

impl DesktopApp {
    pub(super) fn begin_phases(&mut self) {
        self.phase_draft = Some(PhaseDraft {
            context: Context::new(&self.editor.document),
            phases: self
                .editor
                .document
                .model()
                .ordered_phases()
                .into_iter()
                .cloned()
                .collect(),
            selected: 0,
            error: None,
        });
    }

    pub(super) fn phase_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.phase_draft.take() else {
            return;
        };
        if draft.context.check(&self.editor.document).is_err() {
            self.report(
                Err(Error::Invalid(
                    "Phase edits discarded: document session or revision changed.".into(),
                )),
                "",
            );
            return;
        }
        let mut cancel = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut apply = false;
        let mut action = None;
        egui::Modal::new(egui::Id::new("phase_manager")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Project phases");
            ui.label("Changes stay local until Apply. The first phase is pinned.");
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for (index, phase) in draft.phases.iter().enumerate() {
                        ui.selectable_value(
                            &mut draft.selected,
                            index,
                            format!("{}. {}", index + 1, phase.parameters.name),
                        );
                    }
                });
            ui.label("Phase name");
            ui.text_edit_singleline(&mut draft.phases[draft.selected].parameters.name);
            ui.horizontal(|ui| {
                if ui.button("Add phase").clicked() {
                    action = Some(PhaseAction::Add);
                }
                if ui
                    .add_enabled(draft.selected > 1, egui::Button::new("Move up"))
                    .clicked()
                {
                    action = Some(PhaseAction::Up);
                }
                if ui
                    .add_enabled(
                        draft.selected > 0 && draft.selected + 1 < draft.phases.len(),
                        egui::Button::new("Move down"),
                    )
                    .clicked()
                {
                    action = Some(PhaseAction::Down);
                }
                if ui
                    .add_enabled(draft.selected > 0, egui::Button::new("Delete phase"))
                    .clicked()
                {
                    action = Some(PhaseAction::Delete);
                }
            });
            if let Some(error) = &draft.error {
                ui.colored_label(theme::ERROR, error);
            }
            ui.horizontal(|ui| {
                apply = ui.button("Apply phases").clicked();
                cancel |= ui.button("Cancel phases").clicked();
            });
        });
        if cancel {
            return;
        }
        if let Some(action) = action {
            draft.error = draft
                .edit(&self.editor.document, action)
                .err()
                .map(|e| e.to_string());
        }
        if apply {
            let result = draft.commands(&self.editor.document).and_then(|commands| {
                if commands.is_empty() {
                    Ok(())
                } else {
                    self.editor
                        .document
                        .execute("Edit project phases", commands)
                }
            });
            if let Err(error) = &result {
                draft.error = Some(error.to_string());
            }
            let success = result.is_ok();
            self.report(result, "Project phases updated.");
            if success {
                return;
            }
        }
        self.phase_draft = Some(draft);
    }

    fn lifecycle_selection(&self) -> Option<Id> {
        self.selected.filter(|id| {
            self.selected_ids.len() == 1
                && self.selected_ids.contains(id)
                && self.editor.document.model().is_phaseable_element(*id)
        })
    }

    pub(super) fn sync_lifecycle_draft(&mut self) {
        if self.lifecycle_draft.as_ref().is_some_and(|draft| {
            Some(draft.element) != self.lifecycle_selection()
                || draft.context != Context::new(&self.editor.document)
        }) {
            self.lifecycle_draft = None;
        }
    }

    pub(super) fn cancel_lifecycle_on_escape(&mut self, ctx: &egui::Context) {
        if self.lifecycle_draft.as_ref().is_some_and(|draft| {
            Some(draft.lifecycle)
                != self
                    .editor
                    .document
                    .model()
                    .element_lifecycle(draft.element)
                || draft.error.is_some()
        }) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.lifecycle_draft = None;
            // Escape owns this frame, even when a document shortcut accompanies it.
            ctx.input_mut(|i| {
                i.events
                    .retain(|event| !matches!(event, egui::Event::Key { .. }))
            });
        }
    }

    pub(super) fn lifecycle_properties(&mut self, ui: &mut egui::Ui) {
        self.sync_lifecycle_draft();
        let Some(element) = self.lifecycle_selection() else {
            return;
        };
        egui::CollapsingHeader::new("Phasing").show(ui, |ui| {
            let model = self.editor.document.model();
            let draft = self.lifecycle_draft.get_or_insert_with(|| LifecycleDraft {
                context: Context::new(&self.editor.document),
                element,
                lifecycle: model
                    .element_lifecycle(element)
                    .expect("phaseable selection"),
                error: None,
            });
            ui.horizontal(|ui| {
                ui.label("Created in");
                egui::ComboBox::from_id_salt("created_in")
                    .selected_text(&model.phases[&draft.lifecycle.created_in].parameters.name)
                    .show_ui(ui, |ui| {
                        for phase in model.ordered_phases() {
                            ui.selectable_value(
                                &mut draft.lifecycle.created_in,
                                phase.id(),
                                &phase.parameters.name,
                            );
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Demolished in");
                egui::ComboBox::from_id_salt("demolished_in")
                    .selected_text(
                        draft
                            .lifecycle
                            .demolished_in
                            .map(|id| model.phases[&id].parameters.name.as_str())
                            .unwrap_or("None"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.lifecycle.demolished_in, None, "None");
                        for phase in model.ordered_phases() {
                            ui.selectable_value(
                                &mut draft.lifecycle.demolished_in,
                                Some(phase.id()),
                                &phase.parameters.name,
                            );
                        }
                    });
            });
            if draft.lifecycle.demolished_in == Some(draft.lifecycle.created_in) {
                ui.label("Temporary");
            }
            if let Some(error) = &draft.error {
                ui.colored_label(theme::ERROR, error);
            }
            ui.horizontal(|ui| {
                if ui.button("Apply lifecycle").clicked() {
                    self.apply_lifecycle();
                }
                if ui.button("Cancel lifecycle").clicked() {
                    self.lifecycle_draft = None;
                }
            });
        });
    }

    fn apply_lifecycle(&mut self) {
        let Some(mut draft) = self.lifecycle_draft.take() else {
            return;
        };
        let result = draft.context.check(&self.editor.document).and_then(|()| {
            os_core::ensure(
                self.lifecycle_selection() == Some(draft.element),
                "Selection changed; reselect the element",
            )?;
            self.editor.document.execute(
                "Set element lifecycle",
                vec![Command::SetElementLifecycle {
                    element: draft.element,
                    lifecycle: draft.lifecycle,
                }],
            )
        });
        if let Err(error) = &result {
            draft.error = Some(error.to_string());
            if draft.context == Context::new(&self.editor.document)
                && self.lifecycle_selection() == Some(draft.element)
            {
                self.lifecycle_draft = Some(draft);
            }
        }
        self.report(result, "Element lifecycle updated.");
    }
}
