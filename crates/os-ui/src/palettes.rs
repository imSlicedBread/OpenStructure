use super::*;

impl DesktopApp {
    pub(super) fn palettes(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("workspace_palettes")
            .default_width(320.0)
            .width_range(280.0..=420.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(theme::SURFACE))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let usable = (rect.height() - 6.0).max(1.0);
                let mut top_height =
                    (usable * self.properties_fraction).clamp(160.0, (usable - 120.0).max(160.0));
                let divider = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.top() + top_height),
                    egui::vec2(rect.width(), 6.0),
                );
                let response = ui
                    .interact(
                        divider,
                        ui.id().with("palette_divider"),
                        egui::Sense::drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                    .on_hover_text("Drag to resize Properties and Project Browser.");
                if response.dragged() {
                    top_height = (top_height + ctx.input(|i| i.pointer.delta().y))
                        .clamp(160.0, (usable - 120.0).max(160.0));
                    self.properties_fraction = top_height / usable;
                }
                let top = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), top_height));
                let bottom =
                    egui::Rect::from_min_max(egui::pos2(rect.left(), top.bottom() + 6.0), rect.max);
                let mut properties = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("properties_palette")
                        .max_rect(top.shrink(8.0)),
                );
                properties.set_clip_rect(top);
                self.properties_contents(&mut properties);
                let mut browser = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("browser_palette")
                        .max_rect(bottom.shrink(8.0)),
                );
                browser.set_clip_rect(bottom);
                self.browser_contents(&mut browser);
                let grip = egui::Rect::from_min_max(
                    egui::pos2(rect.left(), top.bottom()),
                    egui::pos2(rect.right(), bottom.top()),
                );
                ui.painter().rect_filled(grip, 0, theme::BACKGROUND);
                ui.painter().line_segment(
                    [grip.left_center(), grip.right_center()],
                    egui::Stroke::new(1.0_f32, theme::BORDER),
                );
                ui.painter().line_segment(
                    [
                        grip.center() - egui::vec2(16.0, 0.0),
                        grip.center() + egui::vec2(16.0, 0.0),
                    ],
                    egui::Stroke::new(
                        2.0_f32,
                        if response.hovered() {
                            theme::ACCENT
                        } else {
                            theme::MUTED
                        },
                    ),
                );
                ui.advance_cursor_after_rect(rect);
            });
    }

    fn properties_contents(&mut self, ui: &mut egui::Ui) {
        ui.heading("Properties");
        if let Some(grid) = self
            .selected
            .and_then(|id| self.editor.document.model().grids.get(&id))
        {
            ui.label(format!("Architectural grid {}", grid.parameters.name));
            ui.label(format!(
                "Building: {}",
                self.editor.document.model().buildings[&grid.parameters.building]
                    .parameters
                    .name
            ));
            ui.label(format!(
                "Start: {}, {} m",
                grid.parameters.start.x, grid.parameters.start.y
            ));
            ui.label(format!(
                "End: {}, {} m",
                grid.parameters.end.x, grid.parameters.end.y
            ));
            let id = grid.id();
            if ui.button("Edit grid").clicked() {
                self.begin_grid_form(Some(id));
            }
            return;
        }
        #[cfg(feature = "external-plugins")]
        if self
            .editor
            .host
            .catalog(os_plugin_api::wall::OWNER)
            .is_some()
            && self
                .selected
                .is_none_or(|id| self.editor.document.model().walls.contains_key(&id))
        {
            ui.label("External Wall provider");
            ui.label("Wall parameters come from the registered Plugin tools form.");
            if ui.button("Review Wall parameters").clicked() {
                self.apply_wall();
            }
            ui.separator();
            ui.label("Next pointer wall (metres)");
            ui.add_enabled_ui(
                self.wall_gesture.is_none() && !self.editor.plugin_work_pending(),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Height");
                        ui.add(egui::DragValue::new(&mut self.draft.height).speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Thickness");
                        ui.add(egui::DragValue::new(&mut self.draft.thickness).speed(0.01));
                    });
                },
            );
            ui.small("Creation uses provider naming and no material assignment. Guest bounds are checked before submission.");
            return;
        }
        if let Some(entity) = self
            .selected
            .and_then(|id| self.editor.document.model().extensions.get(&id))
        {
            ui.label(&entity.name);
            ui.label(&entity.type_id);
            if self.editor.host.catalog(&entity.owner).is_some() {
                ui.label("Use the registered command in Plugin tools to edit this element.");
            } else {
                ui.label("Plugin unavailable. Preserved data is read-only here.");
            }
            if ui.button("Inspect selected plugin data").clicked() {
                self.extension_inspection = Some(entity.id);
                self.show_details = true;
            }
            return;
        }
        ui.label(
            egui::RichText::new(if self.selected.is_some() {
                "Wall · Instance"
            } else {
                "Create wall · Between points"
            })
            .color(theme::MUTED),
        );
        ui.separator();
        let available = ui.available_rect_before_wrap();
        let footer = egui::Rect::from_min_max(
            egui::pos2(available.left(), available.bottom() - 54.0),
            available.max,
        );
        let fields = egui::Rect::from_min_max(
            available.min,
            egui::pos2(available.right(), footer.top() - 4.0),
        );
        let mut body = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("wall_property_body")
                .max_rect(fields),
        );
        body.set_clip_rect(fields);
        egui::ScrollArea::vertical()
            .id_salt("wall_property_scroll")
            .auto_shrink([false, false])
            .max_height(fields.height())
            .show(&mut body, |ui| self.wall_fields(ui));
        let mut actions = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("wall_property_footer")
                .max_rect(footer),
        );
        actions.set_clip_rect(footer);
        actions.separator();
        actions.label(
            egui::RichText::new(format!(
                "Volume: {:.3} m³",
                self.draft.length() * self.draft.height * self.draft.thickness
            ))
            .size(12.0)
            .color(theme::MUTED),
        );
        if actions
            .add_sized(
                [actions.available_width(), 24.0],
                egui::Button::new(
                    egui::RichText::new(if self.selected.is_some() {
                        "Apply changes"
                    } else {
                        "Create wall"
                    })
                    .color(theme::ACCENT),
                )
                .fill(theme::SELECTED),
            )
            .on_hover_text(
                "Commit these parameters to the model. Field edits remain a draft until applied.",
            )
            .clicked()
        {
            self.apply_wall();
        }
    }

    fn wall_fields(&mut self, ui: &mut egui::Ui) {
        theme::section(ui, "Identity");
        ui.horizontal(|ui| {
            ui.add_sized([78.0, 24.0], egui::Label::new("Name"));
            ui.add(
                egui::TextEdit::singleline(&mut self.draft.name)
                    .desired_width(ui.available_width()),
            );
        });
        theme::section(ui, "Constraints");
        egui::Grid::new("wall_constraints")
            .num_columns(2)
            .min_col_width(78.0)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Level");
                let model = self.editor.document.model();
                egui::ComboBox::from_id_salt("wall_level")
                    .width((ui.available_width() - 8.0).clamp(80.0, 200.0))
                    .selected_text(
                        model
                            .levels
                            .get(&self.draft.level)
                            .map(|l| l.parameters.name.as_str())
                            .unwrap_or("Choose level"),
                    )
                    .show_ui(ui, |ui| {
                        for (id, level) in &model.levels {
                            ui.selectable_value(&mut self.draft.level, *id, &level.parameters.name);
                        }
                    });
                ui.end_row();
                ui.label("Height");
                ui.add(
                    egui::DragValue::new(&mut self.draft.height)
                        .speed(0.1)
                        .range(0.001..=100000.0)
                        .suffix(" m"),
                );
                ui.end_row();
            });
        theme::section(ui, "Dimensions");
        egui::Grid::new("wall_dimensions")
            .num_columns(2)
            .min_col_width(78.0)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Length");
                self.draft_length = self.draft.length();
                if ui
                    .add(
                        egui::DragValue::new(&mut self.draft_length)
                            .speed(0.1)
                            .range(0.001..=100000.0)
                            .suffix(" m"),
                    )
                    .changed()
                {
                    match self.draft.with_length(self.draft_length) {
                        Ok(parameters) => self.draft = parameters,
                        Err(error) => self.report(Err(error), ""),
                    }
                }
                ui.end_row();
                ui.label("Thickness");
                ui.add(
                    egui::DragValue::new(&mut self.draft.thickness)
                        .speed(0.01)
                        .range(0.001..=1000.0)
                        .suffix(" m"),
                );
                ui.end_row();
            });
        theme::section(ui, "Endpoints");
        egui::Grid::new("wall_endpoints")
            .num_columns(2)
            .min_col_width(78.0)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for (label, value) in [
                    ("Start X", &mut self.draft.start.x),
                    ("Start Y", &mut self.draft.start.y),
                    ("End X", &mut self.draft.end.x),
                    ("End Y", &mut self.draft.end.y),
                ] {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.1).suffix(" m"));
                    ui.end_row();
                }
            });
        if let Some(id) = self.selected {
            theme::section(ui, "Element information");
            ui.label(
                egui::RichText::new("Stable ID")
                    .size(12.0)
                    .color(theme::MUTED),
            );
            ui.add(egui::Label::new(egui::RichText::new(id.to_string()).size(11.0)).wrap());
        }
        ui.add_space(8.0);
    }

    fn browser_contents(&mut self, ui: &mut egui::Ui) {
        ui.heading("Project Browser");
        ui.separator();
        let model = self.editor.document.model().clone();
        egui::ScrollArea::vertical().id_salt("project_browser_scroll").auto_shrink([false,false]).show(ui, |ui| {
            // A fixed ID preserves expansion when the project is renamed.
            egui::CollapsingHeader::new(&model.project.parameters.name).id_salt("project_root").default_open(true).show(ui, |ui| {
                egui::CollapsingHeader::new("Views").id_salt("views").default_open(true).show(ui, |ui| {
                    if ui.selectable_label(self.plans.active.is_none(),"3D").on_hover_text("Focus the current 3D view.").clicked() {
                        self.focus_plan(None);
                        ui.ctx().memory_mut(|m| { if let Some(id)=m.focused() { m.surrender_focus(id); } });
                    }
                    for (id, view) in &model.views {
                        if view.parameters.kind == os_model::ViewKind::Plan && ui.selectable_label(self.plans.active == Some(*id), &view.parameters.name).clicked() {
                            self.focus_plan(Some(*id));
                        }
                    }
                });
                egui::CollapsingHeader::new("Levels").id_salt("levels").default_open(true).show(ui, |ui| {
                    for (id,level) in &model.levels {
                        ui.push_id(id, |ui| {
                            let label=format!("{} · {:.2} m",level.parameters.name,level.parameters.elevation);
                            if ui.add(egui::Button::selectable(self.active_level==*id, label).wrap_mode(egui::TextWrapMode::Truncate))
                                .on_hover_text("Set the active level. Edit its elevation in Manage.").clicked() { self.set_active_level(*id); }
                        });
                    }
                });
                egui::CollapsingHeader::new(format!("Walls ({})", model.walls.len())).id_salt("walls").default_open(true).show(ui, |ui| {
                    if model.walls.is_empty() { ui.label(egui::RichText::new("No walls yet").color(theme::MUTED)); }
                    for (id,wall) in &model.walls {
                        ui.push_id(id, |ui| {
                            if ui.add(egui::Button::selectable(self.selected==Some(*id), &wall.parameters.name).wrap_mode(egui::TextWrapMode::Truncate))
                                .on_hover_text(&wall.parameters.name).clicked() { self.select(Some(*id)); }
                        });
                    }
                });
                if !model.grids.is_empty() {
                    egui::CollapsingHeader::new(format!("Grids ({})", model.grids.len())).id_salt("architectural_grids").default_open(true).show(ui, |ui| {
                        for (id, grid) in &model.grids {
                            ui.push_id(id, |ui| {
                                if ui.add(egui::Button::selectable(self.selected == Some(*id), &grid.parameters.name).wrap_mode(egui::TextWrapMode::Truncate)).clicked() {
                                    self.select(Some(*id));
                                }
                            });
                        }
                    });
                }
                if !model.extensions.is_empty() {
                    egui::CollapsingHeader::new(format!("Plugin elements ({})", model.extensions.len())).id_salt("plugin_elements").default_open(true).show(ui, |ui| {
                        for (id, entity) in &model.extensions {
                            ui.push_id(id, |ui| {
                                if ui.selectable_label(self.selected == Some(*id), &entity.name).clicked() { self.select(Some(*id)); }
                            });
                        }
                    });
                }
            });
        });
    }
}
