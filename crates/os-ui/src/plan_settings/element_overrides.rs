use super::*;
use eframe::egui;
use os_document::Command;
use os_model::{Model, PlanElementGraphicsOverride, PlanLinePattern, PlanStrokeStyle};
use std::collections::BTreeMap;

pub(super) struct ElementOverridesDraft {
    pub(super) overrides: BTreeMap<Id, PlanElementGraphicsOverride>,
    selected: Option<Id>,
}

impl ElementOverridesDraft {
    pub(super) fn begin(model: &Model, view: Id) -> Self {
        let overrides = model
            .plan_element_graphics_overrides
            .get(&view)
            .cloned()
            .unwrap_or_default();
        Self {
            selected: overrides.keys().next().copied(),
            overrides,
        }
    }

    pub(super) fn command(&self, model: &Model, view: Id) -> Option<Command> {
        let saved = model
            .plan_element_graphics_overrides
            .get(&view)
            .cloned()
            .unwrap_or_default();
        (saved != self.overrides).then(|| Command::SetPlanElementGraphicsOverrides {
            view,
            overrides: self.overrides.clone(),
        })
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, model: &Model, view: Id) {
        ui.label("Native wall, door/window, and slab strokes. Overrides are view-specific and take precedence over rules, phase and category/template strokes.");
        ui.label("Each role is a complete stroke (color, paper weight, pattern). Turning it off inherits the current appearance. Hidden or cropped assignments are retained.");
        let choices = target_choices(model, view);
        egui::ComboBox::from_id_salt("plan_element_graphics_target")
            .selected_text(
                self.selected
                    .and_then(|id| choices.iter().find(|(candidate, _)| *candidate == id))
                    .map_or("Choose a native element", |(_, label)| label.as_str()),
            )
            .show_ui(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        ui.selectable_value(&mut self.selected, None, "Choose a native element");
                        for (id, label) in &choices {
                            ui.selectable_value(&mut self.selected, Some(*id), label);
                        }
                    });
            });

        let Some(selected) = self.selected else {
            return;
        };
        if !choices.iter().any(|(id, _)| *id == selected) {
            ui.colored_label(
                theme::ERROR,
                "The selected element is no longer eligible in this plan.",
            );
            return;
        }
        let floor = model.floors.contains_key(&selected);
        let mut value =
            self.overrides
                .get(&selected)
                .copied()
                .unwrap_or(PlanElementGraphicsOverride {
                    cut: None,
                    projected: None,
                });

        ui.horizontal(|ui| {
            ui.label("Cut stroke");
            ui.add_enabled_ui(!floor, |ui| {
                role_editor(ui, &mut value.cut, 0.35, "cut");
            });
            if floor {
                ui.small("not applicable to a slab boundary");
            }
        });
        ui.horizontal(|ui| {
            ui.label("Projected stroke");
            role_editor(ui, &mut value.projected, 0.18, "projected");
        });
        if ui
            .button("Reset this element to inherited graphics")
            .clicked()
        {
            self.overrides.remove(&selected);
            return;
        }
        if value.cut.is_some() || value.projected.is_some() {
            self.overrides.insert(selected, value);
        } else {
            self.overrides.remove(&selected);
        }
    }
}

pub(super) fn role_editor(
    ui: &mut egui::Ui,
    value: &mut Option<PlanStrokeStyle>,
    weight: f64,
    salt: &str,
) {
    let mut enabled = value.is_some();
    if ui.checkbox(&mut enabled, "Override").changed() {
        *value = enabled.then_some(PlanStrokeStyle {
            color: [0, 0, 0],
            weight_mm: weight,
            pattern: PlanLinePattern::Solid,
        });
    }
    let Some(style) = value else {
        return;
    };
    ui.color_edit_button_srgb(&mut style.color);
    ui.add(
        egui::DragValue::new(&mut style.weight_mm)
            .range(0.05..=2.0)
            .speed(0.01)
            .suffix(" mm"),
    );
    egui::ComboBox::from_id_salt(("element_override_pattern", salt))
        .selected_text(match style.pattern {
            PlanLinePattern::Solid => "Solid",
            PlanLinePattern::Dashed => "Dashed",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut style.pattern, PlanLinePattern::Solid, "Solid");
            ui.selectable_value(&mut style.pattern, PlanLinePattern::Dashed, "Dashed");
        });
}

fn target_choices(model: &Model, view: Id) -> Vec<(Id, String)> {
    let mut choices = Vec::new();
    for wall in model.walls.values() {
        if model.plan_element_graphics_compatible(view, wall.id()) {
            choices.push((
                wall.id(),
                format!("Wall · {} · {}", wall.parameters.name, wall.id()),
            ));
        }
    }
    for opening in model.openings.values() {
        if model.plan_element_graphics_compatible(view, opening.id()) {
            let kind = model
                .resolve_opening(&opening.parameters)
                .map(|resolved| match resolved.kind {
                    os_model::OpeningKind::Door => "Door",
                    os_model::OpeningKind::Window => "Window",
                })
                .unwrap_or("Opening");
            choices.push((
                opening.id(),
                format!("{kind} · {} · {}", opening.parameters.name, opening.id()),
            ));
        }
    }
    for floor in model.floors.values() {
        if model.plan_element_graphics_compatible(view, floor.id()) {
            choices.push((
                floor.id(),
                format!("Slab · {} · {}", floor.parameters.name, floor.id()),
            ));
        }
    }
    choices.sort_by(|a, b| a.1.cmp(&b.1));
    choices
}
