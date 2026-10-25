//! Disposable rectangle authoring and reference management.
use super::*;
use os_model::{CalloutReference, SectionCallout, SectionCalloutBounds, SectionCalloutParams};

#[derive(Default)]
pub(super) struct State {
    open: bool,
    name: String,
    scale: f64,
    reference: Option<Id>,
    selected: Option<Id>,
    pub draft: Option<Draft>,
    edit: Option<(Id, Id, u64, SectionCalloutParams)>,
    pub claimed: bool,
}
pub(super) struct Draft {
    session: Id,
    revision: u64,
    context: PlanContext,
    providers: Vec<(String, Id)>,
    name: String,
    scale: f64,
    reference: Option<Id>,
    moving: Option<(Id, SectionCalloutParams)>,
    start: Option<Point2>,
    rectangle: Option<SectionCalloutBounds>,
    camera: Option<(PlanCamera, egui::Rect)>,
}
impl Draft {
    fn current(&self, editor: &Editor, active: Option<Id>) -> bool {
        active == Some(self.context.view_id)
            && self.session == editor.document.session_id()
            && self.revision == editor.document.revision()
            && editor.native_section_context(self.context.view_id).ok() == Some(self.context)
            && plan_provider_signature(editor) == self.providers
    }
    fn commands(&self, editor: &Editor) -> Result<Vec<Command>> {
        let boundary = self
            .rectangle
            .ok_or_else(|| Error::Invalid("Draw a nonempty rectangle".into()))?;
        if let Some((id, original)) = &self.moving {
            let mut parameters = original.clone();
            parameters.boundary = boundary;
            return Ok(vec![Command::UpdateSectionCallout {
                id: *id,
                parameters,
            }]);
        }
        if let Some(target) = self.reference {
            return Ok(vec![Command::AddSectionCallout(SectionCallout::new(
                "core.section_callout",
                SectionCalloutParams {
                    parent: self.context.view_id,
                    target,
                    owns_target: false,
                    boundary,
                    destination: None,
                },
            ))]);
        }
        editor
            .document
            .enlarged_section_commands(
                self.context.view_id,
                self.name.clone(),
                boundary,
                self.scale,
            )
            .map(|(_, _, c)| c)
    }
}

fn paint_rectangle(
    painter: &egui::Painter,
    camera: PlanCamera,
    rect: egui::Rect,
    context: PlanContext,
    crop: SectionCalloutBounds,
    label: &str,
    color: egui::Color32,
) {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let project = |p| {
        camera
            .project(p, size)
            .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
    };
    let corners = [
        crop.min,
        Point2::new(crop.max.x, crop.min.y),
        crop.max,
        Point2::new(crop.min.x, crop.max.y),
    ];
    for i in 0..4 {
        if let Some((a, b)) = clip_plan_segment(corners[i], corners[(i + 1) % 4], context.crop)
            && let (Ok(a), Ok(b)) = (project(a), project(b))
        {
            painter.line_segment([a, b], egui::Stroke::new(1.5, color));
        }
    }
    if point_in_plan_crop(context, corners[3])
        && let Ok(p) = project(corners[3])
    {
        painter.text(
            p,
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(13.0),
            color,
        );
    }
}
impl DesktopApp {
    pub(super) fn begin_section_callout(&mut self, moving: Option<(Id, SectionCalloutParams)>) {
        let Some(view) = self.plans.active else {
            return;
        };
        let Ok(context) = self.editor.native_section_context(view) else {
            return;
        };
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.cancel_opening_controls();
        self.plans.area_selection.cancel();
        self.plans.overlap_selection.cancel();
        self.plans.crop = crop::State::default();
        self.plans.transform = None;
        self.plans.column_placement = None;
        self.plans.casework_placement = None;
        self.plans.floor_sketch = None;
        self.plans.floor_hole_sketch = None;
        self.plans.curtain_draft = None;
        self.plans.stair_placement = None;
        self.plans.ramp_placement = None;
        self.plans.roof_draft = None;
        self.plans.ceiling_draft = None;
        self.plans.section_placement = None;
        self.plans.section_dimension = None;
        self.plans.callouts = callouts::State::default();
        self.plans.room_placement_active = false;
        self.plan_draft = None;
        let s = &mut self.plans.section_callouts;
        s.draft = Some(Draft {
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            context,
            providers: plan_provider_signature(&self.editor),
            name: s.name.clone(),
            scale: s.scale,
            reference: s.reference,
            moving,
            start: None,
            rectangle: None,
            camera: None,
        });
        s.open = false;
    }
    pub(super) fn validate_section_callouts(&mut self, ctx: &egui::Context) {
        let cancel = self.plans.active_sheet.is_some()
            || self
                .plans
                .section_callouts
                .draft
                .as_ref()
                .is_some_and(|d| !d.current(&self.editor, self.plans.active))
            || ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::PointerGone | egui::Event::WindowFocused(false)
                        )
                    })
            });
        if cancel {
            self.plans.section_callouts.draft = None;
            self.plans.section_callouts.edit = None;
        }
        // Keep a canceled held press consumed through its release.
        if !ctx.input(|i| i.pointer.primary_down() || i.pointer.primary_released()) {
            self.plans.section_callouts.claimed = false;
        }
    }
    pub(super) fn section_callout_button(&mut self, ui: &mut egui::Ui) {
        let enabled = self
            .plans
            .active
            .and_then(|id| self.editor.document.model().views.get(&id))
            .is_some_and(|v| {
                v.parameters.kind == os_model::ViewKind::Section && v.parameters.section.is_some()
            });
        if !enabled || self.plans.active_sheet.is_some() {
            return;
        }
        if ui
            .add_enabled(enabled, egui::Button::new("Section callouts"))
            .clicked()
        {
            let s = &mut self.plans.section_callouts;
            s.open = !s.open;
            if s.name.is_empty() {
                s.name = "Enlarged section".into();
                s.scale = 25.0;
            }
        }
    }
    pub(super) fn section_callout_window(&mut self, ctx: &egui::Context) {
        if !self.plans.section_callouts.open {
            return;
        }
        let Some(parent) = self.plans.active else {
            return;
        };
        let model = self.editor.document.model();
        let mut begin = None;
        let mut commands = None;
        let mut navigate = None;
        let mut sheet = None;
        let mut edit_crop = false;
        egui::Window::new("Section callouts").default_width(390.0).show(ctx, |ui| {
            let s = &mut self.plans.section_callouts;
            ui.label("Name and paper scale for a new enlarged section");
            ui.text_edit_singleline(&mut s.name);
            ui.add(egui::DragValue::new(&mut s.scale).prefix("1:").range(1.0..=1e6));
            egui::ComboBox::from_id_salt("section_callout_reference").selected_text(s.reference.and_then(|id| model.views.get(&id)).map_or("Create enlarged section",|v| v.parameters.name.as_str())).show_ui(ui,|ui| {
                ui.selectable_value(&mut s.reference,None,"Create enlarged section");
                for v in model.views.values().filter(|v| model.compatible_callout_sections(parent,v.id())) {
                    ui.selectable_value(&mut s.reference,Some(v.id()),&v.parameters.name);
                }
            });
            if ui.button("Draw rectangular callout").clicked() { begin = Some(None); }
            ui.label("Drag in the section to preview; release applies. Escape cancels.");
            ui.separator();
            for c in model.section_callouts.values().filter(|c| c.parameters.parent == parent || c.parameters.target == parent) {
                let label = format!("{}: {}",model.views[&c.parameters.target].parameters.name,model.section_callout_reference(&c.parameters).label());
                if ui.selectable_label(s.selected == Some(c.id()), label).clicked() { s.selected = Some(c.id()); s.edit = None; }
            }
            if let Some(id) = s.selected && let Some(c) = model.section_callouts.get(&id) {
                let session = self.editor.document.session_id(); let revision = self.editor.document.revision();
                if s.edit.as_ref().is_none_or(|(i,ss,r,_)| *i!=id || *ss!=session || *r!=revision) {
                    s.edit=Some((id,session,revision,c.parameters.clone()));
                }
                let p = &mut s.edit.as_mut().unwrap().3;
                if !p.owns_target {
                    egui::ComboBox::from_id_salt("repair_section_callout_target").selected_text(&model.views[&p.target].parameters.name).show_ui(ui,|ui| {
                        for v in model.views.values().filter(|v| model.compatible_callout_sections(p.parent,v.id())) {
                            if ui.selectable_value(&mut p.target,v.id(),&v.parameters.name).changed() { p.destination=None; }
                        }
                    });
                }
                ui.label("Marker rectangle in parent station/elevation metres");
                ui.horizontal(|ui| { ui.add(egui::DragValue::new(&mut p.boundary.min.x).prefix("Left ")); ui.add(egui::DragValue::new(&mut p.boundary.min.y).prefix("Bottom ")); });
                ui.horizontal(|ui| { ui.add(egui::DragValue::new(&mut p.boundary.max.x).prefix("Right ")); ui.add(egui::DragValue::new(&mut p.boundary.max.y).prefix("Top ")); });
                egui::ComboBox::from_id_salt("section_callout_destination").selected_text(model.section_callout_reference(p).label()).show_ui(ui,|ui| {
                    ui.selectable_value(&mut p.destination,None,"Automatic (unique placement only)");
                    for sh in model.sheets.values() { for vp in sh.parameters.viewports.iter().filter(|v| v.view==p.target) {
                        ui.selectable_value(&mut p.destination,Some(vp.id),format!("{} / {}",vp.detail_number,sh.parameters.number));
                    }}
                });
                if ui.button("Apply marker / reference").clicked() { commands=Some(vec![Command::UpdateSectionCallout { id, parameters:p.clone() }]); }
                if ui.add_enabled(p.parent==parent,egui::Button::new("Drag marker")).clicked() { begin=Some(Some((id,c.parameters.clone()))); }
                ui.horizontal(|ui| {
                    if ui.button("Open parent").clicked() { navigate=Some(p.parent); }
                    if ui.button("Open target").clicked() { navigate=Some(p.target); }
                    let crop_available = model.views.get(&p.target).and_then(|v| v.parameters.section).is_some();
                    if ui.add_enabled(crop_available,egui::Button::new("Edit target crop")).clicked() { navigate=Some(p.target); edit_crop=true; }
                });
                if let CalloutReference::Placed { sheet:id,.. } = model.section_callout_reference(p) && ui.button("Open referenced sheet").clicked() { sheet=Some(id); }
                if ui.button("Remove marker (keep target section)").clicked() { commands=Some(vec![Command::RemoveSectionCallout(id)]); }
                ui.label("Remove or repair references before deleting their target or chosen placement. Target crop and marker bounds are edited independently.");
            }
            if ui.button("Close").clicked() { s.open=false; }
        });
        if let Some(commands) = commands {
            let result = self
                .editor
                .document
                .execute("Edit section callout", commands)
                .and_then(|()| self.editor.regenerate());
            self.report(result, "Callout updated.");
        }
        if let Some(moving) = begin {
            self.begin_section_callout(moving);
        }
        if let Some(view) = navigate {
            self.focus_plan(Some(view));
            self.plans.active_sheet = None;
            self.plans.section_callouts.open = false;
            if edit_crop {
                let result = self.begin_section_settings(view);
                self.report(result, "Edit target station/elevation crop and scale.");
            }
        }
        if let Some(id) = sheet {
            self.open_sheet(id);
            self.plans.section_callouts.open = false;
        }
    }
    pub(super) fn section_callout_input(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        camera: PlanCamera,
        ctx: &egui::Context,
    ) {
        let Some(d) = self.plans.section_callouts.draft.as_mut() else {
            return;
        };
        let rect = response.rect;
        if d.camera.is_some_and(|c| c != (camera, rect)) {
            self.plans.section_callouts.draft = None;
            return;
        }
        let point = ctx
            .input(|i| i.pointer.interact_pos())
            .filter(|p| rect.contains(*p))
            .and_then(|p| {
                camera
                    .unproject(
                        Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())),
                        [f64::from(rect.width()), f64::from(rect.height())],
                    )
                    .ok()
            });
        if response.contains_pointer()
            && ctx.input(|i| i.pointer.primary_pressed())
            && let Some(p) = point.filter(|p| point_in_plan_crop(d.context, *p))
            && d.moving.as_ref().is_none_or(|(_, m)| {
                p.x >= m.boundary.min.x
                    && p.x <= m.boundary.max.x
                    && p.y >= m.boundary.min.y
                    && p.y <= m.boundary.max.y
            })
        {
            d.start = Some(p);
            d.camera = Some((camera, rect));
            self.plans.section_callouts.claimed = true;
        }
        if let Some(a) = d.start
            && let Some(b) = point
        {
            let c = if let Some((_, m)) = &d.moving {
                SectionCalloutBounds {
                    min: Point2::new(m.boundary.min.x + b.x - a.x, m.boundary.min.y + b.y - a.y),
                    max: Point2::new(m.boundary.max.x + b.x - a.x, m.boundary.max.y + b.y - a.y),
                }
            } else {
                SectionCalloutBounds {
                    min: Point2::new(a.x.min(b.x), a.y.min(b.y)),
                    max: Point2::new(a.x.max(b.x), a.y.max(b.y)),
                }
            };
            d.rectangle = Some(c);
            paint_rectangle(
                painter,
                camera,
                rect,
                d.context,
                c,
                &format!("{}  1:{}", d.name, d.scale),
                theme::ACCENT,
            );
        }
        if d.start.is_some() && ctx.input(|i| i.pointer.primary_released()) {
            let result = if point.is_some() {
                d.commands(&self.editor)
                    .and_then(|commands| self.editor.document.execute("Section callout", commands))
                    .and_then(|()| self.editor.regenerate())
            } else {
                Err(Error::Invalid("Callout canceled outside canvas".into()))
            };
            self.plans.section_callouts.draft = None;
            self.report(result, "Callout applied.");
        }
    }
}

#[cfg(test)]
#[path = "section_callouts_tests.rs"]
mod tests;
