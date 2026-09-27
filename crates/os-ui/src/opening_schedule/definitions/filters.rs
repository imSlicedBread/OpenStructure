use crate::egui;
use os_model::{
    MAX_SCHEDULE_FILTERS, ScheduleFilter, ScheduleNumericField, ScheduleNumericOperator,
    ScheduleParams, ScheduleTextField, ScheduleTextOperator,
};

fn choice<T: Copy + PartialEq>(ui: &mut egui::Ui, id: &str, value: &mut T, choices: &[(T, &str)]) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(choices.iter().find(|(v, _)| v == value).unwrap().1)
        .show_ui(ui, |ui| {
            for &(option, label) in choices {
                ui.selectable_value(value, option, label);
            }
        });
}

pub(super) fn controls(ui: &mut egui::Ui, parameters: &mut ScheduleParams) {
    ui.label("Filters · all rules must match (AND); no rules includes all category rows.");
    ui.label("Text ignores case using Rust lowercase; no accent or Unicode normalization.");
    ui.label("Numbers compare full precision in metres, before cell rounding.");
    let mut remove = None;
    let mut move_up = None;
    // Bound the editor height so Save/Cancel remain accessible with many rules.
    egui::ScrollArea::vertical()
        .id_salt("schedule_filters")
        .max_height(180.0)
        .show(ui, |ui| {
            for (index, filter) in parameters.filters.iter_mut().enumerate() {
                ui.push_id(("filter", index), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{}.", index + 1));
                        let mut numeric = matches!(filter, ScheduleFilter::Numeric { .. });
                        let previous = numeric;
                        choice(
                            ui,
                            "kind",
                            &mut numeric,
                            &[(false, "Text"), (true, "Number")],
                        );
                        if numeric != previous {
                            *filter = if numeric {
                                ScheduleFilter::Numeric {
                                    field: ScheduleNumericField::Width,
                                    operator: ScheduleNumericOperator::GreaterOrEqual,
                                    value: 0.0,
                                }
                            } else {
                                ScheduleFilter::Text {
                                    field: ScheduleTextField::Name,
                                    operator: ScheduleTextOperator::Contains,
                                    value: String::new(),
                                }
                            };
                        }
                        match filter {
                            ScheduleFilter::Text {
                                field,
                                operator,
                                value,
                            } => {
                                choice(
                                    ui,
                                    "field",
                                    field,
                                    &[
                                        (ScheduleTextField::Name, "Name"),
                                        (ScheduleTextField::Type, "Type"),
                                        (ScheduleTextField::Level, "Level"),
                                        (ScheduleTextField::Host, "Host"),
                                    ],
                                );
                                choice(
                                    ui,
                                    "operator",
                                    operator,
                                    &[
                                        (ScheduleTextOperator::Equals, "Equals"),
                                        (ScheduleTextOperator::NotEquals, "Not equals"),
                                        (ScheduleTextOperator::Contains, "Contains"),
                                        (ScheduleTextOperator::StartsWith, "Starts with"),
                                    ],
                                );
                                ui.add(
                                    egui::TextEdit::singleline(value)
                                        .desired_width(140.0)
                                        .hint_text("Filter text"),
                                );
                            }
                            ScheduleFilter::Numeric {
                                field,
                                operator,
                                value,
                            } => {
                                choice(
                                    ui,
                                    "field",
                                    field,
                                    &[
                                        (ScheduleNumericField::Width, "Width"),
                                        (ScheduleNumericField::Height, "Height"),
                                        (ScheduleNumericField::Sill, "Sill"),
                                    ],
                                );
                                choice(
                                    ui,
                                    "operator",
                                    operator,
                                    &[
                                        (ScheduleNumericOperator::Equals, "Equals"),
                                        (ScheduleNumericOperator::NotEquals, "Not equals"),
                                        (ScheduleNumericOperator::Less, "Less"),
                                        (ScheduleNumericOperator::LessOrEqual, "Less or equal"),
                                        (ScheduleNumericOperator::Greater, "Greater"),
                                        (
                                            ScheduleNumericOperator::GreaterOrEqual,
                                            "Greater or equal",
                                        ),
                                    ],
                                );
                                ui.add(
                                    egui::DragValue::new(value)
                                        .speed(0.01)
                                        .max_decimals(15)
                                        .suffix(" m"),
                                );
                            }
                        }
                        if ui
                            .add_enabled(index > 0, egui::Button::new(format!("↑ {}", index + 1)))
                            .on_hover_text("Move filter up")
                            .clicked()
                        {
                            move_up = Some(index);
                        }
                        if ui.button("Remove filter").clicked() {
                            remove = Some(index);
                        }
                    });
                });
            }
        });
    if let Some(index) = remove {
        parameters.filters.remove(index);
    } else if let Some(index) = move_up {
        parameters.filters.swap(index, index - 1);
    }
    if ui
        .add_enabled(
            parameters.filters.len() < MAX_SCHEDULE_FILTERS,
            egui::Button::new("Add filter"),
        )
        .clicked()
    {
        parameters.filters.push(ScheduleFilter::Text {
            field: ScheduleTextField::Name,
            operator: ScheduleTextOperator::Contains,
            value: String::new(),
        });
    }
}
