//! Native straight-run ramp placement and transactional properties.
use super::*;
use os_model::{Model, Ramp, RampParams};
use os_render::plan::PlanRampItem;

pub(super) struct Placement {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    id: Id,
    pub first: Option<Point2>,
    pub parameters: RampParams,
}

impl Placement {
    fn stale(&self, editor: &Editor, active: Option<Id>) -> bool {
        active != Some(self.context.view_id)
            || editor.document.session_id() != self.context.session_id
            || editor.document.revision() != self.context.model_revision
            || editor.native_plan_context(self.context.view_id).ok() != Some(self.context)
            || plan_provider_signature(editor) != self.providers
    }
}

pub(super) struct Edit {
    id: Id,
    session: Id,
    revision: u64,
    view: Option<Id>,
    providers: Vec<(String, Id)>,
    pub parameters: RampParams,
}

fn upper_levels(model: &Model, lower: Id) -> Vec<Id> {
    let Some(lower) = model.levels.get(&lower) else {
        return Vec::new();
    };
    let mut levels: Vec<_> = model
        .levels
        .values()
        .filter(|level| {
            level.parameters.building == lower.parameters.building
                && level.parameters.elevation > lower.parameters.elevation
        })
        .collect();
    levels.sort_by(|a, b| {
        a.parameters
            .elevation
            .total_cmp(&b.parameters.elevation)
            .then_with(|| a.id().cmp(&b.id()))
    });
    levels.into_iter().map(|level| level.id()).collect()
}

impl DesktopApp {
    pub(crate) fn begin_ramp(&mut self, view: Id) {
        let result = (|| {
            os_core::ensure(
                self.plans.active == Some(view) && self.plans.active_sheet.is_none(),
                "Open a floor plan to draw a ramp",
            )?;
            let context = self.editor.native_plan_context(view)?;
            let lower_level = self.editor.document.model().views[&view]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Ramps need a floor plan level".into()))?;
            let upper_level = upper_levels(self.editor.document.model(), lower_level)
                .first()
                .copied()
                .ok_or_else(|| {
                    Error::Invalid(
                        "Add a higher level in this building before drawing a ramp".into(),
                    )
                })?;
            self.select(None);
            self.cancel_opening_placement();
            self.plans.floor_sketch = None;
            self.plans.floor_hole_sketch = None;
            self.plans.floor_vertex_drag = None;
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
            self.grid_draft = None;
            self.plan_draft = None;
            self.plans.ramp_edit = None;
            self.plans.ramp_placement = Some(Placement {
                context,
                providers: plan_provider_signature(&self.editor),
                id: Id::new(),
                first: None,
                parameters: RampParams {
                    name: format!("Ramp {}", self.editor.document.model().ramps.len() + 1),
                    lower_level,
                    upper_level,
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(5.0, 0.0),
                    width: 1.5,
                    structural_thickness: 0.18,
                    material: None,
                },
            });
            Ok(())
        })();
        self.report(
            result,
            "Ramp · click the lower edge, then the upper edge. Escape cancels.",
        );
    }

    pub(super) fn validate_ramp_interaction(&mut self, ctx: &egui::Context) {
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let gone = ctx.input(|i| {
            i.events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerGone))
        });
        let stale = self.plans.ramp_placement.as_ref().is_some_and(|draft| {
            draft.stale(&self.editor, self.plans.active) || self.plans.active_sheet.is_some()
        });
        if (stale || escape || gone) && self.plans.ramp_placement.take().is_some() {
            self.status = if stale {
                "Ramp placement canceled because its plan or document changed."
            } else {
                "Ramp placement canceled."
            }
            .into();
            self.status_error = false;
        }
        if self.plans.ramp_edit.as_ref().is_some_and(|draft| {
            self.selected != Some(draft.id)
                || self.plans.active != draft.view
                || self.editor.document.session_id() != draft.session
                || self.editor.document.revision() != draft.revision
                || plan_provider_signature(&self.editor) != draft.providers
        }) {
            self.plans.ramp_edit = None;
        }
    }

    pub(super) fn ramp_click(&mut self, target: Point2) {
        let result = (|| {
            let draft = self
                .plans
                .ramp_placement
                .as_mut()
                .ok_or_else(|| Error::Invalid("Ramp placement canceled".into()))?;
            os_core::ensure(
                !draft.stale(&self.editor, self.plans.active),
                "Ramp placement is stale",
            )?;
            os_core::ensure(
                point_in_plan_crop(draft.context, draft.context.basis.world_to_plane(target)?),
                "Place ramp endpoints inside the plan crop",
            )?;
            if draft.first.is_none() {
                draft.first = Some(target);
                draft.parameters.start = target;
                return Ok(false);
            }
            let parameters = candidate(&self.editor, draft, target)?;
            let context = draft.context;
            let mut ramp = Ramp::new("core.ramp", parameters.clone());
            ramp.header.id = draft.id;
            let id = ramp.id();
            self.editor.command("Place ramp", Command::AddRamp(ramp))?;
            self.select(Some(id));
            self.plans.ramp_placement = Some(Placement {
                context: self.editor.native_plan_context(context.view_id)?,
                providers: plan_provider_signature(&self.editor),
                id: Id::new(),
                first: None,
                parameters: RampParams {
                    name: format!("Ramp {}", self.editor.document.model().ramps.len() + 1),
                    ..parameters
                },
            });
            Ok(true)
        })();
        match result {
            Ok(true) => self.report(
                Ok(()),
                "Ramp placed. Click another pair of endpoints, or Escape to edit the selected ramp.",
            ),
            Ok(false) => self.report(Ok(()), "Ramp · click the upper edge."),
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(crate) fn ramp_properties(&mut self, ui: &mut egui::Ui) -> bool {
        self.validate_ramp_interaction(ui.ctx());
        if let Some(draft) = &mut self.plans.ramp_placement {
            let mut cancel = false;
            egui::ScrollArea::vertical()
                .id_salt("ramp_placement_properties")
                .show(ui, |ui| {
                    ui.label("Draw straight ramp · metres");
                    controls(
                        ui,
                        &mut draft.parameters,
                        self.editor.document.model(),
                        true,
                    );
                    if let Some(first) = draft.first {
                        ui.label(format!("Lower edge: ({:.3}, {:.3})", first.x, first.y));
                    }
                    cancel = ui.button("Cancel ramp placement").clicked();
                });
            if cancel {
                self.plans.ramp_placement = None;
            }
            return true;
        }
        let Some(ramp) = self
            .selected
            .and_then(|id| self.editor.document.model().ramps.get(&id))
            .cloned()
        else {
            self.plans.ramp_edit = None;
            return false;
        };
        let id = ramp.id();
        let session = self.editor.document.session_id();
        let revision = self.editor.document.revision();
        if self.plans.ramp_edit.as_ref().is_none_or(|draft| {
            draft.id != id || draft.session != session || draft.revision != revision
        }) {
            self.plans.ramp_edit = Some(Edit {
                id,
                session,
                revision,
                view: self.plans.active,
                providers: plan_provider_signature(&self.editor),
                parameters: ramp.parameters.clone(),
            });
        }
        let draft = self.plans.ramp_edit.as_mut().unwrap();
        let model = self.editor.document.model();
        let (mut apply, mut cancel, mut delete) = (false, false, false);
        egui::ScrollArea::vertical()
            .id_salt("ramp_properties")
            .show(ui, |ui| {
                ui.label("Straight ramp · metres");
                controls(ui, &mut draft.parameters, model, false);
                apply = ui
                    .add_enabled(
                        draft.parameters != ramp.parameters
                            && draft.parameters.validate_in(model).is_ok(),
                        egui::Button::new("Apply ramp"),
                    )
                    .clicked();
                cancel = ui.button("Cancel ramp edits").clicked();
                delete = ui.button("Delete ramp").clicked();
                ui.label(format!("Stable ID: {id}"));
            });
        if cancel {
            self.plans.ramp_edit = None;
        } else if apply {
            self.apply_ramp_properties();
        }
        if delete {
            self.delete_ramp();
        }
        true
    }

    pub(super) fn apply_ramp_properties(&mut self) {
        let result = (|| {
            let draft = self
                .plans
                .ramp_edit
                .as_ref()
                .ok_or_else(|| Error::Invalid("No ramp edit".into()))?;
            os_core::ensure(
                self.selected == Some(draft.id)
                    && self.plans.active == draft.view
                    && self.editor.document.session_id() == draft.session
                    && self.editor.document.revision() == draft.revision
                    && plan_provider_signature(&self.editor) == draft.providers,
                "Ramp edit is stale",
            )?;
            draft.parameters.validate_in(self.editor.document.model())?;
            self.editor.command(
                "Edit ramp",
                Command::UpdateRamp {
                    id: draft.id,
                    parameters: draft.parameters.clone(),
                },
            )
        })();
        if result.is_ok() {
            self.plans.ramp_edit = None;
        }
        self.report(result, "Ramp updated.");
    }

    pub(super) fn delete_ramp(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().ramps.contains_key(id))
        else {
            return;
        };
        let result = self.editor.command("Delete ramp", Command::RemoveRamp(id));
        if result.is_ok() {
            self.select(None);
            self.plans.ramp_edit = None;
        }
        self.report(result, "Ramp deleted.");
    }
}

fn controls(ui: &mut egui::Ui, p: &mut RampParams, model: &Model, placement: bool) {
    ui.text_edit_singleline(&mut p.name);
    let level_name = |id: Id| {
        model
            .levels
            .get(&id)
            .map_or("Missing level", |level| level.parameters.name.as_str())
    };
    if placement {
        ui.label(format!("From: {} (active plan)", level_name(p.lower_level)));
    } else {
        egui::ComboBox::from_id_salt("ramp_lower")
            .selected_text(level_name(p.lower_level))
            .show_ui(ui, |ui| {
                for (id, level) in &model.levels {
                    ui.selectable_value(&mut p.lower_level, *id, &level.parameters.name);
                }
            });
    }
    let uppers = upper_levels(model, p.lower_level);
    egui::ComboBox::from_id_salt("ramp_upper")
        .selected_text(format!("To: {}", level_name(p.upper_level)))
        .show_ui(ui, |ui| {
            for id in &uppers {
                ui.selectable_value(&mut p.upper_level, *id, level_name(*id));
            }
        });
    if uppers.is_empty() {
        ui.colored_label(theme::ERROR, "Add a higher level in this building.");
    }
    egui::Grid::new("ramp_dimensions")
        .num_columns(2)
        .show(ui, |ui| {
            if !placement {
                for (label, value) in [
                    ("Start X", &mut p.start.x),
                    ("Start Y", &mut p.start.y),
                    ("End X", &mut p.end.x),
                    ("End Y", &mut p.end.y),
                ] {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                    ui.end_row();
                }
            }
            for (label, value) in [
                ("Width", &mut p.width),
                ("Vertical thickness", &mut p.structural_thickness),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                ui.end_row();
            }
        });
    egui::ComboBox::from_id_salt("ramp_material")
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
    match p.dimensions_in(model) {
        Ok(dimensions) => {
            ui.label(format!(
                "Run {:.3} m · rise {:.3} m · slope {:.1}%",
                dimensions.run,
                dimensions.rise,
                dimensions.rise_per_run * 100.0
            ));
            ui.label("Single straight slab; no landing or code-compliance analysis.");
        }
        Err(error) => {
            ui.colored_label(theme::ERROR, error.to_string());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn target(
    draft: &Placement,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    pointer: egui::Pos2,
    snaps: SnapOptions,
) -> Result<Point2> {
    let viewport = [f64::from(rect.width()), f64::from(rect.height())];
    let pointer = Point2::new(
        f64::from(pointer.x - rect.left()),
        f64::from(pointer.y - rect.top()),
    );
    let mut point = camera.unproject(pointer, viewport)?;
    if snaps.enabled {
        let query = os_render::snapping::SnapQuery {
            camera,
            viewport,
            pointer,
            radius_pixels: 12.0,
            endpoints: snaps.endpoints,
            midpoints: snaps.midpoints,
            intersections: snaps.intersections,
            perpendicular_from: if snaps.perpendicular {
                draft
                    .first
                    .map(|p| context.basis.world_to_plane(p))
                    .transpose()?
            } else {
                None
            },
            nearest: snaps.nearest,
            axis_extensions: snaps.axis_extensions,
            exclude_entity: None,
        };
        if let Some(candidate) = drawing.snap(context, query)?.candidate(context, query)? {
            point = candidate.point;
        }
    }
    os_core::ensure(
        point_in_plan_crop(context, point),
        "Place ramp endpoints inside the plan crop",
    )?;
    context.basis.plane_to_world(point)
}

fn candidate(editor: &Editor, draft: &Placement, end: Point2) -> Result<RampParams> {
    let start = draft
        .first
        .ok_or_else(|| Error::Invalid("Choose the lower ramp edge".into()))?;
    let parameters = RampParams {
        start,
        end,
        ..draft.parameters.clone()
    };
    parameters.validate_in(editor.document.model())?;
    Ok(parameters)
}

pub(super) fn preview(
    editor: &Editor,
    draft: &Placement,
    end: Point2,
) -> Result<Option<PlanDrawing>> {
    if draft.first.is_none() {
        return Ok(None);
    }
    let parameters = candidate(editor, draft, end)?;
    let model = editor.document.model();
    let item = PlanRampItem::from_plan_data(
        draft.id,
        &parameters,
        model.levels[&parameters.lower_level].parameters.elevation,
        model.levels[&parameters.upper_level].parameters.elevation,
        draft.context,
    )?;
    let Some(item) = item else {
        return Ok(None);
    };
    PlanDrawing::from_prisms(draft.context, &BTreeMap::new(), Vec::new())?
        .with_ramps(vec![item])
        .map(Some)
}

pub(super) fn paint(
    painter: &egui::Painter,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    for item in drawing.items(context).unwrap_or_default() {
        columns::paint(painter, &item.footprint, camera, rect);
    }
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    if let Ok(ramps) = drawing.ramps(context) {
        for ramp in ramps {
            for line in ramp.visible_strokes() {
                if let (Ok(a), Ok(b)) = (
                    camera.project(line.start, size),
                    camera.project(line.end, size),
                ) && [a.x, a.y, b.x, b.y].iter().all(|value| value.abs() < 1e8)
                {
                    painter.line_segment(
                        [
                            rect.min + egui::vec2(a.x as f32, a.y as f32),
                            rect.min + egui::vec2(b.x as f32, b.y as f32),
                        ],
                        egui::Stroke::new(1.5, theme::ACCENT),
                    );
                }
            }
        }
    }
}
