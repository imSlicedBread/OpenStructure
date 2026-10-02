//! Staged project Length management, using the document commit evaluator.
use super::*;
use os_core::ensure;
use os_model::{LengthParameter, LengthParameterParams, LengthUnit};
use std::collections::BTreeMap;

pub(super) struct LengthDraft {
    session: Id,
    revision: u64,
    pub(super) parameters: BTreeMap<Id, LengthParameter>,
    pub(super) selected: Option<Id>,
    error: Option<String>,
}
impl LengthDraft {
    fn new(editor: &Editor) -> Self {
        Self {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            parameters: editor.document.model().length_parameters.clone(),
            selected: None,
            error: None,
        }
    }
    fn current(&self, editor: &Editor) -> bool {
        self.session == editor.document.session_id() && self.revision == editor.document.revision()
    }
    fn commands(&self, editor: &Editor) -> Result<Vec<Command>> {
        ensure(self.current(editor), "Length parameter draft is stale")?;
        let saved = &editor.document.model().length_parameters;
        let mut commands = Vec::new();
        for (id, parameter) in &self.parameters {
            match saved.get(id) {
                None => commands.push(Command::AddLengthParameter(parameter.clone())),
                Some(old) if old != parameter => commands.push(Command::UpdateLengthParameter {
                    id: *id,
                    parameters: parameter.parameters.clone(),
                }),
                _ => {}
            }
        }
        for id in saved.keys().filter(|id| !self.parameters.contains_key(id)) {
            commands.push(Command::RemoveLengthParameter(*id));
        }
        Ok(commands)
    }
    fn preview(&self, editor: &Editor) -> Result<Model> {
        let commands = self.commands(editor)?;
        let model = if commands.is_empty() {
            editor.document.model().clone()
        } else {
            editor.document.preview_commands(commands)?
        };
        preflight_lengths(&model)?;
        Ok(model)
    }
    fn apply(&self, editor: &mut Editor) -> Result<()> {
        self.preview(editor)?;
        let commands = self.commands(editor)?;
        if !commands.is_empty() {
            editor
                .document
                .execute("Edit project Length parameters", commands)?;
            editor.regenerate()?;
        }
        Ok(())
    }
}
pub(super) fn preflight_lengths(model: &Model) -> Result<()> {
    for opening in model.openings.values() {
        super::opening_tools::panel_mesh(model, opening.id())?;
    }
    let hosts: std::collections::BTreeSet<_> =
        model.openings.values().map(|o| o.parameters.host).collect();
    for host in hosts {
        super::opening_tools::host_mesh(model, host)?;
    }
    Ok(())
}
impl DesktopApp {
    pub(super) fn begin_length_parameters(&mut self) {
        self.cancel_plan_wall();
        self.cancel_opening_placement();
        self.opening_type_draft = None;
        self.length_draft = Some(LengthDraft::new(&self.editor));
    }
    pub(super) fn length_parameter_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.length_draft.take() else {
            return;
        };
        if !draft.current(&self.editor) {
            self.status = "Length parameter editor canceled because its document changed.".into();
            return;
        }
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        egui::Modal::new(egui::Id::new("project_length_parameters")).show(ctx, |ui| {
            ui.set_width(560.);
            ui.heading("Project Length parameters");
            ui.small("Values are in metres. Bind dimensions in Manage opening type. Instance overrides remain pinned.");
            ui.horizontal(|ui| {
                if ui.add_enabled(draft.parameters.len() < os_model::MAX_LENGTH_PARAMETERS, egui::Button::new("New Length")).clicked() {
                    let p = LengthParameter::new("core.length_parameter", LengthParameterParams { name: "New Length".into(), unit: LengthUnit::Metres, value: 1. });
                    draft.selected = Some(p.id());
                    draft.parameters.insert(p.id(), p);
                }
                if let Some(id) = draft.selected {
                    if ui.add_enabled(draft.parameters.len() < os_model::MAX_LENGTH_PARAMETERS, egui::Button::new("Duplicate Length")).clicked() {
                        let mut p = draft.parameters[&id].clone();
                        p.header.id = Id::new();
                        p.parameters.name.push_str(" Copy");
                        draft.selected = Some(p.id());
                        draft.parameters.insert(p.id(), p);
                    }
                    if ui.button("Delete Length").clicked() {
                        let uses = self.editor.document.model().length_parameter_uses(id);
                        if uses.is_empty() {
                            draft.parameters.remove(&id);
                            draft.selected = None;
                            draft.error = None;
                        } else {
                            draft.error = Some(format!("Cannot delete: {} dimension binding(s). Unbind the affected types first.", uses.len()));
                        }
                    }
                }
            });
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-220.).max(140.)).show(ui, |ui| {
                for (id, p) in &draft.parameters {
                    if ui.selectable_label(draft.selected == Some(*id), format!("{} · {:.3} m · {}", p.parameters.name, p.parameters.value, id)).clicked() { draft.selected = Some(*id); }
                }
                if let Some(id) = draft.selected && let Some(p) = draft.parameters.get_mut(&id) {
                    ui.separator();
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut p.parameters.name).char_limit(256));
                    ui.horizontal(|ui| { ui.label("Length (m)"); ui.add(egui::DragValue::new(&mut p.parameters.value).speed(0.01)); });
                    ui.small(format!("Identity: {id}"));
                    let model = self.editor.document.model();
                    let uses = model.length_parameter_uses(id);
                    ui.label(format!("Affected dimension bindings: {}", uses.len()));
                    for (ty, dimension) in uses {
                        let instances = model.openings.values().filter(|o| o.parameters.type_id() == Some(ty)).count();
                        ui.small(format!("{} · {dimension} · {instances} placed instance(s) · {ty}", model.opening_types[&ty].parameters.name));
                    }
                }
                match draft.preview(&self.editor) {
                    Ok(candidate) => {
                        let model = self.editor.document.model();
                        let types = candidate.opening_types.keys().filter(|id| candidate.resolve_opening_type(**id).ok() != model.resolve_opening_type(**id).ok()).count();
                        let instances = candidate.openings.values().filter(|o| candidate.resolve_opening(&o.parameters).ok() != model.openings.get(&o.id()).and_then(|old| model.resolve_opening(&old.parameters).ok())).count();
                        ui.label(format!("Preview valid: {types} type(s), {instances} effective placement(s) change."));
                    }
                    Err(error) => { ui.colored_label(theme::ERROR, format!("Preview rejected: {error}")); }
                }
            });
            if let Some(error) = &draft.error { ui.colored_label(theme::ERROR, error); }
            ui.horizontal(|ui| {
                if ui.button("Cancel Length edits").clicked() { close = true; }
                if ui.button("Apply Length edits").clicked() && !close {
                    match draft.apply(&mut self.editor) {
                        Ok(()) => { close = true; self.status = "Project Length parameters applied.".into(); }
                        Err(error) => draft.error = Some(error.to_string()),
                    }
                }
            });
        });
        if !close {
            self.length_draft = Some(draft);
        }
    }
}
