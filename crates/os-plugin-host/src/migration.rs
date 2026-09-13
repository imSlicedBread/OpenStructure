//! Multi-batch migration of an isolated candidate; one final live transaction.
use crate::{
    PluginHost,
    generic::{Invocation, Scope},
    worker::{JobOutcome, PendingJob},
};
use os_core::{Error, Id, Result, ensure};
use os_document::{Command, Document};
use os_model::PluginRequirement;
use os_plugin_api::{
    Permission,
    generic::{self as wire, Mode},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant},
};

/// Explicit reviewed command and inputs for one owned element type.
#[derive(Clone)]
pub struct MigrationCommand {
    pub command_id: String,
    pub inputs: BTreeMap<String, serde_json::Value>,
}
struct State {
    candidate: Document,
    source_session: Id,
    source_revision: u64,
    owner: String,
    activation: Id,
    version: String,
    plan: BTreeMap<String, MigrationCommand>,
    queue: VecDeque<(String, Vec<Id>)>,
    targets: BTreeSet<Id>,
    job: Option<PendingJob>,
    batch_size: usize,
    completed: usize,
    deadline: Instant,
}
pub struct MigrationSession {
    state: Option<State>,
}
impl MigrationSession {
    pub fn cancel(&mut self) {
        self.state = None;
    }
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.state.as_ref().map(|s| (s.completed, s.targets.len()))
    }
}
impl PluginHost {
    /// Review all type commands up front. File contents never initiate this call.
    /// 16 edits/256 scoped reads/1 MiB remain the per-invocation bounds.
    pub fn start_migration(
        &self,
        document: &Document,
        owner: &str,
        plan: BTreeMap<String, MigrationCommand>,
        timeout: Duration,
    ) -> Result<MigrationSession> {
        ensure(
            !timeout.is_zero() && timeout <= Duration::from_secs(300),
            "migration deadline must be positive and at most five minutes",
        )?;
        let deadline = Instant::now() + timeout;
        let loaded = self.plugin(owner)?;
        for permission in [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ] {
            crate::require(loaded, permission)?;
        }
        ensure(
            self.worker_supported(owner) && owner != os_plugin_api::wall::OWNER,
            "staged extension migration requires an external non-native provider",
        )?;
        let catalog = loaded
            .catalog
            .as_ref()
            .ok_or_else(|| Error::Invalid("API-2 catalog required".into()))?;
        let mut groups: BTreeMap<String, Vec<Id>> = BTreeMap::new();
        for e in document
            .model()
            .extensions
            .values()
            .filter(|e| e.owner == owner)
        {
            groups.entry(e.type_id.clone()).or_default().push(e.id);
        }
        ensure(
            !groups.is_empty() && plan.keys().eq(groups.keys()),
            "review exactly one migration command for each owned type",
        )?;
        for (kind, command) in &plan {
            let descriptor = catalog
                .commands
                .iter()
                .find(|c| {
                    c.id == command.command_id
                        && c.element_type == *kind
                        && c.mode == Mode::Migrate
                        && c.enabled
                })
                .ok_or_else(|| Error::Invalid("enabled migration descriptor missing".into()))?;
            wire::validate_values(&descriptor.fields, &command.inputs, false)?;
        }
        let targets = groups.values().flatten().copied().collect();
        let queue = groups
            .into_iter()
            .flat_map(|(kind, ids)| {
                // Leave fuel/message headroom for independently compiled guests.
                // The protocol ceiling is 16; it is not a throughput guarantee.
                ids.chunks(4)
                    .map(|chunk| (kind.clone(), chunk.to_vec()))
                    .collect::<Vec<_>>()
            })
            .collect();
        let candidate = Document::from_model(document.model().clone())?;
        ensure(
            Instant::now() < deadline,
            "migration preparation deadline expired",
        )?;
        Ok(MigrationSession {
            state: Some(State {
                candidate,
                source_session: document.session_id(),
                source_revision: document.revision(),
                owner: owner.into(),
                activation: loaded.activation_id,
                version: loaded.manifest.version.clone(),
                plan,
                queue,
                targets,
                job: None,
                batch_size: 0,
                completed: 0,
                deadline,
            }),
        })
    }
    /// Returns false while pending. Errors discard only the candidate; true commits
    /// all payloads and the version requirement in one normal undoable transaction.
    pub fn poll_migration(
        &self,
        session: &mut MigrationSession,
        document: &mut Document,
    ) -> Result<bool> {
        let mut state = session
            .state
            .take()
            .ok_or_else(|| Error::Invalid("migration cancelled or settled".into()))?;
        ensure(
            document.session_id() == state.source_session
                && document.revision() == state.source_revision,
            "document changed during staged migration",
        )?;
        ensure(
            self.activation_id(&state.owner) == Some(state.activation),
            "provider changed during staged migration",
        )?;
        ensure(
            Instant::now() < state.deadline,
            "migration deadline expired",
        )?;
        if let Some(mut job) = state.job.take() {
            match self
                .poll_job(&mut job, &mut state.candidate, None, "Candidate migration")
                .map_err(|e| Error::Invalid(e.to_string()))?
            {
                None => state.job = Some(job),
                Some(JobOutcome::Committed) => {
                    state.completed += state.batch_size;
                    // Keep only the current candidate, not a full undo snapshot per batch.
                    state.candidate = Document::from_model(state.candidate.model().clone())?;
                }
                Some(_) => return Err(Error::Invalid("migration returned geometry".into())),
            }
        }
        if state.job.is_none() && !state.queue.is_empty() && self.worker_available(&state.owner) {
            let (kind, ids) = state.queue.pop_front().unwrap();
            let write: BTreeSet<_> = ids.into_iter().collect();
            let mut read = write.clone();
            for id in &write {
                read.extend(state.candidate.model().extensions[id].references());
            }
            let command = &state.plan[&kind];
            let timeout = state
                .deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(5));
            let invocation = Invocation {
                command_id: command.command_id.clone(),
                inputs: command.inputs.clone(),
                scope: Scope {
                    read,
                    write,
                    create_count: 0,
                },
                timeout,
            };
            state.batch_size = invocation.scope.write.len();
            state.job = Some(self.start_generic_job_inner(
                &state.owner,
                &state.candidate,
                invocation,
                None,
                true,
            )?);
        }
        ensure(
            Instant::now() < state.deadline,
            "migration deadline expired",
        )?;
        if state.job.is_some() || !state.queue.is_empty() {
            session.state = Some(state);
            return Ok(false);
        }
        let mut commands: Vec<_> = state
            .targets
            .iter()
            .map(|id| Command::ReplaceExtension {
                expected_schema_version: document.model().extensions[id].payload_schema_version,
                entity: state.candidate.model().extensions[id].clone(),
            })
            .collect();
        commands.push(Command::SetPluginRequirement {
            plugin_id: state.owner,
            requirement: Some(PluginRequirement {
                version: state.version,
            }),
        });
        document.execute("Migrate plugin data", commands)?;
        Ok(true)
    }
}
