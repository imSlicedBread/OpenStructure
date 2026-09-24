use eframe::egui;
use os_core::Id;
use os_document::Command;
use os_model::{
    Model, PlanGraphicsBinding, PlanGraphicsStyles, PlanGraphicsTemplate,
    PlanGraphicsTemplateParams, PlanLinePattern,
};
use std::collections::BTreeMap;

pub(super) struct GraphicsDraft {
    pub(super) templates: BTreeMap<Id, PlanGraphicsTemplate>,
    pub(super) binding: Option<PlanGraphicsBinding>,
}
impl GraphicsDraft {
    pub(super) fn begin(model: &Model, view: Id) -> Self {
        Self {
            templates: model.plan_graphics_templates.clone(),
            binding: model.plan_graphics.get(&view).copied(),
        }
    }
    pub(super) fn commands(&self, model: &Model, view: Id) -> Vec<Command> {
        let mut commands = Vec::new();
        for (id, template) in &self.templates {
            match model.plan_graphics_templates.get(id) {
                None => commands.push(Command::AddPlanGraphicsTemplate(template.clone())),
                Some(old) if old != template => {
                    commands.push(Command::UpdatePlanGraphicsTemplate {
                        id: *id,
                        parameters: template.parameters.clone(),
                    })
                }
                _ => {}
            }
        }
        if model.plan_graphics.get(&view).copied() != self.binding {
            commands.push(Command::SetPlanGraphics {
                view,
                binding: self.binding,
            });
        }
        for id in model
            .plan_graphics_templates
            .keys()
            .filter(|id| !self.templates.contains_key(id))
        {
            commands.push(Command::RemovePlanGraphicsTemplate(*id));
        }
        commands
    }
    pub(super) fn create_template(&mut self) {
        let styles = self.binding.map_or_else(PlanGraphicsStyles::default, |b| {
            b.template
                .and_then(|id| self.templates.get(&id))
                .map_or(b.local, |t| t.parameters.styles)
        });
        let template = PlanGraphicsTemplate::new(
            "core.plan_graphics_template",
            PlanGraphicsTemplateParams {
                name: "New graphics template".into(),
                styles,
            },
        );
        self.binding.get_or_insert_with(Default::default).template = Some(template.id());
        self.templates.insert(template.id(), template);
    }
    pub(super) fn ui(&mut self, ui: &mut egui::Ui, model: &Model, view: Id) {
        ui.label("Native wall, door/window, and slab strokes flow to the plan canvas and sheet/PDF output. Selection accents and section/provider graphics keep their own styles.");
        ui.label("A linked template controls all four categories. Unlinking restores this view's local styles. Apply saves all changes together; Cancel discards them.");
        let mut enabled = self.binding.is_some();
        if ui
            .checkbox(&mut enabled, "Store graphics standards for this plan")
            .changed()
        {
            self.binding = enabled.then(PlanGraphicsBinding::default);
        }
        if ui
            .add_enabled(
                self.templates.len() < 256,
                egui::Button::new("New template and link"),
            )
            .clicked()
        {
            self.create_template();
        }
        let Some(binding) = &mut self.binding else {
            return;
        };
        egui::ComboBox::from_id_salt("plan_graphics_template")
            .selected_text(
                binding
                    .template
                    .and_then(|id| self.templates.get(&id))
                    .map_or("Local view styles", |t| t.parameters.name.as_str()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut binding.template, None, "Local view styles (unlink)");
                for (id, template) in &self.templates {
                    ui.selectable_value(
                        &mut binding.template,
                        Some(*id),
                        &template.parameters.name,
                    );
                }
            });
        if let Some(id) = binding.template {
            let others = model
                .plan_graphics
                .iter()
                .filter(|(other, b)| **other != view && b.template == Some(id))
                .count();
            ui.label(format!(
                "Shared template: edits also affect {others} other linked plan(s)."
            ));
            if ui
                .add_enabled(
                    others == 0,
                    egui::Button::new("Unlink here and delete template"),
                )
                .clicked()
            {
                self.templates.remove(&id);
                binding.template = None;
            }
        }
        let styles = if let Some(id) = binding.template {
            let template = self.templates.get_mut(&id).expect("draft template link");
            ui.label("Template name");
            ui.add(egui::TextEdit::singleline(&mut template.parameters.name).char_limit(256));
            &mut template.parameters.styles
        } else {
            &mut binding.local
        };
        for (name, category) in [
            ("Walls", &mut styles.walls),
            ("Doors", &mut styles.doors),
            ("Windows", &mut styles.windows),
            ("Slabs", &mut styles.slabs),
        ] {
            ui.push_id(name, |ui| {
                ui.label(name);
                for (role, stroke) in [
                    ("Cut", &mut category.cut),
                    ("Projected", &mut category.projected),
                ] {
                    ui.push_id(role, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(role);
                            ui.color_edit_button_srgb(&mut stroke.color);
                            ui.add(
                                egui::DragValue::new(&mut stroke.weight_mm)
                                    .range(0.05..=2.0)
                                    .speed(0.01)
                                    .suffix(" mm"),
                            );
                            egui::ComboBox::from_id_salt("pattern")
                                .selected_text(format!("{:?}", stroke.pattern))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut stroke.pattern,
                                        PlanLinePattern::Solid,
                                        "Solid",
                                    );
                                    ui.selectable_value(
                                        &mut stroke.pattern,
                                        PlanLinePattern::Dashed,
                                        "Dashed",
                                    );
                                });
                        });
                    });
                }
            });
        }
    }
}
