//! Disposable world-baseline authoring; no wall-local elevation semantics.
use super::*;
use os_model::{ElevationSide, FacadeViewSettings};

pub(super) struct Draft {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    level: Id,
    editing: Option<Id>,
    name: String,
    first: Option<Point2>,
    settings: Option<FacadeViewSettings>,
}
impl Draft {
    fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active == Some(self.context.view_id)
            && app.plans.active_sheet.is_none()
            && app.editor.native_view_context(self.context.view_id).ok() == Some(self.context)
            && plan_provider_signature(&app.editor) == self.providers
    }
}
impl DesktopApp {
    pub(super) fn active_facade(&self) -> Option<(Id, FacadeViewSettings)> {
        let id = self.plans.active?;
        self.editor
            .document
            .model()
            .views
            .get(&id)?
            .parameters
            .facade
            .map(|s| (id, s))
    }
    pub(super) fn validate_facade(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.plans.facade_pointer_claimed = false;
        }
        let cancel = ctx.input(|i| {
            i.key_pressed(egui::Key::Escape)
                || i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::PointerGone | egui::Event::WindowFocused(false)
                    )
                })
        });
        if self
            .plans
            .facade
            .as_ref()
            .is_some_and(|d| cancel || !d.current(self))
        {
            self.plans.facade = None;
            self.plans.facade_pointer_claimed = ctx.input(|i| i.pointer.primary_down());
        }
    }
    pub(super) fn begin_facade(&mut self) {
        let result = (|| {
            let id = self
                .plans
                .active
                .ok_or_else(|| Error::Invalid("Open a floor plan".into()))?;
            let context = self.editor.native_plan_context(id)?;
            let level = self.editor.document.model().views[&id]
                .parameters
                .level
                .unwrap();
            Ok(Draft {
                context,
                providers: plan_provider_signature(&self.editor),
                level,
                editing: None,
                name: "Facade".into(),
                first: None,
                settings: None,
            })
        })();
        match result {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_aligned_dimension();
                self.cancel_opening_placement();
                self.plans.section_placement = None;
                self.plans.facade = Some(draft);
            }
            Err(e) => self.report(Err(e), ""),
        }
    }
    fn apply_facade(&mut self, draft: &Draft) -> Result<Id> {
        os_core::ensure(draft.current(self), "facade draft is stale")?;
        let settings = draft
            .settings
            .ok_or_else(|| Error::Invalid("Choose two baseline points".into()))?;
        settings.validate()?;
        os_geometry::facade::FacadeSnapshot::capture(
            self.editor.document.model(),
            draft.level,
            settings,
        )?;
        if let Some(id) = draft.editing {
            let mut parameters = self.editor.document.model().views[&id].parameters.clone();
            parameters.name = draft.name.clone();
            parameters.facade = Some(settings);
            self.editor.command(
                "Update facade settings",
                Command::UpdateView { id, parameters },
            )?;
            Ok(id)
        } else {
            self.editor
                .create_facade_view(&draft.name, draft.level, settings)
        }
    }
    pub(super) fn facade_toolbar(&mut self, ui: &mut egui::Ui) {
        if self.plans.facade.is_none()
            && let Some((id, s)) = self.active_facade()
            && ui.button("Facade settings").clicked()
            && let Ok(context) = self.editor.native_facade_context(id)
        {
            let p = &self.editor.document.model().views[&id].parameters;
            self.plans.facade = Some(Draft {
                context,
                providers: plan_provider_signature(&self.editor),
                level: p.level.unwrap(),
                editing: Some(id),
                name: p.name.clone(),
                first: Some(s.origin),
                settings: Some(s),
            });
        }
        let Some(mut draft) = self.plans.facade.take() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.label("World-oriented facade");
            ui.add(
                egui::TextEdit::singleline(&mut draft.name)
                    .desired_width(120.)
                    .char_limit(256),
            );
            if draft.settings.is_none() {
                ui.label(
                    "Click baseline start and end in plan; then review view side, crop and depth.",
                );
            }
            if let Some(s) = &mut draft.settings {
                ui.selectable_value(&mut s.side, ElevationSide::Left, "View from left");
                ui.selectable_value(&mut s.side, ElevationSide::Right, "View from right");
                apply = ui.button("Apply facade").clicked();
            }
            cancel = ui.button("Cancel facade").clicked();
        });
        if let Some(s) = &mut draft.settings {
            ui.horizontal_wrapped(|ui| {
                for (label, value) in [
                    ("Crop start", &mut s.min_station),
                    ("Crop end", &mut s.max_station),
                    ("Bottom", &mut s.bottom_elevation),
                    ("Top", &mut s.top_elevation),
                    ("Near", &mut s.near_depth),
                    ("Far", &mut s.far_depth),
                ] {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.1).max_decimals(3));
                }
            });
            if let Err(e) = s.validate() {
                ui.colored_label(theme::ERROR, e.to_string());
            }
        }
        if apply {
            match self.apply_facade(&draft) {
                Ok(id) => {
                    self.focus_plan(Some(id));
                    cancel = true;
                    self.report(
                        Ok(()),
                        "Facade and linked marker applied in one transaction.",
                    );
                }
                Err(e) => self.report(Err(e), ""),
            }
        }
        if !cancel {
            self.plans.facade = Some(draft);
        }
    }
    pub(super) fn facade_plan_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(mut draft) = self.plans.facade.take() else {
            return;
        };
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        #[cfg(test)]
        {
            self.plans.canvas_rect = Some(rect);
        }
        let camera = self
            .plans
            .cameras
            .get(&draft.context.view_id)
            .copied()
            .unwrap_or_default();
        let screen = |p: Point2| {
            camera
                .project(p, size)
                .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
        };
        painter.rect_filled(rect, 0., theme::CANVAS);
        if let Some(d) = &self.plans.drawing
            && let Ok(items) = d.items(draft.context)
        {
            for item in items {
                for (a, b) in item.outline() {
                    if let (Ok(a), Ok(b)) = (screen(a), screen(b)) {
                        painter.line_segment([a, b], egui::Stroke::new(1., theme::MUTED));
                    }
                }
            }
            for line in d.provider_lines(draft.context).unwrap_or_default() {
                if let (Ok(a), Ok(b)) = (screen(line.start), screen(line.end)) {
                    painter.line_segment([a, b], egui::Stroke::new(1., theme::MUTED));
                }
            }
        }
        let pointer = response
            .hover_pos()
            .and_then(|p| {
                camera
                    .unproject(
                        Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())),
                        size,
                    )
                    .ok()
            })
            .and_then(|p| draft.context.basis.plane_to_world(p).ok());
        if response.clicked()
            && draft.settings.is_none()
            && let Some(p) = pointer
        {
            if let Some(first) = draft.first {
                let z = self.editor.document.model().levels[&draft.level]
                    .parameters
                    .elevation;
                match FacadeViewSettings::new(first, p, z - 0.5, z + 10.) {
                    Ok(s) => draft.settings = Some(s),
                    Err(e) => self.report(Err(e), ""),
                }
            } else {
                draft.first = Some(p);
            }
        }
        let segment = |a, b| {
            if let (Ok(a), Ok(b)) = (
                draft.context.basis.world_to_plane(a).and_then(screen),
                draft.context.basis.world_to_plane(b).and_then(screen),
            ) {
                painter.line_segment([a, b], egui::Stroke::new(2., theme::ACCENT));
            }
        };
        if let Some(s) = draft.settings {
            if s.validate().is_ok() {
                let corners = [
                    s.world_point(s.min_station, s.near_depth),
                    s.world_point(s.max_station, s.near_depth),
                    s.world_point(s.max_station, s.far_depth),
                    s.world_point(s.min_station, s.far_depth),
                ];
                for i in 0..4 {
                    segment(corners[i], corners[(i + 1) % 4]);
                }
                segment(s.origin, s.marker_end());
                segment(
                    s.world_point(s.marker_length * 0.5, 0.),
                    s.world_point(s.marker_length * 0.5, 1.),
                );
            }
        } else if let Some(first) = draft.first
            && let Some(p) = pointer
        {
            segment(first, p);
        }
        if ctx.input(|i| i.pointer.primary_down()) {
            self.plans.facade_pointer_claimed = true;
        }
        self.plans.facade = Some(draft);
    }
    pub(super) fn paint_facade_crop(
        &self,
        painter: &egui::Painter,
        camera: PlanCamera,
        rect: egui::Rect,
        size: [f64; 2],
    ) {
        if let Some(s) = self.plans.facade.as_ref().and_then(|d| d.settings)
            && s.validate().is_ok()
        {
            let a = camera.project(Point2::new(s.min_station, s.top_elevation), size);
            let b = camera.project(Point2::new(s.max_station, s.bottom_elevation), size);
            if let (Ok(a), Ok(b)) = (a, b) {
                painter.rect_stroke(
                    egui::Rect::from_min_max(
                        rect.min + egui::vec2(a.x as f32, a.y as f32),
                        rect.min + egui::vec2(b.x as f32, b.y as f32),
                    ),
                    0.,
                    egui::Stroke::new(2., theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests;
