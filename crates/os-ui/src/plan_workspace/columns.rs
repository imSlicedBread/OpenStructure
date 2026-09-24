//! Native column placement and explicit, revision-bound Properties edits.
use super::*;
use os_model::{Column, ColumnParams};

pub(super) struct Placement {
    context: PlanContext,
    session: Id,
    revision: u64,
    providers: Vec<(String, Id)>,
    pub parameters: ColumnParams,
}
impl Placement {
    pub fn stale(&self, editor: &Editor, active: Option<Id>) -> bool {
        active != Some(self.context.view_id)
            || editor.document.session_id() != self.session
            || editor.document.revision() != self.revision
            || editor.native_plan_context(self.context.view_id).ok() != Some(self.context)
            || plan_provider_signature(editor) != self.providers
    }
}

pub(super) struct Edit {
    id: Id,
    session: Id,
    revision: u64,
    pub parameters: ColumnParams,
}

impl DesktopApp {
    pub(super) fn begin_column(&mut self, view: Id) {
        let result = (|| {
            let context = self.editor.native_plan_context(view)?;
            let level = self.editor.document.model().views[&view]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Column placement needs a plan level".into()))?;
            self.cancel_plan_wall();
            self.cancel_aligned_dimension();
            self.cancel_opening_placement();
            self.plans.floor_sketch = None;
            self.plans.section_placement = None;
            self.plans.detail_line_draft = None;
            self.plans.room_separation_line_draft = None;
            self.plans.room_tag_draft = None;
            self.plans.room_placement_active = false;
            self.plans.crop.mode = None;
            self.plans.crop.cancel();
            self.grid_draft = None;
            self.opening_draft = None;
            self.plan_draft = None;
            self.plans.column_placement = Some(Placement {
                context,
                session: self.editor.document.session_id(),
                revision: self.editor.document.revision(),
                providers: plan_provider_signature(&self.editor),
                parameters: ColumnParams {
                    name: format!("Column {}", self.editor.document.model().columns.len() + 1),
                    level,
                    center: Point2::new(0.0, 0.0),
                    width: 0.4,
                    depth: 0.4,
                    height: 3.0,
                    base_offset: 0.0,
                    material: None,
                },
            });
            Ok(())
        })();
        self.report(result, "Column · click a snapped center to place; Escape cancels. Edit dimensions in Properties after placement.");
    }

    pub(super) fn commit_column(&mut self, parameters: ColumnParams) {
        let result = (|| {
            let draft = self
                .plans
                .column_placement
                .as_ref()
                .ok_or_else(|| Error::Invalid("Column placement canceled".into()))?;
            os_core::ensure(
                !draft.stale(&self.editor, self.plans.active),
                "Column placement is stale",
            )?;
            parameters.validate_in(self.editor.document.model())?;
            let center = draft.context.basis.world_to_plane(parameters.center)?;
            os_core::ensure(
                point_in_plan_crop(draft.context, center),
                "Column center is outside the crop",
            )?;
            let column = Column::new("core.column", parameters);
            let id = column.id();
            self.editor
                .command("Place column", Command::AddColumn(column))?;
            self.plans.column_placement = None;
            self.select(Some(id));
            Ok(())
        })();
        self.report(result, "Column placed.");
    }

    pub(crate) fn column_properties(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(column) = self
            .selected
            .and_then(|id| self.editor.document.model().columns.get(&id))
            .cloned()
        else {
            self.plans.column_edit = None;
            return false;
        };
        let id = column.id();
        let session = self.editor.document.session_id();
        let revision = self.editor.document.revision();
        if self
            .plans
            .column_edit
            .as_ref()
            .is_none_or(|d| d.id != id || d.session != session || d.revision != revision)
        {
            self.plans.column_edit = Some(Edit {
                id,
                session,
                revision,
                parameters: column.parameters.clone(),
            });
        }
        let mut apply = false;
        let mut delete = false;
        let model = self.editor.document.model();
        let draft = self.plans.column_edit.as_mut().unwrap();
        egui::ScrollArea::vertical()
            .id_salt("column_properties")
            .show(ui, |ui| {
                ui.label("Architectural column · metres");
                ui.text_edit_singleline(&mut draft.parameters.name);
                let p = &mut draft.parameters;
                egui::ComboBox::from_id_salt("column_level")
                    .selected_text(&model.levels[&p.level].parameters.name)
                    .show_ui(ui, |ui| {
                        for (id, level) in &model.levels {
                            ui.selectable_value(&mut p.level, *id, &level.parameters.name);
                        }
                    });
                egui::Grid::new("column_numeric")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("Center X", &mut p.center.x),
                            ("Center Y", &mut p.center.y),
                            ("Width", &mut p.width),
                            ("Depth", &mut p.depth),
                            ("Height", &mut p.height),
                            ("Base offset", &mut p.base_offset),
                        ] {
                            ui.label(label);
                            ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                            ui.end_row();
                        }
                    });
                egui::ComboBox::from_id_salt("column_material")
                    .selected_text(
                        p.material
                            .and_then(|id| model.materials.get(&id))
                            .map_or("No material", |m| m.parameters.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut p.material, None, "No material");
                        for (id, material) in &model.materials {
                            ui.selectable_value(
                                &mut p.material,
                                Some(*id),
                                &material.parameters.name,
                            );
                        }
                    });
                if let Err(error) = p.validate_in(model) {
                    ui.colored_label(theme::ERROR, error.to_string());
                }
                apply = ui
                    .add_enabled(
                        p != &column.parameters && p.validate_in(model).is_ok(),
                        egui::Button::new("Apply column"),
                    )
                    .clicked();
                delete = ui.button("Delete column").clicked();
                ui.label(format!("Stable ID: {id}"));
            });
        if apply {
            self.apply_column_properties();
        }
        if delete {
            self.delete_column();
        }
        true
    }

    pub(super) fn apply_column_properties(&mut self) {
        let result = (|| {
            let draft = self
                .plans
                .column_edit
                .as_ref()
                .ok_or_else(|| Error::Invalid("No column edit".into()))?;
            os_core::ensure(
                self.selected == Some(draft.id)
                    && self.editor.document.session_id() == draft.session
                    && self.editor.document.revision() == draft.revision,
                "Column edit is stale",
            )?;
            self.editor.command(
                "Edit column",
                Command::UpdateColumn {
                    id: draft.id,
                    parameters: draft.parameters.clone(),
                },
            )
        })();
        if result.is_ok() {
            self.plans.column_edit = None;
        }
        self.report(result, "Column updated.");
    }

    pub(super) fn delete_column(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().columns.contains_key(id))
        else {
            return;
        };
        let result = self
            .editor
            .command("Delete column", Command::RemoveColumn(id));
        if result.is_ok() {
            self.select(None);
            self.plans.column_edit = None;
        }
        self.report(result, "Column deleted.");
    }
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
) -> Result<(ColumnParams, os_geometry::plan::PlanFootprint)> {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let local = Point2::new(
        f64::from(pointer.x - rect.left()),
        f64::from(pointer.y - rect.top()),
    );
    let target = floor_snap_target(drawing, context, camera, size, local, snaps)?;
    os_core::ensure(
        point_in_plan_crop(context, target),
        "Column center is outside the crop",
    )?;
    let mut parameters = draft.parameters.clone();
    parameters.center = context.basis.plane_to_world(target)?;
    parameters.validate_in(editor.document.model())?;
    let elevation = editor.document.model().levels[&parameters.level]
        .parameters
        .elevation;
    let solid = os_geometry::columns::column_solid(&parameters, elevation)?;
    let footprint =
        os_geometry::plan::rectangular_plan(&solid, context.range, context.basis, context.crop)?
            .ok_or_else(|| Error::Invalid("Column is outside the visible plan range".into()))?;
    Ok((parameters, footprint))
}

pub(super) fn paint(
    painter: &egui::Painter,
    footprint: &os_geometry::plan::PlanFootprint,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let points: Option<Vec<_>> = footprint
        .vertices()
        .iter()
        .map(|point| {
            let p = camera.project(*point, size).ok()?;
            (p.x.abs() < f64::from(f32::MAX) / 2.0 && p.y.abs() < f64::from(f32::MAX) / 2.0)
                .then_some(rect.min + egui::vec2(p.x as f32, p.y as f32))
        })
        .collect();
    if let Some(points) = points {
        painter.add(egui::Shape::convex_polygon(
            points,
            theme::ACCENT.gamma_multiply(0.18),
            egui::Stroke::new(2.0, theme::ACCENT),
        ));
    }
}
