use super::*;
use os_model::{
    Model, PhaseStatus, PlanGraphicsPredicate as Predicate, PlanGraphicsRule,
    PlanRuleCategory as Category, PlanRuleNumberField as NumberField, PlanRuleNumberOp as NumberOp,
    PlanRulePhaseOp as PhaseOp, PlanRuleTextField as TextField, PlanRuleTextOp as TextOp,
};

pub(super) struct RulesDraft {
    pub(super) rules: Vec<PlanGraphicsRule>,
}

impl RulesDraft {
    pub(super) fn begin(model: &Model, view: Id) -> Self {
        Self {
            rules: model
                .plan_graphics_rules
                .get(&view)
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub(super) fn command(&self, model: &Model, view: Id) -> Option<Command> {
        let saved = model
            .plan_graphics_rules
            .get(&view)
            .map(Vec::as_slice)
            .unwrap_or_default();
        (saved != self.rules).then(|| Command::SetPlanGraphicsRules {
            view,
            rules: self.rules.clone(),
        })
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Highest priority first. All conditions must match. Each stroke inherits when omitted; explicit element overrides take precedence.");
        ui.small("Text is case-sensitive; dimensions are effective metres. Rules update automatically when the model changes.");
        if ui
            .add_enabled(
                self.rules.len() < os_model::MAX_PLAN_GRAPHICS_RULES_PER_VIEW,
                egui::Button::new("Add graphics rule"),
            )
            .clicked()
        {
            self.rules.push(PlanGraphicsRule {
                id: Id::new(),
                name: "New rule".into(),
                enabled: true,
                category: Category::Walls,
                predicates: vec![default_predicate()],
                cut: None,
                projected: Some(os_model::PlanStrokeStyle {
                    color: [40, 90, 180],
                    weight_mm: 0.35,
                    pattern: os_model::PlanLinePattern::Solid,
                }),
            });
        }
        let count = self.rules.len();
        let mut remove = None;
        let mut swap = None;
        for (index, rule) in self.rules.iter_mut().enumerate() {
            ui.push_id(rule.id, |ui| {
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut rule.enabled, "Enabled");
                        ui.add(
                            egui::TextEdit::singleline(&mut rule.name)
                                .desired_width(140.0)
                                .char_limit(256),
                        );
                        if index > 0 && ui.button("Up").clicked() {
                            swap = Some((index, index - 1));
                        }
                        if index + 1 < count && ui.button("Down").clicked() {
                            swap = Some((index, index + 1));
                        }
                        if ui.button("Remove").clicked() {
                            remove = Some(index);
                        }
                    });
                    let old_category = rule.category;
                    choice(
                        ui,
                        "Category",
                        &mut rule.category,
                        &[
                            (Category::Walls, "Walls"),
                            (Category::Doors, "Doors"),
                            (Category::Windows, "Windows"),
                            (Category::Slabs, "Slabs"),
                        ],
                    );
                    if old_category != rule.category {
                        rule.predicates.retain(|p| match p {
                            Predicate::Number { field, .. } => {
                                field.category_compatible(rule.category)
                            }
                            Predicate::Text {
                                field: TextField::TypeName,
                                ..
                            } => rule.category != Category::Slabs,
                            _ => true,
                        });
                        if rule.predicates.is_empty() {
                            rule.predicates.push(default_predicate());
                        }
                        if rule.category == Category::Slabs {
                            rule.cut = None;
                        }
                    }
                    let mut delete_predicate = None;
                    let predicate_count = rule.predicates.len();
                    for (row, predicate) in rule.predicates.iter_mut().enumerate() {
                        ui.push_id(row, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                predicate_editor(ui, predicate, rule.category);
                                if ui
                                    .add_enabled(
                                        predicate_count > 1,
                                        egui::Button::new("Remove condition"),
                                    )
                                    .clicked()
                                {
                                    delete_predicate = Some(row);
                                }
                            });
                        });
                    }
                    if let Some(row) = delete_predicate {
                        rule.predicates.remove(row);
                    }
                    if ui
                        .add_enabled(
                            rule.predicates.len() < os_model::MAX_PLAN_GRAPHICS_PREDICATES,
                            egui::Button::new("Add AND condition"),
                        )
                        .clicked()
                    {
                        rule.predicates.push(default_predicate());
                    }
                    if rule.category != Category::Slabs {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Cut");
                            super::element_overrides::role_editor(ui, &mut rule.cut, 0.35, "cut");
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Projected / depth");
                        super::element_overrides::role_editor(
                            ui,
                            &mut rule.projected,
                            0.18,
                            "projected",
                        );
                    });
                    if let Err(error) = rule.validate() {
                        ui.colored_label(theme::ERROR, error.to_string());
                    }
                });
            });
        }
        if let Some(index) = remove {
            self.rules.remove(index);
        } else if let Some((from, to)) = swap {
            self.rules.swap(from, to);
        }
    }
}

fn default_predicate() -> Predicate {
    Predicate::Text {
        field: TextField::InstanceName,
        op: TextOp::Contains,
        value: String::new(),
    }
}

fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    salt: &str,
    current: &mut T,
    choices: &[(T, &str)],
) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(
            choices
                .iter()
                .find(|(v, _)| v == current)
                .map_or("Choose", |(_, label)| *label),
        )
        .show_ui(ui, |ui| {
            for (value, label) in choices {
                ui.selectable_value(current, *value, *label);
            }
        });
}

fn predicate_editor(ui: &mut egui::Ui, predicate: &mut Predicate, category: Category) {
    let number_fields: Vec<_> = [
        (NumberField::WallLength, "Wall length"),
        (NumberField::WallHeight, "Wall height"),
        (NumberField::WallThickness, "Wall thickness"),
        (NumberField::OpeningWidth, "Opening width"),
        (NumberField::OpeningHeight, "Opening height"),
        (NumberField::OpeningSill, "Opening sill"),
        (NumberField::SlabThickness, "Slab thickness"),
        (NumberField::SlabTopOffset, "Slab top offset"),
    ]
    .into_iter()
    .filter(|(field, _)| field.category_compatible(category))
    .collect();
    let old_kind = match predicate {
        Predicate::Text { .. } => 0,
        Predicate::Number { .. } => 1,
        Predicate::Phase { .. } => 2,
    };
    let mut kind = old_kind;
    choice(
        ui,
        "Condition",
        &mut kind,
        &[(0, "Text"), (1, "Dimension"), (2, "Phase status")],
    );
    if kind != old_kind {
        *predicate = match kind {
            1 => Predicate::Number {
                field: number_fields[0].0,
                op: NumberOp::GreaterOrEqual,
                value: 0.0,
            },
            2 => Predicate::Phase {
                op: PhaseOp::Equals,
                value: PhaseStatus::New,
            },
            _ => default_predicate(),
        };
    }
    match predicate {
        Predicate::Text { field, op, value } => {
            let fields = [
                (TextField::InstanceName, "Instance name"),
                (TextField::TypeName, "Effective type name"),
            ];
            choice(
                ui,
                "Field",
                field,
                if category == Category::Slabs {
                    &fields[..1]
                } else {
                    &fields
                },
            );
            choice(
                ui,
                "Operator",
                op,
                &[
                    (TextOp::Equals, "Equals"),
                    (TextOp::NotEquals, "Does not equal"),
                    (TextOp::Contains, "Contains"),
                ],
            );
            ui.add(
                egui::TextEdit::singleline(value)
                    .desired_width(120.0)
                    .char_limit(256),
            );
        }
        Predicate::Number { field, op, value } => {
            choice(ui, "Field", field, &number_fields);
            choice(
                ui,
                "Operator",
                op,
                &[
                    (NumberOp::Equals, "="),
                    (NumberOp::NotEquals, "≠"),
                    (NumberOp::Less, "<"),
                    (NumberOp::LessOrEqual, "≤"),
                    (NumberOp::Greater, ">"),
                    (NumberOp::GreaterOrEqual, "≥"),
                ],
            );
            ui.add(egui::DragValue::new(value).speed(0.01).suffix(" m"));
        }
        Predicate::Phase { op, value } => {
            choice(
                ui,
                "Operator",
                op,
                &[
                    (PhaseOp::Equals, "Equals"),
                    (PhaseOp::NotEquals, "Does not equal"),
                ],
            );
            choice(
                ui,
                "Status",
                value,
                &[
                    (PhaseStatus::Existing, "Existing"),
                    (PhaseStatus::New, "New"),
                    (PhaseStatus::Demolished, "Demolished"),
                    (PhaseStatus::Temporary, "Temporary"),
                    (PhaseStatus::Future, "Future"),
                    (PhaseStatus::PreviouslyDemolished, "Previously demolished"),
                ],
            );
        }
    }
}
