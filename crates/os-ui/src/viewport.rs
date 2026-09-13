use super::*;

pub(super) struct ViewportCache {
    triangles: Vec<os_render::Triangle>,
    size: [f32; 2],
    scale: f32,
    pub(super) selected: Option<Id>,
    pub(super) frame: os_render::RasterFrame,
    pub(super) texture: egui::TextureHandle,
}

impl DesktopApp {
    pub(super) fn model_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::TopBottomPanel::top("view_tab")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(8, 3)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("3D").strong().color(theme::ACCENT));
                    ui.separator();
                    ui.label(
                        egui::RichText::new("Model view")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                });
            });
        let unavailable = self
            .editor
            .document
            .model()
            .extensions
            .keys()
            .filter(|id| !self.editor.scene.contains_key(id))
            .count();
        if unavailable > 0 {
            egui::TopBottomPanel::top("unavailable_extensions")
                    .frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(8))
                    .show_inside(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(theme::ERROR, format!("Incomplete view: {unavailable} plugin elements have unavailable geometry."));
                            if ui.button("Inspect preserved data").clicked() { self.show_details = true; }
                        });
                    });
        }
        egui::TopBottomPanel::bottom("view_controls")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(8, 3)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("Fit model")
                        .on_hover_text("Frame all walls.")
                        .clicked()
                    {
                        self.fit_requested = true;
                    }
                    ui.separator();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("Drag to orbit · Scroll to zoom · Click to select")
                                .size(12.0)
                                .color(theme::MUTED),
                        )
                        .truncate(),
                    );
                });
            });
        let (canvas, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        let rect = canvas.rect;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        // Leave the shared edge available to the neighboring palette resize handle.
        let response = ui.interact(
            rect.shrink2(egui::vec2(4.0, 0.0)),
            ui.id().with("model_input"),
            egui::Sense::click_and_drag(),
        );
        painter.rect_filled(rect, 0.0, theme::CANVAS);
        if response.dragged() {
            let delta = ctx.input(|i| i.pointer.delta());
            self.camera.orbit(delta.x, delta.y);
        }
        if response.hovered() {
            self.camera.zoom(ctx.input(|i| i.smooth_scroll_delta.y));
        }
        if self.fit_requested {
            self.camera
                .fit(&self.editor.scene, rect.width(), rect.height());
            self.fit_requested = false;
        }
        let pos = |p: os_render::ProjectedPoint| egui::pos2(rect.left() + p.x, rect.top() + p.y);
        let project = |v| pos(self.camera.project(v, rect.width(), rect.height()));
        for i in -10..=10 {
            let i = f64::from(i);
            for (a, b) in [
                (Vec3::new(i, -10.0, 0.0), Vec3::new(i, 10.0, 0.0)),
                (Vec3::new(-10.0, i, 0.0), Vec3::new(10.0, i, 0.0)),
            ] {
                painter.line_segment(
                    [project(a), project(b)],
                    egui::Stroke::new(1.0_f32, theme::GRID),
                );
            }
        }
        for (v, color) in [
            (
                Vec3::new(2.0, 0.0, 0.0),
                egui::Color32::from_rgb(210, 100, 95),
            ),
            (
                Vec3::new(0.0, 2.0, 0.0),
                egui::Color32::from_rgb(105, 175, 120),
            ),
            (
                Vec3::new(0.0, 0.0, 2.0),
                egui::Color32::from_rgb(100, 150, 230),
            ),
        ] {
            painter.line_segment(
                [project(Vec3::default()), project(v)],
                egui::Stroke::new(2.0_f32, color),
            );
        }
        let triangles = os_render::project_scene(
            &self.editor.scene,
            &self.camera,
            rect.width(),
            rect.height(),
        );
        let size = [rect.width(), rect.height()];
        let scale = ctx.pixels_per_point();
        let changed = self.viewport_cache.as_ref().is_none_or(|c| {
            c.triangles != triangles
                || c.size != size
                || c.scale != scale
                || c.selected != self.selected
        });
        if changed {
            match os_render::RasterFrame::render(
                &triangles,
                size,
                scale,
                self.selected,
                os_render::RasterStyle::default(),
            ) {
                Ok(frame) => {
                    let pixels = frame
                        .rgba
                        .iter()
                        .map(|p| egui::Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
                        .collect();
                    let image = egui::ColorImage::new([frame.width, frame.height], pixels);
                    let texture = if let Some(mut cache) = self.viewport_cache.take() {
                        cache.texture.set(image, egui::TextureOptions::NEAREST);
                        cache.texture
                    } else {
                        ctx.load_texture(
                            "depth_buffered_model",
                            image,
                            egui::TextureOptions::NEAREST,
                        )
                    };
                    self.viewport_cache = Some(ViewportCache {
                        triangles,
                        size,
                        scale,
                        selected: self.selected,
                        frame,
                        texture,
                    });
                }
                Err(error) => {
                    self.viewport_cache = None;
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        error.to_string(),
                        egui::FontId::proportional(14.0),
                        theme::ERROR,
                    );
                }
            }
        }
        if let Some(cache) = &self.viewport_cache {
            painter.image(
                cache.texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let selected = self.viewport_cache.as_ref().and_then(|c| {
                c.frame
                    .pick(pointer.x - rect.left(), pointer.y - rect.top())
            });
            self.select(selected);
            ctx.request_repaint();
        }
        if self.editor.scene.is_empty() {
            let welcome = egui::Rect::from_center_size(
                rect.center(),
                egui::vec2((rect.width() - 32.0).min(390.0), 116.0),
            );
            painter.rect(
                welcome,
                2,
                theme::SURFACE,
                egui::Stroke::new(1.0_f32, theme::BORDER),
                egui::StrokeKind::Inside,
            );
            painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    if self.editor.document.model().extensions.is_empty() {
                        "Start your model\n\nArchitecture / Wall\nEnter endpoints in Properties, then Create wall"
                    } else {
                        "Plugin geometry unavailable\n\nThis is not an empty project.\nNative save preserves the original plugin data."
                    },
                    egui::FontId::proportional(14.0),
                    theme::MUTED,
                );
        }
    }
}
