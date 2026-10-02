//! Staged native curtain assembly properties, grids and component assignments.
use super::*;
use os_core::ensure;
use os_model::{
    CurtainGrid, CurtainMullion, CurtainMullionType, CurtainMullionTypeParams, CurtainPanel,
    CurtainPanelKind, CurtainPanelType, CurtainPanelTypeParams, CurtainSystemParams,
    MAX_CURTAIN_GRIDS,
};

type PanelTypeChoice = (Id, String);
type MullionTypeChoice = (Id, String);
type PanelKey = [Id; 4];
type MullionKey = (Id, [Id; 2]);

pub(crate) struct Draft {
    id: Id,
    view: Id,
    context: PlanContext,
    drawing: Id,
    session: Id,
    revision: u64,
    providers: Vec<(String, Id)>,
    selected_ids: BTreeSet<Id>,
    original: CurtainSystemParams,
    pub(super) parameters: CurtainSystemParams,
    panel_cache: BTreeMap<PanelKey, CurtainPanel>,
    mullion_cache: BTreeMap<MullionKey, CurtainMullion>,
    pub(super) height: String,
    base_offset: String,
    pub(super) panel_types: Vec<(Id, CurtainPanelTypeParams)>,
    pub(super) mullion_types: Vec<(Id, CurtainMullionTypeParams)>,
    reset_panels: BTreeSet<Id>,
    reset_mullions: BTreeSet<Id>,
    pub(super) panel_name: String,
    pub(super) panel_kind: CurtainPanelKind,
    pub(super) panel_thickness: String,
    panel_material: Option<Id>,
    pub(super) mullion_name: String,
    pub(super) mullion_width: String,
    pub(super) mullion_depth: String,
    mullion_material: Option<Id>,
    pub(super) validation: Result<()>,
    changed: bool,
    #[cfg(test)]
    pub(super) remove_vertical_rect: Option<egui::Rect>,
}

fn number(text: &str, label: &str) -> Result<f64> {
    let value: f64 = text
        .trim()
        .parse()
        .map_err(|_| Error::Invalid(format!("{label} must be a number in metres")))?;
    ensure(value.is_finite(), format!("{label} must be finite"))?;
    Ok(value)
}

fn type_choices(model: &Model, draft: &Draft) -> (Vec<PanelTypeChoice>, Vec<MullionTypeChoice>) {
    let panels = model
        .curtain_panel_types
        .iter()
        .map(|(id, entity)| (*id, entity.parameters.name.clone()))
        .chain(
            draft
                .panel_types
                .iter()
                .map(|(id, parameters)| (*id, parameters.name.clone())),
        )
        .collect();
    let mullions = model
        .curtain_mullion_types
        .iter()
        .map(|(id, entity)| (*id, entity.parameters.name.clone()))
        .chain(
            draft
                .mullion_types
                .iter()
                .map(|(id, parameters)| (*id, parameters.name.clone())),
        )
        .collect();
    (panels, mullions)
}

fn panel_choices(ui: &mut egui::Ui, label: &str, selected: &mut Id, choices: &[PanelTypeChoice]) {
    let current = choices
        .iter()
        .find(|(id, _)| id == selected)
        .map_or("Missing panel type", |(_, name)| name.as_str());
    egui::ComboBox::from_id_salt(label)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (id, name) in choices {
                ui.selectable_value(selected, *id, format!("{name} · {id}"));
            }
        });
}

fn mullion_choices(
    ui: &mut egui::Ui,
    label: &str,
    selected: &mut Id,
    choices: &[MullionTypeChoice],
) {
    let current = choices
        .iter()
        .find(|(id, _)| id == selected)
        .map_or("Missing mullion type", |(_, name)| name.as_str());
    egui::ComboBox::from_id_salt(label)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (id, name) in choices {
                ui.selectable_value(selected, *id, format!("{name} · {id}"));
            }
        });
}

fn proposed_topology(
    parameters: &CurtainSystemParams,
) -> (BTreeSet<PanelKey>, BTreeSet<MullionKey>) {
    let mut panels = BTreeSet::new();
    let mut mullions = BTreeSet::new();
    if let (Some(bottom), Some(top)) = (parameters.horizontal.first(), parameters.horizontal.last())
    {
        for vertical in &parameters.vertical {
            mullions.insert((vertical.id, [bottom.id, top.id]));
        }
        for vertical_pair in parameters.vertical.windows(2) {
            for horizontal in &parameters.horizontal {
                mullions.insert((horizontal.id, [vertical_pair[0].id, vertical_pair[1].id]));
            }
            for horizontal_pair in parameters.horizontal.windows(2) {
                panels.insert([
                    vertical_pair[0].id,
                    vertical_pair[1].id,
                    horizontal_pair[0].id,
                    horizontal_pair[1].id,
                ]);
            }
        }
    }
    (panels, mullions)
}

fn disappearing_customized_from(
    previous: &CurtainSystemParams,
    candidate: &CurtainSystemParams,
) -> (Vec<Id>, Vec<Id>) {
    let (panels, mullions) = proposed_topology(candidate);
    let removed_panels = previous
        .panels
        .iter()
        .filter(|panel| {
            !panels.contains(&panel.boundaries) && panel.panel_type != previous.panel_type
        })
        .map(|panel| panel.id)
        .collect();
    let removed_mullions = previous
        .mullions
        .iter()
        .filter(|member| {
            !mullions.contains(&(member.grid, member.span))
                && member.mullion_type != previous.mullion_type
        })
        .map(|member| member.id)
        .collect();
    (removed_panels, removed_mullions)
}

fn disappearing_customized(parameters: &CurtainSystemParams) -> (Vec<Id>, Vec<Id>) {
    disappearing_customized_from(parameters, parameters)
}

fn midpoint_of_widest_gap(grids: &[CurtainGrid]) -> Option<f64> {
    grids
        .windows(2)
        .max_by(|a, b| (a[1].position - a[0].position).total_cmp(&(b[1].position - b[0].position)))
        .map(|pair| (pair[0].position + pair[1].position) / 2.0)
}

impl Draft {
    fn reset_candidates(&self) -> (BTreeSet<Id>, BTreeSet<Id>) {
        let (mut panels, mut mullions) = disappearing_customized(&self.parameters);
        let (previous_panels, previous_mullions) =
            disappearing_customized_from(&self.original, &self.parameters);
        panels.extend(previous_panels);
        mullions.extend(previous_mullions);
        (panels.into_iter().collect(), mullions.into_iter().collect())
    }

    fn current(&self, app: &DesktopApp) -> bool {
        self.session == app.editor.document.session_id()
            && self.revision == app.editor.document.revision()
            && app.plans.active == Some(self.view)
            && app.plans.active_sheet.is_none()
            && app.selected == Some(self.id)
            && app.selected_ids == self.selected_ids
            && app.selected_ids.len() == 1
            && !app.editor.plugin_work_pending()
            && curtains::providers_ready(app, self.view)
            && plan_provider_signature(&app.editor) == self.providers
            && app.editor.native_plan_context(self.view).ok() == Some(self.context)
            && curtains::displayed_drawing(app, self.view).is_some_and(|drawing| {
                drawing.identity() == self.drawing
                    && drawing
                        .provider_lines(self.context)
                        .is_ok_and(|lines| lines.iter().any(|line| line.entity == self.id))
            })
    }

    fn parameters(&self) -> Result<CurtainSystemParams> {
        let mut parameters = self.parameters.clone();
        parameters.height = number(&self.height, "Height")?;
        parameters.base_offset = number(&self.base_offset, "Base offset")?;
        let height = parameters.height;
        let top = parameters
            .horizontal
            .last_mut()
            .ok_or_else(|| Error::Invalid("Curtain needs a top perimeter grid".into()))?;
        top.position = height;
        Ok(parameters)
    }

    fn reconcile_candidate(&mut self) -> Result<()> {
        for panel in &self.parameters.panels {
            self.panel_cache.insert(panel.boundaries, panel.clone());
        }
        for member in &self.parameters.mullions {
            self.mullion_cache
                .insert((member.grid, member.span), member.clone());
        }

        // Preserve identities and assignments for topology that temporarily
        // disappears while the user stages grid edits.
        let (panel_keys, mullion_keys) = proposed_topology(&self.parameters);
        for key in panel_keys {
            if !self
                .parameters
                .panels
                .iter()
                .any(|panel| panel.boundaries == key)
                && let Some(panel) = self.panel_cache.get(&key)
            {
                self.parameters.panels.push(panel.clone());
            }
        }
        for key in mullion_keys {
            if !self
                .parameters
                .mullions
                .iter()
                .any(|member| (member.grid, member.span) == key)
                && let Some(member) = self.mullion_cache.get(&key)
            {
                self.parameters.mullions.push(member.clone());
            }
        }
        self.parameters.reconcile()?;
        for panel in &self.parameters.panels {
            self.panel_cache.insert(panel.boundaries, panel.clone());
        }
        for member in &self.parameters.mullions {
            self.mullion_cache
                .insert((member.grid, member.span), member.clone());
        }
        Ok(())
    }

    fn commands(&self) -> Result<Vec<Command>> {
        let parameters = self.parameters()?;
        ensure(
            parameters != self.original
                || !self.panel_types.is_empty()
                || !self.mullion_types.is_empty(),
            "No curtain properties changed",
        )?;
        let mut commands = Vec::new();
        for (id, parameters) in &self.panel_types {
            let mut entity = CurtainPanelType::new("core.curtain_panel_type", parameters.clone());
            entity.header.id = *id;
            commands.push(Command::AddCurtainPanelType(entity));
        }
        for (id, parameters) in &self.mullion_types {
            let mut entity =
                CurtainMullionType::new("core.curtain_mullion_type", parameters.clone());
            entity.header.id = *id;
            commands.push(Command::AddCurtainMullionType(entity));
        }

        if parameters.validate_transition(&self.original).is_ok() {
            commands.push(Command::UpdateCurtainSystem {
                id: self.id,
                parameters,
            });
            return Ok(commands);
        }

        let (custom_panels, custom_mullions) =
            disappearing_customized_from(&self.original, &parameters);
        ensure(
            custom_panels
                .iter()
                .all(|id| self.reset_panels.contains(id))
                && custom_mullions
                    .iter()
                    .all(|id| self.reset_mullions.contains(id)),
            "Removing customized bays requires the explicit reset-and-continue action",
        )?;

        // The document deliberately rejects a single replacement that discards
        // customized children. Reset only removed children first, retaining the
        // original topology, then replace topology in the same transaction.
        let (new_panels, new_mullions) = proposed_topology(&parameters);
        let mut intermediate = self.original.clone();
        for panel in &mut intermediate.panels {
            if !new_panels.contains(&panel.boundaries) {
                panel.panel_type = self.original.panel_type;
            }
        }
        for member in &mut intermediate.mullions {
            if !new_mullions.contains(&(member.grid, member.span)) {
                member.mullion_type = self.original.mullion_type;
            }
        }
        intermediate.validate_transition(&self.original)?;
        parameters.validate_transition(&intermediate)?;
        commands.push(Command::UpdateCurtainSystem {
            id: self.id,
            parameters: intermediate,
        });
        commands.push(Command::UpdateCurtainSystem {
            id: self.id,
            parameters,
        });
        Ok(commands)
    }

    pub(super) fn refresh(&mut self, editor: &Editor) {
        self.changed = self.parameters != self.original
            || !self.panel_types.is_empty()
            || !self.mullion_types.is_empty()
            || self.height.trim() != self.original.height.to_string()
            || self.base_offset.trim() != self.original.base_offset.to_string();
        let result = (|| {
            let inputs = self.parameters()?;
            self.parameters.height = inputs.height;
            self.parameters.base_offset = inputs.base_offset;
            let top_position = inputs.height;
            self.parameters
                .horizontal
                .last_mut()
                .ok_or_else(|| Error::Invalid("Curtain needs a top perimeter grid".into()))?
                .position = top_position;
            // Moving an interior station past another reorders only the interior
            // UUIDs; perimeter identities remain locked at the ends.
            let vertical_len = self.parameters.vertical.len();
            if vertical_len > 2 {
                self.parameters.vertical[1..vertical_len - 1]
                    .sort_by(|a, b| a.position.total_cmp(&b.position));
            }
            let horizontal_len = self.parameters.horizontal.len();
            if horizontal_len > 2 {
                self.parameters.horizontal[1..horizontal_len - 1]
                    .sort_by(|a, b| a.position.total_cmp(&b.position));
            }
            self.reconcile_candidate()?;
            let candidate = self.parameters()?;
            self.changed = self.parameters != self.original
                || !self.panel_types.is_empty()
                || !self.mullion_types.is_empty();
            if !self.changed {
                return Ok(());
            }
            let commands = self.commands()?;
            let model = editor.document.preview_commands(commands)?;
            candidate.resolve(&model)?;
            Ok(())
        })();
        self.validation = result;
    }

    fn prepare(&self, editor: &Editor) -> Result<(Vec<Command>, Editor)> {
        let commands = self.commands()?;
        let mut staged = Editor::new()?;
        staged.document = Document::from_model(editor.document.model().clone())?;
        staged.scene = editor.scene.clone();
        staged.pending_geometry = editor.pending_geometry.clone();
        staged
            .document
            .execute("Edit curtain assembly", commands.clone())?;
        staged.regenerate()?;
        staged.native_drawing(self.view)?;
        Ok((commands, staged))
    }

    pub(super) fn add_grid(&mut self, vertical: bool) -> Result<()> {
        let grids = if vertical {
            &mut self.parameters.vertical
        } else {
            &mut self.parameters.horizontal
        };
        ensure(
            grids.len() < MAX_CURTAIN_GRIDS,
            "Curtain grid limit is 18 per direction",
        )?;
        ensure(
            grids.len() >= 2
                && grids.first().is_some_and(|grid| grid.position == 0.0)
                && grids
                    .windows(2)
                    .all(|pair| pair[1].position - pair[0].position > 0.001),
            "Fix the ordered perimeter and interior stations before adding a grid",
        )?;
        let position = midpoint_of_widest_gap(grids)
            .ok_or_else(|| Error::Invalid("Curtain perimeter grids are invalid".into()))?;
        ensure(
            grids
                .windows(2)
                .all(|pair| pair[1].position - pair[0].position > 0.002),
            "Curtain grid spacing is too small to add another station",
        )?;
        let index = grids
            .iter()
            .position(|grid| grid.position > position)
            .unwrap_or(grids.len() - 1);
        grids.insert(
            index,
            CurtainGrid {
                id: Id::new(),
                position,
            },
        );
        Ok(())
    }

    pub(super) fn remove_grid(&mut self, vertical: bool, id: Id) {
        let grids = if vertical {
            &mut self.parameters.vertical
        } else {
            &mut self.parameters.horizontal
        };
        if grids.first().is_some_and(|grid| grid.id == id)
            || grids.last().is_some_and(|grid| grid.id == id)
        {
            return;
        }
        grids.retain(|grid| grid.id != id);
    }

    pub(super) fn reset_removed_custom_assignments(&mut self) -> Result<()> {
        let (custom_panels, custom_mullions) = self.reset_candidates();
        ensure(
            !custom_panels.is_empty() || !custom_mullions.is_empty(),
            "There are no customized assignments to reset",
        )?;
        self.reset_panels.extend(custom_panels.iter().copied());
        self.reset_mullions.extend(custom_mullions.iter().copied());
        let default_panel = self.parameters.panel_type;
        let default_mullion = self.parameters.mullion_type;
        for panel in &mut self.parameters.panels {
            if custom_panels.contains(&panel.id) {
                panel.panel_type = default_panel;
                self.reset_panels.insert(panel.id);
            }
        }
        for member in &mut self.parameters.mullions {
            if custom_mullions.contains(&member.id) {
                member.mullion_type = default_mullion;
                self.reset_mullions.insert(member.id);
            }
        }
        self.reconcile_candidate()?;
        Ok(())
    }

    pub(super) fn add_panel_type(&mut self, model: &Model) -> Result<()> {
        let parameters = CurtainPanelTypeParams {
            name: self.panel_name.trim().to_owned(),
            kind: self.panel_kind,
            thickness: number(&self.panel_thickness, "Panel thickness")?,
            material: self.panel_material,
        };
        ensure(
            !parameters.name.is_empty()
                && parameters.name.len() <= 256
                && !parameters.name.chars().any(char::is_control),
            "Panel type name must be nonempty and at most 256 characters",
        )?;
        ensure(
            parameters.thickness.is_finite()
                && parameters.thickness > 0.001
                && parameters.thickness <= 1000.0,
            "Panel thickness must be greater than 0.001 m",
        )?;
        ensure(
            parameters
                .material
                .is_none_or(|id| model.materials.contains_key(&id)),
            "Panel material is missing",
        )?;
        ensure(
            model.curtain_panel_types.len() + self.panel_types.len() < 256,
            "Curtain panel type limit reached",
        )?;
        let id = Id::new();
        self.panel_types.push((id, parameters));
        self.panel_name = format!("Curtain panel {}", self.panel_types.len() + 1);
        Ok(())
    }

    pub(super) fn add_mullion_type(&mut self, model: &Model) -> Result<()> {
        let parameters = CurtainMullionTypeParams {
            name: self.mullion_name.trim().to_owned(),
            width: number(&self.mullion_width, "Mullion width")?,
            depth: number(&self.mullion_depth, "Mullion depth")?,
            material: self.mullion_material,
        };
        ensure(
            !parameters.name.is_empty()
                && parameters.name.len() <= 256
                && !parameters.name.chars().any(char::is_control),
            "Mullion type name must be nonempty and at most 256 characters",
        )?;
        ensure(
            parameters.width.is_finite()
                && parameters.depth.is_finite()
                && parameters.width > 0.001
                && parameters.depth > 0.001
                && parameters.width <= 1000.0
                && parameters.depth <= 1000.0,
            "Mullion width and depth must be greater than 0.001 m",
        )?;
        ensure(
            parameters
                .material
                .is_none_or(|id| model.materials.contains_key(&id)),
            "Mullion material is missing",
        )?;
        ensure(
            model.curtain_mullion_types.len() + self.mullion_types.len() < 256,
            "Curtain mullion type limit reached",
        )?;
        let id = Id::new();
        self.mullion_types.push((id, parameters));
        self.mullion_name = format!("Curtain mullion {}", self.mullion_types.len() + 1);
        Ok(())
    }
}

impl DesktopApp {
    pub(crate) fn can_edit_curtain_properties(&mut self, id: Id) -> bool {
        if self.selected != Some(id)
            || self.selected_ids.len() != 1
            || self.plans.active_sheet.is_some()
            || self.editor.plugin_work_pending()
        {
            return false;
        }
        let Some(view) = self.plans.active else {
            return false;
        };
        if !curtains::providers_ready(self, view) {
            return false;
        }
        let Ok(context) = self.editor.native_plan_context(view) else {
            return false;
        };
        curtains::displayed_drawing(self, view).is_some_and(|drawing| {
            drawing
                .provider_lines(context)
                .is_ok_and(|lines| lines.iter().any(|line| line.entity == id))
        })
    }

    pub(crate) fn begin_curtain_properties(&mut self, id: Id) {
        if !self.can_edit_curtain_properties(id) {
            return;
        }
        let Some(view) = self.plans.active else {
            return;
        };
        let (Ok(context), Some(drawing), Some(assembly)) = (
            self.editor.native_plan_context(view),
            curtains::displayed_drawing(self, view),
            self.editor.document.model().curtain_systems.get(&id),
        ) else {
            return;
        };
        let original = assembly.parameters.clone();
        let panel_cache = original
            .panels
            .iter()
            .map(|panel| (panel.boundaries, panel.clone()))
            .collect();
        let mullion_cache = original
            .mullions
            .iter()
            .map(|member| ((member.grid, member.span), member.clone()))
            .collect();
        let mut draft = Draft {
            id,
            view,
            context,
            drawing: drawing.identity(),
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            providers: plan_provider_signature(&self.editor),
            selected_ids: self.selected_ids.clone(),
            height: original.height.to_string(),
            base_offset: original.base_offset.to_string(),
            parameters: original.clone(),
            panel_cache,
            mullion_cache,
            original,
            panel_types: Vec::new(),
            mullion_types: Vec::new(),
            reset_panels: BTreeSet::new(),
            reset_mullions: BTreeSet::new(),
            panel_name: "New curtain panel".into(),
            panel_kind: CurtainPanelKind::Glazing,
            panel_thickness: "0.02".into(),
            panel_material: None,
            mullion_name: "New curtain frame".into(),
            mullion_width: "0.05".into(),
            mullion_depth: "0.1".into(),
            mullion_material: None,
            validation: Ok(()),
            changed: false,
            #[cfg(test)]
            remove_vertical_rect: None,
        };
        draft.refresh(&self.editor);
        self.cancel_plan_wall();
        self.cancel_opening_placement();
        self.cancel_opening_controls();
        self.cancel_aligned_dimension();
        self.plans.floor_sketch = None;
        self.plans.floor_hole_sketch = None;
        self.plans.floor_vertex_drag = None;
        self.plans.area_selection.cancel();
        self.plans.floor_properties = None;
        self.plans.curtain_properties = Some(draft);
    }

    pub(crate) fn apply_curtain_properties(&mut self) -> Result<()> {
        let (commands, staged) = {
            let draft = self
                .plans
                .curtain_properties
                .as_ref()
                .ok_or_else(|| Error::Invalid("No curtain properties draft".into()))?;
            ensure(
                draft.current(self),
                "Curtain properties discarded: selection, plan or document changed",
            )?;
            let prepared = draft.prepare(&self.editor)?;
            ensure(
                draft.current(self),
                "Curtain properties discarded: plan or provider context changed during preflight",
            )?;
            prepared
        };
        self.editor
            .document
            .execute("Edit curtain assembly", commands)?;
        self.editor.scene = staged.scene;
        self.editor.pending_geometry = staged.pending_geometry;
        self.plans.curtain_properties = None;
        Ok(())
    }

    pub(crate) fn curtain_properties_dialog(&mut self, ctx: &egui::Context) {
        if self
            .plans
            .curtain_properties
            .as_ref()
            .is_some_and(|draft| !draft.current(self))
        {
            self.plans.curtain_properties = None;
            self.status =
                "Curtain properties discarded: selection, plan or document changed.".into();
            return;
        }
        let Some(mut draft) = self.plans.curtain_properties.take() else {
            return;
        };
        let mut cancel = false;
        let mut apply = false;
        let mut action_error = None;
        let modal = egui::Modal::new(egui::Id::new("curtain_properties")).show(ctx, |ui| {
            ui.set_width(640.0);
            ui.heading("Edit curtain assembly");
            ui.label("Changes are staged until Apply. Dimensions and grid stations are in metres.");
            let model = self.editor.document.model();
            let (panel_types, mullion_types) = type_choices(model, &draft);
            let view_level = model.views[&draft.view].parameters.level;
            let level_name = view_level
                .and_then(|level| model.levels.get(&level))
                .map_or("Missing level", |level| level.parameters.name.as_str());

            egui::ScrollArea::vertical()
                .id_salt("curtain_properties_scroll")
                .max_height(520.0)
                .show(ui, |ui| {
                    ui.label(format!("Level: {level_name} · Baseline: {:.3} m", draft.parameters.start.distance(draft.parameters.end)));
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut draft.parameters.name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height (m)");
                        ui.text_edit_singleline(&mut draft.height);
                        ui.label("Base offset (m)");
                        ui.text_edit_singleline(&mut draft.base_offset);
                        ui.checkbox(&mut draft.parameters.normal_flip, "Flip normal");
                    });

                    ui.separator();
                    ui.heading("Defaults for newly created components");
                    ui.horizontal(|ui| {
                        ui.label("New panel type");
                        panel_choices(ui, "curtain_default_panel_type", &mut draft.parameters.panel_type, &panel_types);
                        if ui.button("Assign default to all panels").clicked() {
                            let default = draft.parameters.panel_type;
                            for panel in &mut draft.parameters.panels {
                                panel.panel_type = default;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("New mullion type");
                        mullion_choices(ui, "curtain_default_mullion_type", &mut draft.parameters.mullion_type, &mullion_types);
                        if ui.button("Assign default to all mullions").clicked() {
                            let default = draft.parameters.mullion_type;
                            for member in &mut draft.parameters.mullions {
                                member.mullion_type = default;
                            }
                        }
                    });

                    ui.separator();
                    ui.heading("Vertical grids · station from baseline start");
                    ui.horizontal(|ui| {
                        ui.strong("Perimeter 0.000 m");
                        ui.label("Locked");
                    });
                    let vertical = draft.parameters.vertical.clone();
                    for grid in vertical.iter().skip(1).take(vertical.len().saturating_sub(2)) {
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut draft.parameters.vertical.iter_mut().find(|candidate| candidate.id == grid.id).unwrap().position).speed(0.05).range(0.0..=1000.0));
                            ui.label("m");
                            let remove = ui.small_button("Remove vertical grid");
                            #[cfg(test)]
                            {
                                draft.remove_vertical_rect = Some(remove.rect);
                            }
                            if remove.clicked() {
                                draft.remove_grid(true, grid.id);
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.strong(format!("Perimeter {:.3} m", draft.parameters.start.distance(draft.parameters.end)));
                        ui.label("Locked");
                        if ui.button("Add vertical grid").clicked()
                            && let Err(error) = draft.add_grid(true)
                        {
                            draft.validation = Err(error);
                        }
                    });

                    ui.separator();
                    ui.heading("Horizontal grids · elevation above assembly base");
                    ui.horizontal(|ui| {
                        ui.strong("Perimeter 0.000 m");
                        ui.label("Locked");
                    });
                    let horizontal = draft.parameters.horizontal.clone();
                    for grid in horizontal.iter().skip(1).take(horizontal.len().saturating_sub(2)) {
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut draft.parameters.horizontal.iter_mut().find(|candidate| candidate.id == grid.id).unwrap().position).speed(0.05).range(0.0..=1000.0));
                            ui.label("m");
                            if ui.small_button("Remove horizontal grid").clicked() {
                                draft.remove_grid(false, grid.id);
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.strong(format!("Perimeter {:.3} m", number(&draft.height, "Height").unwrap_or(draft.parameters.height)));
                        ui.label("Locked");
                        if ui.button("Add horizontal grid").clicked()
                            && let Err(error) = draft.add_grid(false)
                        {
                            draft.validation = Err(error);
                        }
                    });

                    let (reset_panels, reset_mullions) = draft.reset_candidates();
                    if !reset_panels.is_empty() || !reset_mullions.is_empty() {
                        ui.colored_label(
                            theme::ERROR,
                            format!(
                                "Removing these grids requires resetting {} panel assignments and {} mullion assignments.",
                                reset_panels.len(),
                                reset_mullions.len()
                            ),
                        );
                        if ui.button("Reset affected assignments and continue").clicked()
                            && let Err(error) = draft.reset_removed_custom_assignments()
                        {
                            draft.validation = Err(error);
                        }
                    }

                    ui.separator();
                    ui.heading("Panel assignments");
                    if draft.parameters.panels.is_empty() {
                        ui.label("No panels in the current grid topology.");
                    }
                    let positions: BTreeMap<_, _> = draft.parameters.vertical.iter().chain(&draft.parameters.horizontal).map(|grid| (grid.id, grid.position)).collect();
                    for panel in &mut draft.parameters.panels {
                        let [left, right, bottom, top] = panel.boundaries;
                        let label = match (
                            positions.get(&left),
                            positions.get(&right),
                            positions.get(&bottom),
                            positions.get(&top),
                        ) {
                            (Some(left), Some(right), Some(bottom), Some(top)) => format!(
                                "Bay {:.2}–{:.2} m · {:.2}–{:.2} m",
                                left, right, bottom, top
                            ),
                            _ => format!("Removing bay · {}", panel.id),
                        };
                        ui.horizontal(|ui| {
                            ui.label(label);
                            panel_choices(ui, &format!("curtain_panel_{}", panel.id), &mut panel.panel_type, &panel_types);
                        });
                    }

                    ui.separator();
                    ui.heading("Mullion assignments");
                    let vertical_ids: BTreeSet<_> = draft.parameters.vertical.iter().map(|grid| grid.id).collect();
                    let positions: BTreeMap<_, _> = draft.parameters.vertical.iter().chain(&draft.parameters.horizontal).map(|grid| (grid.id, grid.position)).collect();
                    for member in &mut draft.parameters.mullions {
                        let vertical = vertical_ids.contains(&member.grid);
                        let label = if vertical {
                            positions.get(&member.grid).map_or_else(
                                || format!("Removing mullion · {}", member.id),
                                |station| format!("Vertical · {:.2} m", station),
                            )
                        } else {
                            match (
                                positions.get(&member.grid),
                                positions.get(&member.span[0]),
                                positions.get(&member.span[1]),
                            ) {
                                (Some(station), Some(start), Some(end)) => format!(
                                    "Horizontal · {:.2} m · span {:.2}–{:.2} m",
                                    station, start, end
                                ),
                                _ => format!("Removing mullion · {}", member.id),
                            }
                        };
                        ui.horizontal(|ui| {
                            ui.label(label);
                            mullion_choices(ui, &format!("curtain_mullion_{}", member.id), &mut member.mullion_type, &mullion_types);
                        });
                    }

                    ui.collapsing("Create a panel type…", |ui| {
                        ui.text_edit_singleline(&mut draft.panel_name);
                        ui.horizontal(|ui| {
                            ui.label("Kind");
                            ui.selectable_value(&mut draft.panel_kind, CurtainPanelKind::Glazing, "Glazing");
                            ui.selectable_value(&mut draft.panel_kind, CurtainPanelKind::Opaque, "Opaque");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Thickness (m)"); ui.text_edit_singleline(&mut draft.panel_thickness);
                            egui::ComboBox::from_id_salt("curtain_panel_material").selected_text(draft.panel_material.and_then(|id| model.materials.get(&id)).map_or("No material", |material| material.parameters.name.as_str())).show_ui(ui, |ui| {
                                ui.selectable_value(&mut draft.panel_material, None, "No material");
                                for (id, material) in &model.materials { ui.selectable_value(&mut draft.panel_material, Some(*id), format!("{} · {id}", material.parameters.name)); }
                            });
                        });
                        if ui.button("Create panel type").clicked() {
                            action_error = draft.add_panel_type(model).err();
                        }
                    });
                    ui.collapsing("Create a mullion type…", |ui| {
                        ui.text_edit_singleline(&mut draft.mullion_name);
                        ui.horizontal(|ui| {
                            ui.label("Width (m)"); ui.text_edit_singleline(&mut draft.mullion_width);
                            ui.label("Depth (m)"); ui.text_edit_singleline(&mut draft.mullion_depth);
                            egui::ComboBox::from_id_salt("curtain_mullion_material").selected_text(draft.mullion_material.and_then(|id| model.materials.get(&id)).map_or("No material", |material| material.parameters.name.as_str())).show_ui(ui, |ui| {
                                ui.selectable_value(&mut draft.mullion_material, None, "No material");
                                for (id, material) in &model.materials { ui.selectable_value(&mut draft.mullion_material, Some(*id), format!("{} · {id}", material.parameters.name)); }
                            });
                        });
                        if ui.button("Create mullion type").clicked() {
                            action_error = draft.add_mullion_type(model).err();
                        }
                    });
                });

            draft.refresh(&self.editor);
            if let Some(error) = action_error {
                draft.validation = Err(error);
            }
            if let Err(error) = &draft.validation {
                ui.colored_label(theme::ERROR, error.to_string());
            }
            ui.horizontal(|ui| {
                let apply_response = ui.add_enabled(draft.changed && draft.validation.is_ok(), egui::Button::new("Apply"));
                apply = apply_response.clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        cancel |= modal.should_close();
        if !cancel {
            self.plans.curtain_properties = Some(draft);
            if apply {
                let result = self.apply_curtain_properties();
                self.report(result, "Curtain assembly updated.");
            }
        }
    }
}
