//! Original desktop shell and application controller, replaceable independently.
use eframe::egui;
use os_core::{Error, Id, Point2, Result};
use os_document::{Command, Document};
use os_geometry::Vec3;
use os_model::{Level, LevelParams, Model, WallParams};
use os_plugin_api::{Permission, Request};
use os_plugin_host::PluginHost;
use os_render::{Camera, Scene};
use os_storage::{StorageBackend, ZipJsonStorage};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

mod exchange;
mod grid_tools;
mod opening_schedule;
mod opening_tools;
mod opening_type_tools;
mod palettes;
pub mod plan;
pub mod plan_gesture;
#[cfg(feature = "external-plugins")]
pub mod plan_providers;
mod plan_settings;
mod plan_workspace;
#[cfg(feature = "external-plugins")]
mod plugin_forms;
#[cfg(feature = "external-plugins")]
mod plugin_jobs;
#[cfg(feature = "external-plugins")]
mod plugin_manager;
#[cfg(feature = "external-plugins")]
mod plugin_open;
#[cfg(feature = "external-plugins")]
pub use plugin_open::OpenedProject;
#[cfg(feature = "external-plugins")]
pub mod migration_tools;
pub mod plugin_tools;
#[cfg(feature = "external-plugins")]
pub use plugin_jobs::PluginPoll;
mod ribbon;
mod room_tools;
pub mod theme;
mod viewport;
mod wall_join_tools;
mod wall_type_tools;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RibbonTab {
    #[default]
    Architecture,
    View,
    Manage,
    ModifyWalls,
}

/// UI-independent orchestration. Model mutations always pass through Document.
pub struct Editor {
    pub document: Document,
    pub host: PluginHost,
    pub scene: Scene,
    pending_geometry: BTreeSet<Id>,
    saved_model: Option<Model>,
    #[cfg(feature = "external-plugins")]
    plugin_jobs: plugin_jobs::Jobs,
    #[cfg(feature = "external-plugins")]
    staged_open: Option<plugin_open::StagedOpen>,
}
#[cfg(all(test, not(feature = "external-plugins")))]
impl Editor {
    // Feature-disabled headless harnesses have no asynchronous plugin jobs.
    fn plugin_work_pending(&self) -> bool {
        false
    }
}
impl Editor {
    pub fn new() -> Result<Self> {
        let document = Document::new("Untitled project")?;
        let mut host = PluginHost::default();
        host.load(
            Box::new(os_walls::WallsPlugin),
            [
                Permission::ModelRead,
                Permission::ModelWrite,
                Permission::UiTool,
                Permission::UiPanel,
            ]
            .into(),
        )?;
        Ok(Self {
            saved_model: Some(document.model().clone()),
            document,
            host,
            scene: Scene::new(),
            pending_geometry: BTreeSet::new(),
            #[cfg(feature = "external-plugins")]
            plugin_jobs: plugin_jobs::Jobs::default(),
            #[cfg(feature = "external-plugins")]
            staged_open: None,
        })
    }
    pub fn is_dirty(&self) -> bool {
        self.saved_model.as_ref() != Some(self.document.model())
    }
    pub fn wall_command(&mut self, label: &str, request: Request) -> Result<()> {
        self.host
            .execute(os_walls::PLUGIN_ID, &mut self.document, label, request)?;
        self.regenerate()
    }
    pub fn command(&mut self, label: &str, command: Command) -> Result<()> {
        self.document.execute(label, vec![command])?;
        self.regenerate()
    }
    pub fn undo(&mut self) -> Result<()> {
        self.document.undo();
        self.regenerate()
    }
    pub fn redo(&mut self) -> Result<()> {
        self.document.redo();
        self.regenerate()
    }
    pub fn save(&mut self, path: &Path) -> Result<()> {
        self.regenerate()?;
        #[cfg(feature = "external-plugins")]
        os_core::ensure(
            !self.plugin_work_pending() && self.plugin_geometry_errors().is_empty(),
            "plugin work is pending or failed; resolve it before saving",
        )?;
        ZipJsonStorage.save(&self.document, path)?;
        self.saved_model = Some(self.document.model().clone());
        Ok(())
    }
    pub fn open(&mut self, path: &Path) -> Result<()> {
        let document = ZipJsonStorage.open(path)?;
        // Stage regeneration before replacing the current editable document.
        let mut scene = Scene::new();
        for id in document.model().walls.keys() {
            scene.insert(*id, opening_tools::host_mesh(document.model(), *id)?);
        }
        self.saved_model = Some(document.model().clone());
        for id in document.model().openings.keys() {
            scene.insert(*id, opening_tools::panel_mesh(document.model(), *id)?);
        }
        for (id, floor) in &document.model().floors {
            let level = &document.model().levels[&floor.parameters.level];
            let top_z = level.parameters.elevation + floor.parameters.top_offset;
            scene.insert(
                *id,
                os_geometry::floors::extrude_floor(
                    &floor.parameters.boundary,
                    top_z,
                    floor.parameters.thickness,
                )?,
            );
        }
        for (id, column) in &document.model().columns {
            let elevation = document.model().levels[&column.parameters.level]
                .parameters
                .elevation;
            scene.insert(
                *id,
                os_geometry::columns::column_mesh(&column.parameters, elevation)?,
            );
        }
        self.document = document;
        self.scene = scene;
        self.pending_geometry.clear();
        Ok(())
    }
    pub fn regenerate(&mut self) -> Result<()> {
        #[cfg(feature = "external-plugins")]
        self.synchronize_plugin_scene();
        for event in self.document.drain_events() {
            self.pending_geometry.extend(event.invalidated);
        }
        let mut error = None;
        for id in self.pending_geometry.clone() {
            self.scene.remove(&id);
            #[cfg(feature = "external-plugins")]
            if self.plugin_jobs.managed.contains_key(&id) {
                continue;
            }
            if self.document.model().walls.contains_key(&id) {
                let result = opening_tools::host_mesh(self.document.model(), id);
                match result {
                    Ok(mesh) => {
                        self.scene.insert(id, mesh);
                    }
                    Err(e) => {
                        error = Some(e);
                        continue;
                    }
                }
            }
            if self.document.model().openings.contains_key(&id) {
                match opening_tools::panel_mesh(self.document.model(), id) {
                    Ok(mesh) => {
                        self.scene.insert(id, mesh);
                    }
                    Err(e) => {
                        error = Some(e);
                        continue;
                    }
                }
            }
            if let Some(floor) = self.document.model().floors.get(&id) {
                let result = (|| {
                    let level = self
                        .document
                        .model()
                        .levels
                        .get(&floor.parameters.level)
                        .ok_or_else(|| Error::Invalid("floor level missing".into()))?;
                    let top_z = level.parameters.elevation + floor.parameters.top_offset;
                    os_geometry::floors::extrude_floor(
                        &floor.parameters.boundary,
                        top_z,
                        floor.parameters.thickness,
                    )
                })();
                match result {
                    Ok(mesh) => {
                        self.scene.insert(id, mesh);
                    }
                    Err(e) => {
                        error = Some(e);
                        continue;
                    }
                }
            }
            if let Some(column) = self.document.model().columns.get(&id) {
                let elevation = self.document.model().levels[&column.parameters.level]
                    .parameters
                    .elevation;
                match os_geometry::columns::column_mesh(&column.parameters, elevation) {
                    Ok(mesh) => {
                        self.scene.insert(id, mesh);
                    }
                    Err(e) => {
                        error = Some(e);
                        continue;
                    }
                }
            }
            self.pending_geometry.remove(&id);
        }
        if let Some(e) = error { Err(e) } else { Ok(()) }
    }
}

#[derive(Clone)]
enum PendingAction {
    New,
    Open,
    Replace(PathBuf),
    Close,
}
pub struct DesktopApp {
    #[cfg(feature = "external-plugins")]
    plugin_manager: plugin_manager::Manager,
    #[cfg(feature = "external-plugins")]
    plugin_form: plugin_forms::FormState,
    pub editor: Editor,
    camera: Camera,
    viewport_cache: Option<viewport::ViewportCache>,
    plans: plan_workspace::PlanWorkspace,
    plan_draft: Option<plan_settings::PlanDraft>,
    grid_draft: Option<grid_tools::GridDraft>,
    opening_draft: Option<opening_tools::OpeningDraft>,
    opening_schedule: opening_schedule::OpeningSchedule,
    opening_type_draft: Option<opening_type_tools::OpeningTypeDraft>,
    wall_type_draft: Option<wall_type_tools::WallTypeDraft>,
    preferred_door_type: Option<Id>,
    preferred_window_type: Option<Id>,
    room_number_draft: String,
    room_name_draft: String,
    room_tag_position: Point2,
    dimension_offset_draft: f64,
    dimension_spacing_draft: f64,
    wall_gesture: Option<plan_gesture::WallGesture>,
    selected: Option<Id>,
    active_level: Id,
    draft: WallParams,
    draft_length: f64,
    path: String,
    save_destination: Option<String>,
    ifc_path: String,
    exchange_pending: Option<exchange::PendingExchange>,
    opened_path: Option<PathBuf>,
    project_name: String,
    status: String,
    status_error: bool,
    show_details: bool,
    extension_inspection: Option<Id>,
    ribbon_tab: RibbonTab,
    properties_fraction: f32,
    pending: Option<PendingAction>,
    allow_close: bool,
    fit_requested: bool,
}
impl DesktopApp {
    pub fn new() -> Result<Self> {
        let editor = Editor::new()?;
        let active_level = *editor
            .document
            .model()
            .levels
            .keys()
            .next()
            .ok_or_else(|| Error::Invalid("initial level missing".into()))?;
        let draft = default_wall(active_level);
        Ok(Self {
            project_name: editor.document.model().project.parameters.name.clone(),
            #[cfg(feature = "external-plugins")]
            plugin_manager: plugin_manager::Manager::default(),
            #[cfg(feature = "external-plugins")]
            plugin_form: plugin_forms::FormState::default(),
            editor,
            camera: Camera::default(),
            viewport_cache: None,
            plans: plan_workspace::PlanWorkspace::default(),
            plan_draft: None,
            grid_draft: None,
            opening_draft: None,
            opening_schedule: opening_schedule::OpeningSchedule::default(),
            opening_type_draft: None,
            wall_type_draft: None,
            preferred_door_type: None,
            preferred_window_type: None,
            room_number_draft: String::new(),
            room_name_draft: String::new(),
            room_tag_position: Point2::new(0.0, 0.0),
            dimension_offset_draft: 0.0,
            dimension_spacing_draft: 0.25,
            wall_gesture: None,
            selected: None,
            active_level,
            draft_length: draft.length(),
            draft,
            path: "project.osb".into(),
            save_destination: None,
            ifc_path: "exchange.ifc".into(),
            exchange_pending: None,
            opened_path: None,
            status: "Ready. Walls plugin loaded. All dimensions are in metres.".into(),
            status_error: false,
            show_details: false,
            extension_inspection: None,
            ribbon_tab: RibbonTab::default(),
            properties_fraction: 0.55,
            pending: None,
            allow_close: false,
            fit_requested: false,
        })
    }
    /// Open after the caller has obtained any required unsaved-change consent.
    /// With external providers enabled, success means staging started; desktop
    /// frames finish adoption or report failure. Built-in-only opening is immediate.
    pub fn open_path(&mut self, path: &Path) -> Result<()> {
        #[cfg(feature = "external-plugins")]
        if self.editor.host.manifests().any(|m| {
            self.editor.host.catalog(&m.id).is_some() || self.editor.host.worker_supported(&m.id)
        }) {
            self.editor
                .start_plugin_open(path, std::time::Duration::from_secs(30))?;
            self.report(Ok(()), "Opening candidate project…");
            return Ok(());
        }
        self.editor.open(path)?;
        self.path = path.display().to_string();
        self.opened_path = Some(path.into());
        self.refresh_document();
        self.fit_requested = true;
        Ok(())
    }
    fn report(&mut self, result: Result<()>, success: &str) {
        self.status_error = result.is_err();
        self.status = match result {
            Ok(()) => success.into(),
            Err(e) => e.to_string(),
        };
    }
    fn select(&mut self, id: Option<Id>) {
        self.cancel_plan_wall();
        self.opening_draft = None;
        self.opening_type_draft = None;
        self.wall_type_draft = None;
        self.cancel_aligned_dimension();
        let selected = id.filter(|id| {
            let model = self.editor.document.model();
            model.walls.contains_key(id)
                || model.floors.contains_key(id)
                || model.columns.contains_key(id)
                || model.openings.contains_key(id)
                || model.opening_types.contains_key(id)
                || model.rooms.contains_key(id)
                || model.room_tags.contains_key(id)
                || model.detail_lines.contains_key(id)
                || model.room_separation_lines.contains_key(id)
                || model.dimensions.contains_key(id)
                || model.extensions.contains_key(id)
                || model.grids.contains_key(id)
        });
        #[cfg(feature = "external-plugins")]
        if self.selected != selected {
            self.editor.cancel_plugin_tool();
            self.plugin_form = plugin_forms::FormState::default();
        }
        self.selected = selected;
        self.ribbon_tab = if self
            .selected
            .is_some_and(|id| self.editor.document.model().walls.contains_key(&id))
        {
            RibbonTab::ModifyWalls
        } else {
            RibbonTab::Architecture
        };
        self.draft = self
            .selected
            .and_then(|id| {
                self.editor
                    .document
                    .model()
                    .walls
                    .get(&id)
                    .map(|w| w.parameters.clone())
            })
            .unwrap_or_else(|| default_wall(self.active_level));
        self.draft_length = self.draft.length();
        if let Some(room) = self
            .selected
            .and_then(|id| self.editor.document.model().rooms.get(&id))
        {
            self.room_number_draft = room.parameters.number.clone();
            self.room_name_draft = room.parameters.name.clone();
        }
        if let Some(tag) = self
            .selected
            .and_then(|id| self.editor.document.model().room_tags.get(&id))
            .cloned()
        {
            self.room_tag_position = tag.parameters.position;
            self.focus_plan(Some(tag.parameters.view));
        }
        self.dimension_spacing_draft = self
            .selected
            .and_then(|id| self.editor.document.model().dimensions.get(&id))
            .map_or(0.25, |dimension| dimension.parameters.baseline_spacing_m);
        self.dimension_offset_draft = self
            .selected
            .and_then(|id| self.editor.document.model().dimensions.get(&id))
            .map_or(0.0, |dimension| dimension.parameters.offset_m);
    }
    fn begin_room_placement(&mut self) {
        if self.plans.active.is_none() {
            self.report(
                Err(Error::Invalid(
                    "Open a floor plan before placing a room".into(),
                )),
                "",
            );
            return;
        }
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.room_placement_active = true;
        self.report(
            Ok(()),
            "Room tool active · click inside an enclosed space. Escape exits.",
        );
    }
    fn create_room_at(&mut self, level: Id, seed: Point2, boundary_signature: Vec<(Id, bool)>) {
        let result = room_tools::create_at(&mut self.editor, level, seed, boundary_signature);
        match result {
            Ok(id) => {
                self.select(Some(id));
                self.plans.room_placement_active = true;
                self.report(
                    Ok(()),
                    "Room placed · click another enclosed space or press Escape.",
                );
            }
            Err(error) => self.report(Err(error), ""),
        }
    }
    fn apply_room_properties(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().rooms.contains_key(id))
        else {
            return;
        };
        let result = room_tools::apply_properties(
            &mut self.editor,
            id,
            &self.room_number_draft,
            &self.room_name_draft,
        );
        self.report(result, "Room properties updated.");
    }
    fn delete_selected_room(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().rooms.contains_key(id))
        else {
            return;
        };
        let result = room_tools::delete(&mut self.editor, id);
        if result.is_ok() {
            self.select(None);
        }
        self.report(result, "Room deleted.");
    }
    fn apply_dimension_properties(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().dimensions.contains_key(id))
        else {
            return;
        };
        let model = self.editor.document.model();
        let Some(dimension) = model.dimensions.get(&id) else {
            return;
        };
        let mut parameters = dimension.parameters.clone();
        parameters.offset_m = self.dimension_offset_draft;
        parameters.baseline_spacing_m = self.dimension_spacing_draft;
        if let Ok(resolved) = parameters.resolve(model) {
            let dx = resolved.second.x - resolved.first.x;
            let dy = resolved.second.y - resolved.first.y;
            let normal = Point2::new(-dy / resolved.length_metres, dx / resolved.length_metres);
            let midpoint = Point2::new(
                (resolved.first.x + resolved.second.x) * 0.5,
                (resolved.first.y + resolved.second.y) * 0.5,
            );
            parameters.orphan_hint = Point2::new(
                midpoint.x + normal.x * parameters.offset_m,
                midpoint.y + normal.y * parameters.offset_m,
            );
        }
        let result = self.editor.command(
            "Move aligned dimension",
            Command::UpdateDimension { id, parameters },
        );
        self.report(result, "Dimension offset updated.");
    }
    fn delete_selected_dimension(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().dimensions.contains_key(id))
        else {
            return;
        };
        let result = self
            .editor
            .command("Delete aligned dimension", Command::RemoveDimension(id));
        if result.is_ok() {
            self.select(None);
        }
        self.report(result, "Dimension deleted.");
    }
    fn refresh_document(&mut self) {
        self.extension_inspection = None;
        self.project_name = self.editor.document.model().project.parameters.name.clone();
        if !self
            .editor
            .document
            .model()
            .levels
            .contains_key(&self.active_level)
            && let Some(id) = self.editor.document.model().levels.keys().next()
        {
            self.active_level = *id;
        }
        self.select(self.selected);
    }
    fn request_action(&mut self, action: PendingAction) {
        if self.has_unsaved_work() {
            self.pending = Some(action);
        } else {
            self.perform(action);
        }
    }
    fn has_unsaved_work(&self) -> bool {
        #[cfg(feature = "external-plugins")]
        if self.editor.plugin_work_pending() {
            return true;
        }
        self.editor.is_dirty()
    }
    fn perform(&mut self, action: PendingAction) {
        match action {
            PendingAction::New => match Editor::new() {
                Ok(mut editor) => {
                    // Plugin installation is application state, not project data.
                    std::mem::swap(&mut editor.host, &mut self.editor.host);
                    self.editor = editor;
                    self.opened_path = None;
                    self.path = "project.osb".into();
                    self.selected = None;
                    self.camera = Camera::default();
                    self.refresh_document();
                    self.status = "New project created.".into();
                    self.status_error = false;
                }
                Err(e) => self.report(Err(e), ""),
            },
            PendingAction::Open => {
                let path = PathBuf::from(self.path.clone());
                let result = self.open_path(&path);
                #[cfg(feature = "external-plugins")]
                if result.is_ok() && self.editor.plugin_open_pending() {
                    return;
                }
                self.report(
                    result,
                    "Project opened. See view warnings for unavailable geometry.",
                );
            }
            PendingAction::Replace(path) => self.save_to(path),
            PendingAction::Close => self.allow_close = true,
        }
    }
    fn save_to(&mut self, path: PathBuf) {
        let result = self.editor.save(&path);
        if result.is_ok() {
            self.path = path.display().to_string();
            self.opened_path = Some(path);
        }
        self.report(result, "Project saved.");
    }
    fn save(&mut self) {
        let path = PathBuf::from(self.path.trim());
        if path.extension().and_then(|x| x.to_str()) != Some("osb") {
            self.status = "Choose a file path ending in .osb.".into();
            self.status_error = true;
            return;
        }
        if path.exists() && self.opened_path.as_ref() != Some(&path) {
            self.pending = Some(PendingAction::Replace(path));
        } else {
            self.save_to(path);
        }
    }
    fn quick_save(&mut self) {
        if let Some(path) = self.opened_path.clone() {
            self.save_to(path);
        } else {
            self.cancel_plan_wall();
            self.save_destination = Some(String::new());
        }
    }

    fn save_destination_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut destination) = self.save_destination.take() else {
            return;
        };
        let mut cancel = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut apply = false;
        egui::Modal::new(egui::Id::new("save_destination")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Save project as");
            ui.label("Enter a full destination path ending in .osb.");
            if self.status_error {
                ui.label(&self.status);
            }
            ui.add(
                egui::TextEdit::singleline(&mut destination)
                    .desired_width(410.0)
                    .hint_text("Full project path")
                    .char_limit(32768),
            );
            let path = PathBuf::from(destination.trim());
            let valid =
                path.is_absolute() && path.extension().and_then(|x| x.to_str()) == Some("osb");
            if !destination.is_empty() && !valid {
                ui.label("Use an absolute path with an .osb extension.");
            }
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(valid, egui::Button::new("Save project"))
                    .clicked();
                cancel |= ui.button("Cancel save").clicked();
            });
        });
        if cancel {
            return;
        }
        if apply {
            self.path = destination.trim().to_owned();
            self.save();
            if self.status_error && self.pending.is_none() {
                self.save_destination = Some(destination);
            }
        } else {
            self.save_destination = Some(destination);
        }
    }
    fn history(&mut self, redo: bool) {
        let result = if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        };
        self.report(result, if redo { "Edit redone." } else { "Edit undone." });
        self.refresh_document();
    }

    fn apply_wall(&mut self) {
        if self
            .selected
            .is_some_and(|id| self.editor.document.model().extensions.contains_key(&id))
        {
            self.report(
                Err(Error::Invalid(
                    "Use the selected element's registered plugin command.".into(),
                )),
                "",
            );
            return;
        }
        #[cfg(feature = "external-plugins")]
        if self.review_wall_plugin_command(if self.selected.is_some() {
            os_plugin_api::generic::Mode::Edit
        } else {
            os_plugin_api::generic::Mode::Create
        }) {
            return;
        }
        let old_ids: BTreeSet<_> = self.editor.document.model().walls.keys().copied().collect();
        let request = if let Some(id) = self.selected {
            Request::EditWall {
                id,
                parameters: self.draft.clone(),
            }
        } else {
            Request::CreateWall(self.draft.clone())
        };
        let label = if self.selected.is_some() {
            "Edit wall"
        } else {
            "Create wall"
        };
        let result = self.editor.wall_command(label, request);
        if result.is_ok() && self.selected.is_none() {
            let id = self
                .editor
                .document
                .model()
                .walls
                .keys()
                .find(|id| !old_ids.contains(id))
                .copied();
            self.select(id);
        }
        self.report(result, "Wall model and geometry updated.");
    }

    fn delete_wall(&mut self) {
        if let Some(id) = self
            .selected
            .filter(|id| opening_tools::has_openings(self.editor.document.model(), *id))
        {
            let mut commands: Vec<_> = self
                .editor
                .document
                .model()
                .openings
                .values()
                .filter(|o| o.parameters.host == id)
                .map(|o| Command::RemoveOpening(o.id()))
                .collect();
            commands.push(Command::RemoveWall(id));
            let result = self
                .editor
                .document
                .execute("Delete wall and openings", commands)
                .and_then(|()| self.editor.regenerate());
            if result.is_ok() {
                self.select(None);
            }
            self.report(result, "Wall and hosted openings deleted.");
            return;
        }
        #[cfg(feature = "external-plugins")]
        if self
            .selected
            .is_some_and(|id| self.editor.document.model().walls.contains_key(&id))
            && self.review_wall_plugin_command(os_plugin_api::generic::Mode::Delete)
        {
            return;
        }
        if let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().walls.contains_key(id))
        {
            let result = self.editor.command("Delete wall", Command::RemoveWall(id));
            if result.is_ok() {
                self.select(None);
            }
            self.report(result, "Wall removed.");
        }
    }

    fn set_active_level(&mut self, id: Id) {
        if self.active_level != id {
            self.cancel_plan_wall();
        }
        if self.editor.document.model().levels.contains_key(&id) {
            self.active_level = id;
            if self.selected.is_none() {
                self.draft.level = id;
            }
        }
    }

    fn add_level(&mut self) {
        let model = self.editor.document.model();
        if let Some(building) = model.buildings.keys().next() {
            let level = Level::new(
                "core.level",
                LevelParams {
                    name: format!("Level {}", model.levels.len()),
                    elevation: model
                        .levels
                        .values()
                        .map(|l| l.parameters.elevation)
                        .fold(0.0, f64::max)
                        + 3.0,
                    building: *building,
                },
            );
            let id = level.id();
            let result = self.editor.command("Add level", Command::AddLevel(level));
            if result.is_ok() {
                self.set_active_level(id);
            }
            self.report(result, "Level created. Edit its elevation in Manage.");
        }
    }

    fn show(&mut self, ctx: &egui::Context) {
        if self
            .wall_gesture
            .as_ref()
            .is_some_and(|g| !g.current(&self.editor, self.plans.active))
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.cancel_plan_wall();
        }
        #[cfg(feature = "external-plugins")]
        self.plugin_manager(ctx);
        #[cfg(feature = "external-plugins")]
        self.poll_desktop_open(ctx);
        #[cfg(feature = "external-plugins")]
        self.plugin_forms(ctx);
        if ctx.input(|i| i.viewport().close_requested())
            && self.has_unsaved_work()
            && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.exchange_pending.is_none() {
                self.pending = Some(PendingAction::Close);
            }
        }
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Modal actions must not let global shortcuts modify the document underneath.
        if self.pending.is_none()
            && self.exchange_pending.is_none()
            && !self.plans.sheet_pdf_pending()
            && self.plan_draft.is_none()
            && self.grid_draft.is_none()
            && self.opening_draft.is_none()
            && self.opening_type_draft.is_none()
            && self.wall_type_draft.is_none()
            && self.save_destination.is_none()
            && !ctx.wants_keyboard_input()
        {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.history(false);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)) {
                self.history(true);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
                self.quick_save();
            }
        }
        self.top_bar(ctx);
        self.status_bar(ctx);
        self.palettes(ctx);
        self.plan_workspace(ctx);
        self.opening_schedule_window(ctx);
        self.sheet_pdf_confirmation(ctx);
        self.finish_endpoint_input(ctx);
        self.plan_settings_dialog(ctx);
        self.grid_dialog(ctx);
        self.opening_dialog(ctx);
        self.opening_type_dialog(ctx);
        self.wall_type_dialog(ctx);
        self.save_destination_dialog(ctx);
        self.confirmation(ctx);
        self.exchange_confirmation(ctx);
        #[cfg(feature = "external-plugins")]
        self.desktop_open_dialog(ctx);
        // Commands in the ribbon/shortcuts run after the early worker poll. They
        // can queue geometry this frame even when that poll reported idle.
        #[cfg(feature = "external-plugins")]
        if self.editor.plugin_work_pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn confirmation(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending.clone() {
            let replace = matches!(action, PendingAction::Replace(_));
            egui::Modal::new(egui::Id::new("confirm_document_action")).show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.heading(if replace {
                    "Replace existing project?"
                } else {
                    "Unsaved changes"
                });
                ui.label(if replace {
                    "Saving will replace the file at the selected path."
                } else {
                    "Save your project first, or discard these changes to continue."
                });
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending = None;
                    }
                    if ui
                        .button(if replace {
                            "Replace file"
                        } else {
                            "Discard and continue"
                        })
                        .clicked()
                    {
                        self.pending = None;
                        self.perform(action);
                    }
                });
            });
        }
    }
}
impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}
fn default_wall(level: Id) -> WallParams {
    WallParams {
        name: "Wall".into(),
        start: Point2::new(0.0, 0.0),
        end: Point2::new(5.0, 0.0),
        thickness: 0.2,
        height: 3.0,
        level,
        material: None,
    }
}

#[cfg(test)]
mod desktop_tests {
    use super::*;
    mod opening_family_tests;
    mod room_tag_tests;
    mod wall_join_tests;
    mod wall_type_tests;

    #[cfg(feature = "external-plugins")]
    #[test]
    fn manager_disable_and_restore_retains_document_and_new_retains_plugins() {
        let mut h = Harness::new();
        h.app.apply_wall();
        let model = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        h.click("Manage");
        h.click("Plugin manager");
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(
            h.visible_text_rect("Explicit grants for this load (all denied by default)")
                .is_some()
        );
        h.click("Disable plugin");
        assert_eq!(h.app.editor.host.manifests().count(), 0);
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert!(h.app.editor.scene.is_empty());
        h.click("Restore bundled Wall");
        h.frame(vec![]);
        assert_eq!(h.app.editor.host.manifests().count(), 1);
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.scene.len(), 1);
        let activation = h.app.editor.host.activation_id(os_plugin_api::wall::OWNER);
        h.app.perform(PendingAction::New);
        assert_eq!(
            h.app.editor.host.activation_id(os_plugin_api::wall::OWNER),
            activation
        );
        assert!(h.app.editor.document.model().walls.is_empty());
    }

    #[cfg(feature = "external-plugins")]
    #[test]
    fn desktop_open_adopts_path_only_after_confirmation_and_candidate_success() {
        let mut h = Harness::new();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate.osb");
        ZipJsonStorage
            .save(&Document::new("Candidate").unwrap(), &path)
            .unwrap();
        h.app
            .editor
            .host
            .load(
                Box::new(crate::plugin_jobs::tests::NoWorker),
                [
                    Permission::ModelRead,
                    Permission::ModelWrite,
                    Permission::UiTool,
                ]
                .into(),
            )
            .unwrap();
        h.app
            .editor
            .command("Rename", Command::RenameProject("Unsaved work".into()))
            .unwrap();
        let session = h.app.editor.document.session_id();
        h.app.path = path.display().to_string();
        h.app.request_action(PendingAction::Open);
        assert!(!h.app.editor.plugin_open_pending());
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Cancel");
        assert_eq!(h.app.editor.document.session_id(), session);
        h.app.request_action(PendingAction::Open);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Discard and continue");
        h.frame(vec![]);
        assert_ne!(h.app.editor.document.session_id(), session);
        assert_eq!(
            h.app.editor.document.model().project.parameters.name,
            "Candidate"
        );
        assert_eq!(h.app.opened_path, Some(path));
        assert!(!h.app.editor.is_dirty());
        assert!(h.app.selected.is_none());
    }

    #[cfg(feature = "external-plugins")]
    #[test]
    fn desktop_candidate_failure_keeps_identity_selection_and_opened_path() {
        let mut h = Harness::new();
        let (editor, _, id) = crate::plugin_jobs::tests::editor();
        h.app.editor = editor;
        h.app.refresh_document();
        h.app.select(Some(id));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate.osb");
        ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
        let original_path = dir.path().join("working.osb");
        h.app.opened_path = Some(original_path.clone());
        let session = h.app.editor.document.session_id();
        let model = h.app.editor.document.model().clone();
        h.app.open_path(&path).unwrap();
        assert!(h.app.editor.plugin_open_pending());
        assert_eq!(h.app.opened_path, Some(original_path.clone()));
        h.app.poll_desktop_open(&h.ctx);
        assert!(h.app.status_error);
        assert!(!h.app.editor.plugin_open_pending());
        assert_eq!(h.app.editor.document.session_id(), session);
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.selected, Some(id));
        assert_eq!(h.app.opened_path, Some(original_path));
    }

    #[test]
    fn plugin_selection_is_shared_by_browser_viewport_and_history() {
        let mut h = Harness::new();
        h.app.apply_wall();
        let wall_id = h.app.selected.unwrap();
        let mesh = h.app.editor.scene[&wall_id].clone();
        h.app.delete_wall();
        let entity: os_model::ExtensionEntity =
            serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json"))
                .unwrap();
        let id = entity.id;
        let mut model = h.app.editor.document.model().clone();
        model.plugin_requirements.insert(
            entity.owner.clone(),
            os_model::PluginRequirement {
                version: "1.0.0".into(),
            },
        );
        model.extensions.insert(id, entity);
        h.app.editor.document = Document::from_model(model.clone()).unwrap();
        h.app.editor.scene.insert(id, mesh);
        h.app.fit_requested = true;
        h.app.properties_fraction = 0.5;
        h.frame(vec![]);
        h.frame(vec![]);
        h.click_browser("Preserved column");
        assert_eq!(h.app.selected, Some(id));
        assert!(h.visible_text_rect("Modify | Walls").is_none());
        assert!(h.visible_text_rect("Incomplete view").is_none());
        assert!(
            h.visible_text_rect("Inspect selected plugin data")
                .is_some()
        );
        h.app.apply_wall();
        h.app.delete_wall();
        assert_eq!(h.app.editor.document.model(), &model);
        h.app.select(None);
        h.frame(vec![]);
        let cache = h.app.viewport_cache.as_ref().unwrap();
        let image_rect = h
            .output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Mesh(m) if m.texture_id == cache.texture.id() => Some(m.calc_bounds()),
                _ => None,
            })
            .unwrap();
        let mut point = None;
        for y in (10..cache.frame.height - 10).step_by(8) {
            for x in (10..cache.frame.width - 10).step_by(8) {
                let lx = (x as f32 + 0.5) / cache.frame.width as f32 * image_rect.width();
                let ly = (y as f32 + 0.5) / cache.frame.height as f32 * image_rect.height();
                if cache.frame.pick(lx, ly) == Some(id) {
                    point = Some(image_rect.min + egui::vec2(lx, ly));
                }
            }
        }
        h.click_at(point.expect("visible extension sample"));
        assert_eq!(h.app.selected, Some(id));
        h.app
            .editor
            .command("Rename", Command::RenameProject("Changed".into()))
            .unwrap();
        h.app.history(false);
        assert_eq!(h.app.selected, Some(id));
        h.app
            .editor
            .command("Remove", Command::RemoveExtension(id))
            .unwrap();
        h.app.refresh_document();
        assert!(h.app.selected.is_none());
    }

    #[cfg(feature = "external-plugins")]
    #[test]
    fn browser_selection_targets_form_and_clearing_selection_discards_review() {
        let mut h = Harness::new();
        let (editor, _, id) = crate::plugin_jobs::tests::editor();
        h.app.editor = editor;
        h.app.refresh_document();
        h.app.properties_fraction = 0.5;
        h.frame(vec![]);
        h.frame(vec![]);
        h.click_browser("Preserved column");
        h.click("org.example.columns.edit");
        assert_eq!(h.app.selected, Some(id));
        assert!(h.visible_text_rect("Apply plugin command").is_some());
        h.app.select(None);
        h.frame(vec![]);
        assert!(h.visible_text_rect("Apply plugin command").is_none());
    }

    #[cfg(feature = "external-plugins")]
    #[test]
    fn descriptor_form_invalid_input_and_discard_never_commit() {
        for scale in [1.0, 1.5] {
            let mut h = Harness::at_size(egui::vec2(1000.0, 650.0), scale);
            h.app
                .editor
                .host
                .load(
                    Box::new(crate::plugin_jobs::tests::NoWorker),
                    [
                        Permission::ModelRead,
                        Permission::ModelWrite,
                        Permission::UiTool,
                    ]
                    .into(),
                )
                .unwrap();
            h.frame(vec![]);
            h.frame(vec![]);
            let model = h.app.editor.document.model().clone();
            h.click("org.example.columns.create");
            assert!(h.visible_text_rect("Width").is_some());
            assert!(h.visible_text_rect("metres · 0.001 to 100").is_some());
            h.click("0.4");
            h.replace_focused("not a number");
            assert!(h.visible_text_rect("not a number").is_some());
            h.click("Apply plugin command");
            assert!(!h.app.editor.plugin_work_pending());
            assert_eq!(h.app.editor.document.model(), &model);
            assert!(!h.app.editor.document.can_undo());
            h.click("Discard plugin draft");
            assert!(h.visible_text_rect("Apply plugin command").is_none());
            assert_eq!(h.app.editor.document.model(), &model);
        }
    }

    #[test]
    fn unavailable_plugin_inspection_is_read_only_and_warning_remains_visible() {
        for scale in [1.0, 1.5] {
            let mut h = Harness::at_size(egui::vec2(1000.0, 650.0), scale);
            let mut model = h.app.editor.document.model().clone();
            let entity: os_model::ExtensionEntity =
                serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json"))
                    .unwrap();
            model.plugin_requirements.insert(
                entity.owner.clone(),
                os_model::PluginRequirement {
                    version: "1.0.0".into(),
                },
            );
            model.extensions.insert(entity.id, entity.clone());
            h.app.editor.document = Document::from_model(model.clone()).unwrap();
            h.frame(vec![]);
            h.frame(vec![]);
            assert!(h.visible_text_rect("Incomplete view").is_some());
            h.click("Inspect preserved data");
            h.frame(vec![]);
            h.click("Preserved column");
            assert_eq!(h.app.extension_inspection, Some(entity.id));
            assert_eq!(h.app.editor.document.model(), &model);
            assert_eq!(h.app.editor.document.revision(), 0);
            assert!(!h.app.editor.document.can_undo());
        }
    }

    #[test]
    fn named_plan_split_selects_the_same_wall_and_preserves_navigation_history() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            h.click("Create wall");
            let wall = *h.app.editor.document.model().walls.keys().next().unwrap();
            h.click("New floor plan");
            let view = h.app.plans.active.unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !h.app.plans.ready() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
                h.frame(vec![]);
            }
            h.click("Split 2D / 3D");
            h.click("Fit plan");
            let rect = h.app.plans.canvas_rect.unwrap();
            assert!(rect.width() > 180.0 && rect.height() > 200.0);
            let model = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.click_at(rect.center());
            assert_eq!(h.app.selected, Some(wall));
            assert_eq!(h.app.viewport_cache.as_ref().unwrap().selected, Some(wall));
            h.drag(rect.center(), rect.center() + egui::vec2(20.0, 12.0));
            assert_eq!(h.app.editor.document.model(), &model);
            assert_eq!(h.app.editor.document.revision(), revision);
            h.app.history(false);
            h.frame(vec![]);
            assert!(!h.app.editor.document.model().views.contains_key(&view));
            assert!(h.app.plans.active.is_none());
            h.app.history(true);
            h.frame(vec![]);
            assert!(h.app.editor.document.model().views.contains_key(&view));
        }
    }

    #[test]
    fn plan_settings_modal_validates_drafts_blocks_shortcuts_and_keeps_actions_visible() {
        for size in [egui::vec2(1280.0, 800.0), egui::vec2(1000.0, 650.0)] {
            for scale in [1.0, 1.25, 1.5] {
                let mut h = Harness::at_size(size, scale);
                h.click("New floor plan");
                let view = h.app.plans.active.unwrap();
                let original = h.app.editor.document.model().clone();
                h.click("Plan settings");
                h.frame(vec![]);
                for label in ["Cancel plan settings", "Apply plan settings"] {
                    let rect = h.text_rect(label);
                    assert!(rect.top() >= 0.0 && rect.bottom() <= size.y);
                }
                h.frame(vec![egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }]);
                assert_eq!(h.app.editor.document.model(), &original);
                h.click("Floor plan 1");
                h.replace_focused(" ");
                assert_eq!(h.app.plan_draft.as_ref().unwrap().name, " ");
                h.click("Apply plan settings");
                assert!(h.app.plan_draft.is_some());
                assert_eq!(h.app.editor.document.model(), &original);
                h.click("Cancel plan settings");
                assert!(h.app.plan_draft.is_none());
                h.click("Plan settings");
                h.click("Floor plan 1");
                h.replace_focused("Ground documentation");
                assert_eq!(h.app.editor.document.model(), &original);
                h.click("Apply plan settings");
                assert!(h.app.plan_draft.is_none());
                assert_eq!(
                    h.app.editor.document.model().views[&view].parameters.name,
                    "Ground documentation"
                );
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &original);
            }
        }
    }

    #[test]
    fn pointer_walls_snap_endpoints_commit_once_and_cancel_without_mutation() {
        let mut h = Harness::new();
        h.click("New floor plan");
        let settle = |h: &mut Harness| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !h.app.plans.ready() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
                h.frame(vec![]);
            }
        };
        settle(&mut h);
        h.click("Draw wall in plan");
        let rect = h.app.plans.canvas_rect.unwrap();
        let start = rect.center();
        let end = start + egui::vec2(130.0, 0.0);
        let before = h.app.editor.document.model().clone();
        h.click_at(start);
        h.text_rect("Choose the other endpoint or enter exact dimensions");
        h.frame(vec![egui::Event::PointerMoved(end)]);
        assert_eq!(h.app.editor.document.model(), &before);
        h.click_at(end);
        settle(&mut h);
        assert!(h.app.wall_gesture.is_none());
        assert_eq!(h.app.editor.document.model().walls.len(), 1);
        let first = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .next()
            .unwrap()
            .clone();
        h.click("Draw wall in plan");
        h.click_at(end + egui::vec2(6.0, 0.0));
        assert!(
            h.app
                .wall_gesture
                .as_ref()
                .unwrap()
                .start
                .unwrap()
                .distance(first.parameters.end)
                < 1e-8
        );
        h.click_at(end - egui::vec2(0.0, 130.0));
        settle(&mut h);
        assert_eq!(h.app.editor.document.model().walls.len(), 2);
        let second = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .find(|w| w.id() != first.id())
            .unwrap();
        assert_eq!(second.parameters.start, first.parameters.end);
        assert_eq!(h.app.editor.scene.len(), 2);
        h.app.history(false);
        h.frame(vec![]);
        settle(&mut h);
        assert_eq!(h.app.editor.document.model().walls.len(), 1);
        let before = h.app.editor.document.model().clone();
        h.click("Draw wall in plan");
        h.click_at(start);
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(h.app.wall_gesture.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
    }

    #[test]
    #[cfg(feature = "external-plugins")]
    #[ignore = "requires explicitly installed independent Wall guest via OPENSTRUCTURE_WALL_TEST_PLUGIN"]
    fn installed_pointer_ui_creates_snapped_walls_through_worker() {
        let directory = std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN")
            .expect("Set OPENSTRUCTURE_WALL_TEST_PLUGIN to an installed Wall directory");
        let mut h = Harness::new();
        h.app
            .editor
            .host
            .unload(os_plugin_api::wall::OWNER)
            .unwrap();
        h.app
            .editor
            .host
            .load_wasm_directory(
                Path::new(&directory),
                [
                    Permission::ModelRead,
                    Permission::ModelWrite,
                    Permission::UiTool,
                ]
                .into(),
            )
            .unwrap();
        h.frame(vec![]);
        h.click("New floor plan");
        let settle = |h: &mut Harness| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                h.frame(vec![]);
                assert!(!h.app.status_error, "{}", h.app.status);
                if h.app.plans.ready() && !h.app.editor.plugin_work_pending() {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
        };
        settle(&mut h);
        h.click("Draw wall in plan");
        assert!(
            h.app
                .wall_gesture
                .as_ref()
                .is_some_and(|gesture| gesture.is_installed()),
            "installed Wall gesture missing after toolbar click: active={:?}, ready={}, status={}",
            h.app.plans.active,
            h.app.plans.ready(),
            h.app.status
        );
        let rect = h.app.plans.canvas_rect.unwrap();
        let start = egui::pos2(rect.left() + 50.0, rect.bottom() - 50.0);
        let end = start + egui::vec2(130.0, 0.0);
        h.click_at(start);
        assert!(h.app.editor.document.model().walls.is_empty());
        h.click_at(end);
        settle(&mut h);
        assert_eq!(h.app.editor.document.model().walls.len(), 1);
        let first = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .next()
            .unwrap()
            .clone();
        h.click("Draw wall in plan");
        h.click_at(end + egui::vec2(4.0, 0.0));
        let gesture = h.app.wall_gesture.as_mut().unwrap();
        assert!(gesture.start.unwrap().distance(first.parameters.end) < 1e-8);
        gesture.length = "3".into();
        gesture.angle_degrees = "90".into();
        h.click_at(end - egui::vec2(0.0, 80.0));
        settle(&mut h);
        assert_eq!(h.app.editor.document.model().walls.len(), 2);
        let second = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .find(|w| w.id() != first.id())
            .unwrap();
        assert_eq!(second.parameters.start, first.parameters.end);
        assert!((second.parameters.length() - 3.0).abs() < 1e-8);
        assert_eq!(h.app.editor.scene.len(), 2);
        let two = h.app.editor.document.model().clone();
        for (button, mode) in [
            ("Move wall", crate::plan_gesture::WallEdit::Move),
            ("Resize start", crate::plan_gesture::WallEdit::ResizeStart),
            ("Resize end", crate::plan_gesture::WallEdit::ResizeEnd),
            ("Offset wall", crate::plan_gesture::WallEdit::OffsetCopy),
        ] {
            h.app.select(Some(first.id()));
            h.frame(vec![]);
            h.click(button);
            assert!(
                h.app
                    .wall_gesture
                    .as_ref()
                    .is_some_and(|g| g.is_installed()),
                "{button}: {}",
                h.app.status
            );
            if mode == crate::plan_gesture::WallEdit::Move {
                h.click_at(start);
            }
            let gesture = h.app.wall_gesture.as_mut().unwrap();
            gesture.length = if mode == crate::plan_gesture::WallEdit::OffsetCopy {
                "-1"
            } else {
                "1"
            }
            .into();
            if mode != crate::plan_gesture::WallEdit::OffsetCopy {
                gesture.angle_degrees = "35".into();
            }
            let expected = gesture.parameters(Point2::default()).unwrap();
            h.click_at(end);
            settle(&mut h);
            let copied = mode == crate::plan_gesture::WallEdit::OffsetCopy;
            assert_eq!(
                h.app.editor.document.model().walls.len(),
                2 + usize::from(copied)
            );
            let actual = if copied {
                assert_eq!(
                    h.app.editor.document.model().walls[&first.id()],
                    two.walls[&first.id()]
                );
                h.app
                    .editor
                    .document
                    .model()
                    .walls
                    .values()
                    .find(|w| !two.walls.contains_key(&w.id()))
                    .unwrap()
            } else {
                &h.app.editor.document.model().walls[&first.id()]
            };
            assert_eq!(actual.parameters.start, expected.start);
            assert_eq!(actual.parameters.end, expected.end);
            h.app.history(false);
            settle(&mut h);
            assert_eq!(*h.app.editor.document.model(), two);
        }
        h.click("Draw wall in plan");
        h.click_at(start);
        let point = start + egui::vec2(90.0, -15.0);
        h.frame(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        // Stop at release: admission happens after this frame's worker poll.
        h.frame(vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(h.app.editor.plugin_work_pending());
        assert!(h.app.wall_gesture.is_none());
        assert!(h.app.status.contains("pending"));
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(h.app.status_error);
        assert_eq!(*h.app.editor.document.model(), two);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !h
            .app
            .editor
            .host
            .worker_available(os_plugin_api::wall::OWNER)
        {
            assert!(std::time::Instant::now() < deadline);
            h.frame(vec![]);
            std::thread::yield_now();
        }
        assert!(!h.app.editor.plugin_work_pending());
        assert_eq!(*h.app.editor.document.model(), two);
        h.app.history(false);
        settle(&mut h);
        assert_eq!(h.app.editor.document.model().walls.len(), 1);
        let before = h.app.editor.document.model().clone();
        h.click("Draw wall in plan");
        h.click_at(start);
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(h.app.wall_gesture.is_none());
        assert_eq!(*h.app.editor.document.model(), before);
    }

    #[test]
    fn untitled_quick_save_requires_destination_and_tracks_successful_file() {
        let mut h = Harness::new();
        h.click("Create wall");
        let original = h.app.editor.document.model().clone();
        h.click("Save");
        assert!(h.app.save_destination.is_some());
        assert!(h.app.opened_path.is_none());
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }]);
        assert_eq!(h.app.editor.document.model(), &original);
        h.click("Cancel save");
        assert!(h.app.save_destination.is_none());
        assert!(h.app.editor.is_dirty());
        h.click("Save");
        h.click("Full project path");
        h.replace_focused("relative.osb");
        h.click("Save project");
        assert!(h.app.save_destination.is_some());
        assert!(h.app.opened_path.is_none());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chosen.osb");
        let missing = dir.path().join("missing").join("failed.osb");
        h.click("relative.osb");
        h.replace_focused(&missing.display().to_string());
        h.click("Save project");
        assert!(h.app.status_error);
        assert!(h.app.save_destination.is_some());
        assert!(h.app.opened_path.is_none());
        assert!(!missing.exists());
        assert!(h.app.editor.is_dirty());
        h.click(&missing.display().to_string());
        h.replace_focused(&path.display().to_string());
        h.click("Save project");
        assert!(h.app.save_destination.is_none());
        assert_eq!(h.app.opened_path.as_ref(), Some(&path));
        assert!(!h.app.editor.is_dirty());
        // Editing the File menu's prospective path must not redirect Ctrl+S.
        let other = dir.path().join("not-selected.osb");
        h.app.path = other.display().to_string();
        h.app
            .editor
            .command("Rename", Command::RenameProject("Saved again".into()))
            .unwrap();
        h.app.quick_save();
        assert!(!other.exists());
        let mut reopened = Editor::new().unwrap();
        reopened.open(&path).unwrap();
        assert_eq!(reopened.document.model(), h.app.editor.document.model());
    }

    #[test]
    fn exact_offset_is_not_blocked_by_dense_intersection_acquisition() {
        let mut h = Harness::new();
        h.click("Create wall");
        let building = h.app.editor.document.model().levels[&h.app.active_level]
            .parameters
            .building;
        let grids = (0..257)
            .map(|index| {
                Command::AddGrid(os_model::Grid::new(
                    "core.grid",
                    os_model::GridParams {
                        name: format!("Dense {index}"),
                        building,
                        start: Point2::new(-3.0, 0.0),
                        end: Point2::new(3.0, 0.0),
                    },
                ))
            })
            .collect();
        h.app
            .editor
            .document
            .execute("Dense fixture", grids)
            .unwrap();
        h.app.editor.regenerate().unwrap();
        h.frame(vec![]);
        h.click("New floor plan");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !h.app.plans.ready() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
            h.frame(vec![]);
        }
        let original = h.app.editor.document.model().clone();
        h.click("Offset wall");
        let center = h.app.plans.canvas_rect.unwrap().center();
        h.frame(vec![egui::Event::PointerMoved(center)]);
        assert!(h.output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(t) if t.galley.job.text.contains("256 nearby segments"))));
        let label = h.text_rect("Offset (m)");
        h.click_at(egui::pos2(label.right() + 35.0, label.center().y));
        h.replace_focused("2");
        let center = h.app.plans.canvas_rect.unwrap().center();
        h.frame(vec![egui::Event::PointerMoved(center)]);
        h.text_rect("Offset +2.000 m");
        assert_eq!(h.app.editor.document.model(), &original);
        h.click_at(center);
        assert!(h.app.wall_gesture.is_none());
        assert_eq!(h.app.editor.document.model().walls.len(), 2);
        assert_eq!(h.app.editor.document.model().grids, original.grids);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &original);
    }

    #[test]
    fn pointer_wall_can_start_on_an_enabled_axis_extension() {
        let mut h = Harness::new();
        h.click("New floor plan");
        let settle = |h: &mut Harness| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !h.app.plans.ready() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
                h.frame(vec![]);
            }
        };
        settle(&mut h);
        h.click("Draw wall in plan");
        let center = h.app.plans.canvas_rect.unwrap().center();
        h.click_at(center);
        h.click_at(center + egui::vec2(130.0, 0.0));
        settle(&mut h);
        let original = h.app.editor.document.model().clone();
        h.click("Snaps");
        h.click("Line axis extensions");
        h.click("Draw wall in plan");
        let center = h.app.plans.canvas_rect.unwrap().center();
        let target = center + egui::vec2(195.0, 5.0);
        h.frame(vec![egui::Event::PointerMoved(target)]);
        h.text_rect("AxisExtension");
        h.click_at(target);
        assert_eq!(
            h.app.wall_gesture.as_ref().unwrap().start,
            Some(Point2::new(3.0, 0.0))
        );
        assert_eq!(h.app.editor.document.model(), &original);
        h.click_at(center + egui::vec2(195.0, -130.0));
        settle(&mut h);
        let model = h.app.editor.document.model();
        assert_eq!(model.walls.len(), 2);
        let added = model
            .walls
            .iter()
            .find(|(id, _)| !original.walls.contains_key(id))
            .unwrap()
            .1;
        assert_eq!(added.parameters.start, Point2::new(3.0, 0.0));
        assert_eq!(added.parameters.end, Point2::new(3.0, 2.0));
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &original);
    }

    #[test]
    fn pointer_wall_endpoint_acquires_perpendicular_foot_from_anchor() {
        let mut h = Harness::new();
        h.click("Create wall");
        h.click("New floor plan");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !h.app.plans.ready() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
            h.frame(vec![]);
        }
        let original = h.app.editor.document.model().clone();
        h.click("Draw wall in plan");
        let center = h.app.plans.canvas_rect.unwrap().center();
        h.click_at(center + egui::vec2(65.0, -130.0));
        let target = center + egui::vec2(70.0, 4.0);
        h.frame(vec![egui::Event::PointerMoved(target)]);
        h.text_rect("Perpendicular");
        assert_eq!(h.app.editor.document.model(), &original);
        h.click_at(target);
        let model = h.app.editor.document.model();
        assert_eq!(model.walls.len(), 2);
        let added = model
            .walls
            .iter()
            .find(|(id, _)| !original.walls.contains_key(id))
            .unwrap()
            .1;
        assert_eq!(added.parameters.start, Point2::new(1.0, 2.0));
        assert_eq!(added.parameters.end, Point2::new(1.0, 0.0));
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &original);
    }

    #[test]
    fn pointer_offset_previews_cancels_and_commits_exact_parallel_copy() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            h.click("Create wall");
            let id = h.app.selected.unwrap();
            h.click("New floor plan");
            let settle = |h: &mut Harness| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !h.app.plans.ready() {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                    h.frame(vec![]);
                }
            };
            settle(&mut h);
            let original = h.app.editor.document.model().clone();
            h.click("Offset wall");
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.frame(vec![egui::Event::PointerMoved(
                center - egui::vec2(0.0, 65.0),
            )]);
            h.text_rect("Offset +1.000 m");
            assert_eq!(h.app.editor.document.model(), &original);
            h.click("Cancel wall");
            assert_eq!(h.app.editor.document.model(), &original);
            h.click("Offset wall");
            let label = h.text_rect("Offset (m)");
            h.click_at(egui::pos2(label.right() + 35.0, label.center().y));
            h.replace_focused("-2");
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.frame(vec![egui::Event::PointerMoved(
                center - egui::vec2(0.0, 65.0),
            )]);
            h.text_rect("Offset -2.000 m");
            assert_eq!(h.app.editor.document.model(), &original);
            h.click_at(center - egui::vec2(0.0, 65.0));
            settle(&mut h);
            let changed = h.app.editor.document.model().clone();
            assert_eq!(changed.walls[&id], original.walls[&id]);
            assert_eq!(changed.walls.len(), 2);
            let copy = changed.walls.values().find(|wall| wall.id() != id).unwrap();
            assert_eq!(copy.parameters.start, Point2::new(0.0, -2.0));
            assert_eq!(copy.parameters.end, Point2::new(5.0, -2.0));
            assert_eq!(h.app.editor.scene.len(), 2);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &original);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &changed);
        }
    }

    #[test]
    fn snap_menu_changes_acquisition_without_model_or_history_edits() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            h.click("New floor plan");
            let settle = |h: &mut Harness| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !h.app.plans.ready() {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                    h.frame(vec![]);
                }
            };
            settle(&mut h);
            h.click("Draw wall in plan");
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.click_at(center);
            h.click_at(center + egui::vec2(130.0, 0.0));
            settle(&mut h);
            let original = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            for enabled in [false, true] {
                h.click(if enabled { "Snaps off" } else { "Snaps" });
                for label in [
                    "Enable snapping",
                    "Endpoints",
                    "Intersections",
                    "Midpoints",
                    "Nearest lines and grid axes",
                ] {
                    assert!(
                        h.visible_text_rect(label).is_some(),
                        "snap option clipped: {label}"
                    );
                }
                h.click("Enable snapping");
                h.click(if enabled { "Snaps" } else { "Snaps off" });
                h.click("Draw wall in plan");
                let center = h.app.plans.canvas_rect.unwrap().center();
                h.click_at(center + egui::vec2(6.0, 4.0));
                let start = h.app.wall_gesture.as_ref().unwrap().start.unwrap();
                if enabled {
                    assert_eq!(start, Point2::default());
                } else {
                    assert!(start.distance(Point2::new(6.0 / 65.0, -4.0 / 65.0)) < 1e-8);
                }
                h.click("Cancel wall");
                assert_eq!(h.app.editor.document.model(), &original);
                assert_eq!(h.app.editor.document.revision(), revision);
            }
        }
    }

    #[test]
    fn pointer_wall_acquires_semantic_intersection_of_two_walls() {
        let mut h = Harness::new();
        h.click("New floor plan");
        let settle = |h: &mut Harness| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !h.app.plans.ready() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
                h.frame(vec![]);
            }
        };
        settle(&mut h);
        for (start, end) in [
            (egui::vec2(-130.0, 0.0), egui::vec2(195.0, 0.0)),
            (egui::vec2(0.0, 130.0), egui::vec2(0.0, -195.0)),
        ] {
            h.click("Draw wall in plan");
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.click_at(center + start);
            h.click_at(center + end);
            settle(&mut h);
        }
        let original = h.app.editor.document.model().clone();
        h.click("Draw wall in plan");
        h.click("Snaps");
        assert!(
            h.visible_text_rect("Intersections").is_some(),
            "first opening"
        );
        h.click("Intersections");
        let center = h.app.plans.canvas_rect.unwrap().center();
        h.frame(vec![egui::Event::PointerMoved(
            center + egui::vec2(5.0, 4.0),
        )]);
        h.text_rect("Nearest");
        assert_eq!(h.app.editor.document.model(), &original);
        h.click("Snaps");
        assert!(
            h.visible_text_rect("Intersections").is_some(),
            "second opening"
        );
        h.click("Intersections");
        let center = h.app.plans.canvas_rect.unwrap().center();
        h.frame(vec![egui::Event::PointerMoved(
            center + egui::vec2(5.0, 4.0),
        )]);
        h.text_rect("Intersection");
        h.click_at(center + egui::vec2(5.0, 4.0));
        assert_eq!(
            h.app.wall_gesture.as_ref().unwrap().start,
            Some(Point2::default())
        );
        assert_eq!(h.app.editor.document.model(), &original);
        h.click_at(center + egui::vec2(65.0, -65.0));
        settle(&mut h);
        assert_eq!(h.app.editor.document.model().walls.len(), 3);
        let wall = h
            .app
            .editor
            .document
            .model()
            .walls
            .iter()
            .find(|(id, _)| !original.walls.contains_key(id))
            .unwrap()
            .1;
        assert_eq!(wall.parameters.start, Point2::default());
        assert_eq!(wall.parameters.end, Point2::new(1.0, 1.0));
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &original);
    }

    #[test]
    fn pointer_wall_edits_preserve_identity_preview_and_undo() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            let settle = |h: &mut Harness| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !h.app.plans.ready() {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                    h.frame(vec![]);
                }
            };
            h.click("New floor plan");
            settle(&mut h);
            h.click("Draw wall in plan");
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.click_at(center);
            h.click_at(center + egui::vec2(130.0, 0.0));
            settle(&mut h);
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.click_at(center + egui::vec2(65.0, 0.0));
            let id = h.app.selected.unwrap();
            let original = h.app.editor.document.model().clone();
            h.click("Move wall");
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.click_at(center);
            let destination = h.app.plans.canvas_rect.unwrap().center() + egui::vec2(65.0, -65.0);
            h.frame(vec![egui::Event::PointerMoved(destination)]);
            assert_eq!(h.app.editor.document.model(), &original);
            h.click_at(destination);
            settle(&mut h);
            let moved = h.app.editor.document.model().clone();
            assert_eq!(moved.walls[&id].header, original.walls[&id].header);
            assert_eq!(moved.walls[&id].parameters.start, Point2::new(1.0, 1.0));
            assert_eq!(moved.walls[&id].parameters.end, Point2::new(3.0, 1.0));
            assert_eq!(h.app.editor.scene.len(), 1);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &original);
            h.app.history(true);
            h.frame(vec![]);
            settle(&mut h);
            assert_eq!(h.app.editor.document.model(), &moved);
            h.click("Resize end");
            let canvas = h.app.plans.canvas_rect.unwrap();
            let destination = canvas.center() + egui::vec2(65.0, -100.0);
            h.frame(vec![egui::Event::PointerMoved(destination)]);
            assert_eq!(h.app.editor.document.model(), &moved);
            h.click_at(destination);
            settle(&mut h);
            let resized = h.app.editor.document.model().clone();
            assert_eq!(resized.walls[&id].header, original.walls[&id].header);
            assert_eq!(resized.walls[&id].parameters.start, Point2::new(1.0, 1.0));
            assert_ne!(
                resized.walls[&id].parameters.end, moved.walls[&id].parameters.end,
                "resize must commit at scale={scale}"
            );
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &moved);
            h.app.history(true);
            h.frame(vec![]);
            settle(&mut h);
            assert_eq!(h.app.editor.document.model(), &resized);
            h.click("Move wall");
            let center = h.app.plans.canvas_rect.unwrap().center();
            h.click_at(center);
            h.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(h.app.wall_gesture.is_none());
            assert_eq!(h.app.editor.document.model(), &resized);
        }
    }

    #[test]
    fn plan_grid_selection_and_wall_axis_snap_use_semantic_datum() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            h.click("New floor plan");
            h.click("New grid");
            h.click("Grid 1");
            h.replace_focused("A");
            h.click("Apply grid");
            let grid = h
                .app
                .editor
                .document
                .model()
                .grids
                .values()
                .next()
                .unwrap()
                .clone();
            let settle = |h: &mut Harness| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !h.app.plans.ready() {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                    h.frame(vec![]);
                }
            };
            settle(&mut h);
            let rect = h.app.plans.canvas_rect.unwrap();
            h.click_at(rect.center() + egui::vec2(65.0, 5.0));
            assert_eq!(h.app.selected, Some(grid.id()));
            h.text_rect("Architectural grid A");
            h.click("Draw wall in plan");
            let rect = h.app.plans.canvas_rect.unwrap();
            h.frame(vec![egui::Event::PointerMoved(
                rect.center() + egui::vec2(65.0, 5.0),
            )]);
            h.text_rect("GridAxis");
            let before = h.app.editor.document.model().clone();
            h.click_at(rect.center() + egui::vec2(65.0, 5.0));
            assert_eq!(
                h.app.wall_gesture.as_ref().unwrap().start,
                Some(Point2::new(1.0, 0.0))
            );
            assert_eq!(h.app.editor.document.model(), &before);
            h.click_at(rect.center() + egui::vec2(65.0, -130.0));
            settle(&mut h);
            let wall = h.app.editor.document.model().walls.values().next().unwrap();
            assert_eq!(wall.parameters.start, Point2::new(1.0, 0.0));
            assert_eq!(wall.parameters.end, Point2::new(1.0, 2.0));
            assert_eq!(h.app.editor.document.model().grids[&grid.id()], grid);
            h.app.history(false);
            h.frame(vec![]);
            settle(&mut h);
            assert_eq!(h.app.editor.document.model(), &before);
        }
    }

    #[test]
    fn grid_forms_create_edit_cancel_and_keep_footer_visible() {
        for size in [egui::vec2(1280.0, 800.0), egui::vec2(1000.0, 650.0)] {
            for scale in [1.0, 1.25, 1.5] {
                let mut h = Harness::at_size(size, scale);
                h.click("New floor plan");
                let original = h.app.editor.document.model().clone();
                h.click("New grid");
                h.frame(vec![]);
                for label in ["Apply grid", "Cancel grid"] {
                    let rect = h.visible_text_rect(label).expect("grid footer clipped");
                    assert!(rect.top() >= 0.0 && rect.bottom() <= size.y);
                }
                h.frame(vec![egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }]);
                assert_eq!(h.app.editor.document.model(), &original);
                h.click("Grid 1");
                h.replace_focused(" ");
                h.click("Apply grid");
                assert!(h.app.grid_draft.is_some());
                let errors=h.output.shapes.iter().filter(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.job.text.starts_with("Invalid data: grid name"))).count();
                assert_eq!(
                    errors, 1,
                    "Apply must not duplicate the preview validation message"
                );
                assert_eq!(h.app.editor.document.model(), &original);
                h.click("Cancel grid");
                assert!(h.app.grid_draft.is_none());
                h.click("New grid");
                h.click("Apply grid");
                assert!(h.app.grid_draft.is_none());
                let id = h.app.selected.unwrap();
                let created = h.app.editor.document.model().clone();
                assert_eq!(created.grids[&id].parameters.start, Point2::new(-3.0, 0.0));
                h.click("Edit grid");
                h.click("Grid 1");
                h.replace_focused("Axis A");
                h.click("3");
                h.replace_focused("4");
                h.text_rect("Preview extent: 7.000000 m");
                assert_eq!(h.app.editor.document.model(), &created);
                h.click("Apply grid");
                assert_eq!(
                    h.app.editor.document.model().grids[&id].parameters.name,
                    "Axis A"
                );
                assert_eq!(
                    h.app.editor.document.model().grids[&id].parameters.end,
                    Point2::new(4.0, 0.0)
                );
                assert_eq!(
                    h.app.editor.document.model().grids[&id].header,
                    created.grids[&id].header
                );
                h.app.history(false);
                h.frame(vec![]);
                assert_eq!(h.app.editor.document.model(), &created);
                h.click("Edit grid");
                h.frame(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]);
                assert!(h.app.grid_draft.is_none());
                assert_eq!(h.app.editor.document.model(), &created);
            }
        }
    }

    #[test]
    fn opening_forms_create_edit_delete_cancel_and_atomic_host_delete() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            let level = h.app.active_level;
            let view = h
                .app
                .editor
                .create_floor_plan("Fixture plan", level)
                .unwrap();
            h.app.focus_plan(Some(view));
            h.app.apply_wall();
            let host = h.app.selected.unwrap();
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            h.click("Exact new…");
            h.click("Door dimensions…");
            h.frame(vec![]);
            assert!(h.app.opening_draft.is_some());
            assert!(h.visible_text_rect("Apply opening").is_some());
            assert_eq!(h.app.editor.document.model(), &before);
            h.click("Cancel opening");
            assert_eq!(h.app.editor.document.model(), &before);
            h.click("Exact new…");
            h.click("Door dimensions…");
            h.click("Apply opening");
            assert!(h.app.opening_draft.is_none(), "{}", h.app.status);
            let door = h.app.selected.unwrap();
            assert!(h.app.editor.document.model().openings.contains_key(&door));
            assert_eq!(h.app.editor.document.model().opening_types.len(), 1);
            let created = h.app.editor.document.model().clone();
            h.click("Edit opening");
            h.frame(vec![]);
            h.click("2.05");
            h.replace_focused("1.0");
            assert_eq!(h.app.editor.document.model(), &created);
            h.click("Apply opening");
            assert_eq!(
                h.app.editor.document.model().openings[&door]
                    .parameters
                    .offset,
                1.0
            );
            h.app.history(false);
            h.frame(vec![]);
            assert_eq!(h.app.editor.document.model(), &created);
            h.click("Edit opening");
            h.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(h.app.opening_draft.is_none());
            assert_eq!(h.app.editor.document.model(), &created);
            h.frame(vec![]);
            h.frame(vec![]);
            h.click("Edit opening");
            h.click("Delete opening");
            assert!(h.app.editor.document.model().openings.is_empty());
            h.app.history(false);
            h.app.select(Some(host));
            h.app.delete_wall();
            assert!(h.app.editor.document.model().walls.is_empty());
            assert!(h.app.editor.document.model().openings.is_empty());
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &created);
            h.app.select(Some(host));
            h.frame(vec![]);
            h.click("Exact new…");
            h.click("Window dimensions…");
            h.frame(vec![]);
            h.click("1.9");
            h.replace_focused("3.0");
            h.click("Apply opening");
            assert!(h.app.opening_draft.is_none(), "{}", h.app.status);
            let window = h.app.selected.unwrap();
            assert_eq!(
                h.app
                    .editor
                    .document
                    .model()
                    .resolve_opening(&h.app.editor.document.model().openings[&window].parameters)
                    .unwrap()
                    .kind,
                os_model::OpeningKind::Window
            );
            assert!(h.app.editor.scene.contains_key(&window));
            h.app.select(Some(door));
            assert_eq!(h.app.selected, Some(door));
        }
    }

    #[test]
    fn room_tool_places_derived_spaces_preserves_identity_and_reports_broken_faces() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            let mut h = Harness::at_size(size, scale);
            let level = h.app.active_level;
            for (name, start, end) in [
                ("South", Point2::new(0.0, 0.0), Point2::new(4.0, 0.0)),
                ("East", Point2::new(4.0, 0.0), Point2::new(4.0, 3.0)),
                ("North", Point2::new(4.0, 3.0), Point2::new(0.0, 3.0)),
                ("West", Point2::new(0.0, 3.0), Point2::new(0.0, 0.0)),
            ] {
                h.app
                    .editor
                    .command(
                        "Add boundary wall",
                        Command::AddWall(os_model::Wall::new(
                            "org.openstructure.walls.wall",
                            WallParams {
                                name: name.into(),
                                start,
                                end,
                                thickness: 0.2,
                                height: 3.0,
                                level,
                                material: None,
                            },
                        )),
                    )
                    .unwrap();
            }
            let view = h.app.editor.create_floor_plan("Room test", level).unwrap();
            h.app.focus_plan(Some(view));
            h.settle_plan();
            h.click("Room");
            let before = h.app.editor.document.model().clone();
            assert!(h.app.plans.room_placement_active);
            assert_eq!(h.app.editor.document.model(), &before);
            h.click_at(h.plan_point(view, Point2::new(1.0, 1.0)));
            assert_eq!(
                h.app.editor.document.model().rooms.len(),
                1,
                "{}",
                h.app.status
            );
            let room_id = *h.app.editor.document.model().rooms.keys().next().unwrap();
            let room = &h.app.editor.document.model().rooms[&room_id];
            assert_eq!(room.parameters.number, "1");
            assert_eq!(room.parameters.level, level);
            assert_eq!(room.parameters.seed, Point2::new(1.0, 1.0));
            assert_eq!(room.parameters.boundary_signature.len(), 4);
            h.settle_plan();
            let context = h.app.editor.native_plan_context(view).unwrap();
            let plan_room = h
                .app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .rooms(context)
                .unwrap()[0]
                .clone();
            assert!((plan_room.area_m2 - 12.0).abs() < 1e-8);
            assert_eq!(plan_room.boundary.len(), 4);
            assert!(plan_room.diagnostic.is_none());

            let after_place = h.app.editor.document.model().clone();
            h.app.history(false);
            assert!(h.app.editor.document.model().rooms.is_empty());
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &after_place);
            h.app.select(Some(room_id));
            h.settle_plan();

            let before_invalid = h.app.editor.document.model().clone();
            assert!(h.app.plans.room_placement_active);
            let outside = h.plan_point(view, Point2::new(4.4, 1.5));
            assert!(h.app.plans.canvas_rect.unwrap().contains(outside));
            h.click_at(outside);
            assert!(
                h.app.plans.room_placement_active,
                "room tool exited after click: {}",
                h.app.status
            );
            assert_eq!(h.app.editor.document.model(), &before_invalid);
            assert!(
                h.app.status_error,
                "unexpected status after outside click: {}",
                h.app.status
            );

            h.app.room_number_draft = "A-101".into();
            h.app.room_name_draft = "Living room".into();
            h.frame(vec![]);
            assert_eq!(h.app.selected, Some(room_id));
            assert!(
                h.visible_text_rect("Native room · area derived from wall boundaries")
                    .is_some()
            );
            h.click("Apply room properties");
            assert_eq!(
                h.app.editor.document.model().rooms[&room_id]
                    .parameters
                    .number,
                "A-101"
            );
            assert_eq!(
                h.app.editor.document.model().rooms[&room_id]
                    .parameters
                    .name,
                "Living room"
            );
            h.app.history(false);
            assert_eq!(
                h.app.editor.document.model().rooms[&room_id]
                    .parameters
                    .number,
                "1"
            );
            h.app.history(true);
            assert_eq!(
                h.app.editor.document.model().rooms[&room_id]
                    .parameters
                    .name,
                "Living room"
            );

            let old_signature = h.app.editor.document.model().rooms[&room_id]
                .parameters
                .boundary_signature
                .clone();
            h.app
                .editor
                .command(
                    "Add partition",
                    Command::AddWall(os_model::Wall::new(
                        "org.openstructure.walls.wall",
                        WallParams {
                            name: "Partition".into(),
                            start: Point2::new(2.0, 0.0),
                            end: Point2::new(2.0, 3.0),
                            thickness: 0.1,
                            height: 3.0,
                            level,
                            material: None,
                        },
                    )),
                )
                .unwrap();
            assert_eq!(
                h.app.editor.document.model().rooms[&room_id]
                    .parameters
                    .boundary_signature,
                old_signature
            );
            h.settle_plan();
            let broken_context = h.app.editor.native_plan_context(view).unwrap();
            let broken = h
                .app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .rooms(broken_context)
                .unwrap()[0]
                .clone();
            assert!(broken.boundary.is_empty());
            assert_eq!(broken.area_m2, 0.0);
            assert!(
                broken
                    .diagnostic
                    .as_deref()
                    .unwrap()
                    .contains("boundary changed")
            );
            h.app.history(false);
            h.settle_plan();
            let restored_context = h.app.editor.native_plan_context(view).unwrap();
            let restored = h
                .app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .rooms(restored_context)
                .unwrap()[0]
                .clone();
            assert!((restored.area_m2 - 12.0).abs() < 1e-8);
            assert_eq!(restored.entity, room_id);

            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("rooms.osb");
            ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
            let reopened = ZipJsonStorage.open(&path).unwrap();
            assert_eq!(reopened.model(), h.app.editor.document.model());
            let escape = egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            h.frame(vec![escape]);
            assert!(!h.app.plans.room_placement_active);
        }
    }

    /// Feed real egui input through the same frame function as the native app.
    struct Harness {
        app: DesktopApp,
        ctx: egui::Context,
        output: egui::FullOutput,
        time: f64,
        size: egui::Vec2,
        scale: f32,
    }
    impl Harness {
        fn new() -> Self {
            Self::at_size(egui::vec2(1280.0, 800.0), 1.0)
        }
        fn at_size(size: egui::Vec2, scale: f32) -> Self {
            let mut harness = Self {
                app: DesktopApp::new().unwrap(),
                ctx: egui::Context::default(),
                output: egui::FullOutput::default(),
                time: 0.0,
                size,
                scale,
            };
            theme::apply(&harness.ctx);
            harness.frame(vec![]);
            harness.frame(vec![]);
            harness
        }
        fn frame(&mut self, events: Vec<egui::Event>) {
            self.time += 1.0 / 60.0;
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .native_pixels_per_point = Some(self.scale);
            self.output = self.ctx.run(input, |ctx| self.app.show(ctx));
        }
        fn visible_text_rect(&self, text: &str) -> Option<egui::Rect> {
            self.output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(t) if t.galley.job.text == text => {
                        let rect = egui::Rect::from_min_size(t.pos, t.galley.size());
                        shape.clip_rect.contains_rect(rect).then_some(rect)
                    }
                    _ => None,
                })
                .next_back()
        }
        fn text_rect(&self, text: &str) -> egui::Rect {
            self.output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(t) if t.galley.job.text == text => {
                        Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
                    }
                    _ => None,
                })
                .next_back()
                .unwrap_or_else(|| panic!("text not rendered: {text}"))
        }
        fn click_at(&mut self, pos: egui::Pos2) {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame(vec![]);
        }
        fn click(&mut self, text: &str) {
            let pos = self.text_rect(text).center();
            self.click_at(pos);
        }
        fn click_browser(&mut self, text: &str) {
            for _ in 0..20 {
                let heading_bottom = self.text_rect("Project Browser").bottom();
                let visible = self
                    .output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(t) if t.galley.job.text == text => {
                            let rect = egui::Rect::from_min_size(t.pos, t.galley.size());
                            (rect.top() > heading_bottom && shape.clip_rect.contains_rect(rect))
                                .then_some(rect)
                        }
                        _ => None,
                    })
                    .next_back();
                if let Some(rect) = visible {
                    self.click_at(rect.center());
                    return;
                }
                let pointer = self.text_rect("Project Browser").center() + egui::vec2(0.0, 75.0);
                self.frame(vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -90.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                for _ in 0..8 {
                    self.frame(vec![]);
                }
            }
            panic!("Browser entry is not reachable: {text}");
        }
        fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
            self.frame(vec![egui::Event::PointerMoved(from)]);
            self.frame(vec![egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }]);
            for step in 1..=8 {
                self.frame(vec![egui::Event::PointerMoved(
                    from + (to - from) * (step as f32 / 8.0),
                )]);
            }
            self.frame(vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame(vec![]);
        }
        fn replace_focused(&mut self, text: &str) {
            self.frame(vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }]);
            self.frame(vec![egui::Event::Text(text.into())]);
            self.frame(vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame(vec![]);
        }
        fn dimension(&mut self, label: &str, value: &str) {
            self.reveal_property(label);
            let label_rect = self.text_rect(label);
            let pos = self
                .output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t)
                        if t.pos.x > label_rect.right()
                            && (t.pos.y - label_rect.top()).abs() < 4.0 =>
                    {
                        Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                    }
                    _ => None,
                })
                .next()
                .expect("dimension value missing");
            self.click_at(pos);
            self.click_at(pos);
            self.replace_focused(value);
        }
        fn scroll_properties(&mut self, delta: f32) {
            let top = self.text_rect("Properties").bottom();
            let label = if self.app.selected.is_some() {
                "Apply changes"
            } else {
                "Create wall"
            };
            let bottom = self.text_rect(label).top();
            self.frame(vec![
                egui::Event::PointerMoved(egui::pos2(180.0, (top + bottom) / 2.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, delta),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            for _ in 0..20 {
                self.frame(vec![]);
            }
        }
        fn settle_plan(&mut self) {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                self.frame(vec![]);
                if self.app.plans.ready() {
                    return;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "plan drawing did not settle"
                );
                std::thread::yield_now();
            }
        }
        fn plan_point(&self, view: Id, point: Point2) -> egui::Pos2 {
            let context = self.app.editor.native_plan_context(view).unwrap();
            let plane = context.basis.world_to_plane(point).unwrap();
            self.app.plans.test_screen_point(view, plane).unwrap()
        }
        fn reveal_property(&mut self, label: &str) {
            if self.visible_text_rect(label).is_some() {
                return;
            }
            self.scroll_properties(2000.0);
            for _ in 0..12 {
                if self.visible_text_rect(label).is_some() {
                    return;
                }
                self.scroll_properties(-70.0);
            }
            panic!("Property is not reachable: {label}");
        }
    }

    #[test]
    fn desktop_controls_complete_the_required_modeling_and_persistence_flow() {
        let mut h = Harness::new();
        assert_eq!(h.app.editor.document.model().levels.len(), 1);
        h.click("Create wall");
        assert_eq!(h.app.editor.scene.len(), 1);
        let id = *h.app.editor.document.model().walls.keys().next().unwrap();
        h.dimension("Length", "8");
        h.dimension("Thickness", "0.3");
        h.dimension("Height", "4");
        h.click("Apply changes");
        let wall = &h.app.editor.document.model().walls[&id].parameters;
        assert_eq!(
            (wall.length(), wall.thickness, wall.height),
            (8.0, 0.3, 4.0)
        );
        h.click("Architecture");
        h.click("Add level");
        assert_eq!(h.app.editor.document.model().levels.len(), 2);
        // Open the wall's level selector, then select the newly created level.
        h.reveal_property("Ground");
        h.click("Ground");
        h.click("Level 1");
        h.click("Apply changes");
        assert_eq!(h.app.editor.scene[&id].vertices[0].z, 3.0);
        h.click("Undo");
        assert_eq!(h.app.editor.scene[&id].vertices[0].z, 0.0);
        h.click("Redo");
        assert_eq!(h.app.editor.scene[&id].vertices[0].z, 3.0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui.osb");
        h.click("File");
        h.click("project.osb");
        h.replace_focused(&path.display().to_string());
        h.click("Save");
        assert!(
            path.is_file(),
            "{}; actual path {:?}; expected {:?}",
            h.app.status,
            h.app.path,
            path
        );
        assert!(!h.app.editor.is_dirty());
        let saved = h.app.editor.document.model().clone();
        h.click("File");
        h.click("New");
        assert!(h.app.editor.document.model().walls.is_empty());
        assert_ne!(
            h.app.editor.document.model().project.id(),
            saved.project.id()
        );
        h.click("File");
        h.click("project.osb");
        h.replace_focused(&path.display().to_string());
        h.click("Open");
        assert_eq!(h.app.editor.document.model(), &saved, "{}", h.app.status);
        assert!((h.app.editor.scene[&id].signed_volume() - 9.6).abs() < 1e-8);
    }

    #[test]
    fn contextual_commands_manage_and_selection_use_the_same_document() {
        let mut h = Harness::new();
        h.click("Create wall");
        let id = h.app.selected.unwrap();
        assert_eq!(h.app.ribbon_tab, RibbonTab::ModifyWalls);
        h.click("Architecture");
        // The ribbon Wall button is above the browser's Wall row.
        let pos = h
            .output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Wall" && t.pos.y < 138.0 => {
                    Some(t.pos + egui::vec2(8.0, 4.0))
                }
                _ => None,
            })
            .unwrap();
        h.click_at(pos);
        assert!(h.app.selected.is_none());
        h.click_browser("Wall");
        assert_eq!(h.app.selected, Some(id));
        h.click("Manage");
        h.click("Untitled project");
        // Select the editable project-name field inside the command area.
        let pos = h
            .output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t)
                    if t.galley.job.text == "Untitled project"
                        && t.pos.y > 58.0
                        && t.pos.y < 138.0 =>
                {
                    Some(t.pos + egui::vec2(10.0, 5.0))
                }
                _ => None,
            })
            .unwrap();
        h.click_at(pos);
        h.replace_focused("Studio model");
        h.click("Rename project");
        assert_eq!(
            h.app.editor.document.model().project.parameters.name,
            "Studio model"
        );
        h.dimension("Elevation", "2");
        assert_eq!(h.app.editor.scene[&id].vertices[0].z, 2.0);
        h.click("Modify | Walls");
        h.click("Delete wall");
        assert!(h.app.editor.scene.is_empty());
        assert!(h.app.selected.is_none());
        h.click("Undo");
        assert!(h.app.editor.scene.contains_key(&id));
        // Sample the actual displayed depth buffer instead of assuming a fixed canvas inset.
        h.frame(vec![]);
        let cache = h.app.viewport_cache.as_ref().unwrap();
        let image_rect = h
            .output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == cache.texture.id() => {
                    Some(mesh.calc_bounds())
                }
                _ => None,
            })
            .unwrap();
        let mut point = None;
        for y in (10..cache.frame.height - 10).step_by(8) {
            for x in (10..cache.frame.width - 10).step_by(8) {
                let lx = (x as f32 + 0.5) / cache.frame.width as f32 * image_rect.width();
                let ly = (y as f32 + 0.5) / cache.frame.height as f32 * image_rect.height();
                if cache.frame.pick(lx, ly) == Some(id) {
                    point = Some(image_rect.min + egui::vec2(lx, ly));
                }
            }
        }
        h.click_at(point.expect("visible wall sample"));
        assert_eq!(h.app.selected, Some(id));
        assert_eq!(h.app.ribbon_tab, RibbonTab::ModifyWalls);
    }

    #[test]
    fn invalid_drafts_and_file_confirmations_preserve_committed_work() {
        let mut h = Harness::new();
        h.app.draft.end = h.app.draft.start;
        h.frame(vec![]);
        h.click("Create wall");
        assert!(h.app.status_error);
        assert!(h.app.editor.scene.is_empty());
        h.click("Details");
        assert!(h.app.show_details);
        h.app.show_details = false;
        h.app.draft = default_wall(h.app.active_level);
        h.frame(vec![]);
        h.click("Create wall");
        let saved = h.app.editor.document.model().clone();
        h.click("File");
        h.click("New");
        assert!(h.app.pending.is_some());
        assert_eq!(h.app.editor.document.model(), &saved);
        h.click("Cancel");
        assert!(h.app.pending.is_none());
        // An invalid file remains an atomic failed open through the relocated action.
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.osb");
        std::fs::write(&bad, b"invalid archive").unwrap();
        h.app.path = bad.display().to_string();
        h.click("File");
        h.click("Open");
        h.click("Discard and continue");
        assert!(h.app.status_error);
        assert_eq!(h.app.editor.document.model(), &saved);
        h.click("File");
        h.click("Save");
        assert!(matches!(h.app.pending, Some(PendingAction::Replace(_))));
        h.click("Cancel");
        assert_eq!(std::fs::read(&bad).unwrap(), b"invalid archive");
        h.click("File");
        h.click("Save");
        h.click("Replace file");
        assert!(!h.app.editor.is_dirty());
        let mut reopened = Editor::new().unwrap();
        reopened.open(&bad).unwrap();
        assert_eq!(reopened.document.model(), &saved);
    }

    #[test]
    fn bounded_history_notice_and_actions_remain_visible_at_supported_sizes() {
        for size in [egui::vec2(1280.0, 800.0), egui::vec2(1000.0, 650.0)] {
            for scale in [1.0, 1.25, 1.5] {
                let mut h = Harness::at_size(size, scale);
                h.app
                    .editor
                    .document
                    .set_history_limits(os_document::HistoryLimits {
                        max_entries: 1,
                        ..Default::default()
                    })
                    .unwrap();
                assert!(h.visible_text_rect("History limited").is_none());
                for name in ["First", "Second"] {
                    h.app
                        .editor
                        .command("Rename", Command::RenameProject(name.into()))
                        .unwrap();
                }
                h.frame(vec![]);
                assert!(h.visible_text_rect("History limited").is_some());
                assert!(h.visible_text_rect("Unsaved").is_some());
                h.click("Undo");
                assert_eq!(
                    h.app.editor.document.model().project.parameters.name,
                    "First"
                );
                assert!(!h.app.editor.document.can_undo());
                h.click("Redo");
                assert_eq!(
                    h.app.editor.document.model().project.parameters.name,
                    "Second"
                );
                assert!(h.visible_text_rect("History limited").is_some());
            }
        }
    }

    #[test]
    fn compact_palettes_keep_actions_and_endpoints_reachable_at_each_dpi() {
        for size in [egui::vec2(1280.0, 800.0), egui::vec2(1000.0, 650.0)] {
            for scale in [1.0, 1.25, 1.5] {
                let mut h = Harness::at_size(size, scale);
                assert_eq!(h.output.pixels_per_point, scale);
                for label in [
                    "Properties",
                    "Project Browser",
                    "Create wall",
                    "File",
                    "Fit model",
                    "Details",
                ] {
                    assert!(
                        h.visible_text_rect(label).is_some(),
                        "{label} clipped at {size:?}, {scale}"
                    );
                }
                h.dimension("End X", "7");
                assert_eq!(h.app.draft.end.x, 7.0);
                assert!(
                    h.app.editor.scene.is_empty(),
                    "Draft field edit committed prematurely"
                );
                assert!(h.visible_text_rect("Create wall").is_some());
                h.click("Create wall");
                assert_eq!(h.app.editor.scene.len(), 1);
                assert!(h.visible_text_rect("Apply changes").is_some());
                h.click("Manage");
                assert!(h.visible_text_rect("Rename project").is_some());
                h.click("File");
                assert!(h.visible_text_rect("project.osb").is_some());
                assert!(h.visible_text_rect("Export IFC…").is_some());
                assert!(h.visible_text_rect("Import IFC…").is_some());
            }
        }
    }

    #[test]
    fn palette_drag_handles_resize_without_committing_drafts() {
        let mut h = Harness::new();
        let initial = h.app.properties_fraction;
        let divider_y = h
            .output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, .. }
                    if (points[0].y - points[1].y).abs() < 0.1
                        && (points[0].x - points[1].x).abs() >= 31.0
                        && (points[0].x - points[1].x).abs() <= 33.0
                        && (400.0..700.0).contains(&points[0].y)
                        && points.iter().all(|point| (0.0..=320.0).contains(&point.x)) =>
                {
                    Some(points[0].y)
                }
                _ => None,
            })
            .expect("visible Properties/Browser resize grip");
        let from = egui::pos2(160.0, divider_y);
        h.drag(from, from + egui::vec2(0.0, 70.0));
        assert!(
            h.app.properties_fraction > initial,
            "Divider did not move: {:?}",
            h.app.properties_fraction
        );
        assert!(h.visible_text_rect("Create wall").is_some());
        let before = h.text_rect("Model view").left();
        h.drag(egui::pos2(320.0, 380.0), egui::pos2(400.0, 380.0));
        let after = h.text_rect("Model view").left();
        assert!(
            after > before + 50.0,
            "Column did not resize: {before} -> {after}"
        );
        assert!(!h.app.editor.is_dirty());
    }

    #[test]
    fn ifc_import_is_staged_cancelable_and_unsaved_until_native_save() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.ifc");
        std::fs::write(
            &source,
            include_bytes!("../../../fixtures/rotated-walls.ifc"),
        )
        .unwrap();
        let mut h = Harness::new();
        h.click("Create wall");
        let original = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let draft = h.app.draft.clone();
        let selection = h.app.selected;
        let path = h.app.path.clone();
        h.app.ifc_path = source.display().to_string();
        h.app.prepare_ifc_import();
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(h.app.editor.document.model(), &original);
        h.click("Cancel");
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.draft, draft);
        assert_eq!(h.app.selected, selection);
        assert_eq!(h.app.path, path);
        assert!(h.app.editor.document.can_undo());
        h.app.prepare_ifc_import();
        h.frame(vec![]);
        h.frame(vec![]);
        // The confirmed snapshot does not depend on subsequent source-file changes.
        std::fs::write(&source, b"changed after preview").unwrap();
        h.click("Discard and import");
        assert_eq!(h.app.editor.scene.len(), 2);
        assert!(h.app.editor.is_dirty());
        assert!(h.app.path.is_empty());
        assert!(h.app.opened_path.is_none());
        assert!(!h.app.editor.document.can_undo());
        let destination = dir.path().join("imported.osb");
        h.app.path = destination.display().to_string();
        h.app.save();
        assert!(!h.app.editor.is_dirty());
        assert!(destination.exists());
    }

    #[test]
    fn ifc_failure_preserves_document_and_export_requires_explicit_consent() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.ifc");
        std::fs::write(&bad, b"not IFC").unwrap();
        let mut h = Harness::new();
        h.click("Create wall");
        let original = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        h.app.ifc_path = bad.display().to_string();
        h.app.prepare_ifc_import();
        assert!(h.app.status_error);
        assert!(h.app.exchange_pending.is_none());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.scene, scene);
        let native = dir.path().join("native.osb");
        h.app.path = native.display().to_string();
        h.app.save();
        let output = dir.path().join("copy.ifc");
        h.app.ifc_path = output.display().to_string();
        h.app.prepare_ifc_export();
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Cancel");
        assert!(!output.exists());
        h.app.prepare_ifc_export();
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Accept losses and export");
        assert!(output.exists());
        assert!(!h.app.editor.is_dirty());
        assert_eq!(h.app.opened_path, Some(native.clone()));
        assert_eq!(h.app.path, native.display().to_string());
        let bytes = std::fs::read(&output).unwrap();
        h.app.prepare_ifc_export();
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Cancel");
        assert_eq!(std::fs::read(&output).unwrap(), bytes);
        h.app
            .editor
            .command("Rename", Command::RenameProject("Edited".into()))
            .unwrap();
        h.app.prepare_ifc_export();
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Accept losses and replace IFC");
        assert!(h.app.editor.is_dirty());
        assert_ne!(std::fs::read(&output).unwrap(), bytes);
    }

    #[test]
    fn ifc_export_reconfirms_race_created_destination_and_blocks_shortcuts() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("race.ifc");
        let mut h = Harness::at_size(egui::vec2(1000., 650.), 1.25);
        h.click("Create wall");
        let model = h.app.editor.document.model().clone();
        h.app.ifc_path = output.display().to_string();
        h.app.prepare_ifc_export();
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }]);
        assert_eq!(h.app.editor.document.model(), &model);
        std::fs::write(&output, b"concurrent file").unwrap();
        h.click("Accept losses and export");
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(std::fs::read(&output).unwrap(), b"concurrent file");
        assert!(
            h.visible_text_rect("Accept losses and replace IFC")
                .is_some()
        );
        h.click("Cancel");
        assert_eq!(std::fs::read(&output).unwrap(), b"concurrent file");
    }

    #[test]
    fn file_menu_ifc_controls_route_real_input_without_reusing_native_path() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("menu.ifc");
        let native = dir.path().join("source.osb");
        let mut h = Harness::at_size(egui::vec2(1000., 650.), 1.5);
        h.click("Create wall");
        h.app.path = native.display().to_string();
        h.app.save();
        h.click("File");
        h.click("exchange.ifc");
        h.replace_focused(&output.display().to_string());
        assert_eq!(h.app.ifc_path, output.display().to_string());
        h.click("Export IFC…");
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(h.visible_text_rect("Accept losses and export").is_some());
        h.click("Accept losses and export");
        assert!(output.exists());
        assert_eq!(h.app.path, native.display().to_string());
        assert!(!h.app.editor.is_dirty());
        h.click("File");
        h.click("Import IFC…");
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(h.visible_text_rect("Import document").is_some());
        h.click("Import document");
        assert!(h.app.editor.is_dirty());
        assert!(h.app.path.is_empty());
        assert!(h.app.opened_path.is_none());
        assert_eq!(h.app.editor.scene.len(), 1);
        assert!(
            h.app
                .editor
                .document
                .model()
                .walls
                .values()
                .all(|w| w.parameters.length() == 5.)
        );
    }

    #[test]
    fn depth_texture_cache_tracks_edits_camera_resize_and_visible_selection() {
        let mut h = Harness::at_size(egui::vec2(1000., 650.), 1.25);
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/intersecting-walls.osb");
        h.app.open_path(&path).unwrap();
        h.frame(vec![]);
        h.frame(vec![]);
        let cache = h.app.viewport_cache.as_ref().unwrap();
        let texture = cache.texture.id();
        let original = cache.frame.rgba.clone();
        let count = cache.frame.rgba.iter().filter(|p| p[3] != 0).count();
        assert!(count > 1000);
        h.frame(vec![]);
        assert!(
            !h.output
                .textures_delta
                .set
                .iter()
                .any(|(id, _)| *id == texture),
            "unchanged frame uploaded again"
        );
        // Read displayed ID samples then feed actual pointer input at that texel.
        let cache = h.app.viewport_cache.as_ref().unwrap();
        let mut picks = std::collections::BTreeMap::new();
        let image_rect = h
            .output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Mesh(m) if m.texture_id == texture => Some(m.calc_bounds()),
                _ => None,
            })
            .unwrap();
        for y in (10..cache.frame.height - 10).step_by(8) {
            for x in (10..cache.frame.width - 10).step_by(8) {
                let lx = (x as f32 + 0.5) / cache.frame.width as f32 * image_rect.width();
                let ly = (y as f32 + 0.5) / cache.frame.height as f32 * image_rect.height();
                if let Some(id) = cache.frame.pick(lx, ly) {
                    picks.entry(id).or_insert((lx, ly));
                }
            }
        }
        assert_eq!(picks.len(), 3, "all three walls must have visible samples");
        let (&id, &(x, y)) = picks.iter().next().unwrap();
        h.click_at(egui::pos2(image_rect.left() + x, image_rect.top() + y));
        assert_eq!(h.app.selected, Some(id));
        assert_ne!(h.app.viewport_cache.as_ref().unwrap().frame.rgba, original);
        let selected = h.app.viewport_cache.as_ref().unwrap().frame.rgba.clone();
        h.dimension("Height", "5");
        h.click("Apply changes");
        assert_ne!(h.app.viewport_cache.as_ref().unwrap().frame.rgba, selected);
        h.click("Undo");
        assert_eq!(h.app.viewport_cache.as_ref().unwrap().frame.rgba, selected);
        let camera = h.app.camera;
        let from = egui::pos2(image_rect.right() - 120., image_rect.bottom() - 100.);
        h.drag(from, from + egui::vec2(60., 20.));
        assert_ne!(h.app.camera, camera, "viewport drag did not orbit");
        assert_ne!(h.app.viewport_cache.as_ref().unwrap().frame.rgba, selected);
        let before = h.app.viewport_cache.as_ref().unwrap().frame.width;
        h.size = egui::vec2(1280., 800.);
        h.scale = 1.5;
        h.frame(vec![]);
        h.frame(vec![]);
        assert_ne!(h.app.viewport_cache.as_ref().unwrap().frame.width, before);
        assert_eq!(h.app.viewport_cache.as_ref().unwrap().texture.id(), texture);
        assert!(h.app.viewport_cache.as_ref().unwrap().frame.rgba.len() <= os_render::MAX_PIXELS);
    }
}
