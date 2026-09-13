//! Native plan workspace. One native worker plus one optional provider composition
//! worker; obsolete work drains before replacement in each lane.
use super::*;
use os_render::plan::{PlanCamera, PlanContext, PlanDrawing};
use std::{collections::BTreeMap, thread::JoinHandle};
#[cfg(feature = "external-plugins")]
mod providers;

/// Session preferences; never stored as model geometry or history.
struct SnapOptions {
    enabled: bool,
    endpoints: bool,
    intersections: bool,
    perpendicular: bool,
    midpoints: bool,
    nearest: bool,
    axis_extensions: bool,
}
impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoints: true,
            intersections: true,
            perpendicular: true,
            midpoints: true,
            nearest: true,
            axis_extensions: false,
        }
    }
}

#[derive(Default)]
pub(super) struct PlanWorkspace {
    #[cfg(feature = "external-plugins")]
    providers: providers::Providers,
    #[cfg(test)]
    pub(super) canvas_rect: Option<egui::Rect>,
    pub active: Option<Id>,
    pub split: bool,
    session: Option<Id>,
    cameras: BTreeMap<Id, PlanCamera>,
    desired: Option<PlanContext>,
    pending: Option<(JoinHandle<Result<PlanDrawing>>, bool)>,
    drawing: Option<PlanDrawing>,
    error: Option<String>,
    attempted: bool,
    snaps: SnapOptions,
}
impl PlanWorkspace {
    #[cfg(test)]
    pub(super) fn ready(&self) -> bool {
        self.drawing.is_some() && self.pending.is_none()
    }
    fn poll(&mut self, editor: &Editor) {
        let session = editor.document.session_id();
        if self.session != Some(session) {
            self.session = Some(session);
            self.active = None;
            self.cameras.clear();
        }
        if self
            .active
            .is_some_and(|id| !editor.document.model().views.contains_key(&id))
        {
            self.active = None;
        }
        let requested = self.active.map(|id| editor.native_plan_context(id));
        let current = requested.as_ref().and_then(|r| r.as_ref().ok()).copied();
        if self.desired != current {
            self.desired = current;
            self.drawing = None;
            self.error = None;
            self.attempted = false;
            if let Some((_, valid)) = &mut self.pending {
                *valid = false;
            }
        }
        if let Some(Err(error)) = requested {
            self.error = Some(error.to_string());
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(job, _)| job.is_finished())
        {
            let (job, valid) = self.pending.take().expect("finished job");
            let result = job
                .join()
                .unwrap_or_else(|_| Err(Error::Invalid("plan worker failed".into())));
            if valid {
                match result {
                    Ok(drawing) => self.drawing = Some(drawing),
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        if self.pending.is_none()
            && !self.attempted
            && let Some(context) = self.desired
        {
            self.attempted = true;
            match editor.plan_snapshot(context.view_id).and_then(|snapshot| {
                std::thread::Builder::new()
                    .name("native-plan".into())
                    .spawn(move || snapshot.derive())
                    .map_err(|e| Error::Invalid(format!("cannot start plan worker: {e}")))
            }) {
                Ok(job) => self.pending = Some((job, true)),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
}

impl DesktopApp {
    fn begin_plan_wall(&mut self, view: Id) {
        #[cfg(feature = "external-plugins")]
        if self.editor.plugin_work_pending() {
            self.report(
                Err(Error::Invalid(
                    "Finish or cancel pending plugin work before starting a wall".into(),
                )),
                "",
            );
            return;
        }
        #[cfg(feature = "external-plugins")]
        let installed = self
            .editor
            .host
            .worker_supported(os_plugin_api::wall::OWNER);
        #[cfg(feature = "external-plugins")]
        let result = if installed {
            crate::plan_gesture::WallGesture::begin_installed(
                &self.editor,
                view,
                self.draft.clone(),
            )
        } else {
            crate::plan_gesture::WallGesture::begin(&self.editor, view, self.draft.clone())
        };
        #[cfg(not(feature = "external-plugins"))]
        let result =
            crate::plan_gesture::WallGesture::begin(&self.editor, view, self.draft.clone());
        match result {
            Ok(gesture) => self.wall_gesture = Some(gesture),
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn begin_plan_wall_edit(&mut self, view: Id, id: Id, mode: crate::plan_gesture::WallEdit) {
        #[cfg(feature = "external-plugins")]
        if self.editor.plugin_work_pending() {
            self.report(
                Err(Error::Invalid(
                    "Finish or cancel pending plugin work before editing a wall".into(),
                )),
                "",
            );
            return;
        }
        #[cfg(feature = "external-plugins")]
        let result = if self
            .editor
            .host
            .worker_supported(os_plugin_api::wall::OWNER)
        {
            crate::plan_gesture::WallGesture::begin_edit_installed(&self.editor, view, id, mode)
        } else {
            crate::plan_gesture::WallGesture::begin_edit(&self.editor, view, id, mode)
        };
        #[cfg(not(feature = "external-plugins"))]
        let result = crate::plan_gesture::WallGesture::begin_edit(&self.editor, view, id, mode);
        match result {
            Ok(gesture) => self.wall_gesture = Some(gesture),
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn commit_plan_wall(&mut self, point: Point2) {
        let Some(gesture) = &self.wall_gesture else {
            return;
        };
        #[cfg(feature = "external-plugins")]
        if gesture.is_installed() {
            let prepared = gesture.installed_command(&self.editor, self.plans.active, point);
            let result = prepared.and_then(|(draft, context, view)| {
                self.submit_installed_wall(draft, context, view)
            });
            self.report(
                result,
                "Wall command pending… Escape or Cancel plugin command revokes it.",
            );
            return;
        }
        let result = gesture.commit(&mut self.editor, self.plans.active, point);
        if result.is_ok() {
            self.wall_gesture = None;
            self.select(self.selected);
        }
        self.report(result, "Wall gesture applied.");
    }
    pub(super) fn focus_plan(&mut self, id: Option<Id>) {
        if self.plans.active != id {
            self.wall_gesture = None;
        }
        self.plans.active = id;
        if let Some(level) = id
            .and_then(|id| self.editor.document.model().views.get(&id))
            .and_then(|v| v.parameters.level)
        {
            self.set_active_level(level);
        }
    }
    pub(super) fn plan_workspace(&mut self, ctx: &egui::Context) {
        self.plans.poll(&self.editor);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::CANVAS))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.plans.active.is_none(), "3D")
                        .clicked()
                    {
                        self.focus_plan(None);
                    }
                    let plans: Vec<_> = self
                        .editor
                        .document
                        .model()
                        .views
                        .values()
                        .filter(|v| v.parameters.kind == os_model::ViewKind::Plan)
                        .map(|v| (v.id(), v.parameters.name.clone()))
                        .collect();
                    egui::ComboBox::from_id_salt("named_plan_picker")
                        .selected_text(
                            plans
                                .iter()
                                .find(|(id, _)| Some(*id) == self.plans.active)
                                .map_or("Choose plan", |(_, name)| name.as_str()),
                        )
                        .show_ui(ui, |ui| {
                            for (id, name) in plans {
                                if ui
                                    .selectable_label(self.plans.active == Some(id), name)
                                    .clicked()
                                {
                                    self.focus_plan(Some(id));
                                }
                            }
                        });
                    if ui
                        .button("New floor plan")
                        .on_hover_text("Create a named plan on the active level.")
                        .clicked()
                    {
                        let name =
                            format!("Floor plan {}", self.editor.document.model().views.len());
                        match self.editor.create_floor_plan(&name, self.active_level) {
                            Ok(id) => {
                                self.focus_plan(Some(id));
                                self.report(Ok(()), "Floor plan created.");
                            }
                            Err(error) => self.report(Err(error), ""),
                        }
                    }
                    ui.checkbox(&mut self.plans.split, "Split 2D / 3D");
                    if ui
                        .add_enabled(
                            self.plans.active.is_some(),
                            egui::Button::new("Plan settings"),
                        )
                        .clicked()
                        && let Some(id) = self.plans.active
                    {
                        match crate::plan_settings::PlanDraft::begin(&self.editor, id) {
                            Ok(draft) => self.plan_draft = Some(draft),
                            Err(error) => self.report(Err(error), ""),
                        }
                    }
                });
                // UI view changes invalidate work before either drawing or picking.
                if self.plan_draft.is_some() { self.wall_gesture = None; }
                ui.horizontal_wrapped(|ui| {
                    ui.menu_button(if self.plans.snaps.enabled { "Snaps" } else { "Snaps off" }, |ui| {
                        let snaps = &mut self.plans.snaps;
                        ui.checkbox(&mut snaps.enabled, "Enable snapping");
                        ui.separator();
                        ui.checkbox(&mut snaps.endpoints, "Endpoints");
                        ui.checkbox(&mut snaps.intersections, "Intersections");
                        ui.checkbox(&mut snaps.perpendicular, "Perpendicular from anchor");
                        ui.checkbox(&mut snaps.midpoints, "Midpoints");
                        ui.checkbox(&mut snaps.nearest, "Nearest lines and grid axes");
                        ui.checkbox(&mut snaps.axis_extensions, "Line axis extensions");
                        ui.label("Priority: endpoint, intersection, perpendicular, midpoint, grid axis, nearest.");
                        ui.label("Axis extensions are last priority; they do not extend model geometry.");
                        ui.label("Exact input overrides snapping. Session preferences only.");
                    });
                    if self.wall_gesture.is_none() {
                        if ui.add_enabled(self.plans.active.is_some(),egui::Button::new("New grid")).clicked() {
                            self.begin_grid_form(None);
                        }
                        if ui.add_enabled(self.plans.active.is_some(),egui::Button::new("Draw wall in plan")).on_hover_text("Two clicks. Uses Properties height/thickness. Installed Wall gestures submit bounded commands; edit the source in its level plan.").clicked()
                            && let Some(view)=self.plans.active {
                            self.begin_plan_wall(view);
                        }
                        use crate::plan_gesture::WallEdit;
                        if let (Some(view),Some(id))=(self.plans.active,self.selected)
                            && self.editor.document.model().walls.contains_key(&id) {
                            for (label,mode) in [("Move wall",WallEdit::Move),("Resize start",WallEdit::ResizeStart),("Resize end",WallEdit::ResizeEnd),("Offset wall",WallEdit::OffsetCopy)] {
                                if ui.button(label).clicked() {
                                    self.begin_plan_wall_edit(view,id,mode);
                                }
                            }
                        }
                    } else {
                        let gesture=self.wall_gesture.as_mut().unwrap();
                        ui.label(gesture.prompt());
                        let offset = gesture.edit_mode()==Some(crate::plan_gesture::WallEdit::OffsetCopy);
                        ui.label(if offset {"Offset (m)"} else if gesture.edit_mode()==Some(crate::plan_gesture::WallEdit::Move) {"Distance (m)"} else {"Length (m)"}); ui.add(egui::TextEdit::singleline(&mut gesture.length).desired_width(70.0).char_limit(64));
                        if !offset { ui.label("Angle (deg)"); ui.add(egui::TextEdit::singleline(&mut gesture.angle_degrees).desired_width(70.0).char_limit(64)); }
                        if ui.button("Cancel wall").clicked() {self.wall_gesture=None;}
                    }
                });
                if self.wall_gesture.as_ref().is_some_and(|g| !g.current(&self.editor,self.plans.active)) { self.wall_gesture=None; }
                self.plans.poll(&self.editor);
                #[cfg(feature = "external-plugins")]
                self.plans.providers.poll(&mut self.editor,self.plans.active);
                if self.plans.active.is_some() {
                    if self.plans.split {
                        ui.columns(2, |columns| {
                            self.plan_canvas(&mut columns[0], ctx);
                            self.model_view(&mut columns[1], ctx);
                        });
                    } else {
                        self.plan_canvas(ui, ctx);
                    }
                } else {
                    self.model_view(ui, ctx);
                }
            });
        if self.plans.pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        #[cfg(feature = "external-plugins")]
        if self.plans.providers.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn plan_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(id) = self.plans.active else {
            return;
        };
        let camera = self.plans.cameras.entry(id).or_default();
        let fit = ui
            .horizontal(|ui| {
                let fit = ui.button("Fit plan").clicked();
                ui.label("Drag to pan · Scroll to zoom · Click to select");
                fit
            })
            .inner;
        if let Some(error) = &self.plans.error {
            ui.colored_label(theme::ERROR, error);
        }
        let Some(context) = self.plans.desired else {
            return;
        };
        #[cfg(feature = "external-plugins")]
        {
            if self.plans.providers.busy() {
                ui.label("Generating plugin plan graphics…");
            }
            if let Some(error) = &self.plans.providers.error {
                ui.colored_label(theme::ERROR, error);
            }
            let diagnostics = &self.plans.providers.diagnostics;
            if !diagnostics.is_empty() {
                ui.collapsing(
                    format!("Plan provider diagnostics ({})", diagnostics.len()),
                    |ui| {
                        egui::ScrollArea::vertical().max_height(100.0).show_rows(
                            ui,
                            18.0,
                            diagnostics.len(),
                            |ui, range| {
                                for (id, error) in
                                    diagnostics.iter().skip(range.start).take(range.len())
                                {
                                    ui.label(format!("{id}: {error}"));
                                }
                            },
                        );
                    },
                );
            }
        }
        let Some(drawing) = &self.plans.drawing else {
            if self.plans.error.is_none() {
                ui.label("Generating plan…");
            }
            return;
        };
        #[cfg(feature = "external-plugins")]
        let drawing = self
            .plans
            .providers
            .drawing(&self.editor, id)
            .unwrap_or(drawing);
        let Ok(items) = drawing.items(context) else {
            return;
        };
        let Ok(grids) = drawing.grids(context) else {
            return;
        };
        let Ok(provider_lines) = drawing.provider_lines(context) else {
            return;
        };
        let unavailable = drawing.unavailable(context).map_or(0, |ids| ids.len());
        if unavailable > 0 {
            ui.colored_label(
                theme::ERROR,
                format!("Incomplete plan: {unavailable} plugin elements unavailable."),
            );
        }
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        #[cfg(test)]
        {
            self.plans.canvas_rect = Some(rect);
        }
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        if fit {
            let points: Vec<_> = items
                .iter()
                .flat_map(|item| item.footprint.vertices().iter().copied())
                .chain(grids.iter().flat_map(|grid| [grid.start, grid.end]))
                .chain(
                    provider_lines
                        .iter()
                        .flat_map(|line| [line.start, line.end]),
                )
                .collect();
            if !points.is_empty() {
                let min = points
                    .iter()
                    .fold(Point2::new(f64::INFINITY, f64::INFINITY), |p, q| {
                        Point2::new(p.x.min(q.x), p.y.min(q.y))
                    });
                let max = points
                    .iter()
                    .fold(Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |p, q| {
                        Point2::new(p.x.max(q.x), p.y.max(q.y))
                    });
                let candidate = PlanCamera {
                    center: Point2::new(
                        min.x + (max.x - min.x) * 0.5,
                        min.y + (max.y - min.y) * 0.5,
                    ),
                    pixels_per_metre: (0.85
                        * (size[0] / (max.x - min.x).max(0.001))
                            .min(size[1] / (max.y - min.y).max(0.001)))
                    .clamp(0.001, 10000.0),
                };
                if candidate.project(candidate.center, size).is_ok() {
                    *camera = candidate;
                }
            } else {
                *camera = PlanCamera::default();
            }
        }
        if response.dragged() {
            let delta = ctx.input(|i| i.pointer.delta());
            let _ = camera.pan(Point2::new(f64::from(delta.x), f64::from(delta.y)), size);
        }
        if response.hovered()
            && let Some(pointer) = response.hover_pos()
        {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            let _ = camera.zoom_at(
                Point2::new(
                    f64::from(pointer.x - rect.left()),
                    f64::from(pointer.y - rect.top()),
                ),
                (f64::from(scroll) * 0.002).exp(),
                size,
            );
        }
        painter.rect_filled(rect, 0.0, theme::CANVAS);
        // Architectural datum lines are a background layer, not thin wall solids.
        for grid in grids {
            let screen = |p: Point2| -> Result<egui::Pos2> {
                let p = camera.project(p, size)?;
                os_core::ensure(
                    p.x.abs() < 1e8 && p.y.abs() < 1e8,
                    "grid exceeds screen range",
                )?;
                Ok(egui::pos2(
                    rect.left() + p.x as f32,
                    rect.top() + p.y as f32,
                ))
            };
            let (Ok(a), Ok(b)) = (screen(grid.start), screen(grid.end)) else {
                ui.colored_label(
                    theme::ERROR,
                    "Grid cannot be displayed at this navigation scale.",
                );
                return;
            };
            let color = if self.selected == Some(grid.entity) {
                theme::ACCENT
            } else {
                theme::MUTED
            };
            painter.line_segment([a, b], egui::Stroke::new(1.0_f32, color));
            painter.text(
                a + egui::vec2(4.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                &grid.name,
                egui::FontId::proportional(12.0),
                color,
            );
        }
        for line in provider_lines {
            let screen = |point| -> Result<egui::Pos2> {
                let p = camera.project(point, size)?;
                os_core::ensure(
                    p.x.abs() < 1e8 && p.y.abs() < 1e8,
                    "provider line exceeds screen range",
                )?;
                Ok(egui::pos2(
                    rect.left() + p.x as f32,
                    rect.top() + p.y as f32,
                ))
            };
            let (Ok(a), Ok(b)) = (screen(line.start), screen(line.end)) else {
                ui.colored_label(
                    theme::ERROR,
                    "Provider graphics cannot be displayed at this navigation scale.",
                );
                return;
            };
            let color = if self.selected == Some(line.entity) {
                theme::ACCENT
            } else {
                theme::TEXT
            };
            let width = if line.role == os_geometry::plan::PlanRole::Cut {
                2.0_f32
            } else {
                1.0_f32
            };
            painter.line_segment([a, b], egui::Stroke::new(width, color));
        }
        for item in items {
            let points: Result<Vec<_>> = item
                .footprint
                .vertices()
                .iter()
                .map(|p| {
                    camera.project(*p, size).and_then(|p| {
                        os_core::ensure(
                            p.x.abs() < f64::from(f32::MAX) / 2.0
                                && p.y.abs() < f64::from(f32::MAX) / 2.0,
                            "plan exceeds screen coordinate range",
                        )?;
                        Ok(egui::pos2(
                            rect.left() + p.x as f32,
                            rect.top() + p.y as f32,
                        ))
                    })
                })
                .collect();
            let Ok(points) = points else {
                ui.colored_label(
                    theme::ERROR,
                    "Plan cannot be displayed at this navigation scale.",
                );
                return;
            };
            let selected = self.selected == Some(item.entity);
            let fill = if selected {
                theme::SELECTED
            } else {
                theme::SURFACE
            };
            let color = if selected { theme::ACCENT } else { theme::TEXT };
            let width = if item.footprint.role == os_geometry::plan::PlanRole::Cut {
                2.0_f32
            } else {
                1.0_f32
            };
            painter.add(egui::Shape::convex_polygon(
                points,
                fill,
                egui::Stroke::new(width, color),
            ));
        }
        if items.is_empty() && grids.is_empty() && provider_lines.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No visible plan geometry with these settings",
                egui::FontId::proportional(13.0),
                theme::MUTED,
            );
        }
        if self.wall_gesture.is_some()
            && response.hovered()
            && let Some(pointer) = response.hover_pos()
        {
            let screen = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            let query = os_render::snapping::SnapQuery {
                camera: *camera,
                viewport: size,
                pointer: screen,
                radius_pixels: 12.0,
                endpoints: self.plans.snaps.enabled && self.plans.snaps.endpoints,
                midpoints: self.plans.snaps.enabled && self.plans.snaps.midpoints,
                intersections: self.plans.snaps.enabled && self.plans.snaps.intersections,
                perpendicular_from: if self.plans.snaps.enabled && self.plans.snaps.perpendicular {
                    self.wall_gesture.as_ref().and_then(|g| g.start)
                } else {
                    None
                },
                nearest: self.plans.snaps.enabled && self.plans.snaps.nearest,
                axis_extensions: self.plans.snaps.enabled && self.plans.snaps.axis_extensions,
                exclude_entity: self.wall_gesture.as_ref().and_then(|g| g.snap_exclusion()),
            };
            let acquired = if self
                .wall_gesture
                .as_ref()
                .is_some_and(|g| g.has_exact_destination())
            {
                Ok(None)
            } else {
                drawing
                    .snap(context, query)
                    .and_then(|result| result.candidate(context, query))
            };
            let target = acquired.and_then(|candidate| {
                let point = if let Some(hit) = candidate {
                    hit.point
                } else {
                    camera.unproject(screen, size)?
                };
                Ok((point, candidate))
            });
            match target {
                Ok((point, candidate)) => {
                    let project = |point: Point2| -> Result<egui::Pos2> {
                        let p = camera.project(point, size)?;
                        os_core::ensure(
                            p.x.abs() < 1e8 && p.y.abs() < 1e8,
                            "Preview exceeds screen range",
                        )?;
                        Ok(egui::pos2(
                            rect.left() + p.x as f32,
                            rect.top() + p.y as f32,
                        ))
                    };
                    let exact = self.wall_gesture.as_ref().is_some_and(|g| {
                        g.start.is_some()
                            && (!g.length.trim().is_empty() || !g.angle_degrees.trim().is_empty())
                    });
                    if !exact
                        && let Some(hit) = candidate
                        && let Ok(pos) = project(hit.point)
                    {
                        painter.circle_stroke(pos, 5.0, egui::Stroke::new(1.5_f32, theme::ACCENT));
                        painter.text(
                            pos + egui::vec2(9.0, -9.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("{:?}", hit.kind),
                            egui::FontId::proportional(12.0),
                            theme::TEXT,
                        );
                    }
                    let gesture = self.wall_gesture.as_mut().unwrap();
                    let preview = if gesture.start.is_some() {
                        Some(gesture.parameters(point))
                    } else {
                        None
                    };
                    if let Some(Ok(parameters)) = &preview {
                        if let (Ok(a), Ok(b)) = (
                            context
                                .basis
                                .world_to_plane(parameters.start)
                                .and_then(project),
                            context
                                .basis
                                .world_to_plane(parameters.end)
                                .and_then(project),
                        ) {
                            painter.line_segment([a, b], egui::Stroke::new(2.0_f32, theme::ACCENT));
                            if let Some(distance) = gesture.offset_measurement(parameters) {
                                painter.text(
                                    a + egui::vec2(9.0, -9.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    format!("Offset {distance:+.3} m"),
                                    egui::FontId::proportional(12.0),
                                    theme::TEXT,
                                );
                            }
                            if exact {
                                painter.text(
                                    (if gesture.edit_mode()
                                        == Some(crate::plan_gesture::WallEdit::ResizeStart)
                                    {
                                        a
                                    } else {
                                        b
                                    }) + egui::vec2(9.0, -9.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    "Exact input",
                                    egui::FontId::proportional(12.0),
                                    theme::TEXT,
                                );
                            }
                        }
                    } else if let Some(Err(error)) = &preview {
                        let waiting = gesture.length.trim().is_empty()
                            && gesture.angle_degrees.trim().is_empty()
                            && gesture
                                .start
                                .is_some_and(|start| start.distance(point) <= 1e-9);
                        painter.text(
                            rect.left_top() + egui::vec2(8.0, 8.0),
                            egui::Align2::LEFT_TOP,
                            if waiting {
                                "Choose the other endpoint or enter exact dimensions".into()
                            } else {
                                error.to_string()
                            },
                            egui::FontId::proportional(12.0),
                            if waiting { theme::MUTED } else { theme::ERROR },
                        );
                    }
                    if response.clicked() {
                        if gesture.start.is_none() {
                            gesture.start = Some(point);
                        } else {
                            self.commit_plan_wall(point);
                        }
                        ctx.request_repaint();
                    }
                }
                Err(error) => {
                    painter.text(
                        rect.left_top() + egui::vec2(8.0, 8.0),
                        egui::Align2::LEFT_TOP,
                        error.to_string(),
                        egui::FontId::proportional(12.0),
                        theme::ERROR,
                    );
                }
            }
        } else if self.wall_gesture.is_none()
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let point = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            if let Ok(selected) = drawing.pick_screen(context, *camera, size, point, 6.0) {
                self.select(selected);
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    #[test]
    fn provider_only_canvas_renders_lines_without_empty_message() {
        let mut app = DesktopApp::new().unwrap();
        let level = *app.editor.document.model().levels.keys().next().unwrap();
        let view = app.editor.create_floor_plan("Plan", level).unwrap();
        let context = app.editor.native_plan_context(view).unwrap();
        let entity = Id::new();
        let line = os_render::plan::PlanLine {
            entity,
            feature: 1,
            start: Point2::new(-1.0, 0.0),
            end: Point2::new(1.0, 0.0),
            role: os_geometry::plan::PlanRole::Cut,
        };
        app.plans.active = Some(view);
        app.plans.desired = Some(context);
        app.plans.drawing = Some(
            PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![entity])
                .unwrap()
                .with_provider_lines([(entity, vec![line])].into())
                .unwrap(),
        );
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.plan_canvas(ui, ctx));
        });
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape, egui::Shape::LineSegment { stroke, .. }
            if stroke.width == 2.0 && stroke.color == theme::TEXT
        )));
        assert!(!output.shapes.iter().any(|shape| matches!(
            &shape.shape, egui::Shape::Text(text)
            if text.galley.job.text.starts_with("No visible")
        )));
        app.plans.drawing =
            Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap());
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.plan_canvas(ui, ctx));
        });
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape, egui::Shape::Text(text)
            if text.galley.job.text == "No visible plan geometry with these settings"
        )));
    }

    fn settle(workspace: &mut PlanWorkspace, editor: &Editor) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while workspace.pending.is_some() {
            assert!(Instant::now() < deadline, "plan worker did not settle");
            std::thread::yield_now();
            workspace.poll(editor);
        }
    }

    #[test]
    fn retired_view_work_drains_and_cannot_publish_when_view_returns() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        let context = editor.native_plan_context(view).unwrap();
        let mut workspace = PlanWorkspace {
            session: Some(editor.document.session_id()),
            active: Some(view),
            desired: Some(context),
            attempted: true,
            ..Default::default()
        };
        let (tx, rx) = mpsc::channel();
        let snapshot = editor.plan_snapshot(view).unwrap();
        workspace.pending = Some((
            std::thread::spawn(move || {
                rx.recv().unwrap();
                snapshot.derive()
            }),
            true,
        ));
        workspace.active = None;
        workspace.poll(&editor);
        assert!(!workspace.pending.as_ref().unwrap().1);
        workspace.active = Some(view);
        workspace.poll(&editor);
        assert!(
            !workspace.pending.as_ref().unwrap().1,
            "reactivation revived retired work"
        );
        assert!(workspace.drawing.is_none());
        tx.send(()).unwrap();
        settle(&mut workspace, &editor);
        assert!(workspace.drawing.as_ref().unwrap().items(context).is_ok());
        editor
            .command("Rename", Command::RenameProject("Changed".into()))
            .unwrap();
        workspace.poll(&editor);
        assert!(workspace.drawing.is_none());
        settle(&mut workspace, &editor);
        assert!(workspace.drawing.as_ref().unwrap().items(context).is_err());
        editor.document = Document::new("Replacement").unwrap();
        workspace.poll(&editor);
        assert!(workspace.active.is_none());
        assert!(workspace.drawing.is_none());
        assert!(workspace.cameras.is_empty());
    }

    #[test]
    fn failed_worker_is_visible_and_not_retried_every_frame() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        let context = editor.native_plan_context(view).unwrap();
        let mut workspace = PlanWorkspace {
            session: Some(editor.document.session_id()),
            active: Some(view),
            desired: Some(context),
            attempted: true,
            ..Default::default()
        };
        workspace.pending = Some((
            std::thread::spawn(|| Err(Error::Invalid("test failure".into()))),
            true,
        ));
        settle(&mut workspace, &editor);
        for _ in 0..5 {
            workspace.poll(&editor);
        }
        assert!(workspace.pending.is_none());
        assert!(workspace.drawing.is_none());
        assert!(workspace.error.as_ref().unwrap().contains("test failure"));
    }
}
