//! Disposable view-owned note editor and pointer placement.
use super::*;
use os_model::{NoteAlignment, TextNote, TextNoteParams, ViewKind};
use os_render::plan::NoteGraphic;

#[derive(Default)]
pub(super) struct State {
    pub draft: Option<Draft>,
    pub claimed: bool,
    hit: Option<Id>,
}
pub(super) struct Draft {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    id: Option<Id>,
    parameters: TextNoteParams,
    placing: bool,
    camera: Option<(PlanCamera, egui::Rect)>,
}
impl Draft {
    pub(super) fn editing_id(&self) -> Option<Id> {
        self.id
    }
    fn current(&self, editor: &Editor, active: Option<Id>) -> bool {
        active == Some(self.context.view_id)
            && editor.native_view_context(self.context.view_id).ok() == Some(self.context)
            && plan_provider_signature(editor) == self.providers
    }
}
pub(super) enum Action {
    Select(Id),
    Commit,
}
impl DesktopApp {
    pub(crate) fn begin_text_note(&mut self, id: Option<Id>) {
        let Some(view) = self.plans.active else {
            return;
        };
        let model = self.editor.document.model();
        if !model.views.get(&view).is_some_and(|v| {
            matches!(
                v.parameters.kind,
                ViewKind::Plan | ViewKind::Section | ViewKind::Drafting
            )
        }) {
            return;
        }
        let Ok(context) = self.editor.native_view_context(view) else {
            return;
        };
        let parameters = if let Some(id) = id {
            let Some(note) = model
                .text_notes
                .get(&id)
                .filter(|n| n.parameters.view == view)
            else {
                return;
            };
            note.parameters.clone()
        } else {
            TextNoteParams::new(view)
        };
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.cancel_opening_controls();
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
        self.plans.room_placement_active = false;
        self.plans.room_tag_draft = None;
        self.plans.opening_tag_draft = None;
        self.plans.room_separation_line_draft = None;
        self.plan_draft = None;
        self.grid_draft = None;
        self.opening_draft = None;
        self.plans.detail_line_draft = None;
        self.plans.section_dimension = None;
        self.plans.section_placement = None;
        self.plans.callouts = callouts::State::default();
        self.plans.section_callouts = section_callouts::State::default();
        self.plans.crop.cancel();
        self.plans.area_selection.cancel();
        self.plans.overlap_selection.cancel();
        self.plans.text_notes.draft = Some(Draft {
            context,
            providers: plan_provider_signature(&self.editor),
            id,
            parameters,
            placing: false,
            camera: None,
        });
    }
    pub(super) fn note_window(&mut self, ctx: &egui::Context) {
        let cancel = self.plans.active_sheet.is_some()
            || self.wall_gesture.is_some()
            || self.plans.detail_line_draft.is_some()
            || self.plans.section_dimension.is_some()
            || self.plans.section_placement.is_some()
            || self.plans.callouts.draft.is_some()
            || self.plans.section_callouts.draft.is_some()
            || self
                .plans
                .text_notes
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
            self.plans.text_notes.draft = None;
        }
        if !ctx.input(|i| i.pointer.primary_down() || i.pointer.primary_released()) {
            self.plans.text_notes.claimed = false;
        }
        let Some(d) = self.plans.text_notes.draft.as_mut().filter(|d| !d.placing) else {
            return;
        };
        let mut apply = false;
        let mut close = false;
        let mut delete = None;
        egui::Window::new("Text note")
            .default_width(360.)
            .show(ctx, |ui| {
                ui.label("Printable ASCII and newlines; paper dimensions in mm");
                ui.add(
                    egui::TextEdit::multiline(&mut d.parameters.text)
                        .desired_rows(4)
                        .desired_width(330.),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut d.parameters.width_mm)
                            .prefix("Width ")
                            .range(1.0..=500.),
                    );
                    ui.add(
                        egui::DragValue::new(&mut d.parameters.text_height_mm)
                            .prefix("Height ")
                            .range(1.0..=20.),
                    );
                });
                ui.horizontal(|ui| {
                    for (label, value) in [
                        ("Left", NoteAlignment::Left),
                        ("Center", NoteAlignment::Center),
                        ("Right", NoteAlignment::Right),
                    ] {
                        ui.selectable_value(&mut d.parameters.alignment, value, label);
                    }
                });
                ui.label("Anchor: Plan world XY; Section station / elevation; Drafting XY (m)");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut d.parameters.position.x).prefix("X / station "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut d.parameters.position.y).prefix("Y / elevation "),
                    );
                });
                let mut leader = d.parameters.leader.is_some();
                if ui.checkbox(&mut leader, "Straight leader").changed() {
                    d.parameters.leader = leader.then(|| {
                        Point2::new(d.parameters.position.x - 0.5, d.parameters.position.y - 0.5)
                    });
                }
                if let Some(tip) = &mut d.parameters.leader {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut tip.x).prefix("Tip X "));
                        ui.add(egui::DragValue::new(&mut tip.y).prefix("Tip Y "));
                    });
                }
                let valid = d.parameters.validate(self.editor.document.model());
                if let Err(error) = &valid {
                    ui.colored_label(theme::ERROR, error.to_string());
                }
                if ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Place / move on drawing"))
                    .clicked()
                {
                    d.placing = true;
                    d.camera = None;
                }
                if ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Apply note"))
                    .clicked()
                {
                    apply = true;
                }
                if let Some(id) = d.id
                    && ui.button("Delete note").clicked()
                {
                    delete = Some(id);
                }
                if ui.button("Cancel note").clicked() {
                    close = true;
                }
            });
        if apply {
            self.finish_note_action(Action::Commit);
        }
        if let Some(id) = delete {
            self.plans.text_notes.draft = None;
            let result = self
                .editor
                .command("Delete note", Command::RemoveTextNote(id));
            self.report(result, "Note deleted.");
        }
        if close {
            self.plans.text_notes.draft = None;
        }
    }
    pub(super) fn finish_note_action(&mut self, action: Action) {
        match action {
            Action::Select(id) => {
                self.select(Some(id));
                self.begin_text_note(Some(id));
            }
            Action::Commit => {
                let Some(d) = self.plans.text_notes.draft.take() else {
                    return;
                };
                if !d.current(&self.editor, self.plans.active) {
                    self.status = "Note canceled: context changed".into();
                    return;
                }
                let (id, command) = if let Some(id) = d.id {
                    (
                        id,
                        Command::UpdateTextNote {
                            id,
                            parameters: d.parameters,
                        },
                    )
                } else {
                    let note = TextNote::new("core.text_note", d.parameters);
                    (note.id(), Command::AddTextNote(note))
                };
                let result = self.editor.command("Text note", command);
                if result.is_ok() {
                    self.select(Some(id));
                }
                self.report(result, "Note saved.");
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    editor: &Editor,
    state: &mut State,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    response: &egui::Response,
    ctx: &egui::Context,
    snaps: SnapOptions,
    allow: bool,
) -> Option<Action> {
    let rect = response.rect;
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let point = ctx
        .input(|i| i.pointer.interact_pos())
        .filter(|p| rect.contains(*p))
        .and_then(|p| {
            camera
                .unproject(
                    Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())),
                    size,
                )
                .ok()
        });
    if let Some(d) = state.draft.as_mut().filter(|d| d.placing) {
        if !d.current(editor, Some(context.view_id))
            || d.camera.is_some_and(|c| c != (camera, rect))
        {
            state.draft = None;
            return None;
        }
        if let Some(mut point) = point.filter(|p| point_in_annotation_crop(context, *p)) {
            let query = os_render::snapping::SnapQuery {
                camera,
                viewport: size,
                pointer: camera.project(point, size).ok()?,
                radius_pixels: 12.,
                endpoints: snaps.enabled,
                midpoints: snaps.enabled,
                intersections: false,
                perpendicular_from: None,
                nearest: false,
                axis_extensions: false,
                exclude_entity: d.id,
            };
            if let Ok(result) = drawing.snap(context, query)
                && let Ok(Some(c)) = result.candidate(context, query)
            {
                point = c.point;
            }
            let position = context.basis.plane_to_world(point).ok()?;
            if let Some(tip) = &mut d.parameters.leader {
                tip.x += position.x - d.parameters.position.x;
                tip.y += position.y - d.parameters.position.y;
            }
            d.parameters.position = position;
        }
        if response.contains_pointer() && ctx.input(|i| i.pointer.primary_pressed()) {
            state.claimed = true;
            d.camera = Some((camera, rect));
        }
        if state.claimed && ctx.input(|i| i.pointer.primary_released()) {
            if point.is_some_and(|p| point_in_annotation_crop(context, p)) {
                return Some(Action::Commit);
            }
            state.draft = None;
        }
    } else if allow && state.draft.is_none() {
        if response.contains_pointer()
            && ctx.input(|i| i.pointer.primary_pressed())
            && let Some(point) = point
        {
            state.hit = drawing
                .pick_text_note(context, point, 6. / camera.pixels_per_metre)
                .ok()
                .flatten();
            state.claimed = state.hit.is_some();
        }
        if state.claimed && ctx.input(|i| i.pointer.primary_released()) {
            return state.hit.take().map(Action::Select);
        }
    }
    None
}
pub(super) fn paint(
    painter: &egui::Painter,
    drawing: &PlanDrawing,
    state: &State,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let screen = |p| {
        camera
            .project(p, size)
            .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
    };
    let mut clip = rect;
    if let Some(c) = context.annotation_bounds()
        && let (Ok(a), Ok(b)) = (screen(c.min), screen(c.max))
    {
        clip = clip.intersect(egui::Rect::from_two_pos(a, b));
    }
    let painter = painter.with_clip_rect(clip);
    let mut notes = drawing.text_notes(context).unwrap_or_default().to_vec();
    if context.annotation_crop.is_some() {
        notes.clear();
    }
    if let Some(d) = &state.draft {
        notes.retain(|n| Some(n.entity) != d.id);
        let mut parameters = d.parameters.clone();
        if let Ok(position) = context.basis.world_to_plane(parameters.position) {
            parameters.position = position;
            parameters.leader = parameters
                .leader
                .and_then(|p| context.basis.world_to_plane(p).ok());
            notes.push(NoteGraphic {
                entity: d.id.unwrap_or_default(),
                parameters,
            });
        }
    }
    for note in notes {
        if context.annotation_crop.is_some() {
            if let Ok(drawing) = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
                .and_then(|d| d.with_text_notes(vec![note]))
                && let Ok(graphics) = drawing.native_annotation_graphics(context, camera.center)
            {
                let marks: Vec<_> = graphics.into_iter().flat_map(|g| g.marks).collect();
                let _ = paint_native_annotation_marks(&painter, rect, context, camera, &marks);
            }
            continue;
        }
        let Ok(layout) = note.layout(context.scale_denominator) else {
            continue;
        };
        let color = if state
            .draft
            .as_ref()
            .is_some_and(|d| d.id == Some(note.entity) || d.id.is_none())
        {
            theme::ACCENT
        } else {
            egui::Color32::BLACK
        };
        let height =
            (note.parameters.text_height_mm * context.scale_denominator * camera.pixels_per_metre
                / 1000.) as f32;
        for (anchor, ch) in layout.glyphs {
            if let Ok(p) = screen(anchor) {
                painter.text(
                    p,
                    egui::Align2::LEFT_BOTTOM,
                    ch,
                    egui::FontId::proportional(height.max(1.)),
                    color,
                );
            }
        }
        for (a, b) in layout.leader {
            if let (Ok(a), Ok(b)) = (screen(a), screen(b)) {
                painter.line_segment([a, b], egui::Stroke::new(1., color));
            }
        }
    }
}

#[cfg(test)]
#[path = "text_note_tests.rs"]
mod tests;
