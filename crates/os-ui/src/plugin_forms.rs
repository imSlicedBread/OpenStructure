//! Host-rendered descriptor form. Draft strings preserve invalid partial input.
use crate::{
    DesktopApp,
    plugin_tools::{ToolContext, ToolDraft},
};
use eframe::egui;
use os_plugin_api::generic::{FieldKind, Mode, Unit};
use os_plugin_host::worker::ViewContext;
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

#[derive(Default)]
pub(super) struct FormState {
    from_gesture: bool,
    view: Option<ViewContext>,
    view_revoked: bool,
    draft: Option<ToolDraft>,
    text: BTreeMap<String, String>,
    migration: Option<crate::migration_tools::MigrationDraft>,
    migration_text: BTreeMap<String, BTreeMap<String, String>>,
}
impl DesktopApp {
    pub(super) fn submit_installed_wall(
        &mut self,
        draft: ToolDraft,
        context: ToolContext,
        view: ViewContext,
    ) -> os_core::Result<()> {
        // Admission failure leaves the live gesture intact. On admission, the
        // existing form owns retryable values and pending/cancel presentation.
        self.editor.start_plugin_tool(&draft, context, Some(view))?;
        self.plugin_form = FormState {
            from_gesture: true,
            view: Some(view),
            text: draft
                .values()
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
            draft: Some(draft),
            ..Default::default()
        };
        self.cancel_plan_wall();
        Ok(())
    }
    fn plugin_view_context(&self) -> Option<ViewContext> {
        self.plans.active.and_then(|id| {
            self.editor
                .document
                .model()
                .views
                .get(&id)
                .map(|view| ViewContext {
                    id,
                    settings_revision: view.parameters.settings_revision,
                })
        })
    }
    pub(super) fn review_wall_plugin_command(&mut self, mode: Mode) -> bool {
        let Some(catalog) = self.editor.host.catalog(os_plugin_api::wall::OWNER) else {
            return false;
        };
        let Some(command) = catalog.commands.iter().find(|c| c.mode == mode).cloned() else {
            self.report(
                Err(os_core::Error::Invalid(
                    "Wall provider has no matching registered command.".into(),
                )),
                "",
            );
            return true;
        };
        let context = ToolContext {
            level: self.active_level,
            selection: if mode == Mode::Create {
                None
            } else {
                self.selected
            },
        };
        match ToolDraft::begin(
            &self.editor.host,
            &self.editor.document,
            os_plugin_api::wall::OWNER,
            &command.id,
            context,
        ) {
            Ok(draft) => {
                self.plugin_form.view = self.plugin_view_context();
                self.plugin_form.view_revoked = false;
                self.plugin_form.text = draft
                    .values()
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string()),
                        )
                    })
                    .collect();
                self.plugin_form.draft = Some(draft);
                self.report(
                    Ok(()),
                    "Review the registered Wall form, then Apply plugin command.",
                );
            }
            Err(error) => self.report(Err(error), ""),
        }
        true
    }
    pub(super) fn plugin_forms(&mut self, ctx: &egui::Context) {
        if self.plugin_form.from_gesture && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.editor.cancel_plugin_tool();
        }
        let view = self.plugin_view_context();
        if (self.plugin_form.draft.is_some() || self.plugin_form.migration.is_some())
            && self.plugin_form.view != view
        {
            self.plugin_form.view_revoked = true;
        }
        if self.plugin_form.view_revoked {
            self.editor.cancel_plugin_tool();
        }
        if self.pending.is_some() || self.exchange_pending.is_some() {
            self.editor.cancel_plugin_tool();
        }
        let commands: Vec<_> = self
            .editor
            .host
            .manifests()
            .flat_map(|m| {
                self.editor
                    .host
                    .catalog(&m.id)
                    .into_iter()
                    .flat_map(|c| c.commands.iter().map(|d| (m.id.clone(), d.clone())))
            })
            .collect();
        let context = ToolContext {
            level: self.active_level,
            selection: self.selected,
        };
        let poll_context = ToolContext {
            selection: if self
                .plugin_form
                .draft
                .as_ref()
                .is_some_and(|d| d.descriptor().mode == Mode::Create)
            {
                None
            } else {
                context.selection
            },
            ..context
        };
        match self.editor.poll_plugin_work(poll_context, view) {
            Ok(report) => {
                if report.committed {
                    self.plugin_form.from_gesture = false;
                    self.plugin_form.draft = None;
                    self.plugin_form.migration = None;
                    self.refresh_document();
                    self.report(Ok(()), "Plugin edit committed.");
                }
                if let Some(error) = report.errors.first() {
                    self.report(Err(os_core::Error::Invalid(error.clone())), "");
                }
                if report.busy {
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
            }
            Err(error) => self.report(Err(error), ""),
        }
        if commands.is_empty() {
            return;
        }
        // Prefer lower-right placement so the floating palette leaves plan actions usable.
        let content = ctx.content_rect();
        let default_position = egui::pos2(
            (content.right() - 350.0).max(content.left()),
            (content.bottom() - 340.0).max(content.top()),
        );
        egui::Window::new("Plugin tools")
            .collapsible(!self.plugin_form.from_gesture)
            .default_pos(default_position)
            .default_width(330.0)
            .vscroll(true)
            .show(ctx, |ui| {
                let busy = self.editor.plugin_work_pending();
                if busy {
                    ui.label("Plugin work pending…");
                    if let Some((done, total)) = self.editor.plugin_migration_progress() {
                        ui.label(format!("Migration: {done}/{total} staged; live document unchanged until commit."));
                    }
                    if ui.button("Cancel plugin command").clicked() {
                        self.editor.cancel_plugin_tool();
                    }
                }
                if self.plugin_form.view_revoked {
                    ui.colored_label(crate::theme::ERROR,
                        "View changed; select the command again to review a new draft. Values below are retained but cannot be applied.");
                }
                for error in self.editor.plugin_geometry_errors().values() {
                    ui.colored_label(crate::theme::ERROR, error);
                }
                if !self.editor.plugin_geometry_errors().is_empty()
                    && ui.button("Retry plugin geometry").clicked()
                {
                    self.editor.retry_plugin_geometry();
                    ctx.request_repaint();
                }
                ui.add_enabled_ui(
                    !busy && self.pending.is_none() && self.exchange_pending.is_none(),
                    |ui| {
                        ui.label("Target element");
                        let mut target = self.selected;
                        egui::ComboBox::from_id_salt("plugin_target")
                            .selected_text(
                                self.selected
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "New element".into()),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut target, None, "New element");
                                for (id, wall) in &self.editor.document.model().walls {
                                    ui.selectable_value(
                                        &mut target,
                                        Some(*id),
                                        &wall.parameters.name,
                                    );
                                }
                                for (id, entity) in &self.editor.document.model().extensions {
                                    ui.selectable_value(&mut target, Some(*id), &entity.name);
                                }
                            });
                        if target != self.selected {
                            self.select(target);
                        }
                        ui.label("Registered commands");
                        let owners: std::collections::BTreeSet<_> = commands.iter().filter(|(_, c)| c.mode == Mode::Migrate).map(|(owner, _)| owner).collect();
                        for owner in owners {
                            if ui.button(format!("Review all {owner} migrations…")).clicked() {
                                match crate::migration_tools::MigrationDraft::begin(&self.editor.host, &self.editor.document, owner, context) {
                                    Ok(plan) => {
                                        self.plugin_form.view = view;
                                        self.plugin_form.view_revoked = false;
                                        self.plugin_form.draft = None;
                                        self.plugin_form.migration = Some(plan);
                                        self.plugin_form.migration_text.clear();
                                    }
                                    Err(error) => self.report(Err(error), ""),
                                }
                            }
                        }
                        for (owner, command) in &commands {
                            if ui
                                .add_enabled(command.enabled, egui::Button::new(&command.id))
                                .on_hover_text(command.disabled_reason.as_deref().unwrap_or(owner))
                                .clicked()
                            {
                                let context = ToolContext {
                                    level: self.active_level,
                                    selection: if command.mode == Mode::Create {
                                        None
                                    } else {
                                        self.selected
                                    },
                                };
                                match ToolDraft::begin(
                                    &self.editor.host,
                                    &self.editor.document,
                                    owner,
                                    &command.id,
                                    context,
                                ) {
                                    Ok(draft) => {
                                        self.plugin_form.view = view;
                                        self.plugin_form.view_revoked = false;
                                        self.plugin_form.migration = None;
                                        self.plugin_form.text = draft
                                            .values()
                                            .iter()
                                            .map(|(k, v)| {
                                                (
                                                    k.clone(),
                                                    v.as_str()
                                                        .map(str::to_owned)
                                                        .unwrap_or_else(|| v.to_string()),
                                                )
                                            })
                                            .collect();
                                        self.plugin_form.draft = Some(draft);
                                    }
                                    Err(error) => self.report(Err(error), ""),
                                }
                            }
                        }
                        ui.separator();
                        if let Some(plan) = &mut self.plugin_form.migration {
                            ui.colored_label(crate::theme::ERROR, "Review every owned type below. Apply stages all conversions, then commits once. Save a backup first; no automatic backup is created.");
                            let types: Vec<_> = plan.types().cloned().collect();
                            let mut choose_error = None;
                            for kind in types {
                                ui.push_id(&kind, |ui| {
                                    ui.label(&kind);
                                    let selected = plan.choice(&kind).map(|d| d.descriptor().id.clone()).unwrap_or_else(|| "Choose migration command".into());
                                    egui::ComboBox::from_id_salt("migration_command").selected_text(selected).show_ui(ui, |ui| {
                                        for (owner, command) in &commands {
                                            if owner == plan.owner() && command.element_type == kind && command.mode == Mode::Migrate && command.enabled && ui.button(&command.id).clicked() {
                                                match plan.choose(&self.editor.host, &self.editor.document, context, &kind, &command.id) {
                                                    Ok(()) => { self.plugin_form.migration_text.remove(&kind); }
                                                    Err(error) => choose_error = Some(error),
                                                }
                                            }
                                        }
                                    });
                                    if let Some(draft) = plan.choice_mut(&kind) {
                                        let text = self.plugin_form.migration_text.entry(kind.clone()).or_insert_with(|| draft.values().iter().map(|(k,v)| (k.clone(), v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))).collect());
                                        descriptor_fields(ui, draft, text);
                                    }
                                });
                            }
                            if let Some(error) = choose_error { ui.colored_label(crate::theme::ERROR, error.to_string()); }
                            let valid = plan.validate();
                            if let Err(error) = &valid { ui.colored_label(crate::theme::ERROR, error.to_string()); }
                            if ui.add_enabled(valid.is_ok() && !self.plugin_form.view_revoked, egui::Button::new("Apply complete migration plan")).clicked() {
                                let result = if self.plugin_form.view_revoked {
                                    Err(os_core::Error::Invalid("View changed; review a new plugin draft".into()))
                                } else {
                                    self.editor.start_plugin_migration(plan, context, view)
                                };
                                self.report(result, "Migration plan pending…");
                                ctx.request_repaint();
                            }
                            if ui.button("Discard migration plan").clicked() {
                                self.plugin_form.migration = None;
                                self.plugin_form.migration_text.clear();
                            }
                        }
                        let mut apply = false;
                        let mut discard = false;
                        if let Some(draft) = &mut self.plugin_form.draft {
                            ui.label(&draft.descriptor().id);
                            if draft.descriptor().mode == Mode::Migrate {
                                ui.colored_label(crate::theme::ERROR, "Migration stages ALL extensions owned by this plugin, then updates them and its required version in one undoable transaction. This form supports one owned type; multiple types require a reviewed migration plan. Save a backup before applying.");
                            }
                            descriptor_fields(ui, draft, &mut self.plugin_form.text);
                            let valid = draft.validate();
                            if let Err(error) = &valid {
                                ui.colored_label(crate::theme::ERROR, error.to_string());
                            }
                            apply = ui
                                .add_enabled(
                                    valid.is_ok() && !self.plugin_form.view_revoked,
                                    egui::Button::new("Apply plugin command"),
                                )
                                .clicked();
                            discard = ui.button("Discard plugin draft").clicked();
                        }
                        if apply && let Some(draft) = &self.plugin_form.draft {
                            let context = ToolContext {
                                level: self.active_level,
                                selection: if draft.descriptor().mode == Mode::Create {
                                    None
                                } else {
                                    self.selected
                                },
                            };
                            let result = if self.plugin_form.view_revoked {
                                Err(os_core::Error::Invalid("View changed; review a new plugin draft".into()))
                            } else {
                                self.editor.start_plugin_tool(draft, context, view)
                            };
                            self.report(result, "Plugin command pending…");
                            ctx.request_repaint();
                        }
                        if discard {
                            self.plugin_form.draft = None;
                        }
                    },
                );
            });
    }
}

#[cfg(test)]
mod view_tests;

fn descriptor_fields(
    ui: &mut egui::Ui,
    draft: &mut ToolDraft,
    text: &mut BTreeMap<String, String>,
) {
    for field in draft.descriptor().fields.clone() {
        ui.push_id(&field.key, |ui| {
            ui.label(&field.label);
            let input = text.entry(field.key.clone()).or_default();
            let value = match field.kind {
                FieldKind::Number { unit, min, max, .. } => {
                    let unit = match unit {
                        Unit::Metres => "metres",
                        Unit::Radians => "radians",
                        Unit::Scalar => "scalar",
                    };
                    ui.small(format!("{unit} · {min} to {max}"));
                    ui.text_edit_singleline(input);
                    input
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .map(Value::from)
                        .unwrap_or_else(|| Value::String(input.clone()))
                }
                FieldKind::Text { max_bytes, .. } => {
                    ui.small(format!("Maximum {max_bytes} UTF-8 bytes"));
                    ui.text_edit_singleline(input);
                    Value::String(input.clone())
                }
                FieldKind::Boolean { .. } => {
                    let mut value = draft.values()[&field.key].as_bool().unwrap_or(false);
                    ui.checkbox(&mut value, "Enabled");
                    Value::Bool(value)
                }
                FieldKind::Choice { values, .. } => {
                    egui::ComboBox::from_id_salt("choice")
                        .selected_text(input.as_str())
                        .show_ui(ui, |ui| {
                            for value in values {
                                ui.selectable_value(input, value.clone(), value);
                            }
                        });
                    Value::String(input.clone())
                }
            };
            draft.set(&field.key, value).expect("registered field");
        });
    }
    if let Err(error) = draft.validate() {
        ui.colored_label(crate::theme::ERROR, error.to_string());
    }
}
