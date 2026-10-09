//! Single-host elevation navigation and transient opening placement.
use super::*;
use crate::opening_tools::OpeningDraft;
use os_model::{ElevationSide, ElevationViewSettings};

#[derive(Default)]
pub(super) struct State {
    pub(super) placement: Option<OpeningDraft>,
    pub(super) dimension: Option<super::elevation_dimensions::Draft>,
    crop: Option<(PlanContext, ElevationViewSettings)>,
}

#[cfg(test)]
#[path = "elevation_tests.rs"]
mod tests;

impl DesktopApp {
    pub(super) fn active_elevation(&self) -> Option<(Id, ElevationViewSettings)> {
        let id = self.plans.active?;
        self.editor
            .document
            .model()
            .views
            .get(&id)?
            .parameters
            .elevation
            .map(|s| (id, s))
    }

    pub(super) fn validate_elevation_interaction(&mut self, ctx: &egui::Context) {
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if escape
            || self
                .plans
                .elevation
                .dimension
                .as_ref()
                .is_some_and(|d| !d.current(self))
        {
            self.plans.elevation.dimension = None;
        }
        let stale = self
            .plans
            .elevation
            .placement
            .as_ref()
            .is_some_and(|d| !d.current(&self.editor, self.plans.active, self.selected));
        if escape || stale {
            if self.plans.elevation.placement.take().is_some() {
                self.status = "Elevation placement canceled.".into();
            }
            self.plans.elevation.crop = None;
        }
        if self.plans.elevation.crop.as_ref().is_some_and(|(c, _)| {
            self.plans.active != Some(c.view_id)
                || self.editor.native_view_context(c.view_id).ok() != Some(*c)
        }) {
            self.plans.elevation.crop = None;
        }
    }

    pub(super) fn open_wall_elevation(&mut self, host: Id, side: ElevationSide) {
        match self
            .editor
            .create_wall_elevation("Wall elevation", host, side)
        {
            Ok(id) => {
                self.focus_plan(Some(id));
                self.select(Some(host));
                self.report(
                    Ok(()),
                    "Wall elevation created. Station runs from wall start to end.",
                );
            }
            Err(e) => self.report(Err(e), ""),
        }
    }

    fn begin_elevation_placement(&mut self, kind: OpeningKind) {
        self.plans.elevation.dimension = None;
        let Some((view, s)) = self.active_elevation() else {
            return;
        };
        self.select(Some(s.host));
        self.opening_draft = None;
        match OpeningDraft::begin(
            &self.editor,
            Some(view),
            self.selected,
            Some(kind),
            self.preferred_type(kind),
        ) {
            Ok(draft) => {
                self.plans.elevation.placement = Some(draft);
                self.status = "Click opening centre / window sill, then review exact dimensions. Escape cancels.".into();
            }
            Err(e) => self.report(Err(e), ""),
        }
    }

    pub(super) fn elevation_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (label, layout) in [
                (
                    "Horizontal dimension",
                    os_model::DimensionLayout::ElevationHorizontal,
                ),
                (
                    "Vertical dimension",
                    os_model::DimensionLayout::ElevationVertical,
                ),
            ] {
                if ui.button(label).clicked() {
                    self.begin_elevation_dimension(layout);
                }
            }
            for (kind, label) in [
                (OpeningKind::Door, "Place door"),
                (OpeningKind::Window, "Place window"),
            ] {
                if ui.button(label).clicked() {
                    self.begin_elevation_placement(kind);
                }
                ui.menu_button(format!("{kind:?} type"), |ui| {
                    let types: Vec<_> = self
                        .editor
                        .document
                        .model()
                        .opening_types
                        .values()
                        .filter(|t| t.parameters.kind == kind)
                        .map(|t| (t.id(), t.parameters.name.clone()))
                        .collect();
                    for (id, name) in types {
                        if ui
                            .selectable_label(self.preferred_type(kind) == Some(id), name)
                            .clicked()
                        {
                            self.remember_type(kind, Some(id));
                            self.plans.elevation.placement = None;
                            ui.close();
                        }
                    }
                });
            }
            if ui
                .add_enabled(
                    self.selected
                        .is_some_and(|id| self.editor.document.model().openings.contains_key(&id)),
                    egui::Button::new("Edit opening"),
                )
                .clicked()
            {
                self.plans.elevation.placement = None;
                self.plans.elevation.dimension = None;
                self.begin_opening(None);
            }
            if ui.button("Elevation crop").clicked()
                && let Some((id, settings)) = self.active_elevation()
            {
                match self.editor.native_elevation_context(id) {
                    Ok(context) => self.plans.elevation.crop = Some((context, settings)),
                    Err(e) => self.report(Err(e), ""),
                }
            }
            ui.label("Station → / project elevation ↑ (m)");
        });
        if let Some((context, mut settings)) = self.plans.elevation.crop.take() {
            let mut close = false;
            ui.horizontal_wrapped(|ui| {
                for (label, value) in [
                    ("Start", &mut settings.min_station),
                    ("End", &mut settings.max_station),
                    ("Bottom", &mut settings.bottom_elevation),
                    ("Top", &mut settings.top_elevation),
                ] {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.1));
                }
                ui.selectable_value(&mut settings.side, ElevationSide::Left, "Left side");
                ui.selectable_value(&mut settings.side, ElevationSide::Right, "Right side");
                if ui.button("Apply crop").clicked() {
                    let result = (|| {
                        os_core::ensure(
                            self.editor.native_elevation_context(context.view_id)? == context,
                            "Elevation settings changed",
                        )?;
                        settings.validate()?;
                        let mut parameters = self.editor.document.model().views[&context.view_id]
                            .parameters
                            .clone();
                        parameters.elevation = Some(settings);
                        self.editor.command(
                            "Update elevation crop",
                            Command::UpdateView {
                                id: context.view_id,
                                parameters,
                            },
                        )
                    })();
                    close = result.is_ok();
                    self.report(result, "Elevation crop updated.");
                }
                close |= ui.button("Cancel crop").clicked();
            });
            if !close {
                self.plans.elevation.crop = Some((context, settings));
            }
        }
    }

    pub(super) fn elevation_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(id) = self
            .active_elevation()
            .map(|v| v.0)
            .or_else(|| self.active_facade().map(|v| v.0))
        else {
            return;
        };
        let context = match self.editor.native_view_context(id) {
            Ok(c) => c,
            Err(e) => {
                ui.colored_label(theme::ERROR, e.to_string());
                return;
            }
        };
        let fit = ui.button("Fit elevation").clicked();
        if let Some(error) = &self.plans.error {
            ui.colored_label(theme::ERROR, error);
        }
        let Some(drawing) = &self.plans.drawing else {
            ui.label("Generating elevation…");
            return;
        };
        let Ok(lines) = drawing.provider_lines(context) else {
            return;
        };
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        #[cfg(test)]
        {
            self.plans.canvas_rect = Some(rect);
        }
        if rect.width() < 1. || rect.height() < 1. {
            return;
        }
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let fresh = !self.plans.cameras.contains_key(&id);
        let camera = self.plans.cameras.entry(id).or_default();
        if fit || fresh {
            let crop = context.crop.unwrap();
            camera.center = Point2::new(
                (crop.min.x + crop.max.x) * 0.5,
                (crop.min.y + crop.max.y) * 0.5,
            );
            camera.pixels_per_metre = (0.9
                * (size[0] / (crop.max.x - crop.min.x)).min(size[1] / (crop.max.y - crop.min.y)))
            .clamp(0.001, 10000.);
        }
        let pointer = response
            .hover_pos()
            .map(|p| Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())));
        if self.plans.elevation.placement.is_none()
            && self.plans.facade.is_none()
            && self.plans.elevation.dimension.is_none()
            && self.opening_draft.is_none()
            && response.dragged()
        {
            let d = ctx.input(|i| i.pointer.delta());
            let _ = camera.pan(Point2::new(f64::from(d.x), f64::from(d.y)), size);
        }
        if let Some(p) = pointer {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0. {
                let _ = camera.zoom_at(p, (f64::from(scroll) * 0.002).exp(), size);
            }
        }
        let camera = *camera;
        let screen = |p| {
            camera
                .project(p, size)
                .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
        };
        for line in lines {
            if let (Ok(a), Ok(b)) = (screen(line.start), screen(line.end)) {
                painter.line_segment(
                    [a, b],
                    egui::Stroke::new(
                        if self.selected == Some(line.entity) {
                            2.0
                        } else {
                            1.2
                        },
                        if self.selected == Some(line.entity) {
                            theme::ACCENT
                        } else {
                            ui.visuals().text_color()
                        },
                    ),
                );
            }
        }
        for graphic in drawing.dimensions(context).unwrap_or_default() {
            let _ = paint_dimension_graphic(
                &painter,
                context,
                camera,
                rect,
                size,
                graphic,
                self.selected == Some(graphic.entity),
            );
        }
        self.paint_facade_crop(&painter, camera, rect, size);
        if self.plans.facade.is_some() {
            return;
        }
        if self.plans.elevation.dimension.is_some() {
            self.elevation_dimension_input(&response, &painter, context, camera);
            return;
        }
        if let Some(mut draft) = self.plans.elevation.placement.take() {
            if let Some(point) = pointer.and_then(|p| camera.unproject(p, size).ok()) {
                let crop = context.crop.unwrap();
                let inside = point.x >= crop.min.x
                    && point.x <= crop.max.x
                    && point.y >= crop.min.y
                    && point.y <= crop.max.y;
                let preview = draft
                    .elevation_point(self.editor.document.model(), point)
                    .and_then(|_| {
                        os_core::ensure(inside, "Pointer is outside the elevation crop")?;
                        draft.preview(&self.editor, self.plans.active, self.selected)
                    });
                match preview {
                    Ok(lines) => {
                        for line in lines {
                            if let (Ok(a), Ok(b)) = (screen(line.start), screen(line.end)) {
                                painter.line_segment([a, b], egui::Stroke::new(2., theme::ACCENT));
                            }
                        }
                        if response.clicked() {
                            self.opening_draft = Some(draft);
                            return;
                        }
                    }
                    Err(e) => {
                        self.status = e.to_string();
                        self.status_error = true;
                    }
                }
            }
            self.plans.elevation.placement = Some(draft);
        } else if self.opening_draft.is_none()
            && response.clicked()
            && let Some(pointer) = pointer
        {
            let selected = drawing
                .hits_screen(context, camera, size, pointer, 6.0)
                .ok()
                .and_then(|hits| hits.into_iter().next());
            self.select(selected);
        }
    }
}
