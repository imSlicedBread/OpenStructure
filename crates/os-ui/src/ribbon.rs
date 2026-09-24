use super::*;
use theme::{Icon, command};

impl DesktopApp {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("quick_access")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(8, 3)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("OS").strong().color(theme::ACCENT));
                    ui.separator();
                    if command(ui, Icon::Save, "Save", false)
                        .on_hover_text("Save project (Ctrl+S). Untitled projects ask for a destination.")
                        .clicked()
                    {
                        self.quick_save();
                    }
                    let history = self.editor.document.history_stats();
                    let history_help = format!(
                        "{} undo / {} redo transactions. History uses {} KiB estimated of {} KiB; at most {} transactions. Older whole transactions are released when a limit is reached. {} released this session. History is not saved in the project file.",
                        history.undo_entries, history.redo_entries, history.estimated_bytes / 1024,
                        history.limits.max_estimated_bytes / 1024, history.limits.max_entries, history.evicted_entries
                    );
                    if ui
                        .add_enabled_ui(self.editor.document.can_undo(), |ui| {
                            command(ui, Icon::Undo, "Undo", false)
                        })
                        .inner
                        .on_hover_text(format!("Undo (Ctrl+Z). {history_help}"))
                        .on_disabled_hover_text(&history_help)
                        .clicked()
                    {
                        self.history(false);
                    }
                    if ui
                        .add_enabled_ui(self.editor.document.can_redo(), |ui| {
                            command(ui, Icon::Redo, "Redo", false)
                        })
                        .inner
                        .on_hover_text(format!("Redo (Ctrl+Y). {history_help}"))
                        .on_disabled_hover_text(&history_help)
                        .clicked()
                    {
                        self.history(true);
                    }
                    if history.evicted_entries > 0 {
                        ui.label(egui::RichText::new("History limited").small().color(theme::MUTED))
                            .on_hover_text(&history_help);
                    }
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("OpenStructure").color(theme::MUTED));
                        if self.editor.is_dirty() {
                            ui.label(egui::RichText::new("Unsaved").color(theme::ACCENT));
                        }
                        ui.add(
                            egui::Label::new(&self.editor.document.model().project.parameters.name)
                                .truncate(),
                        );
                    });
                });
            });
        egui::TopBottomPanel::top("ribbon_tabs")
            .exact_height(28.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    egui::containers::menu::MenuButton::new(
                        egui::RichText::new("File").strong().color(theme::ACCENT),
                    )
                    .config(
                        egui::containers::menu::MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .style(|_: &mut egui::Style| {}),
                    )
                    .ui(ui, |ui| self.file_menu(ui));
                    for (tab, label) in [
                        (RibbonTab::Architecture, "Architecture"),
                        (RibbonTab::View, "View"),
                        (RibbonTab::Manage, "Manage"),
                    ] {
                        ui.selectable_value(&mut self.ribbon_tab, tab, label);
                    }
                    if self
                        .selected
                        .is_some_and(|id| self.editor.document.model().walls.contains_key(&id))
                    {
                        ui.separator();
                        ui.selectable_value(
                            &mut self.ribbon_tab,
                            RibbonTab::ModifyWalls,
                            egui::RichText::new("Modify | Walls").color(theme::ACCENT),
                        );
                    }
                });
            });
        egui::TopBottomPanel::top("ribbon_commands")
            .exact_height(80.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("ribbon_overflow")
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| match self.ribbon_tab {
                            RibbonTab::Architecture => {
                                ribbon_group(ui, "Openings", |ui| self.opening_commands(ui));
                                ribbon_group(ui, "Spaces", |ui| {
                                    if command(ui, Icon::Room, "Room", true)
                                        .on_hover_text(
                                            "Place a named room inside an enclosed native wall boundary.",
                                        )
                                        .clicked()
                                    {
                                        self.begin_room_placement();
                                    }
                                    if ui.add_enabled(self.plans.active.is_some(),egui::Button::new("Room Tag")).clicked() {
                                        self.begin_room_tag();
                                    }
                                    if ui.add_enabled(self.plans.active.is_some(), egui::Button::new("Room Separator"))
                                        .on_hover_text("Draw or edit a level-owned room boundary line.")
                                        .clicked()
                                    {
                                        self.begin_room_separation_line();
                                    }
                                });
                                ribbon_group(ui, "Annotate", |ui| {
                                    if ui.add_enabled(self.plans.active.is_some(), egui::Button::new("Detail Line")).clicked() {
                                        self.begin_detail_line();
                                    }
                                    if ui
                                        .add_enabled_ui(self.plans.active.is_some(), |ui| {
                                            command(
                                                ui,
                                                Icon::Measure,
                                                "Aligned dimension",
                                                true,
                                            )
                                        })
                                        .inner
                                        .on_hover_text(
                                            "Dimension between two native wall endpoints in the active floor plan.",
                                        )
                                        .clicked()
                                        && let Some(view) = self.plans.active
                                    {
                                        self.begin_aligned_dimension(view);
                                    }
                                    for (label, layout) in [("Chain", os_model::DimensionLayout::Chain), ("Baseline", os_model::DimensionLayout::Baseline), ("Angular", os_model::DimensionLayout::Angular)] {
                                        if ui.add_enabled(self.plans.active.is_some(), egui::Button::new(label)).clicked()
                                            && let Some(view) = self.plans.active {
                                            self.begin_dimension(view, layout);
                                        }
                                    }
                                });
                                ribbon_group(ui, "Build", |ui| {
                                    if command(ui, Icon::Wall, "Wall", true)
                                        .on_hover_text(
                                            "Create a wall from numeric endpoints in Properties.",
                                        )
                                        .clicked()
                                    {
                                        self.select(None);
                                    }
                                });
                                ribbon_group(ui, "Datum", |ui| {
                                    if command(ui, Icon::Level, "Add level", true)
                                        .on_hover_text("Add a level 3 m above the highest level.")
                                        .clicked()
                                    {
                                        self.add_level();
                                    }
                                });
                            }
                            RibbonTab::View => {
                                ribbon_group(ui, "Navigate", |ui| {
                                    if command(ui, Icon::Fit, "Fit model", true)
                                        .on_hover_text("Frame all walls in the 3D view.")
                                        .clicked()
                                    {
                                        self.fit_requested = true;
                                    }
                                });
                                ribbon_group(ui, "Schedules", |ui| {
                                    if ui.button("Door/window schedule").clicked() {
                                        self.opening_schedule.open = true;
                                    }
                                });
                            }
                            RibbonTab::Manage => self.manage_commands(ui),
                            RibbonTab::ModifyWalls => {
                                ribbon_group(ui, "Openings", |ui| self.opening_commands(ui));
                                ribbon_group(ui, "Edit selection", |ui| {
                                    ui.horizontal(|ui| {
                                        if command(ui, Icon::Apply, "Apply changes", true)
                                            .on_hover_text(
                                                "Commit the wall parameters shown in Properties.",
                                            )
                                            .clicked()
                                        {
                                            self.apply_wall();
                                        }
                                        if command(ui, Icon::Delete, "Delete wall", true)
                                            .on_hover_text(
                                                "Delete the selected wall. Undo restores it.",
                                            )
                                            .clicked()
                                        {
                                            self.delete_wall();
                                        }
                                    });
                                });
                            }
                        });
                    });
            });
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_width(350.0);
        ui.label(egui::RichText::new("Project file").strong());
        ui.label("Open or save a native .osb project");
        ui.add(
            egui::TextEdit::singleline(&mut self.path)
                .desired_width(340.0)
                .hint_text("Project .osb path"),
        );
        ui.separator();
        if ui
            .button("New")
            .on_hover_text("Create an empty project.")
            .clicked()
        {
            ui.close();
            self.request_action(PendingAction::New);
        }
        if ui
            .button("Open")
            .on_hover_text("Open the project at the path above.")
            .clicked()
        {
            ui.close();
            self.request_action(PendingAction::Open);
        }
        if ui
            .button("Save")
            .on_hover_text("Save to the path above. Quick Save uses the current project file.")
            .clicked()
        {
            ui.close();
            self.save();
        }
        ui.separator();
        ui.label(egui::RichText::new("IFC4 wall exchange (experimental)").strong());
        ui.label("Separate exchange copy; not a native backup.");
        ui.add(
            egui::TextEdit::singleline(&mut self.ifc_path)
                .desired_width(340.0)
                .hint_text("Exchange .ifc path"),
        );
        ui.add_enabled_ui(os_ifc::AVAILABLE, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Export IFC…").clicked() {
                    ui.close();
                    self.prepare_ifc_export();
                }
                if ui.button("Import IFC…").clicked() {
                    ui.close();
                    self.prepare_ifc_import();
                }
            });
        })
        .response
        .on_disabled_hover_text(os_ifc::UNAVAILABLE_REASON);
    }

    fn manage_commands(&mut self, ui: &mut egui::Ui) {
        ribbon_group(ui, "Project", |ui| {
            ui.set_width(230.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.project_name)
                    .desired_width(220.0)
                    .hint_text("Project name"),
            );
            if ui.button("Rename project").clicked() {
                let result = self.editor.command(
                    "Rename project",
                    Command::RenameProject(self.project_name.clone()),
                );
                self.report(result, "Project renamed.");
            }
        });
        ribbon_group(ui, "Active level", |ui| {
            ui.set_width(210.0);
            if let Some(level) = self
                .editor
                .document
                .model()
                .levels
                .get(&self.active_level)
                .cloned()
            {
                ui.add(egui::Label::new(&level.parameters.name).truncate());
                ui.horizontal(|ui| {
                    ui.label("Elevation");
                    let mut elevation = level.parameters.elevation;
                    if ui
                        .add(egui::DragValue::new(&mut elevation).speed(0.1).suffix(" m"))
                        .changed()
                    {
                        let mut parameters = level.parameters;
                        parameters.elevation = elevation;
                        let result = self.editor.command(
                            "Change level elevation",
                            Command::UpdateLevel {
                                id: self.active_level,
                                parameters,
                            },
                        );
                        self.report(result, "Level and dependent walls updated.");
                    }
                });
            }
        });
        ribbon_group(ui, "Plugins", |ui| {
            ui.label(format!("{} loaded", self.editor.host.manifests().count()));
            #[cfg(feature = "external-plugins")]
            if ui.button("Plugin manager").clicked() {
                self.plugin_manager.visible = true;
            }
            #[cfg(not(feature = "external-plugins"))]
            ui.label(egui::RichText::new("External loading disabled").color(theme::MUTED));
        });
    }

    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let color = if self.status_error {
                        theme::ERROR
                    } else {
                        theme::MUTED
                    };
                    ui.label(
                        egui::RichText::new(if self.status_error {
                            "Error"
                        } else if self
                            .editor
                            .document
                            .model()
                            .extensions
                            .keys()
                            .any(|id| !self.editor.scene.contains_key(id))
                        {
                            "Incomplete view"
                        } else {
                            "Ready"
                        })
                        .strong()
                        .color(color),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Details")
                            .on_hover_text("Show the complete message and document diagnostics.")
                            .clicked()
                        {
                            self.show_details = !self.show_details;
                        }
                        ui.label(format!(
                            "{} walls",
                            self.editor.document.model().walls.len()
                        ));
                        ui.separator();
                        ui.label("m").on_hover_text("All dimensions are in metres.");
                        let level = self
                            .editor
                            .document
                            .model()
                            .levels
                            .get(&self.active_level)
                            .map(|l| l.parameters.name.as_str())
                            .unwrap_or("No level");
                        ui.add_sized([120.0, 24.0], egui::Label::new(level).truncate())
                            .on_hover_text(format!("Active level: {level}"));
                        ui.separator();
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(self.status.lines().next().unwrap_or(""))
                                    .color(color),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&self.status);
                    });
                });
            });
        if self.show_details {
            egui::Window::new("Operation details")
                .open(&mut self.show_details)
                .default_width(440.0)
                .resizable(true)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(&self.status).color(
                                if self.status_error {
                                    theme::ERROR
                                } else {
                                    theme::TEXT
                                },
                            ));
                            ui.separator();
                            ui.label(format!("Revision {}", self.editor.document.revision()));
                            ui.label(format!("{} loaded plugins. Dimensions use metres.", self.editor.host.manifests().count()));
                            let model = self.editor.document.model();
                            if !model.extensions.is_empty() {
                                ui.separator();
                                ui.strong("Preserved plugin elements (read-only)");
                                ui.label("Geometry and editing providers are unavailable in this build. Native save retains these payloads; IFC export is blocked.");
                                let entities: Vec<_> = model.extensions.values().collect();
                                egui::ScrollArea::vertical().id_salt("extension_list").max_height(100.0).show_rows(ui, 22.0, entities.len(), |ui, rows| {
                                    for row in rows {
                                        let entity = entities[row];
                                        if ui.selectable_label(self.extension_inspection == Some(entity.id), &entity.name).on_hover_text(entity.id.to_string()).clicked() {
                                            self.extension_inspection = Some(entity.id);
                                        }
                                    }
                                });
                                if let Some(entity) = self.extension_inspection.and_then(|id| model.extensions.get(&id)) {
                                    ui.label(format!("ID: {}", entity.id));
                                    ui.label(format!("Type: {}", entity.type_id));
                                    let required = &model.plugin_requirements[&entity.owner].version;
                                    let installed = self.editor.host.manifests().find(|m| m.id == entity.owner);
                                    let state = match installed {
                                        None => "missing or disabled".to_owned(),
                                        Some(m) if m.version != *required => format!("loaded version {}; required version unavailable", m.version),
                                        Some(_) => "loaded; generic provider not implemented".to_owned(),
                                    };
                                    ui.label(format!("Owner: {} @ {required} — {state}", entity.owner));
                                    ui.label(format!("Payload schema {} · {} references", entity.payload_schema_version, entity.references().count()));
                                    let json = serde_json::to_string_pretty(entity).unwrap_or_else(|e| e.to_string());
                                    let preview: String = json.chars().take(4096).collect();
                                    ui.label(egui::RichText::new(&preview).monospace());
                                    if preview.len() < json.len() { ui.label("Inspection preview truncated to 4096 characters; full data remains in the native file."); }
                                }
                            }
                        });
                });
        }
    }
}

fn ribbon_group(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(96.0, 52.0),
            egui::Layout::top_down(egui::Align::Min),
            contents,
        );
        ui.label(egui::RichText::new(label).size(12.0).color(theme::MUTED));
    });
    ui.separator();
}
