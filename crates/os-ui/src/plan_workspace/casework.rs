//! Native casework placement and revision-bound shared type/instance edits.
use super::*;
use os_geometry::plan::PlanFootprint;
use os_model::{Casework, CaseworkParams, CaseworkType, CaseworkTypeParams};

pub(super) enum TypeSource {
    Existing(Id),
    Transient(CaseworkType),
}

pub(super) struct Placement {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    pub parameters: CaseworkParams,
    pub type_source: TypeSource,
}

impl Placement {
    pub fn stale(&self, editor: &Editor, active: Option<Id>) -> bool {
        active != Some(self.context.view_id)
            || editor.document.session_id() != self.context.session_id
            || editor.document.revision() != self.context.model_revision
            || editor.native_plan_context(self.context.view_id).ok() != Some(self.context)
            || plan_provider_signature(editor) != self.providers
    }

    fn resolved_type<'a>(&'a self, model: &'a Model) -> Result<&'a CaseworkType> {
        match &self.type_source {
            TypeSource::Existing(id) => model
                .casework_types
                .get(id)
                .ok_or_else(|| Error::Invalid("Casework type missing".into())),
            TypeSource::Transient(casework_type) => {
                os_core::ensure(
                    !model.casework_types.contains_key(&casework_type.id()),
                    "Transient casework type already exists",
                )?;
                Ok(casework_type)
            }
        }
    }
}

pub(super) struct Edit {
    id: Id,
    session: Id,
    revision: u64,
    view: Option<Id>,
    context: Option<PlanContext>,
    providers: Vec<(String, Id)>,
    pub parameters: CaseworkParams,
    pub type_parameters: CaseworkTypeParams,
}

impl Edit {
    fn stale(&self, app: &DesktopApp) -> bool {
        app.selected != Some(self.id)
            || app.plans.active != self.view
            || app.plans.active_sheet.is_some()
            || app.editor.document.session_id() != self.session
            || app.editor.document.revision() != self.revision
            || self
                .view
                .and_then(|id| app.editor.native_plan_context(id).ok())
                != self.context
            || plan_provider_signature(&app.editor) != self.providers
    }

    fn commands(&self, model: &Model) -> Result<Vec<Command>> {
        let original = model
            .casework
            .get(&self.id)
            .ok_or_else(|| Error::Invalid("Casework missing".into()))?;
        let original_type = model
            .casework_types
            .get(&self.parameters.type_id)
            .ok_or_else(|| Error::Invalid("Casework type missing".into()))?;
        let mut proposed_type = original_type.clone();
        proposed_type.parameters = self.type_parameters.clone();
        validate_parameters(model, &self.parameters, &proposed_type)?;
        let type_changed = self.type_parameters != original_type.parameters;
        if type_changed {
            // Validate the final state, including the selected instance's proposed
            // placement or type assignment, rather than an intermediate command.
            for instance in model.casework.values().filter(|instance| {
                instance.id() != self.id && instance.parameters.type_id == proposed_type.id()
            }) {
                validate_parameters(model, &instance.parameters, &proposed_type)?;
            }
        }
        let mut commands = Vec::new();
        if type_changed {
            commands.push(Command::UpdateCaseworkType {
                id: proposed_type.id(),
                parameters: self.type_parameters.clone(),
            });
        }
        if self.parameters != original.parameters {
            commands.push(Command::UpdateCasework {
                id: self.id,
                parameters: self.parameters.clone(),
            });
        }
        Ok(commands)
    }
}

fn default_type() -> CaseworkType {
    CaseworkType::new(
        "core.casework_type",
        CaseworkTypeParams {
            name: "Base cabinet 600".into(),
            width: 0.6,
            depth: 0.6,
            height: 0.9,
            material: None,
        },
    )
}

/// Also accepts a transient type without inserting it into the document.
fn validate_parameters(
    model: &Model,
    parameters: &CaseworkParams,
    casework_type: &CaseworkType,
) -> Result<f64> {
    os_core::ensure(
        parameters.type_id == casework_type.id()
            && casework_type.header.type_id == "core.casework_type",
        "Casework type identity mismatch",
    )?;
    casework_type.parameters.validate_in(model)?;
    let elevation = model
        .levels
        .get(&parameters.level)
        .ok_or_else(|| Error::Invalid("Casework level missing".into()))?
        .parameters
        .elevation;
    parameters.elevations(&casework_type.parameters, elevation)?;
    Ok(elevation)
}

fn footprint(
    model: &Model,
    parameters: &CaseworkParams,
    casework_type: &CaseworkType,
    context: PlanContext,
) -> Result<PlanFootprint> {
    let elevation = validate_parameters(model, parameters, casework_type)?;
    os_core::ensure(
        point_in_plan_crop(context, context.basis.world_to_plane(parameters.center)?),
        "Casework center is outside the crop",
    )?;
    os_geometry::casework::casework_footprint(
        parameters,
        &casework_type.parameters,
        elevation,
        context.range,
        context.basis,
        context.crop,
    )?
    .ok_or_else(|| Error::Invalid("Casework is outside the visible plan range".into()))
}

impl DesktopApp {
    pub(crate) fn begin_casework(&mut self, view: Id) {
        let result = (|| {
            os_core::ensure(
                self.plans.active == Some(view) && self.plans.active_sheet.is_none(),
                "Open a floor plan to place casework",
            )?;
            let context = self.editor.native_plan_context(view)?;
            let model = self.editor.document.model();
            let level = model.views[&view]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Casework placement needs a plan level".into()))?;
            let existing = self
                .selected
                .and_then(|id| model.casework.get(&id))
                .map(|instance| instance.parameters.type_id)
                .or_else(|| model.casework_types.keys().next().copied());
            let type_source = existing.map_or_else(
                || TypeSource::Transient(default_type()),
                TypeSource::Existing,
            );
            let type_id = match &type_source {
                TypeSource::Existing(id) => *id,
                TypeSource::Transient(t) => t.id(),
            };
            let parameters = CaseworkParams {
                name: format!("Casework {}", model.casework.len() + 1),
                type_id,
                level,
                center: Point2::new(0.0, 0.0),
                yaw: 0.0,
                base_offset: 0.0,
            };
            self.cancel_plan_wall();
            self.cancel_aligned_dimension();
            self.select(None);
            self.cancel_opening_placement();
            self.plans.curtain_draft = None;
            self.plans.floor_sketch = None;
            self.plans.floor_hole_sketch = None;
            self.plans.floor_vertex_drag = None;
            self.plans.floor_properties = None;
            self.plans.column_placement = None;
            self.plans.stair_placement = None;
            self.plans.ramp_placement = None;
            self.plans.roof_draft = None;
            self.plans.ceiling_draft = None;
            self.plans.section_placement = None;
            self.plans.detail_line_draft = None;
            self.plans.room_separation_line_draft = None;
            self.plans.room_tag_draft = None;
            self.plans.opening_tag_draft = None;
            self.plans.opening_move = None;
            self.plans.opening_flip = None;
            self.plans.opening_rehost = None;
            self.plans.room_placement_active = false;
            self.plans.crop.mode = None;
            self.plans.crop.cancel();
            self.plans.area_selection.cancel();
            self.grid_draft = None;
            self.plan_draft = None;
            self.plans.casework_edit = None;
            self.plans.casework_placement = Some(Placement {
                context,
                providers: plan_provider_signature(&self.editor),
                parameters,
                type_source,
            });
            Ok(())
        })();
        self.report(
            result,
            "Casework · choose a type, then click a snapped center. Escape cancels.",
        );
    }

    /// Call before canvas input routing; the caller retains pointer ownership
    /// through cancellation and release, as for column placement.
    pub(super) fn validate_casework_interaction(&mut self, ctx: &egui::Context) {
        let cancel = ctx.input(|input| {
            input.key_pressed(egui::Key::Escape)
                || input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::PointerGone))
        });
        let stale = self.plans.casework_placement.as_ref().is_some_and(|draft| {
            draft.stale(&self.editor, self.plans.active) || self.plans.active_sheet.is_some()
        });
        if (cancel || stale) && self.plans.casework_placement.take().is_some() {
            self.status = if stale {
                "Casework placement canceled because its plan or document changed."
            } else {
                "Casework placement canceled."
            }
            .into();
            self.status_error = false;
        }
        if cancel
            || self
                .plans
                .casework_edit
                .as_ref()
                .is_some_and(|draft| draft.stale(self))
        {
            self.plans.casework_edit = None;
        }
    }

    pub(super) fn commit_casework(&mut self, parameters: CaseworkParams) {
        if self.plans.casework_placement.as_ref().is_some_and(|draft| {
            draft.stale(&self.editor, self.plans.active) || self.plans.active_sheet.is_some()
        }) {
            self.plans.casework_placement = None;
            self.report(
                Err(Error::Invalid("Casework placement is stale".into())),
                "",
            );
            return;
        }
        let result = (|| {
            let draft = self
                .plans
                .casework_placement
                .as_ref()
                .ok_or_else(|| Error::Invalid("Casework placement canceled".into()))?;
            let model = self.editor.document.model();
            let casework_type = draft.resolved_type(model)?;
            footprint(model, &parameters, casework_type, draft.context)?;
            let mut commands = Vec::new();
            if let TypeSource::Transient(casework_type) = &draft.type_source {
                commands.push(Command::AddCaseworkType(casework_type.clone()));
            }
            let instance = Casework::new("core.casework", parameters);
            let id = instance.id();
            commands.push(Command::AddCasework(instance));
            self.editor.commands("Place casework", commands)?;
            self.plans.casework_placement = None;
            self.select(Some(id));
            Ok(())
        })();
        self.report(result, "Casework placed.");
    }

    pub(crate) fn casework_properties(&mut self, ui: &mut egui::Ui) -> bool {
        self.validate_casework_interaction(ui.ctx());
        if let Some(draft) = &mut self.plans.casework_placement {
            let model = self.editor.document.model();
            let mut cancel = false;
            egui::ScrollArea::vertical().id_salt("casework_placement_properties").show(ui, |ui| {
                ui.label("Place casework · metres");
                let mut selected = match &draft.type_source {
                    TypeSource::Existing(id) => Some(*id),
                    TypeSource::Transient(_) => None,
                };
                let previous = selected;
                egui::ComboBox::from_id_salt("casework_placement_type")
                    .selected_text(selected.and_then(|id| model.casework_types.get(&id))
                        .map_or("New cabinet type", |t| t.parameters.name.as_str()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, None, "New cabinet type");
                        for (id, t) in &model.casework_types {
                            ui.selectable_value(&mut selected, Some(*id), &t.parameters.name);
                        }
                    });
                if selected != previous {
                    draft.type_source = selected.map_or_else(
                        || TypeSource::Transient(default_type()), TypeSource::Existing);
                    draft.parameters.type_id = match &draft.type_source {
                        TypeSource::Existing(id) => *id,
                        TypeSource::Transient(t) => t.id(),
                    };
                }
                instance_controls(ui, &mut draft.parameters, model, true);
                match &mut draft.type_source {
                    TypeSource::Transient(t) => {
                        ui.label("New type · saved with the first placement");
                        type_controls(ui, &mut t.parameters, model);
                    }
                    TypeSource::Existing(id) => {
                        if let Some(t) = model.casework_types.get(id) {
                            ui.label(format!("{:.3} × {:.3} × {:.3} m", t.parameters.width, t.parameters.depth, t.parameters.height));
                        }
                        ui.label("Edit shared dimensions and material in Properties after placement.");
                    }
                }
                let validation = draft.resolved_type(model)
                    .and_then(|t| validate_parameters(model, &draft.parameters, t));
                if let Err(error) = validation {
                    ui.colored_label(theme::ERROR, error.to_string());
                }
                cancel = ui.button("Cancel casework placement").clicked();
            });
            if cancel {
                self.plans.casework_placement = None;
            }
            return true;
        }
        let Some(instance) = self
            .selected
            .and_then(|id| self.editor.document.model().casework.get(&id))
            .cloned()
        else {
            self.plans.casework_edit = None;
            return false;
        };
        if self.plans.active_sheet.is_some() {
            return false;
        }
        if self.plans.casework_edit.is_none() {
            let Some(t) = self
                .editor
                .document
                .model()
                .casework_types
                .get(&instance.parameters.type_id)
            else {
                ui.colored_label(theme::ERROR, "Casework type missing");
                return true;
            };
            self.plans.casework_edit = Some(Edit {
                id: instance.id(),
                session: self.editor.document.session_id(),
                revision: self.editor.document.revision(),
                view: self.plans.active,
                context: self
                    .plans
                    .active
                    .and_then(|id| self.editor.native_plan_context(id).ok()),
                providers: plan_provider_signature(&self.editor),
                parameters: instance.parameters.clone(),
                type_parameters: t.parameters.clone(),
            });
        }
        let draft = self.plans.casework_edit.as_mut().unwrap();
        let model = self.editor.document.model();
        let (mut apply, mut cancel, mut delete) = (false, false, false);
        egui::ScrollArea::vertical()
            .id_salt("casework_properties")
            .show(ui, |ui| {
                ui.label("Casework · metres");
                let previous = draft.parameters.type_id;
                egui::ComboBox::from_id_salt("casework_edit_type")
                    .selected_text(
                        model
                            .casework_types
                            .get(&previous)
                            .map_or("Missing type", |t| t.parameters.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for (id, t) in &model.casework_types {
                            ui.selectable_value(
                                &mut draft.parameters.type_id,
                                *id,
                                &t.parameters.name,
                            );
                        }
                    });
                if previous != draft.parameters.type_id
                    && let Some(t) = model.casework_types.get(&draft.parameters.type_id)
                {
                    draft.type_parameters = t.parameters.clone();
                }
                instance_controls(ui, &mut draft.parameters, model, false);
                ui.separator();
                let users = 1 + model
                    .casework
                    .values()
                    .filter(|other| {
                        other.id() != draft.id
                            && other.parameters.type_id == draft.parameters.type_id
                    })
                    .count();
                ui.label(format!(
                    "Shared type · changes apply to {users} instance(s)"
                ));
                type_controls(ui, &mut draft.type_parameters, model);
                let commands = draft.commands(model);
                if let Err(error) = &commands {
                    ui.colored_label(theme::ERROR, error.to_string());
                }
                apply = ui
                    .add_enabled(
                        commands.is_ok_and(|commands| !commands.is_empty()),
                        egui::Button::new("Apply casework and shared type"),
                    )
                    .clicked();
                cancel = ui.button("Cancel casework edits").clicked();
                delete = ui.button("Delete casework").clicked();
                ui.label(format!("Stable ID: {}", draft.id));
            });
        if cancel {
            self.plans.casework_edit = None;
        } else if apply {
            self.apply_casework_properties();
        }
        if delete {
            self.delete_casework();
        }
        true
    }

    pub(super) fn apply_casework_properties(&mut self) {
        if self
            .plans
            .casework_edit
            .as_ref()
            .is_some_and(|draft| draft.stale(self))
        {
            self.plans.casework_edit = None;
            self.report(Err(Error::Invalid("Casework edit is stale".into())), "");
            return;
        }
        let result = (|| {
            let draft = self
                .plans
                .casework_edit
                .as_ref()
                .ok_or_else(|| Error::Invalid("No casework edit".into()))?;
            let commands = draft.commands(self.editor.document.model())?;
            os_core::ensure(!commands.is_empty(), "No casework properties changed")?;
            self.editor
                .commands("Edit casework and shared type", commands)
        })();
        if result.is_ok() {
            self.plans.casework_edit = None;
        }
        self.report(result, "Casework updated.");
    }

    pub(super) fn delete_casework(&mut self) {
        if self
            .plans
            .casework_edit
            .as_ref()
            .is_some_and(|draft| draft.stale(self))
        {
            self.plans.casework_edit = None;
            self.report(Err(Error::Invalid("Casework edit is stale".into())), "");
            return;
        }
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().casework.contains_key(id))
        else {
            return;
        };
        let result = self
            .editor
            .command("Delete casework", Command::RemoveCasework(id));
        if result.is_ok() {
            self.plans.casework_edit = None;
            self.select(None);
        }
        self.report(result, "Casework deleted. Shared type retained.");
    }
}

fn instance_controls(ui: &mut egui::Ui, p: &mut CaseworkParams, model: &Model, placement: bool) {
    ui.label("Instance name");
    ui.text_edit_singleline(&mut p.name);
    egui::ComboBox::from_id_salt("casework_level")
        .selected_text(
            model
                .levels
                .get(&p.level)
                .map_or("Missing level", |level| level.parameters.name.as_str()),
        )
        .show_ui(ui, |ui| {
            for (id, level) in &model.levels {
                ui.selectable_value(&mut p.level, *id, &level.parameters.name);
            }
        });
    egui::Grid::new("casework_instance_numeric")
        .num_columns(2)
        .show(ui, |ui| {
            if !placement {
                for (label, value) in [("Center X", &mut p.center.x), ("Center Y", &mut p.center.y)]
                {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                    ui.end_row();
                }
            }
            let mut degrees = p.yaw.to_degrees();
            ui.label("Rotation (degrees)");
            if ui
                .add(
                    egui::DragValue::new(&mut degrees)
                        .speed(1.0)
                        .max_decimals(6),
                )
                .changed()
            {
                p.yaw = degrees.to_radians();
            }
            ui.end_row();
            ui.label("Base offset");
            ui.add(
                egui::DragValue::new(&mut p.base_offset)
                    .speed(0.01)
                    .max_decimals(6),
            );
            ui.end_row();
        });
}

fn type_controls(ui: &mut egui::Ui, p: &mut CaseworkTypeParams, model: &Model) {
    ui.label("Type name");
    ui.text_edit_singleline(&mut p.name);
    egui::Grid::new("casework_type_numeric")
        .num_columns(2)
        .show(ui, |ui| {
            for (label, value) in [
                ("Width", &mut p.width),
                ("Depth", &mut p.depth),
                ("Height", &mut p.height),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                ui.end_row();
            }
        });
    egui::ComboBox::from_id_salt("casework_material")
        .selected_text(
            p.material
                .and_then(|id| model.materials.get(&id))
                .map_or("No material", |material| material.parameters.name.as_str()),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut p.material, None, "No material");
            for (id, material) in &model.materials {
                ui.selectable_value(&mut p.material, Some(*id), &material.parameters.name);
            }
        });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn preview(
    editor: &Editor,
    draft: &Placement,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    pointer: egui::Pos2,
    snaps: SnapOptions,
) -> Result<(CaseworkParams, CaseworkTypeParams, PlanFootprint)> {
    os_core::ensure(
        context == draft.context && !draft.stale(editor, Some(context.view_id)),
        "Casework placement is stale",
    )?;
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let local = Point2::new(
        f64::from(pointer.x - rect.left()),
        f64::from(pointer.y - rect.top()),
    );
    let target = floor_snap_target(drawing, context, camera, size, local, snaps)?;
    let mut parameters = draft.parameters.clone();
    parameters.center = context.basis.plane_to_world(target)?;
    let model = editor.document.model();
    let casework_type = draft.resolved_type(model)?;
    let footprint = footprint(model, &parameters, casework_type, context)?;
    Ok((parameters, casework_type.parameters.clone(), footprint))
}

pub(super) fn paint(
    painter: &egui::Painter,
    footprint: &PlanFootprint,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    columns::paint(painter, footprint, camera, rect);
}
