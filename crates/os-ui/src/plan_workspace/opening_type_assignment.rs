//! Existing-type assignment with a disposable, whole-batch candidate.
use super::*;
use os_core::ensure;
use std::collections::BTreeSet;

pub(super) fn type_label(ty: &OpeningType) -> String {
    // Names need not be unique. Include the stable ID in every displayed choice.
    format!("{} · {}", ty.parameters.name, ty.id())
}

pub(super) struct Draft {
    guard: crate::opening_tools::OpeningRehost,
    context: PlanContext,
    ids: BTreeSet<Id>,
    selected: Option<Id>,
    kind: OpeningKind,
    pub destination: Id,
    pub reset_paired_poses: bool,
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
        let destination = model
            .opening_types
            .values()
            .find(|ty| ty.parameters.kind == kind)
            .ok_or_else(|| Error::Invalid("Create a project type of this kind first".into()))?
            .id();
        let mut draft = Self {
            guard,
            context: app.editor.native_plan_context(app.plans.active.unwrap())?,
            ids: app.selected_ids.clone(),
            selected: app.selected,
            kind,
            destination,
            reset_paired_poses: false,
            preview: Err(Error::Invalid("Choose a type".into())),
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
        let ty = model
            .opening_types
            .get(&self.destination)
            .ok_or_else(|| Error::Invalid("Destination type is missing".into()))?;
        ensure(
            ty.parameters.kind == self.kind,
            "Destination type must have the same opening kind",
        )?;
        let mut candidate = model.clone();
        let mut commands = Vec::new();
        let mut hosts = BTreeSet::new();
        for &id in &self.ids {
            let source = &model.openings[&id].parameters;
            let mut parameters = source.clone();
            if self.reset_paired_poses
                && matches!(
                    parameters.open_state,
                    os_model::OpeningState::DoorPairAngles { .. }
                )
            {
                parameters.open_state = os_model::OpeningState::Default;
            }
            if matches!(source.definition, OpeningDefinition::Legacy { .. }) {
                let effective = model.resolve_opening(source)?;
                parameters.width_override = Some(effective.width);
                parameters.height_override = Some(effective.height);
                if self.kind == OpeningKind::Window {
                    parameters.sill_override = Some(effective.sill);
                }
            }
            parameters.definition = OpeningDefinition::Typed {
                type_id: self.destination,
            };
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
            crate::opening_tools::host_mesh(&candidate, host)
                .map_err(|e| Error::Invalid(format!("Host {host}: {e}")))?;
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
    pub(crate) fn begin_opening_type_assignment(&mut self) {
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
                self.plans.opening_type_assignment = Some(draft);
            }
            Err(e) => self.report(Err(e), ""),
        }
    }

    pub(super) fn validate_opening_type_assignment(&mut self, ctx: &egui::Context) {
        if self.plans.opening_type_assignment.is_some() && ctx.input(|i| i.pointer.primary_down()) {
            self.plans.opening_type_assignment_claimed = true;
        }
        if self
            .plans
            .opening_type_assignment
            .as_ref()
            .is_some_and(|d| !d.current(self))
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.plans.opening_type_assignment = None;
        }
    }

    pub(super) fn apply_opening_type_assignment(&mut self) -> Result<()> {
        let draft = self
            .plans
            .opening_type_assignment
            .as_ref()
            .ok_or_else(|| Error::Invalid("No type assignment draft".into()))?;
        if !draft.current(self) {
            self.plans.opening_type_assignment = None;
            return Err(Error::Invalid("Type assignment context changed".into()));
        }
        let preview = draft.candidate(self.editor.document.model())?;
        if !preview.commands.is_empty() {
            self.editor
                .document
                .execute("Change opening type", preview.commands)?;
            self.plans.opening_type_assignment = None;
            self.editor.regenerate()?;
        } else {
            self.plans.opening_type_assignment = None;
        }
        Ok(())
    }

    pub(super) fn opening_type_assignment_dialog(&mut self, ctx: &egui::Context) {
        self.validate_opening_type_assignment(ctx);
        let Some(mut draft) = self.plans.opening_type_assignment.take() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Change opening type")
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_max_width(370.);
                ui.label(format!(
                    "{} selected {:?} instance(s)",
                    draft.ids.len(),
                    draft.kind
                ));
                ui.label(
                    "Instance overrides stay pinned; inherited values follow the chosen type.",
                );
                ui.label(
                    "Legacy openings keep their width, height and window sill as pinned overrides.",
                );
                let previous = draft.destination;
                egui::ComboBox::from_label("Project type")
                    .selected_text(type_label(
                        &self.editor.document.model().opening_types[&draft.destination],
                    ))
                    .show_ui(ui, |ui| {
                        for ty in self
                            .editor
                            .document
                            .model()
                            .opening_types
                            .values()
                            .filter(|ty| ty.parameters.kind == draft.kind)
                        {
                            ui.selectable_value(&mut draft.destination, ty.id(), type_label(ty));
                        }
                    });
                let reset_changed = if draft.ids.iter().any(|id| {
                    matches!(
                        self.editor.document.model().openings[id]
                            .parameters
                            .open_state,
                        os_model::OpeningState::DoorPairAngles { .. }
                    )
                }) {
                    ui.checkbox(
                        &mut draft.reset_paired_poses,
                        "Reset paired poses to Default",
                    )
                    .changed()
                } else {
                    false
                };
                if previous != draft.destination || reset_changed {
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
                        .add_enabled(draft.preview.is_ok(), egui::Button::new("Apply type"))
                        .clicked();
                    cancel = ui.button("Cancel type change").clicked();
                });
            });
        if !cancel {
            self.plans.opening_type_assignment = Some(draft);
        }
        if apply {
            let result = self.apply_opening_type_assignment();
            self.report(result, "Opening type assignment applied.");
        }
    }
}
