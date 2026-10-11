//! Staged project materials and Section cut hatches; one revision-bound Apply.
use super::*;
use os_core::ensure;
use os_model::{Material, MaterialParams, MaterialPattern, MaterialPatternParams};
use std::collections::BTreeMap;

pub(super) struct MaterialGraphicsDraft {
    session: Id,
    revision: u64,
    materials: BTreeMap<Id, Material>,
    patterns: BTreeMap<Id, MaterialPattern>,
    error: Option<String>,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    fn frame(
        app: &mut DesktopApp,
        ctx: &egui::Context,
        size: egui::Vec2,
        scale: f32,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(scale);
        ctx.run(input, |ctx| app.material_graphics_dialog(ctx))
    }
    fn click(app: &mut DesktopApp, ctx: &egui::Context, size: egui::Vec2, scale: f32, label: &str) {
        frame(app, ctx, size, scale, vec![]);
        let output = frame(app, ctx, size, scale, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"));
        frame(
            app,
            ctx,
            size,
            scale,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            app,
            ctx,
            size,
            scale,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn material_graphics_staged_apply_cancel_history_stale_and_section_derivation_at_both_dpis() {
        for (size, scale) in [
            (egui::vec2(1280., 800.), 1.),
            (egui::vec2(1000., 650.), 1.5),
        ] {
            let mut app = DesktopApp::new().unwrap();
            let m = Material::new(
                "core.material",
                MaterialParams {
                    name: "Brick".into(),
                    density_kg_m3: 1800.,
                    color: [180; 3],
                    cut_pattern: None,
                },
            );
            let mid = m.id();
            app.editor
                .command("Material", Command::AddMaterial(m))
                .unwrap();
            app.draft.material = Some(mid);
            app.apply_wall();
            let view = app
                .editor
                .create_section_view(
                    "Cut",
                    app.active_level,
                    os_model::SectionViewSettings::new(
                        Point2::new(-1., 0.),
                        Point2::new(6., 0.),
                        -1.,
                        4.,
                    ),
                )
                .unwrap();
            let before = app.editor.document.model().clone();
            let scene = app.editor.scene.clone();
            let history = app.editor.document.history_stats();
            app.begin_material_graphics();
            let p = MaterialPattern::new(
                "core.material_pattern",
                MaterialPatternParams {
                    name: "Brick hatch".into(),
                    angle_degrees: 45.,
                    spacing: 0.1,
                    origin: [0.; 3],
                },
            );
            let pid = p.id();
            let draft = app.material_graphics_draft.as_mut().unwrap();
            draft.patterns.insert(pid, p.clone());
            draft
                .materials
                .get_mut(&mid)
                .unwrap()
                .parameters
                .cut_pattern = Some(pid);
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.scene, scene);
            assert_eq!(app.editor.document.history_stats(), history);
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            click(&mut app, &ctx, size, scale, "Apply materials / patterns");
            assert!(app.material_graphics_draft.is_none());
            let after = app.editor.document.model().clone();
            assert_eq!(after.materials[&mid].parameters.cut_pattern, Some(pid));
            let drawing = app.editor.section_snapshot(view).unwrap().derive().unwrap();
            let context = app.editor.native_section_context(view).unwrap();
            assert!(!drawing.section_hatches(context).unwrap().is_empty());
            app.editor.undo().unwrap();
            assert_eq!(app.editor.document.model(), &before);
            app.editor.redo().unwrap();
            assert_eq!(app.editor.document.model(), &after);
            app.begin_material_graphics();
            app.material_graphics_draft
                .as_mut()
                .unwrap()
                .patterns
                .get_mut(&pid)
                .unwrap()
                .parameters
                .spacing = 0.;
            let invalid = app
                .material_graphics_draft
                .as_ref()
                .unwrap()
                .commands(&app.editor.document)
                .unwrap();
            assert!(
                app.editor
                    .document
                    .execute("Invalid hatch", invalid)
                    .is_err()
            );
            assert_eq!(app.editor.document.model(), &after);
            click(&mut app, &ctx, size, scale, "Cancel");
            assert!(app.material_graphics_draft.is_none());
            app.begin_material_graphics();
            let stale = app.material_graphics_draft.take().unwrap();
            app.editor.undo().unwrap();
            assert!(stale.commands(&app.editor.document).is_err());
            app.material_graphics_draft = Some(stale);
            frame(&mut app, &ctx, size, scale, vec![]);
            assert!(app.material_graphics_draft.is_none());
            assert_eq!(app.editor.document.model(), &before);
            app.begin_material_graphics();
            frame(
                &mut app,
                &ctx,
                size,
                scale,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(app.material_graphics_draft.is_none());
            assert_eq!(ctx.pixels_per_point(), scale);
        }
    }
}

impl MaterialGraphicsDraft {
    fn new(document: &Document) -> Self {
        Self {
            session: document.session_id(),
            revision: document.revision(),
            materials: document.model().materials.clone(),
            patterns: document.model().material_patterns.clone(),
            error: None,
        }
    }
    fn commands(&self, document: &Document) -> Result<Vec<Command>> {
        ensure(
            self.session == document.session_id() && self.revision == document.revision(),
            "Document changed; close and reopen Project materials",
        )?;
        let mut commands = Vec::new();
        for (id, pattern) in &self.patterns {
            match document.model().material_patterns.get(id) {
                None => commands.push(Command::AddMaterialPattern(pattern.clone())),
                Some(old) if old != pattern => commands.push(Command::UpdateMaterialPattern {
                    id: *id,
                    parameters: pattern.parameters.clone(),
                }),
                _ => {}
            }
        }
        for id in document
            .model()
            .material_patterns
            .keys()
            .filter(|id| !self.patterns.contains_key(id))
        {
            commands.push(Command::RemoveMaterialPattern(*id));
        }
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
        Ok(commands)
    }
}

impl DesktopApp {
    pub(super) fn begin_material_graphics(&mut self) {
        self.material_graphics_draft = Some(MaterialGraphicsDraft::new(&self.editor.document));
    }
    pub(super) fn material_graphics_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.material_graphics_draft.take() else {
            return;
        };
        // Close stale drafts, including document replacement and undo/redo.
        if draft.session != self.editor.document.session_id()
            || draft.revision != self.editor.document.revision()
        {
            self.report(
                Err(Error::Invalid(
                    "Project materials draft closed because the document changed".into(),
                )),
                "",
            );
            return;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Window::new("Project materials — Section cut hatches").open(&mut open)
            .default_width(650.0).show(ctx, |ui| {
                ui.label("Changes are staged until Apply. Shared material edits affect every use in this project.");
                egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                    for material in draft.materials.values_mut() {
                        ui.push_id(material.id(), |ui| { ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut material.parameters.name);
                                ui.color_edit_button_srgb(&mut material.parameters.color);
                                ui.add(egui::DragValue::new(&mut material.parameters.density_kg_m3)
                                    .range(0.001..=100000.0).suffix(" kg/m³"));
                            });
                            egui::ComboBox::from_id_salt("cut_pattern").selected_text(
                                material.parameters.cut_pattern.and_then(|id| draft.patterns.get(&id))
                                    .map_or("Solid cut fill", |p| p.parameters.name.as_str()))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut material.parameters.cut_pattern, None, "Solid cut fill");
                                    for (id, pattern) in &draft.patterns {
                                        ui.selectable_value(&mut material.parameters.cut_pattern, Some(*id), &pattern.parameters.name);
                                    }
                                });
                        }); });
                    }
                    if ui.button("New project material").clicked() {
                        let material = Material::new("core.material", MaterialParams { name: "New material".into(),
                            density_kg_m3: 1000.0, color: [180;3], cut_pattern: None });
                        draft.materials.insert(material.id(), material);
                    }
                    ui.separator();
                    ui.label("Section cut patterns · parallel lines · model metres");
                    ui.label("Angle: from section horizontal toward elevation. Origin: world X, Y, Z, shared across components and views.");
                    let mut remove = None;
                    for (id, pattern) in &mut draft.patterns {
                        ui.push_id(*id, |ui| { ui.group(|ui| {
                            ui.text_edit_singleline(&mut pattern.parameters.name);
                            ui.horizontal(|ui| {
                                ui.label("Angle");
                                ui.add(egui::DragValue::new(&mut pattern.parameters.angle_degrees).range(0.0..=179.999).suffix("°"));
                                ui.label("Spacing");
                                ui.add(egui::DragValue::new(&mut pattern.parameters.spacing).speed(0.001).range(0.001..=1000.0).suffix(" m"));
                            });
                            ui.horizontal(|ui| {
                                for (axis, value) in ["X", "Y", "Z"].into_iter().zip(&mut pattern.parameters.origin) {
                                    ui.label(axis);
                                    ui.add(egui::DragValue::new(value).speed(0.01).range(-1_000_000.0..=1_000_000.0).suffix(" m"));
                                }
                            });
                            let count = draft.materials.values().filter(|m| m.parameters.cut_pattern == Some(*id)).count();
                            ui.label(format!("Assigned to {count} material(s). Spacing stays fixed in the model at every view scale."));
                            if ui.small_button("Remove pattern and use solid fills").clicked() { remove = Some(*id); }
                        }); });
                    }
                    if let Some(id) = remove {
                        draft.patterns.remove(&id);
                        for material in draft.materials.values_mut() {
                            if material.parameters.cut_pattern == Some(id) { material.parameters.cut_pattern = None; }
                        }
                    }
                    if ui.add_enabled(draft.patterns.len() < os_model::MAX_MATERIAL_PATTERNS,
                        egui::Button::new("New cut pattern")).clicked() {
                        let pattern = MaterialPattern::new("core.material_pattern", MaterialPatternParams {
                            name: "Parallel hatch".into(), angle_degrees: 45.0, spacing: 0.1, origin: [0.0;3] });
                        draft.patterns.insert(pattern.id(), pattern);
                    }
                });
                if let Some(error) = &draft.error { ui.colored_label(theme::ERROR, error); }
                ui.horizontal(|ui| { apply = ui.button("Apply materials / patterns").clicked(); cancel |= ui.button("Cancel").clicked(); });
            });
        if apply && !cancel {
            let result = draft.commands(&self.editor.document).and_then(|commands| {
                if commands.is_empty() {
                    Ok(())
                } else {
                    self.editor
                        .document
                        .execute("Apply materials / patterns", commands)
                }
            });
            match result {
                Ok(()) => {
                    let result = self.editor.regenerate();
                    self.report(result, "Project materials applied.");
                    return;
                }
                Err(error) => draft.error = Some(error.to_string()),
            }
        }
        if open && !cancel {
            self.material_graphics_draft = Some(draft);
        }
    }
}
