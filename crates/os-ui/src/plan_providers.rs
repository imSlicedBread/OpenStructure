//! Bounded plan scheduling; no synchronous guest execution in polling.
use crate::Editor;
use os_core::{Id, Result, ensure};
use os_plugin_host::{
    plan_graphics::{PlanGraphics, PlanRequest},
    worker::{JobOutcome, PendingJob, ViewContext},
};
use os_render::plan::{MAX_PLAN_ELEMENTS, PlanContext};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant},
};

pub struct PlanProviderBatch {
    context: PlanContext,
    activations: BTreeMap<String, Id>,
    queued: VecDeque<(String, PlanRequest)>,
    pending: Option<(Id, PendingJob)>,
    results: Vec<PlanGraphics>,
    failures: BTreeMap<Id, String>,
    deadline: Instant,
    revoked: bool,
    segment_count: usize,
}
pub struct PlanProviderResults {
    pub graphics: Vec<PlanGraphics>,
    /// Failed entities remain unavailable when graphics are composed. Present
    /// these diagnostics; never call a partial batch a complete drawing.
    pub failures: BTreeMap<Id, String>,
}
impl PlanProviderBatch {
    /// Explicit routing decisions belong to the caller. Ambiguous registrations
    /// must not be resolved by silently choosing the first provider.
    pub fn begin(
        editor: &Editor,
        view: Id,
        requests: Vec<(String, PlanRequest)>,
        timeout: Duration,
    ) -> Result<Self> {
        ensure(
            requests.len() <= MAX_PLAN_ELEMENTS,
            "too many plan requests",
        )?;
        ensure(
            !timeout.is_zero() && timeout <= Duration::from_secs(300),
            "invalid plan batch deadline",
        )?;
        let context = editor.native_plan_context(view)?;
        let mut activations = BTreeMap::new();
        let mut entities = BTreeSet::new();
        for (plugin, request) in &requests {
            ensure(
                request.view == view && entities.insert(request.element),
                "duplicate target or wrong plan view",
            )?;
            let activation = editor
                .host
                .activation_id(plugin)
                .ok_or_else(|| os_core::Error::Invalid("plan provider unavailable".into()))?;
            ensure(
                editor.host.worker_supported(plugin),
                "plan batches require Wasm providers",
            )?;
            activations.insert(plugin.clone(), activation);
        }
        Ok(Self {
            context,
            activations,
            queued: requests.into(),
            pending: None,
            results: Vec::new(),
            failures: BTreeMap::new(),
            deadline: Instant::now() + timeout,
            revoked: false,
            segment_count: 0,
        })
    }
    pub fn cancel(&mut self) {
        if let Some((_, job)) = &mut self.pending {
            job.cancel();
        }
        self.pending.take();
        self.queued.clear();
        self.results.clear();
        self.failures.clear();
        self.revoked = true;
    }
    fn current(&self, editor: &Editor, active_view: Option<Id>) -> Result<()> {
        ensure(!self.revoked, "plan batch cancelled")?;
        ensure(
            active_view == Some(self.context.view_id),
            "plan batch view inactive",
        )?;
        ensure(
            editor.native_plan_context(self.context.view_id)? == self.context,
            "plan batch document/view changed",
        )?;
        ensure(
            self.activations
                .iter()
                .all(|(p, a)| editor.host.activation_id(p) == Some(*a)),
            "plan batch provider changed",
        )?;
        Ok(())
    }
    /// Nonblocking: accepts at most one reply and admits at most one request per
    /// call. At most one outstanding ticket per batch; the host's global pool
    /// bounds multiple batches and unrelated geometry/command work together.
    pub fn poll(&mut self, editor: &mut Editor, active_view: Option<Id>) -> Result<bool> {
        if let Err(error) = self.current(editor, active_view) {
            self.cancel();
            return Err(error);
        }
        if Instant::now() >= self.deadline {
            self.cancel();
            return Err(os_core::Error::Invalid(
                "plan batch deadline expired".into(),
            ));
        }
        let view = Some(ViewContext {
            id: self.context.view_id,
            settings_revision: self.context.settings_revision,
        });
        if let Some((entity, job)) = &mut self.pending {
            match editor
                .host
                .poll_job(job, &mut editor.document, view, "Read plan graphics")
            {
                Ok(None) => return Ok(false),
                Ok(Some(JobOutcome::PlanGraphics(result))) => {
                    let count = result
                        .segments(&editor.host, &editor.document, self.context.view_id)?
                        .len();
                    if self.segment_count.saturating_add(count) > MAX_PLAN_ELEMENTS {
                        self.cancel();
                        return Err(os_core::Error::Invalid(
                            "plan batch exceeds 10000 segments".into(),
                        ));
                    }
                    self.segment_count += count;
                    self.results.push(result);
                }
                Ok(Some(_)) => {
                    self.failures
                        .insert(*entity, "unexpected plan worker outcome".into());
                }
                Err(error) => {
                    self.failures.insert(*entity, error.to_string());
                }
            }
            self.pending.take();
        }
        if let Some((plugin, _)) = self.queued.front() {
            if !editor.host.worker_available(plugin) {
                return Ok(false);
            }
            let (plugin, request) = self.queued.pop_front().expect("checked queue");
            let entity = request.element;
            match editor
                .host
                .start_plan_graphics_job(&plugin, &editor.document, request)
            {
                Ok(job) => self.pending = Some((entity, job)),
                Err(error) => {
                    self.failures.insert(entity, error.to_string());
                }
            }
        }
        Ok(self.queued.is_empty() && self.pending.is_none())
    }
    pub fn into_results(
        self,
        editor: &Editor,
        active_view: Option<Id>,
    ) -> Result<PlanProviderResults> {
        self.current(editor, active_view)?;
        ensure(
            Instant::now() < self.deadline,
            "plan batch deadline expired",
        )?;
        ensure(
            self.queued.is_empty() && self.pending.is_none(),
            "plan batch incomplete",
        )?;
        Ok(PlanProviderResults {
            graphics: self.results,
            failures: self.failures,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor() -> (Editor, Id) {
        let mut e = Editor::new().unwrap();
        let level = *e.document.model().levels.keys().next().unwrap();
        let view = e.create_floor_plan("Batch", level).unwrap();
        (e, view)
    }
    #[test]
    fn empty_completion_and_revocation_are_read_only_and_do_not_revive() {
        let (mut e, view) = editor();
        let before = e.document.model().clone();
        let revision = e.document.revision();
        let mut done = PlanProviderBatch::begin(&e, view, vec![], Duration::from_secs(5)).unwrap();
        assert!(done.poll(&mut e, Some(view)).unwrap());
        let output = done.into_results(&e, Some(view)).unwrap();
        assert!(output.graphics.is_empty() && output.failures.is_empty());
        for case in ["cancel", "view", "deadline"] {
            let mut batch =
                PlanProviderBatch::begin(&e, view, vec![], Duration::from_secs(5)).unwrap();
            match case {
                "cancel" => batch.cancel(),
                "deadline" => batch.deadline = Instant::now(),
                _ => {}
            }
            let active = if case == "view" { None } else { Some(view) };
            assert!(batch.poll(&mut e, active).is_err());
            assert!(batch.poll(&mut e, Some(view)).is_err());
            assert!(batch.into_results(&e, Some(view)).is_err());
        }
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.document.revision(), revision);
        assert_eq!(e.host.active_jobs(), 0);
    }
    #[test]
    fn model_change_and_reopen_revoke_finished_results() {
        let (mut e, view) = editor();
        let mut batch = PlanProviderBatch::begin(&e, view, vec![], Duration::from_secs(5)).unwrap();
        assert!(batch.poll(&mut e, Some(view)).unwrap());
        e.document = os_document::Document::from_model(e.document.model().clone()).unwrap();
        assert!(batch.into_results(&e, Some(view)).is_err());
        let mut batch = PlanProviderBatch::begin(&e, view, vec![], Duration::from_secs(5)).unwrap();
        e.document
            .execute(
                "Rename",
                vec![os_document::Command::RenameProject("Changed".into())],
            )
            .unwrap();
        assert!(batch.poll(&mut e, Some(view)).is_err());
        assert!(PlanProviderBatch::begin(&e, view, vec![], Duration::ZERO).is_err());
        assert!(PlanProviderBatch::begin(&e, view, vec![], Duration::from_secs(301)).is_err());
    }
}
