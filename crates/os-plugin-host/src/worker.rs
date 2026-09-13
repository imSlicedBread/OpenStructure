//! Host-owned v1/v2 worker tickets; desktop activation remains gated.
use crate::{PluginHost, wasm::WasmPlugin};
use os_core::{Error, Id, Result, ensure};
use os_document::Document;
use os_geometry::Solid;
use os_plugin_api::{Plugin, Request, Response};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

pub const MAX_ACTIVE_JOBS: usize = 4;
pub const MAX_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewContext {
    pub id: Id,
    /// Persisted ViewParams.settings_revision, not a caller-owned navigation counter.
    pub settings_revision: u64,
}

impl ViewContext {
    fn is_current(self, document: &Document) -> bool {
        document
            .model()
            .views
            .get(&self.id)
            .is_some_and(|view| view.parameters.settings_revision == self.settings_revision)
    }
}

#[derive(Clone, Copy, Debug)]
struct Stamp {
    session: Id,
    revision: u64,
    view: Option<ViewContext>,
}
impl Stamp {
    fn validate(
        self,
        document: &Document,
        view: Option<ViewContext>,
    ) -> std::result::Result<(), JobFailure> {
        if document.session_id() != self.session || document.revision() != self.revision {
            return Err(JobFailure::StaleDocument);
        }
        if self.view != view || view.is_some_and(|v| !v.is_current(document)) {
            return Err(JobFailure::StaleView);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum JobFailure {
    Cancelled,
    Deadline,
    StaleDocument,
    StaleView,
    PluginUnloaded,
    Disconnected,
    AlreadySettled,
    Rejected(Error),
}
impl std::fmt::Display for JobFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(error) => write!(f, "plugin reply rejected: {error}"),
            other => write!(f, "plugin job: {other:?}"),
        }
    }
}
impl std::error::Error for JobFailure {}

pub struct CheckedGeometry {
    stamp: Stamp,
    solid: Solid,
}
impl CheckedGeometry {
    /// Re-check at consumption, not merely when the worker finished.
    pub fn solid(
        &self,
        document: &Document,
        view: Option<ViewContext>,
    ) -> std::result::Result<&Solid, JobFailure> {
        self.stamp.validate(document, view)?;
        Ok(&self.solid)
    }
}
pub enum JobOutcome {
    Committed,
    Geometry(CheckedGeometry),
    GenericGeometry(CheckedGenericGeometry),
    PlanGraphics(crate::plan_graphics::PlanGraphics),
}

pub struct CheckedGenericGeometry {
    stamp: Stamp,
    geometry: crate::generic_geometry::GeometryResult,
}
impl CheckedGenericGeometry {
    pub fn get(
        &self,
        document: &Document,
        view: Option<ViewContext>,
    ) -> std::result::Result<(Id, &Solid, &os_geometry::Mesh), JobFailure> {
        self.stamp.validate(document, view)?;
        self.geometry.get(document).map_err(JobFailure::Rejected)
    }
}

pub struct PendingJob {
    id: Id,
    plugin_id: String,
    generation: Id,
    stamp: Stamp,
    kind: Option<JobKind>,
    deadline: Instant,
    receiver: Option<Receiver<Result<String>>>,
    permit: Option<Arc<Permit>>,
    cancelled: bool,
}

enum JobKind {
    Legacy { geometry: bool },
    Generic(Box<crate::generic::Prepared>),
    GenericGeometry(Box<crate::generic_geometry::PreparedGeometry>),
    PlanGraphics(Box<crate::plan_graphics::PreparedPlan>),
}
struct Dispatch {
    id: Id,
    input: String,
    kind: JobKind,
    view: Option<ViewContext>,
    deadline: Instant,
}
impl PendingJob {
    pub fn id(&self) -> Id {
        self.id
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

pub(super) struct Runtime {
    plugin: WasmPlugin,
    generation: Id,
    busy: AtomicBool,
}
impl Runtime {
    pub(super) fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
    pub(super) fn new(plugin: WasmPlugin) -> Self {
        Self {
            plugin,
            generation: Id::new(),
            busy: AtomicBool::new(false),
        }
    }
}
struct Permit {
    runtime: Arc<Runtime>,
    count: Arc<AtomicUsize>,
}
fn reserve(runtime: Arc<Runtime>, count: Arc<AtomicUsize>) -> Result<Permit> {
    ensure(
        runtime
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok(),
        "plugin already has a live worker",
    )?;
    if count
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < MAX_ACTIVE_JOBS).then_some(n + 1)
        })
        .is_err()
    {
        runtime.busy.store(false, Ordering::Release);
        return Err(Error::Invalid("host worker capacity reached".into()));
    }
    Ok(Permit { runtime, count })
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.runtime.busy.store(false, Ordering::Release);
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}

impl PluginHost {
    /// Authorized snapshot preparation is synchronous; guest execution is not.
    /// There is no unbounded task queue. Cancellation does not kill OS threads.
    pub fn start_job(
        &self,
        plugin_id: &str,
        document: &Document,
        request: Request,
        view: Option<ViewContext>,
        timeout: Duration,
    ) -> Result<PendingJob> {
        ensure(
            !timeout.is_zero() && timeout <= MAX_DEADLINE,
            "worker timeout must be positive and at most 30 seconds",
        )?;
        ensure(
            view.is_none_or(|v| v.is_current(document)),
            "active view or settings revision does not match document",
        )?;
        let deadline = Instant::now() + timeout;
        let loaded = self.plugin(plugin_id)?;
        let geometry = matches!(request, Request::GenerateWall { .. });
        let input = Self::prepare_input(loaded, document.model(), request)?;
        ensure(
            input.len() <= crate::wasm::MAX_WASM_MESSAGE_BYTES,
            "Wasm request exceeds 1 MiB",
        )?;
        self.dispatch_job(
            plugin_id,
            document,
            Dispatch {
                id: Id::new(),
                input,
                kind: JobKind::Legacy { geometry },
                view,
                deadline,
            },
        )
    }

    /// Start a generic command on the same bounded Wasm worker pool as v1.
    /// Scope, inputs and reserved IDs are frozen before the worker can run.
    /// Cancel the ticket when host selection/authorization intent is revoked.
    pub fn start_generic_job(
        &self,
        plugin_id: &str,
        document: &Document,
        invocation: crate::generic::Invocation,
        view: Option<ViewContext>,
    ) -> Result<PendingJob> {
        self.start_generic_job_inner(plugin_id, document, invocation, view, false)
    }
    pub(super) fn start_generic_job_inner(
        &self,
        plugin_id: &str,
        document: &Document,
        invocation: crate::generic::Invocation,
        view: Option<ViewContext>,
        migration_batch: bool,
    ) -> Result<PendingJob> {
        let loaded = self.plugin(plugin_id)?;
        ensure(
            loaded.worker.is_some(),
            "background generic jobs require the bounded Wasm adapter",
        )?;
        ensure(
            view.is_none_or(|v| v.is_current(document)),
            "active view or settings revision does not match document",
        )?;
        let mut prepared =
            Self::prepare_generic_inner(loaded, document, invocation, migration_batch)?;
        self.dispatch_job(
            plugin_id,
            document,
            Dispatch {
                id: prepared.id,
                input: std::mem::take(&mut prepared.input),
                deadline: prepared.deadline,
                kind: JobKind::Generic(Box::new(prepared)),
                view,
            },
        )
    }

    /// Scoped model geometry on the shared Wasm pool; view context gates delivery
    /// and consumption, but is not a plan-graphics contract sent to the guest.
    pub fn start_generic_geometry_job(
        &self,
        plugin_id: &str,
        document: &Document,
        element: Id,
        read: std::collections::BTreeSet<Id>,
        view: Option<ViewContext>,
        timeout: Duration,
    ) -> Result<PendingJob> {
        ensure(
            self.plugin(plugin_id)?.worker.is_some(),
            "geometry workers require Wasm",
        )?;
        ensure(
            view.is_none_or(|v| v.is_current(document)),
            "active view or settings revision does not match document",
        )?;
        let mut prepared =
            self.prepare_generic_geometry(plugin_id, document, element, read, timeout)?;
        self.dispatch_job(
            plugin_id,
            document,
            Dispatch {
                id: prepared.id,
                input: std::mem::take(&mut prepared.input),
                deadline: prepared.deadline,
                kind: JobKind::GenericGeometry(Box::new(prepared)),
                view,
            },
        )
    }

    /// Plan graphics share the bounded pool, cancellation and view-stamped
    /// delivery rules. Returned segments must still be host-clipped for display.
    pub fn start_plan_graphics_job(
        &self,
        plugin_id: &str,
        document: &Document,
        invocation: crate::plan_graphics::PlanRequest,
    ) -> Result<PendingJob> {
        ensure(
            self.plugin(plugin_id)?.worker.is_some(),
            "plan workers require Wasm",
        )?;
        let view_id = invocation.view;
        let mut prepared = self.prepare_plan_graphics(plugin_id, document, invocation)?;
        let view = Some(ViewContext {
            id: view_id,
            settings_revision: document.model().views[&view_id]
                .parameters
                .settings_revision,
        });
        self.dispatch_job(
            plugin_id,
            document,
            Dispatch {
                id: prepared.id,
                input: std::mem::take(&mut prepared.input),
                deadline: prepared.deadline,
                kind: JobKind::PlanGraphics(Box::new(prepared)),
                view,
            },
        )
    }

    fn dispatch_job(
        &self,
        plugin_id: &str,
        document: &Document,
        dispatch: Dispatch,
    ) -> Result<PendingJob> {
        let Dispatch {
            id,
            input,
            kind,
            view,
            deadline,
        } = dispatch;
        let loaded = self.plugin(plugin_id)?;
        let runtime = loaded
            .worker
            .as_ref()
            .ok_or_else(|| {
                Error::Unsupported("background jobs require the bounded Wasm adapter".into())
            })?
            .clone();
        let generation = runtime.generation;
        let permit = Arc::new(reserve(runtime, self.worker_count.clone())?);
        let worker_permit = permit.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("os-wasm-job".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker_permit.runtime.plugin.invoke_json(&input)
                }))
                .unwrap_or_else(|_| Err(Error::Invalid("Wasm worker panicked".into())));
                let _ = sender.try_send(result);
                drop(worker_permit);
            })
            .map_err(|e| Error::Invalid(format!("cannot start plugin worker: {e}")))?;
        Ok(PendingJob {
            id,
            plugin_id: plugin_id.into(),
            generation,
            stamp: Stamp {
                session: document.session_id(),
                revision: document.revision(),
                view,
            },
            kind: Some(kind),
            deadline,
            receiver: Some(receiver),
            permit: Some(permit),
            cancelled: false,
        })
    }

    /// Nonblocking, single-consumption delivery. Commands commit inside this call,
    /// so a caller cannot accidentally apply them to a later document revision.
    pub fn poll_job(
        &self,
        job: &mut PendingJob,
        document: &mut Document,
        view: Option<ViewContext>,
        label: &str,
    ) -> std::result::Result<Option<JobOutcome>, JobFailure> {
        if job.receiver.is_none() {
            return Err(JobFailure::AlreadySettled);
        }
        let result = (|| {
            if job.cancelled {
                return Err(JobFailure::Cancelled);
            }
            if Instant::now() >= job.deadline {
                return Err(JobFailure::Deadline);
            }
            job.stamp.validate(document, view)?;
            let loaded = self
                .loaded
                .get(&job.plugin_id)
                .ok_or(JobFailure::PluginUnloaded)?;
            if loaded.worker.as_ref().map(|r| r.generation) != Some(job.generation) {
                return Err(JobFailure::PluginUnloaded);
            }
            let output = match job.receiver.as_ref().expect("checked above").try_recv() {
                Ok(output) => output.map_err(JobFailure::Rejected)?,
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => return Err(JobFailure::Disconnected),
            };
            let response = match job.kind.as_ref().expect("live job retains reply authority") {
                JobKind::Legacy { geometry } => {
                    Self::validate_output(loaded, document.model(), *geometry, &output)
                }
                JobKind::Generic(prepared) => {
                    Self::generic_commands(loaded, document, prepared, &output)
                        .map(Response::Commands)
                }
                JobKind::GenericGeometry(prepared) => {
                    let geometry = Self::finish_generic_geometry(document, prepared, &output);
                    if Instant::now() >= job.deadline {
                        return Err(JobFailure::Deadline);
                    }
                    let geometry = geometry.map_err(JobFailure::Rejected)?;
                    return Ok(Some(JobOutcome::GenericGeometry(CheckedGenericGeometry {
                        stamp: job.stamp,
                        geometry,
                    })));
                }
                JobKind::PlanGraphics(prepared) => {
                    let graphics = self
                        .finish_plan_graphics(document, prepared, &output)
                        .map_err(JobFailure::Rejected)?;
                    if Instant::now() >= job.deadline {
                        return Err(JobFailure::Deadline);
                    }
                    return Ok(Some(JobOutcome::PlanGraphics(graphics)));
                }
            }
            .map_err(JobFailure::Rejected)?;
            // Parsing/validation also counts against acceptance time.
            if Instant::now() >= job.deadline {
                return Err(JobFailure::Deadline);
            }
            match response {
                Response::Commands(commands) => {
                    document
                        .execute(label, commands)
                        .map_err(JobFailure::Rejected)?;
                    Ok(Some(JobOutcome::Committed))
                }
                Response::Solid(solid) => Ok(Some(JobOutcome::Geometry(CheckedGeometry {
                    stamp: job.stamp,
                    solid,
                }))),
            }
        })();
        if !matches!(result, Ok(None)) {
            job.receiver.take();
            job.permit.take();
            job.kind.take();
        }
        result
    }
    /// Includes running jobs, draining revoked jobs and unconsumed buffered replies.
    pub fn active_jobs(&self) -> usize {
        self.worker_count.load(Ordering::Acquire)
    }
    /// Admission hint for a nonblocking editor scheduler. Start still enforces it.
    pub fn worker_supported(&self, plugin: &str) -> bool {
        self.loaded.get(plugin).is_some_and(|p| p.worker.is_some())
    }
    /// Admission hint for a nonblocking editor scheduler. Start still enforces it.
    pub fn worker_available(&self, plugin: &str) -> bool {
        self.loaded
            .get(plugin)
            .and_then(|p| p.worker.as_ref())
            .is_some_and(|r| !r.is_busy() && self.active_jobs() < MAX_ACTIVE_JOBS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consumption_rejects_agreeing_ticket_and_caller_with_wrong_persisted_revision() {
        let document = Document::new("View revision").unwrap();
        let id = *document.model().views.keys().next().unwrap();
        for settings_revision in [1, u64::MAX] {
            let view = Some(ViewContext {
                id,
                settings_revision,
            });
            // A matching caller is not sufficient authority, even if a future
            // dispatch path were to accidentally admit an invalid stamp.
            let stamp = Stamp {
                session: document.session_id(),
                revision: document.revision(),
                view,
            };
            assert!(matches!(
                stamp.validate(&document, view),
                Err(JobFailure::StaleView)
            ));
        }
        let view = Some(ViewContext {
            id,
            settings_revision: 0,
        });
        assert!(
            Stamp {
                session: document.session_id(),
                revision: document.revision(),
                view,
            }
            .validate(&document, view)
            .is_ok()
        );
    }

    fn runtime() -> Arc<Runtime> {
        let manifest = os_plugin_api::Manifest::from_toml(include_str!(
            "../../../fixtures/wasm-probe/plugin.toml"
        ))
        .unwrap();
        let bytes = wat::parse_str(include_str!("../../../fixtures/wasm-probe/probe.wat")).unwrap();
        Arc::new(Runtime::new(WasmPlugin::new(manifest, &bytes).unwrap()))
    }
    #[test]
    fn permits_enforce_single_flight_and_global_limit_until_actual_release() {
        let count = Arc::new(AtomicUsize::new(0));
        let first = runtime();
        let mut permits = vec![reserve(first.clone(), count.clone()).unwrap()];
        assert!(reserve(first.clone(), count.clone()).is_err());
        assert_eq!(count.load(Ordering::Acquire), 1);
        for _ in 1..MAX_ACTIVE_JOBS {
            permits.push(reserve(runtime(), count.clone()).unwrap());
        }
        let fifth = runtime();
        assert!(reserve(fifth.clone(), count.clone()).is_err());
        assert!(!fifth.is_busy());
        assert_eq!(count.load(Ordering::Acquire), MAX_ACTIVE_JOBS);
        // Removing a loaded reference cannot release a worker-owned permit.
        drop(first);
        assert_eq!(count.load(Ordering::Acquire), MAX_ACTIVE_JOBS);
        permits.pop();
        let replacement = reserve(fifth, count.clone()).unwrap();
        drop(permits);
        drop(replacement);
        assert_eq!(count.load(Ordering::Acquire), 0);
    }
    #[test]
    fn permit_is_retained_until_worker_and_reply_owner_both_release() {
        let count = Arc::new(AtomicUsize::new(0));
        let pending_owner = Arc::new(reserve(runtime(), count.clone()).unwrap());
        let executing_owner = pending_owner.clone();
        std::thread::spawn(move || drop(executing_owner))
            .join()
            .unwrap();
        assert_eq!(
            count.load(Ordering::Acquire),
            1,
            "buffered reply retains capacity"
        );
        drop(pending_owner);
        assert_eq!(count.load(Ordering::Acquire), 0);
        let pending_owner = Arc::new(reserve(runtime(), count.clone()).unwrap());
        let executing_owner = pending_owner.clone();
        let (send, receive) = mpsc::sync_channel::<()>(1);
        let thread = std::thread::spawn(move || {
            receive.recv().unwrap();
            drop(executing_owner);
        });
        drop(pending_owner);
        assert_eq!(
            count.load(Ordering::Acquire),
            1,
            "cancelled execution retains capacity"
        );
        send.send(()).unwrap();
        thread.join().unwrap();
        assert_eq!(count.load(Ordering::Acquire), 0);
    }
}
