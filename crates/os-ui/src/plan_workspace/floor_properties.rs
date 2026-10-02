//! Disposable property edits with geometry prepared before publishing a transaction.
use super::*;
use os_core::ensure;
use os_model::FloorParams;

pub(crate) struct Draft {
    id: Id,
    view: Id,
    drawing: Id,
    session: Id,
    revision: u64,
    selected_ids: BTreeSet<Id>,
    original: FloorParams,
    pub name: String,
    pub level: Id,
    pub material: Option<Id>,
    pub thickness: String,
    pub top_offset: String,
    validation: Result<()>,
    changed: bool,
}

impl Draft {
    fn current(&self, app: &DesktopApp) -> bool {
        self.session == app.editor.document.session_id()
            && self.revision == app.editor.document.revision()
            && app.plans.active == Some(self.view)
            && app.selected_ids == self.selected_ids
            && app.plans.active_sheet.is_none()
            && !app.editor.plugin_work_pending()
            && app
                .plans
                .drawing
                .as_ref()
                .is_some_and(|drawing| drawing.identity() == self.drawing)
    }

    fn parameters(&self) -> Result<FloorParams> {
        let mut parameters = self.original.clone();
        parameters.name = self.name.clone();
        parameters.level = self.level;
        parameters.material = self.material;
        parameters.thickness = self
            .thickness
            .trim()
            .parse()
            .map_err(|_| Error::Invalid("Thickness must be a number in metres".into()))?;
        parameters.top_offset = self
            .top_offset
            .trim()
            .parse()
            .map_err(|_| Error::Invalid("Top offset must be a number in metres".into()))?;
        parameters.validate()?;
        Ok(parameters)
    }

    fn validate_candidate(&self, editor: &Editor) -> Result<FloorParams> {
        let parameters = self.parameters()?;
        let candidate = editor
            .document
            .preview_commands(vec![Command::UpdateFloor {
                id: self.id,
                parameters: parameters.clone(),
            }])?;
        let top = candidate.levels[&parameters.level].parameters.elevation + parameters.top_offset;
        os_geometry::floor_holes::extrude_floor_rings(
            &parameters.boundary,
            &parameters.holes,
            top,
            parameters.thickness,
        )?;
        let context = editor.native_plan_context(self.view)?;
        let boundary = parameters
            .boundary
            .iter()
            .map(|point| context.basis.world_to_plane(*point))
            .collect::<Result<Vec<_>>>()?;
        let holes = parameters
            .holes
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|point| context.basis.world_to_plane(*point))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        os_geometry::floor_holes::triangulate_floor_rings(&boundary, &holes)?;
        Ok(parameters)
    }

    fn prepare(&self, editor: &Editor) -> Result<(FloorParams, Editor)> {
        let parameters = self.validate_candidate(editor)?;
        // Regenerate against an isolated document with exactly the same command
        // and invalidations as commit. Neither history nor live meshes are touched.
        let mut staged = Editor::new()?;
        staged.document = Document::from_model(editor.document.model().clone())?;
        staged.scene = editor.scene.clone();
        staged.pending_geometry = editor.pending_geometry.clone();
        staged.document.execute(
            "Edit floor properties",
            vec![Command::UpdateFloor {
                id: self.id,
                parameters: parameters.clone(),
            }],
        )?;
        staged.regenerate()?;
        staged.native_drawing(self.view)?;
        Ok((parameters, staged))
    }
}

fn floor_intersects_plan_crop(floor: &PlanFloorItem, context: PlanContext) -> Result<bool> {
    let Some(crop) = context.crop else {
        return Ok(true);
    };
    for triangle in &floor.triangles {
        let vertices =
            triangle
                .iter()
                .map(|index| {
                    floor.vertices.get(*index as usize).copied().ok_or_else(|| {
                        Error::Invalid("Floor plan triangle is out of bounds".into())
                    })
                })
                .collect::<Result<Vec<_>>>()?;
        if os_geometry::plan::PlanFootprint::from_convex(
            os_geometry::plan::PlanRole::Projected,
            vertices,
            Some(crop),
        )?
        .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl DesktopApp {
    pub(crate) fn can_edit_floor_properties(&mut self, id: Id) -> bool {
        let selected_context_is_current = self.selected == Some(id)
            && self.selected_ids.len() == 1
            && self.plans.active_sheet.is_none()
            && !self.editor.plugin_work_pending();
        if !selected_context_is_current {
            return false;
        }
        if self.plans.floor_properties.as_ref().is_some_and(|draft| {
            draft.id == id
                && draft.session == self.editor.document.session_id()
                && draft.revision == self.editor.document.revision()
                && self.plans.active == Some(draft.view)
                && self.selected_ids == draft.selected_ids
                && self
                    .plans
                    .drawing
                    .as_ref()
                    .is_some_and(|drawing| drawing.identity() == draft.drawing)
        }) {
            return true;
        }
        let Some(context) = self
            .plans
            .active
            .and_then(|view| self.editor.native_plan_context(view).ok())
        else {
            return false;
        };
        let Some(drawing) = self.plans.drawing.as_ref() else {
            return false;
        };
        let drawing_id = drawing.identity();
        if let Some((cached_drawing, cached_floor, visible)) = self.plans.floor_visibility_cache
            && cached_drawing == drawing_id
            && cached_floor == id
        {
            return visible;
        }
        let visible = drawing.floors(context).is_ok_and(|floors| {
            floors.iter().any(|floor| {
                floor.entity == id && floor_intersects_plan_crop(floor, context).unwrap_or(false)
            })
        });
        self.plans.floor_visibility_cache = Some((drawing_id, id, visible));
        visible
    }

    pub(crate) fn begin_floor_properties(&mut self, id: Id) {
        if !self.can_edit_floor_properties(id) {
            return;
        }
        let Some(drawing) = self.plans.drawing.as_ref().map(PlanDrawing::identity) else {
            return;
        };
        let original = self.editor.document.model().floors[&id].parameters.clone();
        let mut draft = Draft {
            id,
            view: self.plans.active.unwrap(),
            drawing,
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            selected_ids: self.selected_ids.clone(),
            name: original.name.clone(),
            level: original.level,
            material: original.material,
            thickness: original.thickness.to_string(),
            top_offset: original.top_offset.to_string(),
            original,
            validation: Ok(()),
            changed: false,
        };
        let validation = draft.validate_candidate(&self.editor);
        draft.changed = validation
            .as_ref()
            .is_ok_and(|parameters| parameters != &draft.original);
        draft.validation = validation.map(|_| ());
        self.cancel_plan_wall();
        self.cancel_opening_placement();
        self.cancel_opening_controls();
        self.cancel_aligned_dimension();
        self.plans.floor_sketch = None;
        self.plans.floor_hole_sketch = None;
        self.plans.floor_vertex_drag = None;
        self.plans.area_selection.cancel();
        self.plans.floor_properties = Some(draft);
    }

    pub(crate) fn apply_floor_properties(&mut self) -> Result<()> {
        let draft = self
            .plans
            .floor_properties
            .as_ref()
            .ok_or_else(|| Error::Invalid("No floor properties draft".into()))?;
        if !draft.current(self) {
            self.plans.floor_properties = None;
            return Err(Error::Invalid(
                "Floor properties discarded: selection or document changed".into(),
            ));
        }
        let (parameters, staged) = draft.prepare(&self.editor)?;
        ensure(parameters != draft.original, "No floor properties changed")?;
        self.editor.document.execute(
            "Edit floor properties",
            vec![Command::UpdateFloor {
                id: draft.id,
                parameters,
            }],
        )?;
        // All fallible geometry work finished before execute. Keep document events
        // for the normal derived-output invalidation path, and publish ready meshes.
        self.editor.scene = staged.scene;
        self.editor.pending_geometry = staged.pending_geometry;
        self.plans.floor_properties = None;
        Ok(())
    }

    pub(crate) fn floor_properties_dialog(&mut self, ctx: &egui::Context) {
        if self
            .plans
            .floor_properties
            .as_ref()
            .is_some_and(|draft| !draft.current(self))
        {
            self.plans.floor_properties = None;
            self.status = "Floor properties discarded: selection, view or document changed.".into();
            return;
        }
        let Some(mut draft) = self.plans.floor_properties.take() else {
            return;
        };
        let mut cancel = false;
        let mut apply = false;
        let modal = egui::Modal::new(egui::Id::new("floor_properties")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading("Edit floor properties");
            ui.label("Changes are staged until Apply. Dimensions are in metres.");
            let previous = (
                draft.name.clone(),
                draft.level,
                draft.material,
                draft.thickness.clone(),
                draft.top_offset.clone(),
            );
            ui.label("Name");
            ui.text_edit_singleline(&mut draft.name);
            let model = self.editor.document.model();
            egui::ComboBox::from_label("Level")
                .selected_text(
                    model
                        .levels
                        .get(&draft.level)
                        .map_or("Missing level", |level| level.parameters.name.as_str()),
                )
                .show_ui(ui, |ui| {
                    for (id, level) in &model.levels {
                        ui.selectable_value(
                            &mut draft.level,
                            *id,
                            format!("{} · {id}", level.parameters.name),
                        );
                    }
                });
            egui::ComboBox::from_label("Material")
                .selected_text(
                    draft
                        .material
                        .and_then(|id| model.materials.get(&id))
                        .map_or("None", |material| material.parameters.name.as_str()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.material, None, "None");
                    for (id, material) in &model.materials {
                        ui.selectable_value(
                            &mut draft.material,
                            Some(*id),
                            format!("{} · {id}", material.parameters.name),
                        );
                    }
                });
            ui.label("Thickness (m)");
            ui.text_edit_singleline(&mut draft.thickness);
            ui.label("Top offset (m)");
            ui.text_edit_singleline(&mut draft.top_offset);
            if previous
                != (
                    draft.name.clone(),
                    draft.level,
                    draft.material,
                    draft.thickness.clone(),
                    draft.top_offset.clone(),
                )
            {
                let validation = draft.validate_candidate(&self.editor);
                draft.changed = validation
                    .as_ref()
                    .is_ok_and(|parameters| parameters != &draft.original);
                draft.validation = validation.map(|_| ());
            }
            if let Err(error) = &draft.validation {
                ui.colored_label(theme::ERROR, error.to_string());
            }
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(
                        draft.changed && draft.validation.is_ok(),
                        egui::Button::new("Apply"),
                    )
                    .clicked();
                cancel |= ui.button("Cancel").clicked();
            });
        });
        cancel |= modal.should_close();
        if !cancel {
            self.plans.floor_properties = Some(draft);
            if apply {
                let result = self.apply_floor_properties();
                self.report(result, "Floor properties updated.");
            }
        }
    }
}
