//! Native roof sketches and exact edits, retained outside the document until Apply.
use super::*;
use os_model::{Roof, RoofParams};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Boundary,
    Opening,
    SlopeStart,
    SlopeEnd,
    Ready,
}

pub(super) struct Draft {
    pub context: PlanContext,
    revision: u64,
    providers: Vec<(String, Id)>,
    pub id: Id,
    pub original: Option<RoofParams>,
    pub parameters: RoofParams,
    pub phase: Phase,
    pub points: Vec<Point2>,
}

impl Draft {
    pub fn sketching(&self) -> bool {
        self.phase != Phase::Ready
    }
    fn stale(&self, editor: &Editor, active: Option<Id>) -> bool {
        active != Some(self.context.view_id)
            || editor.document.revision() != self.revision
            || editor.native_plan_context(self.context.view_id).ok() != Some(self.context)
            || plan_provider_signature(editor) != self.providers
            || self.original.as_ref().is_some_and(|original| {
                editor
                    .document
                    .model()
                    .roofs
                    .get(&self.id)
                    .is_none_or(|r| &r.parameters != original)
            })
    }
}

impl DesktopApp {
    pub(crate) fn begin_roof(&mut self, view: Id) {
        let result = (|| {
            let context = self.editor.native_plan_context(view)?;
            os_core::ensure(
                self.plans.active == Some(view) && self.plans.active_sheet.is_none(),
                "Open a floor plan first",
            )?;
            let level = self.editor.document.model().views[&view]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Roof needs a plan level".into()))?;
            self.cancel_plan_wall();
            self.cancel_opening_placement();
            self.select(None);
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
            self.plans.roof_draft = Some(Draft {
                context,
                revision: self.editor.document.revision(),
                providers: plan_provider_signature(&self.editor),
                id: Id::new(),
                original: None,
                parameters: RoofParams {
                    name: format!("Roof {}", self.editor.document.model().roofs.len() + 1),
                    level,
                    material: None,
                    boundary: Vec::new(),
                    holes: Vec::new(),
                    thickness: 0.2,
                    top_offset: 0.0,
                    slope_start: Point2::new(0., 0.),
                    slope_end: Point2::new(1., 0.),
                    rise_per_run: 0.25,
                },
                phase: Phase::Boundary,
                points: Vec::new(),
            });
            Ok(())
        })();
        self.report(
            result,
            "Roof · click boundary vertices, then Finish roof loop in Properties.",
        );
    }

    pub(super) fn validate_roof_interaction(&mut self, ctx: &egui::Context) {
        let cancel = ctx.input(|i| {
            i.key_pressed(egui::Key::Escape)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::PointerGone))
        });
        let stale = self.plans.roof_draft.as_ref().is_some_and(|d| {
            d.stale(&self.editor, self.plans.active)
                || self.plans.active_sheet.is_some()
                || d.original.is_some() && self.selected != Some(d.id)
        });
        if (cancel || stale) && self.plans.roof_draft.take().is_some() {
            self.status = "Roof draft canceled.".into();
            self.status_error = false;
        }
    }

    pub(super) fn roof_click(&mut self, point: Point2) {
        let result = (|| {
            let d = self
                .plans
                .roof_draft
                .as_mut()
                .ok_or_else(|| Error::Invalid("No roof draft".into()))?;
            os_core::ensure(
                !d.stale(&self.editor, self.plans.active),
                "Roof draft is stale",
            )?;
            os_core::ensure(
                point.is_finite()
                    && point_in_plan_crop(d.context, d.context.basis.world_to_plane(point)?),
                "Place roof points inside the plan crop",
            )?;
            match d.phase {
                Phase::Boundary | Phase::Opening => {
                    os_core::ensure(
                        d.points.len() < 256 && d.points.iter().all(|p| p.distance(point) > 1e-6),
                        "Roof loop needs distinct vertices, at most 256",
                    )?;
                    d.points.push(point);
                }
                Phase::SlopeStart => {
                    d.parameters.slope_start = point;
                    d.phase = Phase::SlopeEnd;
                }
                Phase::SlopeEnd => {
                    let mut p = d.parameters.clone();
                    p.slope_end = point;
                    p.validate_in(self.editor.document.model())?;
                    d.parameters = p;
                    d.phase = Phase::Ready;
                }
                Phase::Ready => {}
            }
            Ok(())
        })();
        self.report(
            result,
            "Roof draft updated; Apply roof commits the whole edit.",
        );
    }

    pub(super) fn finish_roof_loop(&mut self) {
        let result = (|| {
            let d = self
                .plans
                .roof_draft
                .as_mut()
                .ok_or_else(|| Error::Invalid("No roof draft".into()))?;
            os_core::ensure(
                !d.stale(&self.editor, self.plans.active),
                "Roof draft is stale",
            )?;
            let mut p = d.parameters.clone();
            match d.phase {
                Phase::Boundary => p.boundary = d.points.clone(),
                Phase::Opening => p.holes.push(d.points.clone()),
                _ => return Err(Error::Invalid("No active roof loop".into())),
            }
            p.validate_in(self.editor.document.model())?;
            os_geometry::roofs::roof_mesh(
                &p,
                self.editor.document.model().levels[&p.level]
                    .parameters
                    .elevation,
            )?;
            let first_boundary = d.phase == Phase::Boundary && d.original.is_none();
            d.parameters = p;
            d.points.clear();
            d.phase = if first_boundary {
                Phase::SlopeStart
            } else {
                Phase::Ready
            };
            Ok(())
        })();
        self.report(
            result,
            "Loop complete. Place the slope arrow, add openings, or Apply roof.",
        );
    }

    pub(super) fn apply_roof(&mut self) {
        let result = (|| {
            let d = self
                .plans
                .roof_draft
                .as_ref()
                .ok_or_else(|| Error::Invalid("No roof draft".into()))?;
            os_core::ensure(
                !d.stale(&self.editor, self.plans.active) && !d.sketching(),
                "Finish the current roof sketch before applying",
            )?;
            d.parameters.validate_in(self.editor.document.model())?;
            os_geometry::roofs::roof_mesh(
                &d.parameters,
                self.editor.document.model().levels[&d.parameters.level]
                    .parameters
                    .elevation,
            )?;
            let id = d.id;
            let command = if d.original.is_some() {
                Command::UpdateRoof {
                    id,
                    parameters: d.parameters.clone(),
                }
            } else {
                let mut roof = Roof::new("core.roof", d.parameters.clone());
                roof.header.id = id;
                Command::AddRoof(roof)
            };
            self.editor.command("Apply roof", command)?;
            self.plans.roof_draft = None;
            self.select(Some(id));
            Ok(())
        })();
        self.report(result, "Roof committed.");
    }

    pub(crate) fn roof_properties(&mut self, ui: &mut egui::Ui) -> bool {
        self.validate_roof_interaction(ui.ctx());
        if self.plans.roof_draft.is_none() {
            let Some(roof) = self
                .selected
                .and_then(|id| self.editor.document.model().roofs.get(&id))
                .cloned()
            else {
                return false;
            };
            let Some(context) = self
                .plans
                .active
                .and_then(|view| self.editor.native_plan_context(view).ok())
            else {
                ui.label("Open a floor plan to edit this roof.");
                return true;
            };
            self.plans.roof_draft = Some(Draft {
                context,
                revision: self.editor.document.revision(),
                providers: plan_provider_signature(&self.editor),
                id: roof.id(),
                original: Some(roof.parameters.clone()),
                parameters: roof.parameters,
                phase: Phase::Ready,
                points: Vec::new(),
            });
        }
        let mut apply = false;
        let mut cancel = false;
        let mut finish = false;
        let mut delete = false;
        let d = self.plans.roof_draft.as_mut().unwrap();
        egui::ScrollArea::vertical()
            .id_salt("roof_properties")
            .show(ui, |ui| {
                ui.label("Single-plane roof · metres");
                ui.text_edit_singleline(&mut d.parameters.name);
                egui::ComboBox::from_id_salt("roof_level")
                    .selected_text("Roof level")
                    .show_ui(ui, |ui| {
                        for (id, level) in &self.editor.document.model().levels {
                            ui.selectable_value(
                                &mut d.parameters.level,
                                *id,
                                &level.parameters.name,
                            );
                        }
                    });
                egui::ComboBox::from_id_salt("roof_material")
                    .selected_text(
                        d.parameters
                            .material
                            .and_then(|id| self.editor.document.model().materials.get(&id))
                            .map_or("No material", |m| m.parameters.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.parameters.material, None, "No material");
                        for (id, m) in &self.editor.document.model().materials {
                            ui.selectable_value(
                                &mut d.parameters.material,
                                Some(*id),
                                &m.parameters.name,
                            );
                        }
                    });
                for (label, value) in [
                    ("Vertical thickness", &mut d.parameters.thickness),
                    ("Anchor top offset", &mut d.parameters.top_offset),
                    ("Signed rise / run", &mut d.parameters.rise_per_run),
                    ("Arrow start X", &mut d.parameters.slope_start.x),
                    ("Arrow start Y", &mut d.parameters.slope_start.y),
                    ("Arrow end X", &mut d.parameters.slope_end.x),
                    ("Arrow end Y", &mut d.parameters.slope_end.y),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                    });
                }
                ui.label(match d.phase {
                    Phase::Boundary => "Click outer boundary vertices.",
                    Phase::Opening => "Click opening vertices.",
                    Phase::SlopeStart => "Click slope anchor (level + offset).",
                    Phase::SlopeEnd => "Click slope direction endpoint.",
                    Phase::Ready => "Review the preview, then Apply roof.",
                });
                if matches!(d.phase, Phase::Boundary | Phase::Opening) {
                    finish = ui
                        .add_enabled(d.points.len() >= 3, egui::Button::new("Finish roof loop"))
                        .clicked();
                    if ui.button("Undo roof point").clicked() {
                        d.points.pop();
                    }
                }
                if d.phase == Phase::Ready {
                    if ui.button("Sketch roof opening").clicked() {
                        d.phase = Phase::Opening;
                        d.points.clear();
                    }
                    if ui.button("Redraw roof boundary").clicked() {
                        d.phase = Phase::Boundary;
                        d.points.clear();
                    }
                    if ui.button("Place roof slope arrow").clicked() {
                        d.phase = Phase::SlopeStart;
                    }
                    ring_controls(ui, "Boundary vertices", &mut d.parameters.boundary);
                    let mut remove = None;
                    for (index, hole) in d.parameters.holes.iter_mut().enumerate() {
                        ui.push_id(index, |ui| {
                            ring_controls(ui, &format!("Opening {} vertices", index + 1), hole);
                            if ui.button("Remove roof opening").clicked() {
                                remove = Some(index);
                            }
                        });
                    }
                    if let Some(index) = remove {
                        d.parameters.holes.remove(index);
                    }
                }
                let valid = d.parameters.validate_in(self.editor.document.model());
                if let Err(e) = &valid {
                    ui.colored_label(theme::ERROR, e.to_string());
                }
                apply = ui
                    .add_enabled(
                        d.phase == Phase::Ready
                            && valid.is_ok()
                            && d.original.as_ref() != Some(&d.parameters),
                        egui::Button::new("Apply roof"),
                    )
                    .clicked();
                cancel = ui.button("Cancel roof edits").clicked();
                if d.original.is_some() {
                    delete = ui.button("Delete roof").clicked();
                }
            });
        let id = d.id;
        if finish {
            self.finish_roof_loop();
        }
        if apply {
            self.apply_roof();
        }
        if cancel {
            self.plans.roof_draft = None;
        }
        if delete {
            let result = self.editor.command("Delete roof", Command::RemoveRoof(id));
            if result.is_ok() {
                self.plans.roof_draft = None;
                self.select(None);
            }
            self.report(result, "Roof deleted.");
        }
        true
    }
}

fn ring_controls(ui: &mut egui::Ui, label: &str, ring: &mut Vec<Point2>) {
    egui::CollapsingHeader::new(label).show(ui, |ui| {
        let mut remove = None;
        let mut insert = None;
        let count = ring.len();
        for (index, p) in ring.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", index + 1));
                    ui.add(egui::DragValue::new(&mut p.x).speed(0.01).max_decimals(6));
                    ui.add(egui::DragValue::new(&mut p.y).speed(0.01).max_decimals(6));
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
        if let Some(i) = remove {
            ring.remove(i);
        } else if let Some(i) = insert {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            ring.insert(i + 1, Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5));
        }
    });
}

pub(super) fn preview(d: &Draft, editor: &Editor, pointer: Option<Point2>) -> Result<PlanDrawing> {
    let mut p = d.parameters.clone();
    let mut points = d.points.clone();
    if let Some(point) = pointer
        && points.last().is_none_or(|last| last.distance(point) > 1e-6)
    {
        points.push(point);
    }
    match d.phase {
        Phase::Boundary => p.boundary = points,
        Phase::Opening => p.holes.push(points),
        Phase::SlopeEnd => {
            if let Some(point) = pointer {
                p.slope_end = point;
            }
        }
        _ => {}
    }
    let level = editor
        .document
        .model()
        .levels
        .get(&p.level)
        .ok_or_else(|| Error::Invalid("Roof level missing".into()))?;
    let plan = os_geometry::roofs::roof_plan(
        &p,
        level.parameters.elevation,
        d.context.range,
        d.context.basis,
        d.context.crop,
    )?;
    let surface = os_geometry::SurfaceIdentity {
        layer: None,
        material: p.material,
    };
    PlanDrawing::from_layered_footprints(
        d.context,
        &BTreeMap::from([(
            d.id,
            plan.footprints.into_iter().map(|f| (surface, f)).collect(),
        )]),
        Vec::new(),
    )?
    .without_wall_seams(&BTreeMap::from([(d.id, plan.seams)]))
}

pub(super) fn paint_sketch(
    painter: &egui::Painter,
    d: &Draft,
    pointer: Option<Point2>,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let mut points = d.points.clone();
    if matches!(d.phase, Phase::SlopeStart | Phase::SlopeEnd | Phase::Ready) {
        points = vec![d.parameters.slope_start, d.parameters.slope_end];
    }
    if let Some(p) = pointer {
        if matches!(d.phase, Phase::Boundary | Phase::Opening) {
            points.push(p);
        }
        if d.phase == Phase::SlopeEnd {
            points[1] = p;
        }
    }
    let screen: Vec<_> = points
        .into_iter()
        .filter_map(|p| {
            d.context
                .basis
                .world_to_plane(p)
                .and_then(|p| camera.project(p, size))
                .ok()
        })
        .filter(|p| p.x.abs() < 1e8 && p.y.abs() < 1e8)
        .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
        .collect();
    for pair in screen.windows(2) {
        painter.line_segment([pair[0], pair[1]], egui::Stroke::new(2., theme::ACCENT));
    }
    for p in &screen {
        painter.circle_filled(*p, 3., theme::ACCENT);
    }
    if matches!(d.phase, Phase::SlopeEnd | Phase::Ready) && screen.len() == 2 {
        painter.arrow(
            screen[0],
            screen[1] - screen[0],
            egui::Stroke::new(2., theme::ACCENT),
        );
    }
}
