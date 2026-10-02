//! Native straight-flight authoring. Drafts never enter the document or scene.
use super::*;
use os_model::{Model, Stair, StairParams};
use os_render::plan::PlanStairItem;

pub(super) struct Placement {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    id: Id,
    pub first: Option<Point2>,
    pub parameters: StairParams,
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
    pub parameters: StairParams,
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
    pub(crate) fn begin_stair(&mut self, view: Id) {
        let result = (|| {
            os_core::ensure(
                self.plans.active == Some(view) && self.plans.active_sheet.is_none(),
                "Open a floor plan to draw stairs",
            )?;
            let context = self.editor.native_plan_context(view)?;
            let lower_level = self.editor.document.model().views[&view]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Stairs need a floor plan level".into()))?;
            let upper_level = upper_levels(self.editor.document.model(), lower_level)
                .first()
                .copied()
                .ok_or_else(|| {
                    Error::Invalid(
                        "Add a higher level in this building before drawing a stair".into(),
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
            self.plans.stair_edit = None;
            self.plans.stair_placement = Some(Placement {
                context,
                providers: plan_provider_signature(&self.editor),
                id: Id::new(),
                first: None,
                parameters: StairParams {
                    name: format!("Stair {}", self.editor.document.model().stairs.len() + 1),
                    lower_level,
                    upper_level,
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(4.5, 0.0),
                    width: 1.2,
                    riser_count: 15,
                    structural_thickness: 0.2,
                    material: None,
                },
            });
            Ok(())
        })();
        self.report(
            result,
            "Stair · click the first riser, then the upper arrival. Escape finishes placement.",
        );
    }

    pub(super) fn validate_stair_interaction(&mut self, ctx: &egui::Context) {
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let gone = ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerGone))
        });
        let stale = self.plans.stair_placement.as_ref().is_some_and(|d| {
            d.stale(&self.editor, self.plans.active) || self.plans.active_sheet.is_some()
        });
        if stale || escape || gone {
            if self.plans.stair_placement.take().is_some() {
                self.status = if stale {
                    "Stair placement canceled because its plan or document changed."
                } else {
                    "Stair placement canceled."
                }
                .into();
                self.status_error = false;
            }
            self.plans.stair_edit = None;
        }
        if self.plans.stair_edit.as_ref().is_some_and(|d| {
            self.selected != Some(d.id)
                || self.plans.active != d.view
                || self.editor.document.session_id() != d.session
                || self.editor.document.revision() != d.revision
        }) {
            self.plans.stair_edit = None;
        }
    }

    pub(super) fn stair_click(&mut self, target: Point2) {
        let result = (|| {
            let draft = self
                .plans
                .stair_placement
                .as_mut()
                .ok_or_else(|| Error::Invalid("Stair placement canceled".into()))?;
            os_core::ensure(
                !draft.stale(&self.editor, self.plans.active),
                "Stair placement is stale",
            )?;
            os_core::ensure(
                point_in_plan_crop(draft.context, draft.context.basis.world_to_plane(target)?),
                "Place stair endpoints inside the plan crop",
            )?;
            if draft.first.is_none() {
                draft.first = Some(target);
                draft.parameters.start = target;
                return Ok(false);
            }
            let parameters = candidate(&self.editor, draft, target)?;
            let context = draft.context;
            let mut stair = Stair::new("core.stair", parameters.clone());
            stair.header.id = draft.id;
            let id = stair.id();
            self.editor
                .command("Place stair", Command::AddStair(stair))?;
            self.select(Some(id));
            // Keep the tool and settings ready for another flight after one transaction.
            self.plans.stair_placement = Some(Placement {
                context: self.editor.native_plan_context(context.view_id)?,
                providers: plan_provider_signature(&self.editor),
                id: Id::new(),
                first: None,
                parameters: StairParams {
                    name: format!("Stair {}", self.editor.document.model().stairs.len() + 1),
                    ..parameters
                },
            });
            Ok(true)
        })();
        match result {
            Ok(true) => self.report(
                Ok(()),
                "Stair placed. Click to draw another; Escape opens selected stair Properties.",
            ),
            Ok(false) => self.report(
                Ok(()),
                "Stair · click the far edge of the upper arrival tread.",
            ),
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(crate) fn stair_properties(&mut self, ui: &mut egui::Ui) -> bool {
        self.validate_stair_interaction(ui.ctx());
        if let Some(draft) = &mut self.plans.stair_placement {
            let mut cancel = false;
            egui::ScrollArea::vertical()
                .id_salt("stair_placement_properties")
                .show(ui, |ui| {
                    ui.label("Draw straight stair · metres");
                    controls(
                        ui,
                        &mut draft.parameters,
                        self.editor.document.model(),
                        true,
                    );
                    if draft.first.is_some()
                        && let Ok(d) = draft.parameters.dimensions_in(self.editor.document.model())
                    {
                        ui.label(format!("Run {:.3} m · going {:.3} m", d.run, d.going));
                    }
                    ui.label(if draft.first.is_some() {
                        "Click the upper arrival point."
                    } else {
                        "Click the first riser in the plan."
                    });
                    cancel = ui.button("Cancel stair placement").clicked();
                });
            if cancel {
                self.plans.stair_placement = None;
            }
            return true;
        }
        let Some(stair) = self
            .selected
            .and_then(|id| self.editor.document.model().stairs.get(&id))
            .cloned()
        else {
            self.plans.stair_edit = None;
            return false;
        };
        let id = stair.id();
        let session = self.editor.document.session_id();
        let revision = self.editor.document.revision();
        if self
            .plans
            .stair_edit
            .as_ref()
            .is_none_or(|d| d.id != id || d.session != session || d.revision != revision)
        {
            self.plans.stair_edit = Some(Edit {
                id,
                session,
                revision,
                view: self.plans.active,
                parameters: stair.parameters.clone(),
            });
        }
        let draft = self.plans.stair_edit.as_mut().unwrap();
        let model = self.editor.document.model();
        let (mut apply, mut cancel, mut delete, mut add_left, mut add_right) =
            (false, false, false, false, false);
        egui::ScrollArea::vertical()
            .id_salt("stair_properties")
            .show(ui, |ui| {
                ui.label("Straight stair · metres");
                controls(ui, &mut draft.parameters, model, false);
                apply = ui
                    .add_enabled(
                        draft.parameters != stair.parameters
                            && draft.parameters.validate_in(model).is_ok(),
                        egui::Button::new("Apply stair"),
                    )
                    .clicked();
                cancel = ui.button("Cancel stair edits").clicked();
                delete = ui.button("Delete stair").clicked();
                ui.horizontal_wrapped(|ui| {
                    add_left = ui.button("Add left railing").clicked();
                    add_right = ui.button("Add right railing").clicked();
                });
                ui.label(format!("Stable ID: {id}"));
            });
        if cancel {
            self.plans.stair_edit = None;
        } else if apply {
            self.apply_stair_properties();
        }
        if delete {
            self.delete_stair();
        }
        if add_left {
            self.add_stair_railing(id, os_model::StairRailingSide::Left);
        } else if add_right {
            self.add_stair_railing(id, os_model::StairRailingSide::Right);
        }
        true
    }

    pub(super) fn apply_stair_properties(&mut self) {
        let result = (|| {
            let draft = self
                .plans
                .stair_edit
                .as_ref()
                .ok_or_else(|| Error::Invalid("No stair edit".into()))?;
            os_core::ensure(
                self.selected == Some(draft.id)
                    && self.plans.active == draft.view
                    && self.editor.document.session_id() == draft.session
                    && self.editor.document.revision() == draft.revision,
                "Stair edit is stale",
            )?;
            draft.parameters.validate_in(self.editor.document.model())?;
            self.editor.command(
                "Edit stair",
                Command::UpdateStair {
                    id: draft.id,
                    parameters: draft.parameters.clone(),
                },
            )
        })();
        if result.is_ok() {
            self.plans.stair_edit = None;
        }
        self.report(result, "Stair updated.");
    }

    pub(super) fn delete_stair(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().stairs.contains_key(id))
        else {
            return;
        };
        let mut commands: Vec<_> = self
            .editor
            .document
            .model()
            .railings
            .values()
            .filter(|railing| railing.parameters.stair == id)
            .map(|railing| Command::RemoveRailing(railing.id()))
            .collect();
        commands.push(Command::RemoveStair(id));
        let result = self
            .editor
            .commands("Delete stair and hosted railings", commands);
        if result.is_ok() {
            self.select(None);
            self.plans.stair_edit = None;
        }
        self.report(result, "Stair deleted.");
    }
}

fn controls(ui: &mut egui::Ui, p: &mut StairParams, model: &Model, placement: bool) {
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
        egui::ComboBox::from_id_salt("stair_lower")
            .selected_text(level_name(p.lower_level))
            .show_ui(ui, |ui| {
                for (id, level) in &model.levels {
                    ui.selectable_value(&mut p.lower_level, *id, &level.parameters.name);
                }
            });
    }
    let uppers = upper_levels(model, p.lower_level);
    egui::ComboBox::from_id_salt("stair_upper")
        .selected_text(format!("To: {}", level_name(p.upper_level)))
        .show_ui(ui, |ui| {
            for id in &uppers {
                ui.selectable_value(&mut p.upper_level, *id, level_name(*id));
            }
        });
    if uppers.is_empty() {
        ui.colored_label(theme::ERROR, "Add a higher level in this building.");
    }
    egui::Grid::new("stair_dimensions")
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
                ("Structural thickness", &mut p.structural_thickness),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                ui.end_row();
            }
            ui.label("Risers");
            ui.add(egui::DragValue::new(&mut p.riser_count).range(1..=256));
            ui.end_row();
        });
    egui::ComboBox::from_id_salt("stair_material")
        .selected_text(
            p.material
                .and_then(|id| model.materials.get(&id))
                .map_or("No material", |m| m.parameters.name.as_str()),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut p.material, None, "No material");
            for (id, material) in &model.materials {
                ui.selectable_value(&mut p.material, Some(*id), &material.parameters.name);
            }
        });
    match p.dimensions_in(model) {
        Ok(d) => {
            ui.label(format!(
                "Rise {:.3} m · riser {:.3} m",
                d.rise, d.riser_height
            ));
            if !placement {
                ui.label(format!("Run {:.3} m · going {:.3} m", d.run, d.going));
            }
        }
        Err(error) => {
            ui.colored_label(theme::ERROR, error.to_string());
        }
    }
    ui.label("The final going is the upper arrival tread. Coordinate the upper slab and its opening separately.");
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
        "Place stair endpoints inside the plan crop",
    )?;
    context.basis.plane_to_world(point)
}

fn candidate(editor: &Editor, draft: &Placement, end: Point2) -> Result<StairParams> {
    let start = draft
        .first
        .ok_or_else(|| Error::Invalid("Choose the first riser".into()))?;
    let parameters = StairParams {
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
    let p = candidate(editor, draft, end)?;
    let model = editor.document.model();
    let item = PlanStairItem::derive(
        draft.id,
        p.start,
        p.end,
        p.width,
        p.riser_count as usize,
        p.structural_thickness,
        p.material,
        model.levels[&p.lower_level].parameters.elevation,
        model.levels[&p.upper_level].parameters.elevation,
        draft.context,
    )?;
    PlanDrawing::from_prisms(draft.context, &BTreeMap::new(), Vec::new())?
        .with_stairs(vec![item])
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
    for line in drawing.provider_lines(context).unwrap_or_default() {
        if let (Ok(a), Ok(b)) = (
            camera.project(line.start, size),
            camera.project(line.end, size),
        ) && [a.x, a.y, b.x, b.y].iter().all(|v| v.abs() < 1e8)
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
