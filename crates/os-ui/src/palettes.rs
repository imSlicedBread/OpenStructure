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
        if self.column_properties(ui) {
            return;
        }
        if let Some(ty) = self
            .selected
            .and_then(|id| self.editor.document.model().opening_types.get(&id).cloned())
        {
            let id = ty.id();
            let p = &ty.parameters;
            let instances = self
                .editor
                .document
                .model()
                .openings
                .values()
                .filter(|opening| opening.parameters.type_id() == Some(id))
                .count();
            ui.label(format!("{:?} type: {}", p.kind, p.name));
            ui.label(format!(
                "Width: {:.3} m · Height: {:.3} m · Sill: {:.3} m",
                p.width, p.height, p.sill
            ));
            ui.label(format!("Placed instances: {instances}"));
            if ui.button("Manage opening type…").clicked() {
                self.begin_edit_opening_type(id);
            }
            return;
        }
        if let Some(floor) = self
            .selected
            .and_then(|id| self.editor.document.model().floors.get(&id).cloned())
        {
            let id = floor.id();
            let model = self.editor.document.model();
            let level_name = model
                .levels
                .get(&floor.parameters.level)
                .map_or("Missing level", |level| level.parameters.name.as_str());
            ui.label("Native floor / slab");
            ui.label(&floor.parameters.name);
            ui.label(format!("Level: {level_name}"));
            ui.label(format!("Area: {:.2} m²", floor.parameters.area()));
            ui.label(format!(
                "Thickness: {:.3} m · top offset: {:+.3} m",
                floor.parameters.thickness, floor.parameters.top_offset
            ));
            if ui.button("Delete floor").clicked() {
                let result = self
                    .editor
                    .command("Delete floor", Command::RemoveFloor(id));
                if result.is_ok() {
                    self.select(None);
                }
                self.report(result, "Floor deleted.");
            }
            ui.label(format!("Stable ID: {id}"));
            return;
        }
        if let Some(opening) = self
            .selected
            .and_then(|id| self.editor.document.model().openings.get(&id).cloned())
        {
            let p = &opening.parameters;
            let resolved = match self.editor.document.model().resolve_opening(p) {
                Ok(resolved) => resolved,
                Err(error) => {
                    ui.colored_label(theme::ERROR, error.to_string());
                    return;
                }
            };
            ui.label(format!("{:?}: {}", resolved.kind, p.name));
            ui.label(format!(
                "Host: {}",
                self.editor.document.model().walls[&p.host].parameters.name
            ));
            ui.label(format!("Offset: {} m", p.offset));
            if resolved.kind == os_model::OpeningKind::Door {
                ui.label(format!(
                    "Hinge: wall {:?} · Swing: {:?} of wall",
                    p.hinge, p.swing
                ));
            }
            ui.label(format!(
                "{} · {:.3} × {:.3} m · sill {:.3} m",
                resolved.type_name.as_deref().unwrap_or("Legacy dimensions"),
                resolved.width,
                resolved.height,
                resolved.sill
            ));
            if ui.button("Edit opening properties").clicked() {
                self.begin_opening(None);
            }
            if let Some(type_id) = resolved.type_id {
                if ui.button("Edit assigned type").clicked() {
                    self.begin_edit_opening_type(type_id);
                }
            } else if ui.button("Create reusable type from opening").clicked() {
                self.begin_type_from_opening(opening.id());
            }
            return;
        }
        if let Some(dimension) = self
            .selected
            .and_then(|id| self.editor.document.model().dimensions.get(&id).cloned())
        {
            let id = dimension.id();
            let parameters = &dimension.parameters;
            ui.add(
                egui::Label::new(format!(
                    "{:?} dimension · live wall references",
                    parameters.layout
                ))
                .wrap(),
            );
            for (index, reference) in parameters.references().enumerate() {
                ui.add(
                    egui::Label::new(format!(
                        "{}: {} {:?}",
                        index + 1,
                        reference.wall,
                        reference.endpoint
                    ))
                    .wrap(),
                );
            }
            if parameters.layout == os_model::DimensionLayout::Angular {
                match parameters.resolve_angular(self.editor.document.model()) {
                    Ok(angle) => {
                        ui.label(format!("{:.2}°", angle.degrees()));
                    }
                    Err(reason) => {
                        ui.colored_label(theme::ERROR, format!("Broken reference: {reason:?}"));
                    }
                }
            } else {
                match parameters.resolve_points(self.editor.document.model()) {
                    Ok(points) => {
                        for i in 1..points.len() {
                            let from = if parameters.layout == os_model::DimensionLayout::Chain {
                                i - 1
                            } else {
                                0
                            };
                            ui.label(format!(
                                "{}–{}: {:.3} m",
                                from + 1,
                                i + 1,
                                points[from].distance(points[i])
                            ));
                        }
                    }
                    Err(reason) => {
                        ui.colored_label(theme::ERROR, format!("Broken reference: {reason:?}"));
                    }
                }
            }
            if let Some(view) = self.editor.document.model().views.get(&parameters.view) {
                ui.label(format!("Plan: {}", view.parameters.name));
                if self.plans.active != Some(parameters.view)
                    && ui.button("Open owning plan").clicked()
                {
                    self.focus_plan(Some(parameters.view));
                }
            }
            ui.horizontal(|ui| {
                ui.label(if parameters.layout == os_model::DimensionLayout::Angular {
                    "Arc radius (m)"
                } else {
                    "Offset (m)"
                });
                ui.add(
                    egui::DragValue::new(&mut self.dimension_offset_draft)
                        .speed(0.01)
                        .range(-1_000_000.0..=1_000_000.0),
                );
            });
            if parameters.layout == os_model::DimensionLayout::Baseline {
                ui.horizontal(|ui| {
                    ui.label("Baseline spacing (model m)");
                    ui.add(
                        egui::DragValue::new(&mut self.dimension_spacing_draft)
                            .speed(0.01)
                            .range(0.000001..=1_000_000.0),
                    );
                });
            }
            ui.small(
                "Values follow current endpoints; spacing is in model metres, not paper scale.",
            );
            if ui.button("Apply dimension offset").clicked() {
                self.apply_dimension_properties();
            }
            if ui.button("Delete dimension").clicked() {
                self.delete_selected_dimension();
            }
            ui.label(format!("Stable ID: {id}"));
            return;
        }
        if let Some(line) = self
            .selected
            .and_then(|id| self.editor.document.model().room_separation_lines.get(&id))
            .cloned()
        {
            ui.label("Room separator · level-owned room boundary");
            ui.label(format!(
                "Length: {:.3} m",
                line.parameters.start.distance(line.parameters.end)
            ));
            ui.label("Same-level plans · no thickness or 3D geometry");
            ui.small("Drag the selected line to move; drag an endpoint to resize.");
            if ui.button("Delete room separator").clicked() {
                let result = self.editor.command(
                    "Delete room separator",
                    Command::RemoveRoomSeparationLine(line.id()),
                );
                if result.is_ok() {
                    self.select(None);
                }
                self.report(result, "Room separator deleted.");
            }
            ui.label(format!("ID: {}", line.id()));
            return;
        }
        if let Some(line) = self
            .selected
            .and_then(|id| self.editor.document.model().detail_lines.get(&id))
            .cloned()
        {
            ui.label("Detail line · independent plan drafting");
            ui.label(format!(
                "Length: {:.3} m",
                line.parameters.start.distance(line.parameters.end)
            ));
            ui.label("Solid black · 0.25 mm on paper");
            ui.small("Drag the selected line to move; drag an endpoint to resize.");
            if ui.button("Delete detail line").clicked() {
                let result = self
                    .editor
                    .command("Delete detail line", Command::RemoveDetailLine(line.id()));
                if result.is_ok() {
                    self.select(None);
                }
                self.report(result, "Detail line deleted.");
            }
            ui.label(format!("ID: {}", line.id()));
            return;
        }
        if let Some(tag) = self
            .selected
            .and_then(|id| self.editor.document.model().room_tags.get(&id))
            .cloned()
        {
            ui.label("Room tag · live room number and name");
            ui.label(format!("Room: {}", tag.parameters.room));
            match tag.parameters.resolve(self.editor.document.model()) {
                Ok(room) => {
                    ui.label(format!(
                        "{} · {}",
                        room.parameters.number, room.parameters.name
                    ));
                }
                Err(reason) => {
                    ui.colored_label(theme::ERROR, reason);
                }
            }
            ui.horizontal(|ui| {
                ui.label("X (m)");
                ui.add(egui::DragValue::new(&mut self.room_tag_position.x).speed(0.05));
                ui.label("Y (m)");
                ui.add(egui::DragValue::new(&mut self.room_tag_position.y).speed(0.05));
            });
            if ui.button("Apply tag position").clicked() {
                self.apply_room_tag_properties();
            }
            if ui.button("Delete room tag").clicked() {
                let result = self
                    .editor
                    .command("Delete room tag", Command::RemoveRoomTag(tag.id()));
                if result.is_ok() {
                    self.select(None);
                }
                self.report(result, "Room tag deleted.");
            }
            ui.label(format!("ID: {}", tag.id()));
            return;
        }
        if let Some(room) = self
            .selected
            .and_then(|id| self.editor.document.model().rooms.get(&id))
        {
            let room_id = room.id();
            let level = room.parameters.level;
            ui.label("Native room · area derived from wall boundaries");
            if let Some(view) = self.plans.active.filter(|view| {
                self.editor
                    .document
                    .model()
                    .views
                    .get(view)
                    .is_some_and(|view| view.parameters.level == Some(level))
            }) {
                let derived = self
                    .editor
                    .native_plan_context(view)
                    .ok()
                    .and_then(|context| {
                        self.plans
                            .drawing
                            .as_ref()?
                            .rooms(context)
                            .ok()?
                            .iter()
                            .find(|item| item.entity == room_id)
                            .cloned()
                    });
                match derived {
                    Some(item) if item.diagnostic.is_none() => {
                        ui.label(format!("Area: {:.2} m²", item.area_m2));
                    }
                    Some(item) => {
                        ui.colored_label(
                            theme::ERROR,
                            item.diagnostic
                                .as_deref()
                                .unwrap_or("Room boundary unavailable"),
                        );
                    }
                    None => {
                        ui.label("Room area is updating in the floor plan…");
                    }
                }
            } else {
                ui.label("Open a floor plan on this room’s level to inspect its area.");
            }
            ui.separator();
            ui.label("Number");
            ui.text_edit_singleline(&mut self.room_number_draft);
            ui.label("Name");
            ui.text_edit_singleline(&mut self.room_name_draft);
            if ui.button("Apply room properties").clicked() {
                self.apply_room_properties();
            }
            if ui.button("Delete room").clicked() {
                self.delete_selected_room();
            }
            return;
        }
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
            self.wall_join_controls(ui);
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
        let volume = match self
            .selected
            .filter(|id| self.editor.document.model().walls.contains_key(id))
        {
            Some(id) => {
                match os_geometry::walls::NativeWall::from_model(self.editor.document.model(), id)
                    .and_then(|w| w.net_volume())
                {
                    Ok(volume) => format!("Model net volume: {volume:.3} m³"),
                    Err(_) => "Model net volume unavailable".into(),
                }
            }
            None => format!(
                "Draft gross volume: {:.3} m³",
                self.draft.length() * self.draft.height * self.draft.thickness
            ),
        };
        actions.label(egui::RichText::new(volume).size(12.0).color(theme::MUTED));
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
        self.wall_join_controls(ui);
        self.wall_type_controls(ui);
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
                ui.add_enabled(
                    self.selected.is_none_or(|id| {
                        !self
                            .editor
                            .document
                            .model()
                            .wall_type_assignments
                            .contains_key(&id)
                    }),
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
                if !model.columns.is_empty() {
                    egui::CollapsingHeader::new(format!("Columns ({})", model.columns.len())).id_salt("native_columns").default_open(true).show(ui, |ui| {
                        for (id, column) in &model.columns {
                            if ui.selectable_label(self.selected == Some(*id), &column.parameters.name).clicked() { self.select(Some(*id)); }
                        }
                    });
                }
                if !model.floors.is_empty() {
                    egui::CollapsingHeader::new(format!("Floors / Slabs ({})", model.floors.len()))
                        .id_salt("floors")
                        .default_open(true)
                        .show(ui, |ui| {
                            for (id, floor) in &model.floors {
                                ui.push_id(id, |ui| {
                                    if ui
                                        .selectable_label(self.selected == Some(*id), &floor.parameters.name)
                                        .clicked()
                                    {
                                        self.select(Some(*id));
                                    }
                                });
                            }
                        });
                }
                if !model.openings.is_empty() {
                    egui::CollapsingHeader::new(format!("Doors / Windows ({})", model.openings.len())).id_salt("openings").default_open(true).show(ui, |ui| {
                        for (id, opening) in &model.openings {
                            ui.push_id(id, |ui| {
                                let label = model.resolve_opening(&opening.parameters).map_or_else(
                                    |_| format!("Opening: {} (invalid type)", opening.parameters.name),
                                    |resolved| format!("{:?}: {}", resolved.kind, opening.parameters.name),
                                );
                                if ui.selectable_label(self.selected == Some(*id), label).clicked() { self.select(Some(*id)); }
                            });
                        }
                    });
                }
                if !model.opening_types.is_empty() {
                    egui::CollapsingHeader::new(format!("Door / Window Types ({})", model.opening_types.len()))
                        .id_salt("opening_types").default_open(true).show(ui, |ui| {
                            for (id, ty) in &model.opening_types {
                                ui.push_id(id, |ui| {
                                    if ui.selectable_label(self.selected == Some(*id),
                                        format!("{:?}: {} · {:.2} × {:.2} m", ty.parameters.kind,
                                            ty.parameters.name, ty.parameters.width, ty.parameters.height))
                                        .clicked() { self.select(Some(*id)); }
                                });
                            }
                        });
                }
                if !model.rooms.is_empty() {
                    egui::CollapsingHeader::new(format!("Rooms ({})", model.rooms.len()))
                        .id_salt("rooms")
                        .default_open(true)
                        .show(ui, |ui| {
                            for (id, room) in &model.rooms {
                                ui.push_id(id, |ui| {
                                    let label = format!(
                                        "{} · {}",
                                        room.parameters.number, room.parameters.name
                                    );
                                    if ui
                                        .selectable_label(self.selected == Some(*id), label)
                                        .clicked()
                                    {
                                        self.select(Some(*id));
                                    }
                                });
                            }
                        });
                }
                if !model.room_tags.is_empty() {
                    egui::CollapsingHeader::new("Room Tags").id_salt("room_tags").default_open(true).show(ui,|ui| {
                        for view in model.views.values().filter(|view|model.room_tags.values().any(|tag|tag.parameters.view == view.id())) {
                            egui::CollapsingHeader::new(&view.parameters.name).id_salt(("tag_view",view.id())).default_open(true).show(ui,|ui| {
                                for tag in model.room_tags.values().filter(|tag|tag.parameters.view == view.id()) {
                                    let label = tag.parameters.resolve(&model).map_or_else(|reason| format!("Room tag · {reason}"), |room| format!("Tag: {} · {}",room.parameters.number,room.parameters.name));
                                    ui.push_id(tag.id(),|ui| {
                                        if ui.selectable_label(self.selected == Some(tag.id()),label).clicked() { self.select(Some(tag.id())); }
                                    });
                                }
                            });
                        }
                    });
                }
                if !model.dimensions.is_empty() {
                    egui::CollapsingHeader::new(format!(
                        "Dimensions ({})",
                        model.dimensions.len()
                    ))
                    .id_salt("aligned_dimensions")
                    .default_open(true)
                    .show(ui, |ui| {
                        for (id, dimension) in &model.dimensions {
                            let anchor_label = |reference: os_model::DimensionReference| {
                                model.walls.get(&reference.wall).map_or_else(
                                    || "Missing wall".to_owned(),
                                    |wall| {
                                        format!(
                                            "{} {:?}",
                                            wall.parameters.name, reference.endpoint
                                        )
                                    },
                                )
                            };
                            let label = format!(
                                "{:?}: {}",
                                dimension.parameters.layout,
                                dimension.parameters.references().map(anchor_label).collect::<Vec<_>>().join(" → ")
                            );
                            ui.push_id(id, |ui| {
                                if ui
                                    .selectable_label(self.selected == Some(*id), label)
                                    .clicked()
                                {
                                    self.focus_plan(Some(dimension.parameters.view));
                                    self.select(Some(*id));
                                }
                            });
                        }
                    });
                }
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
