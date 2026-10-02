//! Atomic instance-property editing with a disposable whole-selection preview.
use super::*;
use os_core::ensure;
use std::collections::BTreeSet;

pub(super) struct Draft {
    guard: crate::opening_tools::OpeningRehost,
    context: PlanContext,
    ids: BTreeSet<Id>,
    selected: Option<Id>,
    kind: OpeningKind,
    pub fields: [Field; 3],
    pub hinge: Option<os_model::DoorHinge>,
    pub swing: Option<os_model::DoorSwing>,
    all_typed: bool,
    summary: Vec<String>,
    pub preview: Result<Preview>,
}

pub(super) struct Preview {
    pub commands: Vec<Command>,
    pub drawing: PlanDrawing,
    pub hosts: BTreeSet<Id>,
}

impl Draft {
    pub fn begin(app: &DesktopApp) -> Result<Self> {
        ensure(
            app.plans.active_sheet.is_none() && (1..=256).contains(&app.selected_ids.len()),
            "Select 1–256 visible doors or windows in one floor plan",
        )?;
        ensure(
            !app.editor.plugin_work_pending(),
            "Wait for pending document work",
        )?;
        let drawing = app
            .plans
            .drawing
            .as_ref()
            .ok_or_else(|| Error::Invalid("Wait for the plan drawing".into()))?;
        let model = app.editor.document.model();
        ensure(
            app.plans
                .active
                .and_then(|id| model.views.get(&id))
                .and_then(|v| v.parameters.plan)
                .is_some_and(|p| p.view_type == os_model::PlanViewType::FloorPlan),
            "Open a floor plan to edit opening instances",
        )?;
        let mut guards = Vec::new();
        let mut kind = None;
        for &id in &app.selected_ids {
            let opening = model.openings.get(&id).ok_or_else(|| {
                Error::Invalid("Every selected item must be a native opening".into())
            })?;
            let resolved = model.resolve_opening(&opening.parameters)?;
            ensure(
                kind.is_none_or(|k| k == resolved.kind),
                "Select doors or windows, not mixed kinds",
            )?;
            kind = Some(resolved.kind);
            guards.push(
                crate::opening_tools::OpeningRehost::begin(
                    &app.editor,
                    app.plans.active,
                    Some(id),
                    drawing,
                )
                .map_err(|e| Error::Invalid(format!("Opening {id}: {e}")))?,
            );
        }
        let kind = kind.unwrap();
        let guard = guards.remove(0);
        let all_typed = app
            .selected_ids
            .iter()
            .all(|id| model.openings[id].parameters.type_id().is_some());
        let summary = summary(model, &app.selected_ids, kind)?;
        let mut draft = Self {
            guard,
            context: app.editor.native_plan_context(app.plans.active.unwrap())?,
            ids: app.selected_ids.clone(),
            selected: app.selected,
            kind,
            fields: Default::default(),
            hinge: None,
            swing: None,
            all_typed,
            summary,
            preview: Err(Error::Invalid("No candidate".into())),
        };
        draft.preview = draft.candidate(model);
        Ok(draft)
    }

    fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active_sheet.is_none()
            && app.selected_ids == self.ids
            && app.selected == self.selected
            && !app.editor.plugin_work_pending()
            && self.guard.current(
                &app.editor,
                app.plans.active,
                Some(self.guard.id),
                app.plans.drawing.as_ref(),
            )
    }

    pub fn candidate(&self, model: &Model) -> Result<Preview> {
        let mut candidate = model.clone();
        let mut commands = Vec::new();
        let mut hosts = BTreeSet::new();
        for &id in &self.ids {
            let source = &model.openings[&id].parameters;
            let mut parameters = source.clone();
            for (index, field) in self.fields.iter().enumerate() {
                if index == 2 && self.kind == OpeningKind::Door {
                    continue;
                }
                field
                    .apply(&mut parameters, index, self.all_typed)
                    .map_err(|e| Error::Invalid(format!("Opening {id}: {e}")))?;
            }
            if self.kind == OpeningKind::Door {
                if let Some(hinge) = self.hinge {
                    parameters.hinge = hinge;
                }
                if let Some(swing) = self.swing {
                    parameters.swing = swing;
                }
            }
            hosts.insert(parameters.host);
            if parameters != *source {
                candidate.openings.get_mut(&id).unwrap().parameters = parameters.clone();
                commands.push(Command::UpdateOpening { id, parameters });
            }
        }
        candidate.settle_opening_clearances(model)?;
        // Diagnose affected instances before the authoritative whole-model check.
        for &host in &hosts {
            let mut intervals = Vec::new();
            for opening in candidate
                .openings
                .values()
                .filter(|o| o.parameters.host == host)
            {
                let id = opening.id();
                opening
                    .parameters
                    .validate(&candidate)
                    .map_err(|e| Error::Invalid(format!("Opening {id}: {e}")))?;
                let p = candidate.resolve_opening(&opening.parameters)?;
                intervals.push((p.offset, p.width, id));
                crate::opening_tools::panel_mesh(&candidate, id)
                    .map_err(|e| Error::Invalid(format!("Opening {id}: {e}")))?;
            }
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in intervals.windows(2) {
                ensure(
                    pair[0].0 + pair[0].1 + 0.001 <= pair[1].0,
                    format!(
                        "Openings {} and {} require at least 1 mm separation",
                        pair[0].2, pair[1].2
                    ),
                )?;
            }
            crate::opening_tools::host_mesh(&candidate, host).map_err(|e| {
                Error::Invalid(format!(
                    "Host {host} (openings {:?}): {e}",
                    self.ids
                        .iter()
                        .filter(|id| candidate.openings[id].parameters.host == host)
                        .collect::<Vec<_>>()
                ))
            })?;
        }
        candidate.validate()?;
        let drawing = crate::opening_tools::opening_batch_preview(
            &candidate,
            &self.ids.iter().copied().collect::<Vec<_>>(),
            &hosts.iter().copied().collect::<Vec<_>>(),
            self.context,
        )?;
        Ok(Preview {
            commands,
            drawing,
            hosts,
        })
    }

    pub fn contains(&self, id: Id) -> bool {
        self.ids.contains(&id)
    }
}

impl DesktopApp {
    pub(crate) fn begin_opening_batch_edit(&mut self) {
        match Draft::begin(self) {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_opening_placement();
                self.cancel_opening_controls();
                self.cancel_aligned_dimension();
                self.plans.crop.mode = None;
                self.plans.column_placement = None;
                self.plans.casework_placement = None;
                self.plans.floor_sketch = None;
                self.plans.floor_hole_sketch = None;
                self.plans.ceiling_draft = None;
                self.plans.section_placement = None;
                self.plans.room_placement_active = false;
                self.plans.area_selection.cancel();
                self.opening_draft = None;
                self.plans.opening_batch_edit = Some(draft);
            }
            Err(e) => self.report(Err(e), ""),
        }
    }

    pub(super) fn validate_opening_batch_edit(&mut self, ctx: &egui::Context) {
        if self.plans.opening_batch_edit.is_some() && ctx.input(|i| i.pointer.primary_down()) {
            self.plans.opening_batch_edit_claimed = true;
        }
        if self
            .plans
            .opening_batch_edit
            .as_ref()
            .is_some_and(|d| !d.current(self))
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.plans.opening_batch_edit = None;
        }
    }

    pub(super) fn apply_opening_batch_edit(&mut self) -> Result<()> {
        let draft = self
            .plans
            .opening_batch_edit
            .as_ref()
            .ok_or_else(|| Error::Invalid("No instance edit draft".into()))?;
        if !draft.current(self) {
            self.plans.opening_batch_edit = None;
            return Err(Error::Invalid("Instance edit context changed".into()));
        }
        let preview = draft.candidate(self.editor.document.model())?;
        if !preview.commands.is_empty() {
            self.editor
                .document
                .execute("Edit selected instances", preview.commands)?;
            self.plans.opening_batch_edit = None;
            self.editor.regenerate()?;
        } else {
            self.plans.opening_batch_edit = None;
        }
        Ok(())
    }

    pub(super) fn opening_batch_edit_dialog(&mut self, ctx: &egui::Context) {
        self.validate_opening_batch_edit(ctx);
        let Some(mut draft) = self.plans.opening_batch_edit.take() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Edit selected instances")
            .resizable(false)
            .vscroll(true)
            .max_height((ctx.content_rect().height() - 100.).max(200.))
            .show(ctx, |ui| {
                ui.set_max_width(370.);
                ui.label(format!(
                    "{} selected {:?} instance(s)",
                    draft.ids.len(),
                    draft.kind
                ));
                ui.label("Only chosen fields change; Keep unchanged preserves each instance.");
                for line in &draft.summary {
                    ui.label(line);
                }
                let previous = (draft.fields.clone(), draft.hinge, draft.swing);
                for (index, label) in ["Width (m)", "Height (m)", "Window sill (m)"]
                    .iter()
                    .enumerate()
                {
                    if index == 2 && draft.kind == OpeningKind::Door {
                        continue;
                    }
                    draft.fields[index].ui(ui, label, index, draft.all_typed);
                }
                if draft.kind == OpeningKind::Door {
                    ui.label("Handing uses each host's Start → End frame.");
                    choice(
                        ui,
                        "Door hinge",
                        &mut draft.hinge,
                        [
                            (os_model::DoorHinge::Start, "Start"),
                            (os_model::DoorHinge::End, "End"),
                        ],
                    );
                    choice(
                        ui,
                        "Door swing",
                        &mut draft.swing,
                        [
                            (os_model::DoorSwing::Left, "Left"),
                            (os_model::DoorSwing::Right, "Right"),
                        ],
                    );
                }
                if previous != (draft.fields.clone(), draft.hinge, draft.swing) {
                    draft.preview = draft.candidate(self.editor.document.model());
                }
                match &draft.preview {
                    Ok(p) => {
                        ui.label(format!("{} instance(s) will change", p.commands.len()));
                    }
                    Err(e) => {
                        ui.colored_label(theme::ERROR, e.to_string());
                    }
                }
                ui.horizontal(|ui| {
                    apply = ui
                        .add_enabled(draft.preview.is_ok(), egui::Button::new("Apply instances"))
                        .clicked();
                    cancel = ui.button("Cancel instance edit").clicked();
                });
            });
        if !cancel {
            self.plans.opening_batch_edit = Some(draft);
        }
        if apply {
            let result = self.apply_opening_batch_edit();
            self.report(result, "Opening instances updated.");
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Keep,
    Set,
    Inherit,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Field {
    pub mode: Mode,
    pub text: String,
}

impl Field {
    fn apply(&self, p: &mut OpeningParams, index: usize, all_typed: bool) -> Result<()> {
        if self.mode == Mode::Keep {
            return Ok(());
        }
        ensure(
            self.mode != Mode::Inherit || all_typed,
            "Inherit requires every selected instance to be typed",
        )?;
        let value = if self.mode == Mode::Set {
            let value = self
                .text
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::Invalid("Enter a decimal value in metres".into()))?;
            ensure(
                value.is_finite() && value >= if index == 2 { 0. } else { 0.001 },
                "Use a finite value: width/height at least 0.001 m; sill at least 0 m",
            )?;
            Some(value)
        } else {
            None
        };
        match &mut p.definition {
            OpeningDefinition::Typed { .. } => {
                *match index {
                    0 => &mut p.width_override,
                    1 => &mut p.height_override,
                    _ => &mut p.sill_override,
                } = value;
            }
            OpeningDefinition::Legacy {
                width,
                height,
                sill,
                ..
            } => {
                *match index {
                    0 => width,
                    1 => height,
                    _ => sill,
                } = value
                    .ok_or_else(|| Error::Invalid("Legacy dimensions cannot inherit".into()))?;
            }
        }
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, label: &str, index: usize, all_typed: bool) {
        ui.push_id(index, |ui| {
            ui.label(label);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Keep, "Keep unchanged");
                ui.selectable_value(&mut self.mode, Mode::Set, "Set value");
                ui.add_enabled_ui(all_typed, |ui| {
                    ui.selectable_value(&mut self.mode, Mode::Inherit, "Inherit assigned type");
                })
                .response
                .on_disabled_hover_text(
                    "Inherit is unavailable because the selection includes legacy openings.",
                );
            });
            if !all_typed {
                ui.small("Legacy selection: Inherit unavailable.");
            }
            if self.mode == Mode::Set {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.text)
                        .id_salt(("opening_batch_value", index))
                        .char_limit(64)
                        .desired_width(130.),
                );
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    response.surrender_focus();
                }
            }
        });
    }
}

fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<T>,
    options: [(T, &str); 2],
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.selectable_value(value, None, "Keep unchanged");
        for (item, name) in options {
            ui.selectable_value(value, Some(item), name);
        }
    });
}

fn common(values: &[String]) -> String {
    if values.iter().all(|v| v == &values[0]) {
        values[0].clone()
    } else {
        "Mixed".into()
    }
}

fn summary(model: &Model, ids: &BTreeSet<Id>, kind: OpeningKind) -> Result<Vec<String>> {
    let mut result = Vec::new();
    for (index, name) in ["Width", "Height", "Window sill"].iter().enumerate() {
        if index == 2 && kind == OpeningKind::Door {
            continue;
        }
        let mut values = Vec::new();
        let mut states = Vec::new();
        for id in ids {
            let p = &model.openings[id].parameters;
            let r = model.resolve_opening(p)?;
            values.push(
                match index {
                    0 => r.width,
                    1 => r.height,
                    _ => r.sill,
                }
                .to_string(),
            );
            let pin = match index {
                0 => p.width_override,
                1 => p.height_override,
                _ => p.sill_override,
            };
            states.push(
                if p.type_id().is_none() {
                    "Legacy"
                } else if pin.is_some() {
                    "Pinned"
                } else {
                    "Inherited"
                }
                .to_string(),
            );
        }
        result.push(format!(
            "{name}: {} m · {}",
            common(&values),
            common(&states)
        ));
    }
    if kind == OpeningKind::Door {
        result.push(format!(
            "Door hinge: {}",
            common(
                &ids.iter()
                    .map(|id| format!("{:?}", model.openings[id].parameters.hinge))
                    .collect::<Vec<_>>()
            )
        ));
        result.push(format!(
            "Door swing: {}",
            common(
                &ids.iter()
                    .map(|id| format!("{:?}", model.openings[id].parameters.swing))
                    .collect::<Vec<_>>()
            )
        ));
    }
    Ok(result)
}

impl DesktopApp {
    pub(crate) fn opening_batch_properties(&mut self, ui: &mut egui::Ui) -> bool {
        let model = self.editor.document.model();
        // Keep the established single-instance Properties layout intact. The
        // Architecture ribbon still exposes the batch editor for one opening.
        if self.selected_ids.len() < 2
            || !self
                .selected_ids
                .iter()
                .all(|id| model.openings.contains_key(id))
        {
            return false;
        }
        let kinds = self
            .selected_ids
            .iter()
            .filter_map(|id| {
                model
                    .resolve_opening(&model.openings[id].parameters)
                    .ok()
                    .map(|r| r.kind)
            })
            .collect::<Vec<_>>();
        if kinds.len() != self.selected_ids.len() || !kinds.iter().all(|k| *k == kinds[0]) {
            return false;
        }
        ui.label(format!(
            "{} {:?} instance(s)",
            self.selected_ids.len(),
            kinds[0]
        ));
        if let Ok(lines) = summary(model, &self.selected_ids, kinds[0]) {
            for line in lines {
                ui.label(line);
            }
        }
        if ui.button("Edit selected instances…").clicked() {
            self.begin_opening_batch_edit();
        }
        true
    }
}
