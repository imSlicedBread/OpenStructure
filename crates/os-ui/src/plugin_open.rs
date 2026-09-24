//! Atomic candidate opening with asynchronous external geometry.
use crate::{DesktopApp, Editor};
use os_core::{Error, Id, Result, ensure};
use os_document::Document;
use os_plugin_host::worker::{JobOutcome, PendingJob};
use os_render::Scene;
use os_storage::{StorageBackend, ZipJsonStorage};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone)]
enum Route {
    Generic(String),
}

pub(super) struct StagedOpen {
    document: Document,
    scene: Scene,
    queue: BTreeMap<Id, Route>,
    jobs: BTreeMap<Id, (PendingJob, Route)>,
    base_session: Id,
    base_revision: u64,
    activations: Vec<(String, Id)>,
    deadline: Instant,
    path: PathBuf,
    unavailable: Vec<Id>,
}
pub struct OpenedProject {
    pub path: PathBuf,
    /// Preserved extension entities without a loaded callable provider.
    pub unavailable: Vec<Id>,
}
impl Editor {
    fn open_activations(&self) -> Vec<(String, Id)> {
        self.host
            .manifests()
            .filter_map(|m| self.host.activation_id(&m.id).map(|id| (m.id.clone(), id)))
            .collect()
    }
    /// Parse/validate into a separate candidate. File IO and trusted built-in
    /// geometry are synchronous here; external Wasm execution is worker-only.
    /// Callers must obtain unsaved-change consent before starting this operation.
    pub fn start_plugin_open(&mut self, path: &Path, timeout: Duration) -> Result<()> {
        ensure(
            !self.plugin_work_pending(),
            "finish or cancel pending plugin work before opening",
        )?;
        ensure(
            !timeout.is_zero() && timeout <= Duration::from_secs(30),
            "open timeout must be positive and at most 30 seconds",
        )?;
        let deadline = Instant::now() + timeout;
        let document = ZipJsonStorage.open(path)?;
        let mut scene = Scene::new();
        let mut queue = BTreeMap::new();
        let mut unavailable = Vec::new();
        for id in document.model().walls.keys() {
            scene.insert(*id, super::opening_tools::host_mesh(document.model(), *id)?);
        }
        for (id, entity) in &document.model().extensions {
            if self.host.catalog(&entity.owner).is_some()
                && !self.host.extension_needs_migration(document.model(), *id)
            {
                queue.insert(*id, Route::Generic(entity.owner.clone()));
            } else {
                unavailable.push(*id);
            }
        }
        for id in document.model().openings.keys() {
            scene.insert(
                *id,
                super::opening_tools::panel_mesh(document.model(), *id)?,
            );
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
        ensure(
            Instant::now() < deadline,
            "candidate preparation exceeded open deadline",
        )?;
        self.staged_open = Some(StagedOpen {
            document,
            scene,
            queue,
            jobs: BTreeMap::new(),
            base_session: self.document.session_id(),
            base_revision: self.document.revision(),
            activations: self.open_activations(),
            deadline,
            path: path.into(),
            unavailable,
        });
        Ok(())
    }
    /// Dropping tickets revokes replies; worker permits remain until actual drain.
    pub fn cancel_plugin_open(&mut self) -> bool {
        self.staged_open.take().is_some()
    }
    pub fn plugin_open_pending(&self) -> bool {
        self.staged_open.is_some()
    }
    /// Pending returns None. Any failure discards only the candidate. Successful
    /// completion replaces document, saved state and scene together, exactly once.
    pub fn poll_plugin_open(&mut self) -> Result<Option<OpenedProject>> {
        let mut staged = self
            .staged_open
            .take()
            .ok_or_else(|| Error::Invalid("no staged project open".into()))?;
        ensure(
            self.document.session_id() == staged.base_session
                && self.document.revision() == staged.base_revision,
            "working document changed while opening; candidate discarded",
        )?;
        ensure(
            self.open_activations() == staged.activations,
            "plugin activation changed while opening; candidate discarded",
        )?;
        ensure(
            Instant::now() < staged.deadline,
            "project open deadline expired",
        )?;
        let ids: Vec<_> = staged.jobs.keys().copied().collect();
        for id in ids {
            let (mut job, route) = staged.jobs.remove(&id).unwrap();
            match self
                .host
                .poll_job(&mut job, &mut staged.document, None, "Candidate geometry")
                .map_err(|e| Error::Invalid(e.to_string()))?
            {
                None => {
                    staged.jobs.insert(id, (job, route));
                }
                Some(JobOutcome::GenericGeometry(result)) if matches!(route, Route::Generic(_)) => {
                    let (target, _, mesh) = result
                        .get(&staged.document, None)
                        .map_err(|e| Error::Invalid(e.to_string()))?;
                    ensure(target == id, "candidate geometry identity mismatch")?;
                    staged.scene.insert(id, mesh.clone());
                }
                Some(_) => {
                    return Err(Error::Invalid(
                        "candidate returned unexpected geometry outcome".into(),
                    ));
                }
            }
        }
        let queued: Vec<_> = staged
            .queue
            .iter()
            .map(|(id, route)| (*id, route.clone()))
            .collect();
        for (id, route) in queued {
            let owner = match &route {
                Route::Generic(owner) => owner.as_str(),
            };
            ensure(
                self.host.worker_supported(owner),
                "candidate provider has no bounded worker adapter",
            )?;
            if !self.host.worker_available(owner) {
                continue;
            }
            let remaining = staged
                .deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(5));
            ensure(!remaining.is_zero(), "project open deadline expired")?;
            let job = match &route {
                Route::Generic(_) => {
                    let mut read = std::collections::BTreeSet::from([id]);
                    if let Some(w) = staged.document.model().walls.get(&id) {
                        read.insert(w.parameters.level);
                    }
                    if let Some(e) = staged.document.model().extensions.get(&id) {
                        read.extend(
                            e.depends_on
                                .iter()
                                .filter(|id| staged.document.model().levels.contains_key(id))
                                .copied(),
                        );
                    }
                    self.host.start_generic_geometry_job(
                        owner,
                        &staged.document,
                        id,
                        read,
                        None,
                        remaining,
                    )?
                }
            };
            staged.queue.remove(&id);
            staged.jobs.insert(id, (job, route));
        }
        ensure(
            Instant::now() < staged.deadline,
            "project open deadline expired",
        )?;
        if !staged.queue.is_empty() || !staged.jobs.is_empty() {
            self.staged_open = Some(staged);
            return Ok(None);
        }
        self.saved_model = Some(staged.document.model().clone());
        self.document = staged.document;
        // Establish the new managed set/stamp before installing the staged meshes.
        self.synchronize_plugin_scene();
        self.pending_geometry.clear();
        self.scene = staged.scene;
        Ok(Some(OpenedProject {
            path: staged.path,
            unavailable: staged.unavailable,
        }))
    }
}

impl DesktopApp {
    pub(super) fn poll_desktop_open(&mut self, ctx: &eframe::egui::Context) {
        if !self.editor.plugin_open_pending()
            || self.pending.is_some()
            || self.exchange_pending.is_some()
        {
            return;
        }
        match self.editor.poll_plugin_open() {
            Ok(None) => ctx.request_repaint_after(Duration::from_millis(16)),
            Ok(Some(opened)) => {
                self.path = opened.path.display().to_string();
                self.opened_path = Some(opened.path);
                self.selected = None;
                self.plugin_form = crate::plugin_forms::FormState::default();
                self.viewport_cache = None;
                self.refresh_document();
                self.fit_requested = true;
                self.report(
                    Ok(()),
                    &format!(
                        "Project opened. {} plugin elements unavailable.",
                        opened.unavailable.len()
                    ),
                );
            }
            Err(error) => self.report(Err(error), ""),
        }
    }
    pub(super) fn desktop_open_dialog(&mut self, ctx: &eframe::egui::Context) {
        if !self.editor.plugin_open_pending()
            || self.pending.is_some()
            || self.exchange_pending.is_some()
        {
            return;
        }
        eframe::egui::Modal::new(eframe::egui::Id::new("staged_project_open")).show(ctx, |ui| {
            ui.heading("Opening project");
            ui.label("Checking candidate geometry. Your current project remains open until this succeeds.");
            if ui.button("Cancel project open").clicked() {
                self.editor.cancel_plugin_open();
                self.report(Ok(()), "Project open cancelled; current project retained.");
            }
        });
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_document::Command;

    fn candidate(path: &Path) {
        ZipJsonStorage
            .save(&Document::new("Candidate").unwrap(), path)
            .unwrap();
    }
    fn working() -> Editor {
        let mut editor = Editor::new().unwrap();
        editor
            .command("Rename", Command::RenameProject("Working".into()))
            .unwrap();
        editor
    }
    #[test]
    fn candidate_commits_only_on_poll_and_resets_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate.osb");
        candidate(&path);
        let bytes = std::fs::read(&path).unwrap();
        let mut editor = working();
        let session = editor.document.session_id();
        editor
            .start_plugin_open(&path, Duration::from_secs(5))
            .unwrap();
        assert_eq!(editor.document.session_id(), session);
        assert!(editor.is_dirty() && editor.document.can_undo());
        assert!(editor.plugin_work_pending());
        assert!(editor.save(&dir.path().join("blocked.osb")).is_err());
        let opened = editor.poll_plugin_open().unwrap().unwrap();
        assert_eq!(opened.path, path);
        assert!(opened.unavailable.is_empty());
        assert_ne!(editor.document.session_id(), session);
        assert_eq!(editor.document.model().project.parameters.name, "Candidate");
        assert!(!editor.is_dirty() && !editor.document.can_undo());
        assert!(editor.poll_plugin_open().is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    #[test]
    fn cancellation_and_failed_preparation_preserve_working_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate.osb");
        candidate(&path);
        let mut editor = working();
        let model = editor.document.model().clone();
        let session = editor.document.session_id();
        assert!(
            editor
                .start_plugin_open(&dir.path().join("missing.osb"), Duration::from_secs(5))
                .is_err()
        );
        assert!(editor.start_plugin_open(&path, Duration::ZERO).is_err());
        editor
            .start_plugin_open(&path, Duration::from_secs(5))
            .unwrap();
        assert!(
            editor
                .start_plugin_open(&path, Duration::from_secs(5))
                .is_err()
        );
        assert!(editor.cancel_plugin_open());
        assert!(!editor.cancel_plugin_open());
        assert_eq!(editor.document.model(), &model);
        assert_eq!(editor.document.session_id(), session);
        assert!(editor.is_dirty() && editor.document.can_undo());
        editor.undo().unwrap();
        assert!(!editor.is_dirty());
    }
    #[test]
    fn changed_document_activation_and_expired_deadline_discard_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate.osb");
        candidate(&path);
        let mut editor = working();
        editor
            .start_plugin_open(&path, Duration::from_secs(5))
            .unwrap();
        editor
            .command("New work", Command::RenameProject("New work".into()))
            .unwrap();
        assert!(editor.poll_plugin_open().is_err());
        assert_eq!(editor.document.model().project.parameters.name, "New work");
        editor
            .start_plugin_open(&path, Duration::from_secs(5))
            .unwrap();
        editor.host.unload(os_plugin_api::wall::OWNER).unwrap();
        assert!(editor.poll_plugin_open().is_err());
        editor
            .start_plugin_open(&path, Duration::from_secs(5))
            .unwrap();
        editor.staged_open.as_mut().unwrap().deadline = Instant::now();
        assert!(editor.poll_plugin_open().is_err());
        assert!(!editor.plugin_open_pending());
        assert_eq!(editor.document.model().project.parameters.name, "New work");
        assert!(editor.is_dirty() && editor.document.can_undo());
    }
    #[test]
    fn unavailable_extension_and_auxiliary_bytes_survive_staged_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate.osb");
        let mut model = os_model::Model::new("Opaque");
        let entity: os_model::ExtensionEntity =
            serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json"))
                .unwrap();
        let id = entity.id;
        model.plugin_requirements.insert(
            entity.owner.clone(),
            os_model::PluginRequirement {
                version: "1.0.0".into(),
            },
        );
        model.extensions.insert(id, entity);
        let files = BTreeMap::from([("extensions/example/opaque.bin".into(), vec![0, 255, 7])]);
        let document = Document::from_model_and_files(model.clone(), files.clone()).unwrap();
        ZipJsonStorage.save(&document, &path).unwrap();
        // Storage also retains its standard empty assets/previews directories.
        let stored_files = ZipJsonStorage
            .open(&path)
            .unwrap()
            .auxiliary_files()
            .clone();
        let mut editor = working();
        editor
            .start_plugin_open(&path, Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            editor.poll_plugin_open().unwrap().unwrap().unavailable,
            vec![id]
        );
        assert_eq!(editor.document.model(), &model);
        assert_eq!(editor.document.auxiliary_files(), &stored_files);
        assert!(editor.scene.is_empty());
        editor.save(&dir.path().join("preserved.osb")).unwrap();
        let reopened = ZipJsonStorage
            .open(&dir.path().join("preserved.osb"))
            .unwrap();
        assert_eq!(reopened.model(), &model);
        assert_eq!(reopened.auxiliary_files(), &stored_files);
    }
}
