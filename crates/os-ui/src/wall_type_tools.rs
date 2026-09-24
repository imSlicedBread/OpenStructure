//! Revision-bound reusable wall type editor; a draft is one atomic transaction.
use super::*;
use os_model::{LayerFunction, WallLayer, WallType, WallTypeAssignment, WallTypeParams};

pub(super) struct WallTypeDraft {
    materials: std::collections::BTreeMap<Id, os_model::Material>,
    session: Id,
    revision: u64,
    entity: WallType,
    editing: bool,
    assign: Option<Id>,
    error: Option<String>,
}
impl WallTypeDraft {
    fn commands(&self, document: &Document) -> Result<Vec<Command>> {
        os_core::ensure(
            self.session == document.session_id() && self.revision == document.revision(),
            "Document changed; cancel and reopen the wall type editor",
        )?;
        let mut commands = vec![if self.editing {
            Command::UpdateWallType {
                id: self.entity.id(),
                parameters: self.entity.parameters.clone(),
            }
        } else {
            Command::AddWallType(self.entity.clone())
        }];
        for (id, material) in &self.materials {
            match document.model().materials.get(id) {
                None => commands.push(Command::AddMaterial(material.clone())),
                Some(old) if old != material => commands.push(Command::UpdateMaterial {
                    id: *id,
                    parameters: material.parameters.clone(),
                }),
                _ => {}
            }
        }
        if let Some(wall) = self.assign {
            commands.push(Command::AssignWallType {
                wall,
                assignment: Some(WallTypeAssignment {
                    type_id: self.entity.id(),
                    flipped: false,
                }),
            });
        }
        Ok(commands)
    }
}
impl DesktopApp {
    fn begin_wall_type(&mut self, ty: Option<WallType>, wall: Option<Id>) {
        let model = self.editor.document.model();
        let editing = ty.is_some();
        let entity = ty.unwrap_or_else(|| {
            let layers = wall
                .and_then(|id| model.resolve_wall(id).ok())
                .map(|profile| {
                    profile
                        .layers
                        .into_iter()
                        .map(|l| WallLayer {
                            id: Id::new(),
                            name: l.name,
                            thickness: l.max - l.min,
                            function: l.function,
                            material: l.material,
                        })
                        .collect()
                })
                .unwrap_or_else(|| {
                    vec![WallLayer {
                        id: Id::new(),
                        name: "Structure".into(),
                        thickness: 0.2,
                        function: LayerFunction::Structure,
                        material: None,
                    }]
                });
            WallType::new(
                "core.wall_type",
                WallTypeParams {
                    name: "New wall type".into(),
                    layers,
                },
            )
        });
        self.wall_type_draft = Some(WallTypeDraft {
            materials: model.materials.clone(),
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            entity,
            editing,
            assign: if editing { None } else { wall },
            error: None,
        });
    }
    pub(super) fn wall_type_controls(&mut self, ui: &mut egui::Ui) {
        theme::section(ui, "Wall type / layers");
        let types: Vec<_> = self
            .editor
            .document
            .model()
            .wall_types
            .values()
            .cloned()
            .collect();
        if let Some(wall) = self
            .selected
            .filter(|id| self.editor.document.model().walls.contains_key(id))
        {
            let previous = self
                .editor
                .document
                .model()
                .wall_type_assignments
                .get(&wall)
                .copied();
            let mut choice = previous.map(|a| a.type_id);
            egui::ComboBox::from_id_salt("wall_type_assignment")
                .selected_text(
                    choice
                        .and_then(|id| types.iter().find(|t| t.id() == id))
                        .map_or("Independent wall", |t| t.parameters.name.as_str()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut choice, None, "Independent wall (saved dimensions)");
                    for ty in &types {
                        ui.selectable_value(&mut choice, Some(ty.id()), &ty.parameters.name);
                    }
                });
            if choice != previous.map(|a| a.type_id) {
                let result = self.editor.command(
                    "Assign wall type",
                    Command::AssignWallType {
                        wall,
                        assignment: choice.map(|type_id| WallTypeAssignment {
                            type_id,
                            flipped: previous.is_some_and(|a| a.flipped),
                        }),
                    },
                );
                self.report(result, "Wall type assigned.");
            }
            if let Some(assignment) = self
                .editor
                .document
                .model()
                .wall_type_assignments
                .get(&wall)
                .copied()
            {
                let mut flipped = assignment.flipped;
                if ui
                    .checkbox(&mut flipped, "Flip layer order across wall axis")
                    .changed()
                {
                    let result = self.editor.command(
                        "Flip wall layers",
                        Command::AssignWallType {
                            wall,
                            assignment: Some(WallTypeAssignment {
                                flipped,
                                ..assignment
                            }),
                        },
                    );
                    self.report(result, "Layer order flipped.");
                }
                if let Ok(profile) = self.editor.document.model().resolve_wall(wall) {
                    ui.label(format!(
                        "Type thickness: {:.4} m",
                        profile.parameters.thickness
                    ));
                }
                if ui.button("Edit shared type / layers…").clicked() {
                    self.begin_wall_type(
                        types.iter().find(|t| t.id() == assignment.type_id).cloned(),
                        None,
                    );
                }
            }
            if ui.button("Create type from this wall…").clicked() {
                self.begin_wall_type(None, Some(wall));
            }
            if let Ok(quantities) =
                os_geometry::walls::NativeWall::from_model(self.editor.document.model(), wall)
                    .and_then(|w| w.layer_quantities())
            {
                ui.collapsing("Layer quantities", |ui| {
                    for q in quantities {
                        ui.label(format!(
                            "{}: {:.4} m³ · {}",
                            q.name,
                            q.volume_m3,
                            q.mass_kg
                                .map_or("mass unavailable (no material)".into(), |mass| format!(
                                    "{mass:.2} kg"
                                ))
                        ));
                    }
                });
            }
        } else {
            ui.label("Select a placed wall to assign a type.");
        }
        ui.collapsing("Project wall types", |ui| {
            if ui.button("New wall type…").clicked() {
                self.begin_wall_type(None, None);
            }
            for ty in types {
                ui.horizontal(|ui| {
                    ui.label(&ty.parameters.name);
                    if ui.small_button("Edit").clicked() {
                        self.begin_wall_type(Some(ty.clone()), None);
                    }
                    if ui.small_button("Delete").clicked() {
                        let result = self
                            .editor
                            .command("Delete wall type", Command::RemoveWallType(ty.id()));
                        self.report(result, "Wall type deleted.");
                    }
                });
            }
        });
    }
    pub(super) fn wall_type_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.wall_type_draft.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Wall type and material layers").open(&mut open).default_width(650.0).show(ctx, |ui| {
            let users = self.editor.document.model().wall_type_assignments.values().filter(|a| a.type_id==draft.entity.id()).count();
            ui.label(format!("Saving updates all {users} assigned walls together. Ordered from right to left of the start → end axis; flip is per wall."));
            ui.text_edit_singleline(&mut draft.entity.parameters.name);
            let mut action = None;
            let count = draft.entity.parameters.layers.len();
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                for (index, layer) in draft.entity.parameters.layers.iter_mut().enumerate() {
                    ui.push_id(layer.id, |ui| {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("Layer {}", index+1));
                                ui.text_edit_singleline(&mut layer.name);
                                if ui.add_enabled(index>0, egui::Button::new("↑")).clicked() { action=Some((index, -1)); }
                                if ui.add_enabled(index+1<count, egui::Button::new("↓")).clicked() { action=Some((index, 1)); }
                                if ui.add_enabled(count>1, egui::Button::new("Remove")).clicked() { action=Some((index, 0)); }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Thickness");
                                ui.add(egui::DragValue::new(&mut layer.thickness).speed(0.001).range(0.00001..=10.0).suffix(" m"));
                                egui::ComboBox::from_id_salt("function").selected_text(format!("{:?}",layer.function)).show_ui(ui, |ui| {
                                    for function in [LayerFunction::Structure,LayerFunction::Substrate,LayerFunction::Insulation,LayerFunction::Finish,LayerFunction::Other] { ui.selectable_value(&mut layer.function, function, format!("{function:?}")); }
                                });
                                let materials = &draft.materials;
                                egui::ComboBox::from_id_salt("material").selected_text(layer.material.and_then(|id| materials.get(&id)).map_or("No material", |m| m.parameters.name.as_str())).show_ui(ui, |ui| {
                                    ui.selectable_value(&mut layer.material, None, "No material (mass unavailable)");
                                    for (id,m) in materials { ui.selectable_value(&mut layer.material, Some(*id), &m.parameters.name); }
                                });
                            });
                            ui.small(format!("Layer ID: {}",layer.id));
                        });
                    });
                }
            });
            if let Some((index, direction))=action {
                if direction==0 { draft.entity.parameters.layers.remove(index); }
                else { draft.entity.parameters.layers.swap(index, (index as isize + direction) as usize); }
            }
            if ui.add_enabled(count<os_model::MAX_WALL_LAYERS, egui::Button::new("Add layer")).clicked() {
                draft.entity.parameters.layers.push(WallLayer { id: Id::new(), name: "New layer".into(), thickness: 0.01, function: LayerFunction::Finish, material: None });
            }
            ui.label(format!("Total thickness: {:.4} m", draft.entity.parameters.layers.iter().map(|l|l.thickness).sum::<f64>()));
            ui.collapsing("Project materials / density", |ui| {
                ui.label("Density edits update quantities for every use of the material on Save.");
                for material in draft.materials.values_mut() {
                    ui.push_id(material.id(), |ui| { ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut material.parameters.name);
                        ui.add(egui::DragValue::new(&mut material.parameters.density_kg_m3).speed(10.0).range(0.001..=100000.0).suffix(" kg/m³"));
                    }); });
                }
                if ui.button("New project material").clicked() {
                    let material = os_model::Material::new("core.material", os_model::MaterialParams { name: "New material".into(), density_kg_m3: 1000.0 });
                    draft.materials.insert(material.id(), material);
                }
            });
            if let Some(error)=&draft.error { ui.colored_label(theme::ERROR,error); }
            ui.horizontal(|ui| { apply=ui.button("Save type / layers").clicked(); cancel=ui.button("Cancel").clicked(); });
        });
        if apply {
            let result = draft
                .commands(&self.editor.document)
                .and_then(|commands| {
                    self.editor
                        .document
                        .execute("Save wall type / layers", commands)
                })
                .and_then(|()| self.editor.regenerate());
            match result {
                Ok(()) => return,
                Err(error) => draft.error = Some(error.to_string()),
            }
        }
        if open && !cancel {
            self.wall_type_draft = Some(draft);
        }
    }
}
