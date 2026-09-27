//! Import/export UI for portable native door and window types.
use super::*;
use os_core::ensure;
use os_model::{Material, OpeningType, OpeningTypeParams};
use os_storage::{
    OpeningTypeLibraryScan, OpeningTypePackage, read_opening_type_package,
    scan_opening_type_library, write_opening_type_package,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

static OPENING_LIBRARY_SCAN_ACTIVE: AtomicBool = AtomicBool::new(false);

struct ScanActiveReset;

impl Drop for ScanActiveReset {
    fn drop(&mut self) {
        OPENING_LIBRARY_SCAN_ACTIVE.store(false, Ordering::Release);
    }
}

pub(super) struct PackageDialog {
    session: Id,
    revision: u64,
    mode: PackageDialogMode,
    pub(super) path: String,
    selected_type: Option<Id>,
    package: Option<OpeningTypePackage>,
    update_selected: bool,
    import_id: Id,
    type_name: String,
    name_was_adjusted: bool,
    dirty: bool,
    plan: Option<ImportPlan>,
    preview_error: Option<String>,
    overwrite_confirmed: bool,
}

pub(super) struct OpeningTypeLibraryDialog {
    pub(super) path: String,
    pub(super) query: String,
    scan: Option<OpeningTypeLibraryScan>,
    error: Option<String>,
    pending_path: Option<std::path::PathBuf>,
    pending_scan: Option<Receiver<std::result::Result<OpeningTypeLibraryScan, String>>>,
}

impl OpeningTypeLibraryDialog {
    fn new(path: String) -> Self {
        Self {
            path,
            query: String::new(),
            scan: None,
            error: None,
            pending_path: None,
            pending_scan: None,
        }
    }
}

#[derive(Clone, Copy)]
enum PackageDialogMode {
    Import,
    Export(Id),
}

struct ImportPlan {
    id: Id,
    parameters: OpeningTypeParams,
    materials: Vec<Material>,
    affected_instances: usize,
    pinned_dimensions: usize,
    material_remaps: Vec<String>,
    is_update: bool,
}

impl PackageDialog {
    fn new(editor: &Editor, mode: PackageDialogMode, selected_type: Option<Id>) -> Self {
        Self {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            mode,
            path: "opening-type.osot".into(),
            selected_type,
            package: None,
            update_selected: false,
            import_id: Id::new(),
            type_name: String::new(),
            name_was_adjusted: false,
            dirty: false,
            plan: None,
            preview_error: None,
            overwrite_confirmed: false,
        }
    }

    fn current(&self, editor: &Editor) -> bool {
        self.session == editor.document.session_id()
            && self.revision == editor.document.revision()
            && match self.mode {
                PackageDialogMode::Import => true,
                PackageDialogMode::Export(id) => {
                    editor.document.model().opening_types.contains_key(&id)
                }
            }
    }

    fn load(&mut self, package: OpeningTypePackage, editor: &Editor) {
        self.update_selected = false;
        self.import_id = Id::new();
        let requested = package.parameters.name.clone();
        let adjusted = unique_name(
            &requested,
            editor
                .document
                .model()
                .opening_types
                .values()
                .map(|ty| ty.parameters.name.as_str()),
        );
        self.name_was_adjusted = adjusted != requested;
        self.type_name = adjusted;
        self.package = Some(package);
        self.dirty = true;
        self.refresh(editor);
    }

    fn refresh(&mut self, editor: &Editor) {
        let Some(package) = &self.package else {
            self.plan = None;
            self.preview_error = None;
            self.dirty = false;
            return;
        };
        match build_import_plan(
            editor,
            package,
            self.import_id,
            self.type_name.trim(),
            self.update_selected,
            self.selected_type,
        ) {
            Ok(plan) => {
                self.plan = Some(plan);
                self.preview_error = None;
            }
            Err(error) => {
                self.plan = None;
                self.preview_error = Some(error.to_string());
            }
        }
        self.dirty = false;
    }
}

impl DesktopApp {
    pub(super) fn begin_opening_type_library_dialog(&mut self) {
        self.opening_type_library_dialog = Some(OpeningTypeLibraryDialog::new(
            self.opening_type_library_path.clone(),
        ));
    }

    pub(super) fn show_opening_type_library_dialog(&mut self, ctx: &egui::Context) {
        if self.opening_type_package_dialog.is_some() {
            return;
        }
        let Some(mut dialog) = self.opening_type_library_dialog.take() else {
            return;
        };
        let scan_result = dialog.pending_scan.as_ref().map(Receiver::try_recv);
        if let Some(result) = scan_result {
            match result {
                Ok(result) => {
                    let scanned_path = dialog.pending_path.take();
                    dialog.pending_scan = None;
                    if scanned_path.as_deref() == Some(std::path::Path::new(dialog.path.trim())) {
                        match result {
                            Ok(scan) => {
                                dialog.scan = Some(scan);
                                dialog.error = None;
                            }
                            Err(error) => {
                                dialog.scan = None;
                                dialog.error = Some(error);
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(16));
                }
                Err(TryRecvError::Disconnected) => {
                    dialog.pending_scan = None;
                    dialog.pending_path = None;
                    dialog.error = Some("background scanner stopped unexpectedly".into());
                }
            }
        }
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut scan_requested = false;
        let mut preview_path = None;
        let mut path_changed = false;
        egui::Modal::new(egui::Id::new("opening_type_library_dialog")).show(ctx, |ui| {
            ui.set_width(700.0);
            ui.heading("Local door/window type library");
            ui.small("Search a non-recursive folder of .osot packages. Each file and the overall scan are size-bounded.");
            ui.horizontal(|ui| {
                ui.label("Library folder");
                path_changed = ui.add_enabled(
                    dialog.pending_scan.is_none(),
                    egui::TextEdit::singleline(&mut dialog.path),
                ).changed();
                if ui
                    .add_enabled(dialog.pending_scan.is_none(), egui::Button::new("Scan library"))
                    .clicked()
                {
                    scan_requested = true;
                }
            });
            if path_changed {
                dialog.scan = None;
                dialog.error = None;
            }
            if dialog.pending_scan.is_some() {
                ui.label("Scanning selected folder…");
            }
            if let Some(error) = &dialog.error {
                ui.colored_label(theme::ERROR, format!("Library scan failed: {error}"));
            }
            if let Some(scan) = &dialog.scan {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} valid package(s) · {} issue(s) · {} bytes examined",
                        scan.entries.len(),
                        scan.issues.len(),
                        scan.bytes_examined
                    ));
                    ui.label("Search");
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.query)
                            .desired_width(180.0)
                            .hint_text("name, kind, file, material"),
                    );
                });
                let needle = dialog.query.trim().to_lowercase();
                let matches: Vec<_> = scan
                    .entries
                    .iter()
                    .filter(|entry| {
                        if needle.is_empty() {
                            return true;
                        }
                        let package = &entry.package;
                        let material_names = package
                            .materials
                            .iter()
                            .map(|material| material.parameters.name.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        format!(
                            "{} {:?} {} {}",
                            package.parameters.name,
                            package.parameters.kind,
                            entry.path.display(),
                            material_names
                        )
                        .to_lowercase()
                        .contains(&needle)
                    })
                    .collect();
                ui.small(format!("{} matching package(s)", matches.len()));
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 355.0).max(120.0))
                    .show(ui, |ui| {
                        for entry in matches {
                            ui.push_id(&entry.path, |ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(format!(
                                            "{} · {:?}",
                                            entry.package.parameters.name,
                                            entry.package.parameters.kind
                                        ));
                                        ui.small(format!(
                                            "{:.3} × {:.3} m · {} material(s) · {}",
                                            entry.package.parameters.width,
                                            entry.package.parameters.height,
                                            entry.package.materials.len(),
                                            entry.path.display()
                                        ));
                                    });
                                    if ui.button("Preview import").clicked() {
                                        preview_path = Some(entry.path.clone());
                                    }
                                });
                                ui.separator();
                            });
                        }
                    });
                if !scan.issues.is_empty() {
                    ui.colored_label(theme::ACCENT, "Unreadable packages (other valid entries remain available):");
                    egui::ScrollArea::vertical()
                        .max_height(56.0)
                        .show(ui, |ui| {
                            for issue in &scan.issues {
                                ui.small(format!(
                                    "{}: {}",
                                    issue.path.display(), issue.message
                                ));
                            }
                        });
                }
            }
            ui.separator();
            if ui.button("Close library").clicked() {
                close = true;
            }
        });

        if scan_requested && !close {
            if OPENING_LIBRARY_SCAN_ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                dialog.error = Some("another local package scan is still finishing".into());
            } else {
                let path = std::path::PathBuf::from(dialog.path.trim());
                let (sender, receiver) = mpsc::channel();
                let result = std::thread::Builder::new()
                    .name("opening-type-library-scan".into())
                    .spawn(move || {
                        let _active = ScanActiveReset;
                        let result = scan_opening_type_library(&path).map_err(|e| e.to_string());
                        drop(_active);
                        let _ = sender.send(result);
                    });
                match result {
                    Ok(_) => {
                        dialog.pending_path = Some(std::path::PathBuf::from(dialog.path.trim()));
                        dialog.pending_scan = Some(receiver);
                        dialog.scan = None;
                        dialog.error = None;
                        ctx.request_repaint_after(std::time::Duration::from_millis(16));
                    }
                    Err(error) => {
                        OPENING_LIBRARY_SCAN_ACTIVE.store(false, Ordering::Release);
                        dialog.error = Some(error.to_string());
                    }
                }
            }
        }
        if let Some(path) = preview_path
            && !close
        {
            match read_opening_type_package(&path) {
                Ok(package) => {
                    let selected = self
                        .selected
                        .filter(|id| self.editor.document.model().opening_types.contains_key(id));
                    let mut import =
                        PackageDialog::new(&self.editor, PackageDialogMode::Import, selected);
                    import.path = path.display().to_string();
                    import.load(package, &self.editor);
                    self.opening_type_package_dialog = Some(import);
                }
                Err(error) => dialog.error = Some(error.to_string()),
            }
        }
        self.opening_type_library_path = dialog.path.clone();
        if !close {
            self.opening_type_library_dialog = Some(dialog);
        }
    }

    pub(super) fn begin_import_opening_type_package(&mut self) {
        self.opening_type_library_dialog = None;
        self.opening_type_package_dialog = Some(PackageDialog::new(
            &self.editor,
            PackageDialogMode::Import,
            self.selected
                .filter(|id| self.editor.document.model().opening_types.contains_key(id)),
        ));
    }

    pub(super) fn begin_export_opening_type_package(&mut self, id: Id) {
        self.opening_type_package_dialog = Some(PackageDialog::new(
            &self.editor,
            PackageDialogMode::Export(id),
            None,
        ));
    }

    pub(super) fn show_opening_type_package_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.opening_type_package_dialog.take() else {
            return;
        };
        if !dialog.current(&self.editor) {
            self.report(
                Err(Error::Invalid(
                    "Opening type package dialog canceled because its document changed".into(),
                )),
                "",
            );
            return;
        }

        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut read_path = false;
        let mut write_export = false;
        let mut apply_import = false;
        egui::Modal::new(egui::Id::new("opening_type_package_dialog")).show(ctx, |ui| {
            ui.set_width(560.0);
            match dialog.mode {
                PackageDialogMode::Export(id) => {
                    let ty = &self.editor.document.model().opening_types[&id];
                    ui.heading("Export door/window type package");
                    ui.label(format!("{} · {:?}", ty.parameters.name, ty.parameters.kind));
                    ui.small("The package contains the type definition and exact snapshots of its referenced materials.");
                    ui.label("Package file (.osot)");
                    if ui.text_edit_singleline(&mut dialog.path).changed() {
                        dialog.overwrite_confirmed = false;
                    }
                    let exists = !dialog.path.trim().is_empty()
                        && std::path::Path::new(dialog.path.trim()).exists();
                    if exists {
                        ui.colored_label(
                            theme::ACCENT,
                            "This file already exists. Confirm replacement; the write is atomic.",
                        );
                        ui.checkbox(&mut dialog.overwrite_confirmed, "Replace existing package");
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        if ui
                            .add_enabled(
                                !dialog.path.trim().is_empty()
                                    && (!exists || dialog.overwrite_confirmed),
                                egui::Button::new("Export package"),
                            )
                            .clicked()
                        {
                            write_export = true;
                        }
                    });
                }
                PackageDialogMode::Import => {
                    ui.heading("Import door/window type package");
                    ui.small("Import creates a new type by default. Updating an existing type preserves its UUID and all instance dimension pins.");
                    ui.label("Package file (.osot)");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut dialog.path);
                        if ui.button("Read and preview").clicked() {
                            read_path = true;
                        }
                    });

                    if let Some(package) = dialog.package.clone() {
                        ui.separator();
                        ui.label(format!(
                            "Package: {} · {:?} · {:.3} × {:.3} m",
                            package.parameters.name,
                            package.parameters.kind,
                            package.parameters.width,
                            package.parameters.height,
                        ));
                        if let Some(target) = dialog.selected_type.filter(|id| {
                            self.editor
                                .document
                                .model()
                                .opening_types
                                .get(id)
                                .is_some_and(|ty| ty.parameters.kind == package.parameters.kind)
                        }) {
                            let was_update = dialog.update_selected;
                            let mut mode_changed = false;
                            ui.horizontal(|ui| {
                                mode_changed |= ui
                                    .radio_value(&mut dialog.update_selected, false, "Create new type")
                                    .changed();
                                mode_changed |= ui
                                    .radio_value(&mut dialog.update_selected, true, "Update selected compatible type")
                                    .changed();
                            });
                            if mode_changed {
                                if !was_update && dialog.update_selected {
                                    dialog.type_name = self.editor.document.model().opening_types[&target]
                                        .parameters.name.clone();
                                    dialog.name_was_adjusted = false;
                                } else if was_update && !dialog.update_selected {
                                    let requested = package.parameters.name.clone();
                                    let suggested = unique_name(
                                        &requested,
                                        self.editor.document.model().opening_types.values()
                                            .map(|ty| ty.parameters.name.as_str()),
                                    );
                                    dialog.name_was_adjusted = suggested != requested;
                                    dialog.type_name = suggested;
                                }
                                dialog.dirty = true;
                            }
                            ui.small(format!(
                                "Update target: {}",
                                self.editor.document.model().opening_types[&target]
                                    .parameters
                                    .name
                            ));
                        } else {
                            ui.small("No selected type of the same kind; import will create a new type.");
                        }
                        ui.horizontal(|ui| {
                            ui.label("Destination type name");
                            if ui
                                .add(egui::TextEdit::singleline(&mut dialog.type_name).char_limit(256))
                                .changed()
                            {
                                dialog.dirty = true;
                                dialog.name_was_adjusted = false;
                            }
                        });
                        if dialog.name_was_adjusted {
                            ui.colored_label(
                                theme::ACCENT,
                                "The package name already exists here; a unique imported name was suggested.",
                            );
                        }
                        if dialog.dirty {
                            dialog.refresh(&self.editor);
                        }
                        if let Some(error) = &dialog.preview_error {
                            ui.colored_label(theme::ERROR, format!("Preview blocked: {error}"));
                        }
                        if let Some(plan) = &dialog.plan {
                            ui.label(format!(
                                "{} · {} affected instance(s) · {} pinned dimension(s) preserved",
                                if plan.is_update { "Update" } else { "Create" },
                                plan.affected_instances,
                                plan.pinned_dimensions,
                            ));
                            if plan.materials.is_empty() && package.materials.is_empty() {
                                ui.small("No material dependencies.");
                            }
                            for remap in &plan.material_remaps {
                                ui.small(remap);
                            }
                            ui.small("Every affected host and opening component passed preflight. No project changes occur until Import type.");
                        }
                    }
                    if dialog.package.is_none()
                        && let Some(error) = &dialog.preview_error
                    {
                        ui.colored_label(theme::ERROR, format!("Package unavailable: {error}"));
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        if ui
                            .add_enabled(dialog.plan.is_some() && !dialog.dirty, egui::Button::new("Import type"))
                            .clicked()
                        {
                            apply_import = true;
                        }
                    });
                }
            }
        });

        if read_path && !close {
            match read_opening_type_package(std::path::Path::new(dialog.path.trim())) {
                Ok(package) => dialog.load(package, &self.editor),
                Err(error) => {
                    dialog.package = None;
                    dialog.plan = None;
                    dialog.preview_error = Some(error.to_string());
                }
            }
        }
        if write_export
            && !close
            && let PackageDialogMode::Export(id) = dialog.mode
        {
            let result = write_opening_type_package(
                self.editor.document.model(),
                id,
                std::path::Path::new(dialog.path.trim()),
            );
            match result {
                Ok(()) => {
                    self.report(Ok(()), "Opening type package exported.");
                    close = true;
                }
                Err(error) => self.report(Err(error), ""),
            }
        }
        if apply_import
            && !close
            && let Some(plan) = dialog.plan.take()
        {
            match apply_import_plan(&mut self.editor, plan) {
                Ok(id) => {
                    self.select(Some(id));
                    self.report(Ok(()), "Opening type package imported.");
                    self.opening_type_library_dialog = None;
                    close = true;
                }
                Err(error) => {
                    dialog.preview_error = Some(error.to_string());
                    dialog.dirty = true;
                }
            }
        }
        if !close {
            self.opening_type_package_dialog = Some(dialog);
        }
    }
}

fn build_import_plan(
    editor: &Editor,
    package: &OpeningTypePackage,
    import_id: Id,
    name: &str,
    update_selected: bool,
    selected_type: Option<Id>,
) -> Result<ImportPlan> {
    package.validate()?;
    let model = editor.document.model();
    let target_id = if update_selected {
        let id =
            selected_type.ok_or_else(|| Error::Invalid("No update target is selected".into()))?;
        let current = model
            .opening_types
            .get(&id)
            .ok_or_else(|| Error::Invalid("Selected update target no longer exists".into()))?;
        ensure(
            current.parameters.kind == package.parameters.kind,
            "Update target must have the same door/window kind",
        )?;
        id
    } else {
        import_id
    };
    ensure(
        update_selected || !model.opening_types.contains_key(&target_id),
        "new opening type identity already exists",
    )?;
    ensure(
        !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
        "type name must be 1-256 bytes without control characters",
    )?;
    ensure(
        !model.opening_types.iter().any(|(id, ty)| {
            *id != target_id && ty.parameters.name.eq_ignore_ascii_case(name.trim())
        }),
        "type name already exists; choose a unique destination name",
    )?;

    let mut candidate = model.clone();
    let mut additions = Vec::new();
    let mut material_remaps = Vec::new();
    let mut remapped = std::collections::BTreeMap::new();
    for snapshot in &package.materials {
        if let Some((id, _)) = candidate
            .materials
            .iter()
            .find(|(_, material)| material.parameters == snapshot.parameters)
        {
            remapped.insert(snapshot.source_id, *id);
            material_remaps.push(format!(
                "{} → existing identical material",
                snapshot.parameters.name
            ));
            continue;
        }
        let mut parameters = snapshot.parameters.clone();
        let requested = parameters.name.clone();
        parameters.name = unique_name(
            &requested,
            candidate
                .materials
                .values()
                .map(|m| m.parameters.name.as_str()),
        );
        if parameters.name != requested {
            material_remaps.push(format!(
                "{} → {} (name conflict)",
                requested, parameters.name
            ));
        } else {
            material_remaps.push(format!("{} → imported material", requested));
        }
        let material = Material::new("core.material", parameters);
        remapped.insert(snapshot.source_id, material.id());
        candidate.materials.insert(material.id(), material.clone());
        additions.push(material);
    }

    let mut parameters = package.parameters.clone();
    parameters.name = name.trim().to_owned();
    if let Some(id) = parameters.family.panel_material {
        parameters.family.panel_material = remapped.get(&id).copied();
    }
    if let Some(id) = parameters.family.frame_material {
        parameters.family.frame_material = remapped.get(&id).copied();
    }
    parameters.validate()?;
    if update_selected {
        candidate
            .opening_types
            .get_mut(&target_id)
            .ok_or_else(|| Error::Invalid("Selected update target no longer exists".into()))?
            .parameters = parameters.clone();
    } else {
        let mut ty = OpeningType::new("core.opening_type", parameters.clone());
        ty.header.id = target_id;
        candidate.opening_types.insert(target_id, ty);
    }
    candidate.validate()?;

    let affected: Vec<_> = candidate
        .openings
        .values()
        .filter(|opening| opening.parameters.type_id() == Some(target_id))
        .collect();
    let mut hosts = std::collections::BTreeSet::new();
    let mut pinned_dimensions = 0;
    for opening in &affected {
        opening_tools::panel_mesh(&candidate, opening.id())?;
        hosts.insert(opening.parameters.host);
        pinned_dimensions += usize::from(opening.parameters.width_override.is_some())
            + usize::from(opening.parameters.height_override.is_some())
            + usize::from(opening.parameters.sill_override.is_some());
    }
    for host in hosts {
        opening_tools::host_mesh(&candidate, host)?;
    }
    Ok(ImportPlan {
        id: target_id,
        parameters,
        materials: additions,
        affected_instances: affected.len(),
        pinned_dimensions,
        material_remaps,
        is_update: update_selected,
    })
}

fn apply_import_plan(editor: &mut Editor, plan: ImportPlan) -> Result<Id> {
    let mut commands: Vec<_> = plan
        .materials
        .into_iter()
        .map(Command::AddMaterial)
        .collect();
    if plan.is_update {
        commands.push(Command::UpdateOpeningType {
            id: plan.id,
            parameters: plan.parameters,
        });
    } else {
        let mut ty = OpeningType::new("core.opening_type", plan.parameters);
        ty.header.id = plan.id;
        commands.push(Command::AddOpeningType(ty));
    }
    editor
        .document
        .execute("Import opening type package", commands)?;
    if let Err(regenerate_error) = editor.regenerate() {
        let original_error = regenerate_error.to_string();
        return match editor.undo() {
            Ok(()) => match editor.regenerate() {
                Ok(()) => Err(Error::Invalid(format!(
                    "Import was rolled back because 3D regeneration failed: {original_error}"
                ))),
                Err(restore_error) => Err(Error::Invalid(format!(
                    "Import was rolled back after regeneration failed: {original_error}; restoring the previous scene also failed: {restore_error}"
                ))),
            },
            Err(rollback_error) => Err(Error::Invalid(format!(
                "3D regeneration failed: {original_error}; transaction rollback failed: {rollback_error}"
            ))),
        };
    }
    Ok(plan.id)
}

fn unique_name<'a>(requested: &str, existing: impl Iterator<Item = &'a str>) -> String {
    let existing: std::collections::BTreeSet<_> =
        existing.map(|name| name.to_lowercase()).collect();
    let base = requested.trim();
    if !existing.contains(&base.to_lowercase()) {
        return base.to_owned();
    }
    for index in 1..=9999 {
        let suffix = if index == 1 {
            " (Imported)".to_owned()
        } else {
            format!(" (Imported {index})")
        };
        let max_base = 256usize.saturating_sub(suffix.len());
        let shortened = take_utf8_bytes(base, max_base);
        let candidate = format!("{shortened}{suffix}");
        if !existing.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("Imported {}", Id::new())
}

fn take_utf8_bytes(value: &str, limit: usize) -> String {
    let end = value
        .char_indices()
        .take_while(|(index, character)| index + character.len_utf8() <= limit)
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    value[..end].to_owned()
}
