//! Explicit, opt-in application plugin management; never driven by project content.
use crate::{DesktopApp, plugin_forms::FormState};
use eframe::egui;
use os_plugin_api::Permission;
use os_plugin_host::loading::PendingLoad;
use std::{collections::BTreeSet, path::PathBuf, time::Duration};

#[derive(Default)]
pub(super) struct Manager {
    pub(super) visible: bool,
    directory: String,
    grants: BTreeSet<Permission>,
    pending: Option<PendingLoad>,
    disabled: BTreeSet<String>,
    message: String,
}
impl DesktopApp {
    fn start_manager_load(&mut self) {
        if self.plugin_manager.directory.trim().is_empty() {
            self.plugin_manager.message = "Choose an installed plugin directory.".into();
            return;
        }
        match self.editor.host.start_wasm_load(
            PathBuf::from(self.plugin_manager.directory.trim()),
            self.plugin_manager.grants.clone(),
            Duration::from_secs(30),
        ) {
            Ok(ticket) => {
                self.plugin_manager.pending = Some(ticket);
                self.plugin_manager.message = "Preparing plugin in background…".into();
            }
            Err(error) => self.plugin_manager.message = error.to_string(),
        }
    }
    fn disable_plugin(&mut self, id: &str) {
        match self.editor.host.unload(id) {
            Ok(()) => {
                self.editor.cancel_plugin_tool();
                self.editor.cancel_plugin_open();
                self.plugin_form = FormState::default();
                if id == os_plugin_api::wall::OWNER {
                    for id in self.editor.document.model().walls.keys() {
                        self.editor.scene.remove(id);
                    }
                }
                let result = self.editor.regenerate();
                self.plugin_manager.disabled.insert(id.into());
                self.plugin_manager.message = match result {
                    Ok(()) => format!(
                        "Disabled {id}. Document data retained; running calls drain before releasing capacity."
                    ),
                    Err(e) => format!("Disabled {id}; geometry unavailable: {e}"),
                };
            }
            Err(error) => self.plugin_manager.message = error.to_string(),
        }
    }
    pub(super) fn plugin_manager(&mut self, ctx: &egui::Context) {
        // Do not activate providers underneath a document confirmation or staged open.
        if self.pending.is_none()
            && self.exchange_pending.is_none()
            && !self.editor.plugin_open_pending()
            && let Some(mut ticket) = self.plugin_manager.pending.take()
        {
            match self.editor.host.poll_wasm_load(&mut ticket) {
                Ok(None) => self.plugin_manager.pending = Some(ticket),
                Ok(Some(id)) => {
                    self.plugin_manager.disabled.remove(&id);
                    self.plugin_manager.message = format!("Loaded {id}.");
                    self.plugin_form = FormState::default();
                }
                Err(error) => self.plugin_manager.message = error.to_string(),
            }
        }
        if self.editor.host.load_pending() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if !self.plugin_manager.visible {
            return;
        }
        let mut visible = true;
        egui::Window::new("Plugin manager").open(&mut visible).default_width(430.0).vscroll(true).show(ctx, |ui| {
            ui.label("Plugins are application installations. Project files never load executable code.");
            ui.label("Installed directory (contains plugin.toml and adjacent .wasm)");
            let busy = self.editor.host.load_pending();
            ui.add_enabled_ui(!busy, |ui| {
                ui.text_edit_singleline(&mut self.plugin_manager.directory);
                ui.label("Explicit grants for this load (all denied by default)");
                for (permission, label) in [(Permission::ModelRead, "Read model"), (Permission::ModelWrite, "Propose model edits"), (Permission::UiTool, "Register tools"), (Permission::UiPanel, "Register panels")] {
                    let mut granted = self.plugin_manager.grants.contains(&permission);
                    if ui.checkbox(&mut granted, label).changed() {
                        if granted { self.plugin_manager.grants.insert(permission); } else { self.plugin_manager.grants.remove(&permission); }
                    }
                }
                ui.small("Only requested grants become effective. No filesystem, network or process services are exposed to guests.");
                if ui.add_enabled(self.pending.is_none() && self.exchange_pending.is_none() && !self.editor.plugin_work_pending(), egui::Button::new("Load installed plugin")).clicked() { self.start_manager_load(); }
            });
            if busy {
                ui.label("Preparation pending or draining…");
                if ui.button("Cancel plugin load").clicked() {
                    if let Some(mut ticket) = self.plugin_manager.pending.take() { ticket.cancel(); }
                    self.plugin_manager.message = "Load cancelled. Preparation capacity remains occupied until the worker exits.".into();
                }
            }
            ui.label(&self.plugin_manager.message);
            ui.separator();
            let manifests: Vec<_> = self.editor.host.manifests().cloned().collect();
            for manifest in manifests {
                ui.push_id(&manifest.id, |ui| {
                    ui.strong(format!("{} @ {} · loaded", manifest.id, manifest.version));
                    ui.label(format!("API {} · {:?}", manifest.api_version, manifest.capabilities));
                    ui.label(format!("Requested: {:?}", manifest.permissions));
                    ui.label(format!("Granted: {:?}", self.editor.host.granted_permissions(&manifest.id)));
                    for dependency in &manifest.dependencies { ui.label(format!("Requires {} @ {}", dependency.id, dependency.version)); }
                    if ui.add_enabled(self.pending.is_none() && self.exchange_pending.is_none(), egui::Button::new("Disable plugin")).clicked() { self.disable_plugin(&manifest.id); }
                });
                ui.separator();
            }
            for id in &self.plugin_manager.disabled { ui.label(format!("{id} · disabled; use its directory to load again")); }
            if self.editor.host.activation_id(os_plugin_api::wall::OWNER).is_none() && ui.button("Restore bundled Wall").clicked() {
                let result = self.editor.host.load(Box::new(os_walls::WallsPlugin), [Permission::ModelRead, Permission::ModelWrite, Permission::UiTool, Permission::UiPanel].into());
                if result.is_ok() {
                    self.plugin_manager.disabled.remove(os_plugin_api::wall::OWNER);
                    self.editor.pending_geometry.extend(self.editor.document.model().walls.keys().copied());
                }
                self.plugin_manager.message = match result { Ok(()) => "Bundled Wall restored.".into(), Err(error) => error.to_string() };
            }
        });
        self.plugin_manager.visible = visible;
    }
}
