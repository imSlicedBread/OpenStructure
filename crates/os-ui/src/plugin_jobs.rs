//! Opt-in Editor orchestration; no synchronous Wasm invocation in frame polling.
use crate::{
    Editor,
    plugin_tools::{ToolContext, ToolDraft},
};
use os_core::{Id, Result, ensure};
use os_plugin_host::migration::{MigrationCommand, MigrationSession};
use os_plugin_host::worker::{JobOutcome, PendingJob, ViewContext};
use std::{collections::BTreeMap, time::Duration};

type Stamp = (Id, u64, Vec<(String, Id)>);
enum CommandJob {
    Ordinary(PendingJob),
    Migration(Box<MigrationSession>, Option<ViewContext>),
}
impl CommandJob {
    fn cancel(&mut self) {
        match self {
            Self::Ordinary(job) => job.cancel(),
            Self::Migration(session, _) => session.cancel(),
        }
    }
}
#[derive(Default)]
pub(super) struct Jobs {
    stamp: Option<Stamp>,
    pub(super) managed: BTreeMap<Id, String>,
    geometry: BTreeMap<Id, PendingJob>,
    failures: BTreeMap<Id, String>,
    command: Option<(CommandJob, ToolContext)>,
    view: Option<ViewContext>,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use os_plugin_api::{Manifest, Permission, Plugin, generic as wire};
    pub(crate) struct NoWorker;
    impl Plugin for NoWorker {
        fn manifest(&self) -> Manifest {
            let mut manifest =
                Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml"))
                    .unwrap();
            manifest
                .capabilities
                .push(os_plugin_api::Capability::Geometry);
            manifest
        }
        fn invoke_json(&self, input: &str) -> Result<String> {
            let request: wire::Request = serde_json::from_str(input).unwrap();
            assert!(
                matches!(request.operation, wire::Operation::Describe),
                "editor called a synchronous guest"
            );
            Ok(serde_json::to_string(&wire::Response {
                api_version: 2,
                request_id: request.request_id,
                context: None,
                result: wire::Reply::Catalog(
                    serde_json::from_str(include_str!(
                        "../../../fixtures/generic-column/catalog.json"
                    ))
                    .unwrap(),
                ),
            })
            .unwrap())
        }
    }
    pub(crate) fn editor() -> (Editor, ToolContext, Id) {
        let mut editor = Editor::new().unwrap();
        editor
            .host
            .load(
                Box::new(NoWorker),
                [
                    Permission::ModelRead,
                    Permission::ModelWrite,
                    Permission::UiTool,
                ]
                .into(),
            )
            .unwrap();
        let mut model = editor.document.model().clone();
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
        editor.document = Document::from_model(model).unwrap();
        let context = ToolContext {
            level: *editor.document.model().levels.keys().next().unwrap(),
            selection: Some(id),
        };
        (editor, context, id)
    }
    use os_document::Document;
    #[test]
    fn staged_open_rejects_unbounded_provider_without_replacing_work() {
        use os_storage::{StorageBackend, ZipJsonStorage};
        let (mut editor, _, _) = editor();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidate.osb");
        ZipJsonStorage.save(&editor.document, &path).unwrap();
        let session = editor.document.session_id();
        let model = editor.document.model().clone();
        editor
            .start_plugin_open(&path, Duration::from_secs(5))
            .unwrap();
        let error = editor.poll_plugin_open().err().unwrap();
        assert!(error.to_string().contains("no bounded worker adapter"));
        assert_eq!(editor.document.session_id(), session);
        assert_eq!(editor.document.model(), &model);
        assert!(!editor.plugin_open_pending());
    }
    #[test]
    fn unavailable_worker_is_reported_once_and_retry_is_explicit() {
        let (mut editor, context, id) = editor();
        let original = editor.document.model().clone();
        let report = editor.poll_plugin_work(context, None).unwrap();
        assert_eq!(report.errors.len(), 1);
        assert!(!report.busy);
        assert!(editor.plugin_geometry_errors().contains_key(&id));
        for _ in 0..4 {
            let report = editor.poll_plugin_work(context, None).unwrap();
            assert!(report.errors.is_empty());
            assert!(!report.busy);
        }
        assert_eq!(editor.document.model(), &original);
        editor.retry_plugin_geometry();
        assert!(editor.plugin_work_pending());
        assert_eq!(
            editor.poll_plugin_work(context, None).unwrap().errors.len(),
            1
        );
    }
    #[test]
    fn failed_geometry_blocks_save_until_optional_plugin_is_unloaded() {
        let (mut editor, context, id) = editor();
        editor.poll_plugin_work(context, None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preserved.osb");
        assert!(editor.save(&path).is_err());
        assert!(!path.exists());
        editor.host.unload("org.example.columns").unwrap();
        editor.poll_plugin_work(context, None).unwrap();
        assert!(editor.plugin_geometry_errors().is_empty());
        assert!(!editor.scene.contains_key(&id));
        editor.save(&path).unwrap();
        use os_storage::StorageBackend;
        assert_eq!(
            os_storage::ZipJsonStorage.open(&path).unwrap().model(),
            editor.document.model()
        );
    }
}
#[derive(Default, Debug)]
pub struct PluginPoll {
    pub committed: bool,
    pub busy: bool,
    /// Newly settled errors; geometry errors also remain available for inspection.
    pub errors: Vec<String>,
}
impl Editor {
    pub fn start_plugin_migration(
        &mut self,
        draft: &crate::migration_tools::MigrationDraft,
        context: ToolContext,
        view: Option<ViewContext>,
    ) -> Result<()> {
        ensure(
            self.staged_open.is_none(),
            "finish or cancel the staged open first",
        )?;
        ensure(
            self.plugin_jobs.command.is_none(),
            "another plugin tool is pending",
        )?;
        let session = draft.start(&self.host, &self.document, context)?;
        self.plugin_jobs.command = Some((CommandJob::Migration(Box::new(session), view), context));
        Ok(())
    }
    pub fn start_plugin_tool(
        &mut self,
        draft: &ToolDraft,
        context: ToolContext,
        view: Option<ViewContext>,
    ) -> Result<()> {
        ensure(
            self.staged_open.is_none(),
            "finish or cancel the staged open first",
        )?;
        ensure(
            self.plugin_jobs.command.is_none(),
            "another plugin tool is pending",
        )?;
        let job = if draft.descriptor().mode == os_plugin_api::generic::Mode::Migrate {
            // Reuse the exact draft/session/activation/context checks before
            // authorizing the staged service. Never infer commands for other types.
            let invocation =
                draft.invocation(&self.host, &self.document, context, Duration::from_secs(5))?;
            let plan = [(
                draft.descriptor().element_type.clone(),
                MigrationCommand {
                    command_id: invocation.command_id,
                    inputs: invocation.inputs,
                },
            )]
            .into();
            CommandJob::Migration(
                Box::new(self.host.start_migration(
                    &self.document,
                    draft.owner(),
                    plan,
                    Duration::from_secs(300),
                )?),
                view,
            )
        } else {
            CommandJob::Ordinary(draft.start(
                &self.host,
                &self.document,
                context,
                view,
                Duration::from_secs(5),
            )?)
        };
        self.plugin_jobs.command = Some((job, context));
        Ok(())
    }
    pub fn cancel_plugin_tool(&mut self) {
        if let Some((job, _)) = &mut self.plugin_jobs.command {
            job.cancel();
        }
    }
    pub fn plugin_migration_progress(&self) -> Option<(usize, usize)> {
        match self.plugin_jobs.command.as_ref() {
            Some((CommandJob::Migration(session, _), _)) => session.progress(),
            _ => None,
        }
    }
    pub fn plugin_geometry_errors(&self) -> &BTreeMap<Id, String> {
        &self.plugin_jobs.failures
    }
    pub fn plugin_work_pending(&self) -> bool {
        self.staged_open.is_some()
            || self.plugin_jobs.command.is_some()
            || !self.plugin_jobs.geometry.is_empty()
            || self
                .pending_geometry
                .iter()
                .any(|id| self.plugin_jobs.managed.contains_key(id))
    }
    /// Explicit retry; failed geometry does not trigger an unbounded frame retry loop.
    pub fn retry_plugin_geometry(&mut self) {
        self.pending_geometry
            .extend(self.plugin_jobs.failures.keys().copied());
        self.plugin_jobs.failures.clear();
    }
    pub(super) fn synchronize_plugin_scene(&mut self) {
        let activations = self
            .host
            .manifests()
            .filter_map(|m| self.host.activation_id(&m.id).map(|id| (m.id.clone(), id)))
            .collect();
        let stamp = (
            self.document.session_id(),
            self.document.revision(),
            activations,
        );
        if self.plugin_jobs.stamp.as_ref() == Some(&stamp) {
            return;
        }
        // Conservative whole-plugin invalidation until a per-dependency revision
        // cache is implemented. Never display an accepted mesh at a later revision.
        for id in self.plugin_jobs.managed.keys() {
            self.scene.remove(id);
            self.pending_geometry.remove(id);
        }
        self.plugin_jobs.geometry.clear();
        self.plugin_jobs.failures.clear();
        self.plugin_jobs.managed.clear();
        for (id, e) in &self.document.model().extensions {
            if self.host.catalog(&e.owner).is_some()
                && !self
                    .host
                    .extension_needs_migration(self.document.model(), *id)
            {
                self.plugin_jobs.managed.insert(*id, e.owner.clone());
            }
        }
        // Native walls, including worker-created walls, always use host geometry.
        self.pending_geometry
            .extend(self.plugin_jobs.managed.keys().copied());
        self.plugin_jobs.stamp = Some(stamp);
    }
    /// Call repeatedly with current selection/level/view. May perform bounded host
    /// parsing/tessellation/commit, but never waits for or executes a Wasm guest.
    pub fn poll_plugin_work(
        &mut self,
        context: ToolContext,
        view: Option<ViewContext>,
    ) -> Result<PluginPoll> {
        self.regenerate()?;
        if self.plugin_jobs.view != view {
            self.plugin_jobs.geometry.clear();
            self.pending_geometry
                .extend(self.plugin_jobs.failures.keys().copied());
            self.plugin_jobs.failures.clear();
            self.plugin_jobs.view = view;
        }
        let mut report = PluginPoll::default();
        if let Some((mut job, reviewed)) = self.plugin_jobs.command.take() {
            if context != reviewed {
                job.cancel();
            }
            let outcome = match &mut job {
                CommandJob::Ordinary(job) => self
                    .host
                    .poll_job(job, &mut self.document, view, "Plugin tool")
                    .map_err(|e| e.to_string()),
                CommandJob::Migration(session, reviewed_view) => {
                    if *reviewed_view != view {
                        session.cancel();
                    }
                    self.host
                        .poll_migration(session, &mut self.document)
                        .map(|done| done.then_some(JobOutcome::Committed))
                        .map_err(|e| e.to_string())
                }
            };
            match outcome {
                Ok(None) => self.plugin_jobs.command = Some((job, reviewed)),
                Ok(Some(JobOutcome::Committed)) => {
                    report.committed = true;
                    self.regenerate()?;
                }
                Ok(Some(_)) => report
                    .errors
                    .push("Tool returned geometry instead of a commit".into()),
                Err(error) => report.errors.push(error.to_string()),
            }
        }
        let ids: Vec<_> = self.plugin_jobs.geometry.keys().copied().collect();
        for id in ids {
            let mut job = self.plugin_jobs.geometry.remove(&id).unwrap();
            match self
                .host
                .poll_job(&mut job, &mut self.document, view, "Plugin geometry")
            {
                Ok(None) => {
                    self.plugin_jobs.geometry.insert(id, job);
                }
                Ok(Some(JobOutcome::GenericGeometry(result))) => {
                    match result.get(&self.document, view) {
                        Ok((target, _, mesh)) if target == id => {
                            self.scene.insert(id, mesh.clone());
                            self.pending_geometry.remove(&id);
                        }
                        _ => {
                            self.fail_plugin_geometry(
                                id,
                                "Invalid or stale geometry target".into(),
                                &mut report,
                            );
                        }
                    }
                }
                Ok(Some(_)) => self.fail_plugin_geometry(
                    id,
                    "Geometry returned an unexpected outcome".into(),
                    &mut report,
                ),
                Err(error) => self.fail_plugin_geometry(id, error.to_string(), &mut report),
            }
        }
        let queued: Vec<_> = self
            .pending_geometry
            .iter()
            .filter_map(|id| {
                self.plugin_jobs
                    .managed
                    .get(id)
                    .map(|owner| (*id, owner.clone()))
            })
            .collect();
        for (id, owner) in queued {
            if self.plugin_jobs.geometry.contains_key(&id)
                || self.plugin_jobs.failures.contains_key(&id)
            {
                continue;
            }
            // No geometry capability is an explicit unavailable result, not a retry.
            if !self.host.manifests().any(|m| {
                m.id == owner
                    && m.capabilities
                        .contains(&os_plugin_api::Capability::Geometry)
            }) {
                self.fail_plugin_geometry(
                    id,
                    "Plugin has no geometry capability".into(),
                    &mut report,
                );
                continue;
            }
            if !self.host.worker_supported(&owner) {
                self.fail_plugin_geometry(
                    id,
                    "Plugin has no bounded worker adapter".into(),
                    &mut report,
                );
                continue;
            }
            if !self.host.worker_available(&owner) {
                continue;
            }
            let mut read = std::collections::BTreeSet::from([id]);
            if let Some(w) = self.document.model().walls.get(&id) {
                read.insert(w.parameters.level);
            }
            if let Some(e) = self.document.model().extensions.get(&id) {
                read.extend(
                    e.depends_on
                        .iter()
                        .filter(|id| self.document.model().levels.contains_key(id))
                        .copied(),
                );
            }
            match self.host.start_generic_geometry_job(
                &owner,
                &self.document,
                id,
                read,
                view,
                Duration::from_secs(5),
            ) {
                Ok(job) => {
                    self.plugin_jobs.geometry.insert(id, job);
                }
                Err(error) => self.fail_plugin_geometry(id, error.to_string(), &mut report),
            }
        }
        report.busy = self.plugin_work_pending();
        Ok(report)
    }
    fn fail_plugin_geometry(&mut self, id: Id, error: String, report: &mut PluginPoll) {
        self.scene.remove(&id);
        self.pending_geometry.remove(&id);
        self.plugin_jobs.failures.insert(id, error.clone());
        report.errors.push(error);
    }
}
