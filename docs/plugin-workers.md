# Plugin worker execution and reply validity

This is progress within BUILD_PROMPT B, not completion of the external-plugin
or production release gates. The existing v1 messages and buffer ABI remain
unchanged. New host-owned tickets prevent stale replies without trusting a guest
to label its own document/revision correctly. See [ADR 0008](decisions/0008-worker-reply-validity.md).

## Implemented contract

Enable `os-plugin-host/wasm`, explicitly load a Wasm directory, then call
`start_job(plugin, document, request, optional_view_context, timeout)`.
Poll with `poll_job(job, current_document, current_view_context, label)`.
It returns pending, a committed command outcome, checked geometry, or a typed
`JobFailure`. Call `PendingJob::cancel` to revoke acceptance; poll or drop the job
to dispose of its receiver. These APIs do not require a GUI.

For API 2, use `start_generic_job(plugin, document, Invocation, optional_view_context)`
and the same `poll_job`. The ticket retains host-only scope, typed inputs, reserved
identities and request correlation; guest Edits pass the shared generic validator.
V1 and v2 share the four-slot host counter and per-runtime single-flight permits.
Settling a ticket drops its retained request authority along with the receiver.
`start_generic_job` returns committed edits. `start_generic_geometry_job` now uses
the same pool for read-only v2 geometry and returns a `GenericGeometry` outcome;
its accessor also checks document/view validity. See [geometry worker evidence](generic-plugin-geometry.md).
Cancel when the host's
selection/authorization intent changes. `ViewContext.settings_revision` must now
equal the referenced view's persisted `ViewParams.settings_revision`. Dispatch,
polling and checked geometry consumption enforce that value against the document,
not only against the caller's echoed context. A navigation counter must not be
passed in this field; screen-query identity is separate. See the
[persisted-view boundary regression](worker-persisted-view-validation.md).

Every job owns a request UUID, document session/revision, optional view ID and
settings revision, plugin load generation and deadline. `Document::session_id`
is fresh for every new/opened document, including reopening the same project;
it is not saved and does not affect dirty state. Model edits and undo/redo change
the monotonic revision, so restoring the same model does not resurrect an old job.

Permissions/capabilities and input limits are checked before dispatch. Only the
bounded Wasm adapter can start background jobs. Poll rejects cancelled, expired,
wrong-session/revision/view or unloaded/reloaded-plugin replies before parsing or
committing. Existing output version/type/ownership/command-count checks still run.
Commands commit in the poll transaction, once; callers do not receive deferred
commands that could accidentally be applied to another revision. Geometry consumers
must call `CheckedGeometry::solid(current_document, current_view)` before use;
the geometry kernel still performs geometric validation/tessellation afterward.

There is one outstanding job per runtime and four per host, including unread
buffered replies and retired/unloaded executions. A shared permit survives until
both the worker and receiver owner release it. No unbounded queue or cancellation
thread leak is introduced. `active_jobs()` reports occupied slots, not just CPU-
running threads. Synchronous requests cannot bypass a runtime's occupied slot.

`unload` removes registrations and callable access without touching model/files;
loaded dependents must first be removed. In-flight execution retains its module,
but its reply can no longer commit. Reload creates a different generation.

## Timing and remaining limitations

Timeouts must be positive and at most 30 seconds; this cap is a prototype safety
limit, not an approved production performance target. Deadlines cover result
acceptance, checked before response validation and before starting the transaction.
Cancellation/expiry does not forcibly kill an OS thread. Wasm fuel/memory limits
bound guest work after revocation. Rust unwind panics become worker errors;
process abort, OOM and interpreter vulnerabilities are not process-isolated.

Artifact loading/compilation and authorized snapshot preparation remain synchronous;
they need background integration before desktop activation. Document validation/
commit still runs on the caller, not under a hard real-time deadline. Named native
plan views exist, but independent callable plan providers remain required. Generic
wire request IDs/errors/descriptors and extension durability now have partial
implementations, with [generic worker evidence](generic-plugin-commands.md).
Independent Rust guest SDK/geometry, descriptor-driven UI and manager integration
are still required. No default desktop path
has been switched to external plugins or worker jobs.

## Independent probe and verification

After assembling the fixture as documented in [the Wasm guide](wasm-runtime-spike.md):

```powershell
.\tools\cargo.ps1 build -p os-plugin-host --features wasm --example wasm_probe --locked --offline
.\target\debug\examples\wasm_probe.exe --worker outputs\wasm-probe-installation
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs\worker-boundary-verified.osb
```

Observed on Windows x64 / Rust 1.98.1 / MSVC: external installed binary worker
returned checked unit-prism geometry without host source changes; build, formatting,
strict Clippy and native smoke passed. The expanded suite has 86 passing tests,
including 11 new worker/lifecycle tests. All-feature license metadata still passes
for 347 packages. No package version changed; legal approval remains pending.
Clippy retains the old Windows `os-geometry` incremental-cache access-denied note
(exit zero). The last refreshed dependency audit remains the baseline, including
the unmaintained `ttf-parser` warning; it was not rerun for this no-version-change step.

Tests cover real guest threads, one-time commit/history, edit/undo/reopen rejection,
view switches/settings changes, cancellation/deadlines, revoked infinite guests
draining under fuel, unload/reload, dependency order, authorization, traps,
geometry re-checks and deterministic slot ownership in both disposal orders.
CI adds the external worker probe; hosted CI has not run. No desktop controls
changed, so native visual inspection was not repeated. The repository remains an
untracked scaffold, with no commit or remote publication.

Next: generic Rust SDK/2D interaction contracts; durable plugin entity/dependency
envelopes and migrations; independently installed Wall/column descriptors and
desktop lifecycle acceptance. Then D and all E1–E4/G1–G6 requirements remain.
The owner deployment/project/output profile is still undecided, as recorded in
[the coverage ledger](2d-coverage.md); no production guarantee is inferred.

Follow-up generic worker verification: 117 all-feature workspace tests pass on
Windows, including six additional v2 worker tests. Existing v1 deadline behavior
is preserved: a positive deadline that expires during preparation yields a ticket
whose poll rejects it. Generic preparation also checks its own acceptance deadline
before dispatch and may return an error without a ticket. Both reject expiry at
poll and after validation, before commit. No wire or storage schema version changed.
