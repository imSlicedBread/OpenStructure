//! Native ceiling sketch and exact property edits; previews remain transient.
use super::*;
use os_model::{Ceiling, CeilingParams};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Boundary,
    Opening,
    Ready,
}

pub(super) struct Draft {
    context: PlanContext,
    session: Id,
    revision: u64,
    providers: Vec<(String, Id)>,
    pub id: Id,
    pub original: Option<CeilingParams>,
    pub parameters: CeilingParams,
    pub phase: Phase,
    pub points: Vec<Point2>,
}

impl Draft {
    pub fn sketching(&self) -> bool {
        self.phase != Phase::Ready
    }

    fn stale(&self, editor: &Editor, active: Option<Id>) -> bool {
        active != Some(self.context.view_id)
            || editor.document.session_id() != self.session
            || editor.document.revision() != self.revision
            || editor.native_plan_context(self.context.view_id).ok() != Some(self.context)
            || super::plan_provider_signature(editor) != self.providers
            || self.original.as_ref().is_some_and(|original| {
                editor
                    .document
                    .model()
                    .ceilings
                    .get(&self.id)
                    .is_none_or(|ceiling| &ceiling.parameters != original)
            })
    }
}

impl DesktopApp {
    pub(crate) fn begin_ceiling(&mut self, view: Id) {
        let result = self.start_ceiling_draft(view, None);
        self.report(
            result,
            "Ceiling · click boundary vertices, then Finish ceiling loop in Properties.",
        );
    }

    pub(crate) fn begin_ceiling_from_selected_room(&mut self, view: Id) {
        let result = (|| {
            let room_id = self
                .selected
                .ok_or_else(|| Error::Invalid("Select a resolved room first".into()))?;
            let room = self
                .editor
                .document
                .model()
                .rooms
                .get(&room_id)
                .ok_or_else(|| Error::Invalid("Selected element is not a room".into()))?
                .parameters
                .clone();
            let context = self.editor.native_plan_context(view)?;
            let level = self.editor.document.model().views[&view]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Ceiling needs an associated level".into()))?;
            os_core::ensure(
                room.level == level,
                "Selected room belongs to a different level",
            )?;
            let graphic = self
                .plans
                .drawing
                .as_ref()
                .ok_or_else(|| Error::Invalid("Wait for the plan drawing to finish".into()))?
                .rooms(context)?
                .iter()
                .find(|graphic| graphic.entity == room_id)
                .cloned()
                .ok_or_else(|| Error::Invalid("Selected room is not in this plan".into()))?;
            os_core::ensure(
                graphic.diagnostic.is_none() && graphic.boundary.len() >= 3,
                graphic
                    .diagnostic
                    .unwrap_or_else(|| "Selected room has no resolved boundary".into()),
            )?;
            let boundary = graphic
                .boundary
                .iter()
                .map(|point| context.basis.plane_to_world(*point))
                .collect::<Result<Vec<_>>>()?;
            let parameters = CeilingParams {
                name: format!("Ceiling · {}", room.name),
                level,
                material: room.ceiling_material,
                boundary_room: Some(room_id),
                boundary,
                holes: Vec::new(),
                thickness: 0.12,
                elevation_offset: 2.55,
            };
            parameters.validate_in(self.editor.document.model())?;
            let elevation = self.editor.document.model().levels[&level]
                .parameters
                .elevation;
            os_geometry::ceilings::ceiling_mesh(&parameters, elevation)?;
            self.start_ceiling_draft(view, Some(parameters))
        })();
        self.report(
            result,
            "Room-bounded ceiling draft created. Review its properties, then Apply ceiling.",
        );
    }

    fn start_ceiling_draft(&mut self, view: Id, initial: Option<CeilingParams>) -> Result<()> {
        let context = self.editor.native_plan_context(view)?;
        os_core::ensure(
            self.plans.active == Some(view) && self.plans.active_sheet.is_none(),
            "Open a plan or reflected ceiling plan first",
        )?;
        let settings = self.editor.document.model().views[&view]
            .parameters
            .plan
            .ok_or_else(|| Error::Invalid("Ceilings require a plan view".into()))?;
        os_core::ensure(
            settings.view_type == os_model::PlanViewType::ReflectedCeilingPlan,
            "Open a reflected ceiling plan to draw ceilings",
        )?;
        let level = self.editor.document.model().views[&view]
            .parameters
            .level
            .ok_or_else(|| Error::Invalid("Ceiling needs an associated level".into()))?;
        self.cancel_plan_wall();
        self.cancel_opening_placement();
        self.select(None);
        self.plans.floor_sketch = None;
        self.plans.floor_hole_sketch = None;
        self.plans.floor_vertex_drag = None;
        self.plans.roof_draft = None;
        self.plans.stair_placement = None;
        self.plans.column_placement = None;
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
        let parameters = initial.unwrap_or_else(|| CeilingParams {
            name: format!(
                "Ceiling {}",
                self.editor.document.model().ceilings.len() + 1
            ),
            level,
            material: None,
            boundary_room: None,
            boundary: Vec::new(),
            holes: Vec::new(),
            thickness: 0.12,
            elevation_offset: 2.55,
        });
        let phase = if parameters.boundary.is_empty() {
            Phase::Boundary
        } else {
            Phase::Ready
        };
        self.plans.ceiling_draft = Some(Draft {
            context,
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            providers: super::plan_provider_signature(&self.editor),
            id: Id::new(),
            original: None,
            parameters,
            phase,
            points: Vec::new(),
        });
        Ok(())
    }

    pub(super) fn validate_ceiling_interaction(&mut self, ctx: &egui::Context) {
        let cancel = ctx.input(|input| {
            input.key_pressed(egui::Key::Escape)
                || input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::PointerGone))
        });
        let stale = self.plans.ceiling_draft.as_ref().is_some_and(|draft| {
            draft.stale(&self.editor, self.plans.active)
                || self.plans.active_sheet.is_some()
                || (draft.original.is_some() && self.selected != Some(draft.id))
        });
        if (cancel || stale) && self.plans.ceiling_draft.take().is_some() {
            self.status = "Ceiling draft canceled.".into();
            self.status_error = false;
        }
    }

    pub(super) fn ceiling_click(&mut self, point: Point2) {
        let result = (|| {
            let draft = self
                .plans
                .ceiling_draft
                .as_mut()
                .ok_or_else(|| Error::Invalid("No ceiling draft".into()))?;
            os_core::ensure(
                !draft.stale(&self.editor, self.plans.active),
                "Ceiling draft is stale",
            )?;
            os_core::ensure(
                point.is_finite() && point_in_plan_crop(draft.context, point),
                "Ceiling points must be inside the plan crop",
            )?;
            os_core::ensure(
                matches!(draft.phase, Phase::Boundary | Phase::Opening),
                "Finish the current ceiling loop before placing more points",
            )?;
            let total = draft.parameters.boundary.len()
                + draft.parameters.holes.iter().map(Vec::len).sum::<usize>()
                + draft.points.len();
            os_core::ensure(
                draft.points.len() < 256 && total < 1024,
                "Ceiling loops are limited to 256 vertices each and 1024 total",
            )?;
            os_core::ensure(
                draft
                    .points
                    .last()
                    .is_none_or(|last| last.distance(point) > 1e-6),
                "Ceiling loop vertices must be distinct",
            )?;
            draft
                .points
                .push(draft.context.basis.plane_to_world(point)?);
            Ok(())
        })();
        self.report(result, "Ceiling draft updated; Apply ceiling commits it.");
    }

    pub(super) fn finish_ceiling_loop(&mut self) {
        let result = (|| {
            let draft = self
                .plans
                .ceiling_draft
                .as_mut()
                .ok_or_else(|| Error::Invalid("No ceiling draft".into()))?;
            os_core::ensure(
                !draft.stale(&self.editor, self.plans.active),
                "Ceiling draft is stale",
            )?;
            let mut parameters = draft.parameters.clone();
            match draft.phase {
                Phase::Boundary => parameters.boundary = draft.points.clone(),
                Phase::Opening => parameters.holes.push(draft.points.clone()),
                Phase::Ready => return Err(Error::Invalid("No active ceiling loop".into())),
            }
            parameters.validate_in(self.editor.document.model())?;
            if parameters.boundary_room.is_some() {
                os_core::ensure(
                    os_geometry::ceilings::CeilingResolver::default()
                        .effective_parameters(self.editor.document.model(), &parameters)?
                        .is_some(),
                    "ceiling loop does not fit the current resolved room boundary",
                )?;
            }
            let elevation = self.editor.document.model().levels[&parameters.level]
                .parameters
                .elevation;
            os_geometry::ceilings::ceiling_mesh(&parameters, elevation)?;
            draft.parameters = parameters;
            draft.points.clear();
            draft.phase = Phase::Ready;
            Ok(())
        })();
        self.report(
            result,
            "Ceiling loop complete. Add openings or Apply ceiling.",
        );
    }

    pub(super) fn apply_ceiling(&mut self) {
        let result = (|| {
            let draft = self
                .plans
                .ceiling_draft
                .as_ref()
                .ok_or_else(|| Error::Invalid("No ceiling draft".into()))?;
            os_core::ensure(
                !draft.stale(&self.editor, self.plans.active) && !draft.sketching(),
                "Finish the current ceiling sketch before applying",
            )?;
            validate_for_apply(
                self.editor.document.model(),
                &draft.parameters,
                draft.original.as_ref(),
            )?;
            let elevation = self.editor.document.model().levels[&draft.parameters.level]
                .parameters
                .elevation;
            os_geometry::ceilings::ceiling_mesh(&draft.parameters, elevation)?;
            let id = draft.id;
            let command = if draft.original.is_some() {
                Command::UpdateCeiling {
                    id,
                    parameters: draft.parameters.clone(),
                }
            } else {
                let mut ceiling = Ceiling::new("core.ceiling", draft.parameters.clone());
                ceiling.header.id = id;
                Command::AddCeiling(ceiling)
            };
            self.editor.command("Apply ceiling", command)?;
            self.plans.ceiling_draft = None;
            self.select(Some(id));
            Ok(())
        })();
        self.report(result, "Ceiling committed.");
    }

    pub(super) fn detach_ceiling_boundary(&mut self, id: Id) {
        let Some(parameters) = self
            .plans
            .ceiling_draft
            .as_ref()
            .filter(|draft| draft.id == id)
            .map(|draft| draft.parameters.clone())
        else {
            return;
        };
        let boundary = os_geometry::ceilings::CeilingResolver::default()
            .resolved_room_boundary(self.editor.document.model(), &parameters)
            .ok()
            .flatten()
            .unwrap_or_else(|| parameters.boundary.clone());
        if let Some(draft) = self
            .plans
            .ceiling_draft
            .as_mut()
            .filter(|draft| draft.id == id)
        {
            draft.parameters.boundary = boundary;
            draft.parameters.boundary_room = None;
            self.status = "Room boundary detached in the draft; Apply ceiling to save.".into();
            self.status_error = false;
        }
    }

    pub(crate) fn ceiling_properties(&mut self, ui: &mut egui::Ui) -> bool {
        self.validate_ceiling_interaction(ui.ctx());
        if self.plans.ceiling_draft.is_none() {
            let Some(ceiling) = self
                .selected
                .and_then(|id| self.editor.document.model().ceilings.get(&id))
                .cloned()
            else {
                return false;
            };
            let Some(context) = self
                .plans
                .active
                .filter(|view| {
                    self.editor
                        .document
                        .model()
                        .views
                        .get(view)
                        .and_then(|view| view.parameters.plan)
                        .is_some_and(|settings| {
                            settings.view_type == os_model::PlanViewType::ReflectedCeilingPlan
                        })
                })
                .and_then(|view| self.editor.native_plan_context(view).ok())
            else {
                ui.label("Open a reflected ceiling plan to edit this ceiling.");
                return true;
            };
            self.plans.ceiling_draft = Some(Draft {
                context,
                session: self.editor.document.session_id(),
                revision: self.editor.document.revision(),
                providers: super::plan_provider_signature(&self.editor),
                id: ceiling.id(),
                original: Some(ceiling.parameters.clone()),
                parameters: ceiling.parameters,
                phase: Phase::Ready,
                points: Vec::new(),
            });
        }
        if self.plans.ceiling_draft.is_none() {
            return false;
        }
        let mut apply = false;
        let mut cancel = false;
        let mut finish = false;
        let mut delete = false;
        let mut detach = false;
        let draft = self.plans.ceiling_draft.as_mut().unwrap();
        egui::ScrollArea::vertical()
            .id_salt("ceiling_properties")
            .show(ui, |ui| {
                ui.label("Horizontal ceiling · metres; elevation is the underside offset.");
                ui.text_edit_singleline(&mut draft.parameters.name);
                if let Some(room_id) = draft.parameters.boundary_room {
                    let room_name = self
                        .editor
                        .document
                        .model()
                        .rooms
                        .get(&room_id)
                        .map(|room| room.parameters.name.as_str());
                    let mut resolver = os_geometry::ceilings::CeilingResolver::default();
                    match resolver.effective_parameters(
                        self.editor.document.model(),
                        &draft.parameters,
                    ) {
                        Ok(Some(_)) => {
                            ui.label(format!(
                                "Live room boundary · {}",
                                room_name.unwrap_or("room")
                            ));
                        }
                        Ok(None) => {
                            ui.colored_label(
                                theme::ERROR,
                                format!(
                                    "Room boundary unresolved · {}. The ceiling is omitted from 2D/3D until repaired or detached.",
                                    room_name.unwrap_or("source room missing")
                                ),
                            );
                        }
                        Err(error) => {
                            ui.colored_label(theme::ERROR, error.to_string());
                        }
                    }
                    detach = ui.button("Detach room boundary").clicked();
                }
                egui::ComboBox::from_id_salt("ceiling_level")
                    .selected_text(
                        self.editor
                            .document
                            .model()
                            .levels
                            .get(&draft.parameters.level)
                            .map_or("Missing level", |level| level.parameters.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for (id, level) in &self.editor.document.model().levels {
                            ui.selectable_value(
                                &mut draft.parameters.level,
                                *id,
                                &level.parameters.name,
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("ceiling_material")
                    .selected_text(
                        draft
                            .parameters
                            .material
                            .and_then(|id| self.editor.document.model().materials.get(&id))
                            .map_or("No material", |material| material.parameters.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.parameters.material, None, "No material");
                        for (id, material) in &self.editor.document.model().materials {
                            ui.selectable_value(
                                &mut draft.parameters.material,
                                Some(*id),
                                &material.parameters.name,
                            );
                        }
                    });
                for (label, value) in [
                    ("Thickness", &mut draft.parameters.thickness),
                    ("Underside offset", &mut draft.parameters.elevation_offset),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                    });
                }
                ui.label(match draft.phase {
                    Phase::Boundary => "Click the outer ceiling boundary vertices.",
                    Phase::Opening => "Click an opening loop, then finish it.",
                    Phase::Ready => "Review the ceiling and apply the edit.",
                });
                if matches!(draft.phase, Phase::Boundary | Phase::Opening) {
                    finish = ui
                        .add_enabled(
                            draft.points.len() >= 3,
                            egui::Button::new("Finish ceiling loop"),
                        )
                        .clicked();
                    if ui.button("Undo ceiling point").clicked() {
                        draft.points.pop();
                    }
                }
                if draft.phase == Phase::Ready {
                    if ui.button("Sketch ceiling opening").clicked() {
                        draft.phase = Phase::Opening;
                        draft.points.clear();
                    }
                    if draft.parameters.boundary_room.is_some() {
                        ui.label("Boundary follows the source room. Edit that room or detach to edit vertices.");
                    } else {
                        if ui.button("Redraw ceiling boundary").clicked() {
                            draft.phase = Phase::Boundary;
                            draft.points.clear();
                        }
                        ring_controls(ui, "Boundary vertices", &mut draft.parameters.boundary);
                    }
                    let mut remove = None;
                    for (index, hole) in draft.parameters.holes.iter_mut().enumerate() {
                        ui.push_id(index, |ui| {
                            ring_controls(ui, &format!("Opening {} vertices", index + 1), hole);
                            if ui.button("Remove ceiling opening").clicked() {
                                remove = Some(index);
                            }
                        });
                    }
                    if let Some(index) = remove {
                        draft.parameters.holes.remove(index);
                    }
                }
                let valid = validate_for_apply(
                    self.editor.document.model(),
                    &draft.parameters,
                    draft.original.as_ref(),
                );
                if let Err(error) = &valid {
                    ui.colored_label(theme::ERROR, error.to_string());
                }
                apply = ui
                    .add_enabled(
                        draft.phase == Phase::Ready
                            && valid.is_ok()
                            && draft.original.as_ref() != Some(&draft.parameters),
                        egui::Button::new("Apply ceiling"),
                    )
                    .clicked();
                cancel = ui.button("Cancel ceiling edits").clicked();
                if draft.original.is_some() {
                    delete = ui.button("Delete ceiling").clicked();
                }
            });
        let id = draft.id;
        if detach {
            self.detach_ceiling_boundary(id);
        }
        if finish {
            self.finish_ceiling_loop();
        }
        if apply {
            self.apply_ceiling();
        }
        if cancel {
            self.plans.ceiling_draft = None;
        }
        if delete {
            let result = self
                .editor
                .command("Delete ceiling", Command::RemoveCeiling(id));
            if result.is_ok() {
                self.plans.ceiling_draft = None;
                self.select(None);
            }
            self.report(result, "Ceiling deleted.");
        }
        true
    }
}

fn validate_for_apply(
    model: &os_model::Model,
    parameters: &CeilingParams,
    original: Option<&CeilingParams>,
) -> os_core::Result<()> {
    parameters.validate_in(model)?;
    let linked_geometry_changed = parameters.boundary_room.is_some()
        && original.is_none_or(|original| {
            original.boundary_room != parameters.boundary_room
                || original.level != parameters.level
                || original.holes != parameters.holes
        });
    if linked_geometry_changed {
        os_core::ensure(
            os_geometry::ceilings::CeilingResolver::default()
                .effective_parameters(model, parameters)?
                .is_some(),
            "ceiling openings must fit the current resolved room boundary; fix the source room or detach the boundary",
        )?;
    }
    Ok(())
}

fn ring_controls(ui: &mut egui::Ui, label: &str, ring: &mut Vec<Point2>) {
    egui::CollapsingHeader::new(label).show(ui, |ui| {
        let mut remove = None;
        let mut insert = None;
        let count = ring.len();
        for (index, point) in ring.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", index + 1));
                    ui.add(
                        egui::DragValue::new(&mut point.x)
                            .speed(0.01)
                            .max_decimals(6),
                    );
                    ui.add(
                        egui::DragValue::new(&mut point.y)
                            .speed(0.01)
                            .max_decimals(6),
                    );
                    if ui.add_enabled(count > 3, egui::Button::new("−")).clicked() {
                        remove = Some(index);
                    }
                    if ui
                        .add_enabled(count < 256, egui::Button::new("+"))
                        .clicked()
                    {
                        insert = Some(index);
                    }
                });
            });
        }
        if let Some(index) = remove {
            ring.remove(index);
        } else if let Some(index) = insert {
            let a = ring[index];
            let b = ring[(index + 1) % ring.len()];
            ring.insert(index + 1, Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5));
        }
    });
}

pub(super) fn plan_item(
    id: Id,
    parameters: &CeilingParams,
    context: PlanContext,
) -> Result<os_render::plan::PlanCeilingItem> {
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
    let triangulation = os_geometry::floor_holes::triangulate_floor_rings(&boundary, &holes)?;
    Ok(os_render::plan::PlanCeilingItem {
        entity: id,
        boundary,
        holes,
        vertices: triangulation.vertices,
        triangles: triangulation.triangles,
        area_m2: triangulation.net_area,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceiling_sketch_is_transient_until_one_step_apply_and_undo() {
        let mut app = DesktopApp::new().unwrap();
        let level = *app.editor.document.model().levels.keys().next().unwrap();
        let view = app
            .editor
            .create_reflected_ceiling_plan("RCP", level)
            .unwrap();
        app.focus_plan(Some(view));
        let revision = app.editor.document.revision();
        app.begin_ceiling(view);
        for point in [
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 3.0),
            Point2::new(0.0, 3.0),
        ] {
            app.ceiling_click(point);
        }
        app.finish_ceiling_loop();
        assert!(app.editor.document.model().ceilings.is_empty());
        assert_eq!(app.editor.document.revision(), revision);

        app.apply_ceiling();
        assert_eq!(app.editor.document.model().ceilings.len(), 1);
        assert_eq!(app.editor.document.revision(), revision + 1);
        app.editor.undo().unwrap();
        assert!(app.editor.document.model().ceilings.is_empty());
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model().ceilings.len(), 1);
    }

    #[test]
    fn document_changes_cancel_ceiling_sketch_without_touching_model() {
        let mut app = DesktopApp::new().unwrap();
        let level = *app.editor.document.model().levels.keys().next().unwrap();
        let view = app
            .editor
            .create_reflected_ceiling_plan("RCP", level)
            .unwrap();
        app.focus_plan(Some(view));
        app.begin_ceiling(view);
        app.ceiling_click(Point2::new(1.0, 1.0));
        app.editor
            .command("Rename", Command::RenameProject("Changed".into()))
            .unwrap();
        app.validate_ceiling_interaction(&egui::Context::default());
        assert!(app.plans.ceiling_draft.is_none());
        assert!(app.editor.document.model().ceilings.is_empty());
    }

    #[test]
    fn invalid_loop_stays_a_draft_and_escape_cancels_it() {
        let mut app = DesktopApp::new().unwrap();
        let level = *app.editor.document.model().levels.keys().next().unwrap();
        let view = app
            .editor
            .create_reflected_ceiling_plan("RCP", level)
            .unwrap();
        app.focus_plan(Some(view));
        app.begin_ceiling(view);
        for point in [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 0.0),
        ] {
            app.ceiling_click(point);
        }
        app.finish_ceiling_loop();
        assert_eq!(
            app.plans.ceiling_draft.as_ref().unwrap().phase,
            Phase::Boundary
        );
        assert!(app.editor.document.model().ceilings.is_empty());

        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |_| {},
        );
        app.validate_ceiling_interaction(&ctx);
        assert!(app.plans.ceiling_draft.is_none());
        assert!(app.editor.document.model().ceilings.is_empty());
    }

    #[test]
    fn only_reflected_ceiling_plans_project_native_ceilings() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let floor_plan = editor.create_floor_plan("Floor plan", level).unwrap();
        let rcp = editor.create_reflected_ceiling_plan("RCP", level).unwrap();
        let settings = editor.document.model().views[&rcp].parameters.plan.unwrap();
        assert_eq!(
            settings.view_type,
            os_model::PlanViewType::ReflectedCeilingPlan
        );
        assert!(settings.validate().is_ok(), "{settings:?}");
        let ceiling = Ceiling::new(
            "core.ceiling",
            CeilingParams {
                name: "Ceiling".into(),
                level,
                material: None,
                boundary_room: None,
                boundary: vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(4.0, 0.0),
                    Point2::new(4.0, 3.0),
                    Point2::new(0.0, 3.0),
                ],
                holes: Vec::new(),
                thickness: 0.12,
                elevation_offset: 2.55,
            },
        );
        editor
            .command("Place ceiling", Command::AddCeiling(ceiling))
            .unwrap();
        let reflected = editor.plan_snapshot(rcp).unwrap().derive().unwrap();
        let context = editor.native_plan_context(rcp).unwrap();
        assert_eq!(reflected.ceilings(context).unwrap().len(), 1);
        let ordinary = editor.plan_snapshot(floor_plan).unwrap().derive().unwrap();
        let context = editor.native_plan_context(floor_plan).unwrap();
        assert!(ordinary.ceilings(context).unwrap().is_empty());
    }
}
