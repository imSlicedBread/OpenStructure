# Callable plan-graphics service foundation

2026-09-12. D partial: callable read-only host paths, bounded Wasm workers and
independent contract DTOs; not desktop plan-provider support or the required
independent Rust installation result.

## Protocol and SDK

`os-plugin-api::plan` works with `default-features = false`. API-2 manifests can
implement optional service `plan.graphics`, service version 1, through the existing
JSON transport. This is a separate tagged service envelope, not an incompatible
new required field on generic command requests/catalogs. Generic Describe/Invoke/
GenerateGeometry remain unchanged. A guest routes a request with this explicit
service tag to `plan::Request`; unsupported service/version must fail explicitly.
Older guests/hosts do not acquire plan support automatically. A ViewProvider
registration is a routing prerequisite, not capability-negotiation success.

Requests carry API/service versions, request UUID, project/session/model revision,
provider ID, target ID, a complete current horizontal downward-plan context and
the authorized snapshot. Context includes persisted view/settings identity, level
ID/elevation, XY origin/yaw, level-relative top/cut/bottom/depth, rectangular crop,
scale and native-wall/extension visibility. Metres and radians are explicit; this
does not represent ceiling/upward views, arbitrary work planes or screen input.

Responses echo the target and entire request context and return either bounded
Graphics or a structured Error. Graphics currently contain at most 256 finite
nonzero line segments, unique stable u32 feature keys and Cut/Projected/Depth
roles. Their semantic entity is the request target, never another object chosen
by the provider. Host validation rejects the entire result on any invalid segment,
duplicate key, excess count, wrong version/target/context, malformed JSON or
oversized response. Feature ordering is normalized before consumption.

Limits: 1 MiB per request/reply, at most 256 segments, segment length >1e-9 metres,
finite length/coordinates, error message 4096 bytes/field 64 bytes, acceptance
deadline >0 and <=30 seconds. These are development limits, not production budgets.
The host checks the time before invocation and after response parsing/validation;
it cannot interrupt trusted synchronous Rust. Existing Wasm fuel/memory bounds
still apply when that transport is used.

## Host workflow

1. Supply a `plan_graphics::PlanRequest` containing registered provider, target,
   persisted view, read-authority set and timeout.
2. Call `PluginHost::generate_plan_graphics(plugin, document, request)` explicitly.
   Current read authority must contain exactly the target and plan level. The
   snapshot has only those two objects; other model data is not serialized.
3. Consume `PlanGraphics::segments(host, document, active_view)` only while current.
   It rejects document edits/undo/reopen, another view and unloaded/replaced
   provider activation. `element()` is identity metadata, not validity authority.

Admission requires ModelRead, API 2, Views capability, a matching ViewProvider
registration, an owned registered target type/current payload and provider version,
valid visible plan category, and no live Wasm worker for that runtime. Native walls
use the existing explicit native projection; unknown opaque payload geometry is
never guessed. The service takes an immutable Document and cannot return edits.

**No desktop frame may call the synchronous service.** Use
`start_plan_graphics_job(plugin, document, request)` on the bounded Wasm adapter,
then the existing nonblocking `poll_job` with the current view context. It returns
`JobOutcome::PlanGraphics`; consume its checked segments with host/document/view.
The ticket derives view/settings identity from the persisted request view, not
an additional caller-supplied stamp. Synchronous and worker paths share the same
preparation and full response validator. Plan-scene aggregation and native
rendering remain subsequent steps.
The [renderer line foundation](provider-line-rendering.md) now implements the
clipping/drawing/picking/snap representation and validity-retaining controller
composition. Native scheduler/renderer wiring has not yet been added.

Plan jobs share the four-outstanding-job host limit and one-job-per-runtime limit
with commands/geometry, including buffered replies and draining revoked jobs.
Cancel/drop, deadline, model/view changes and unload/reload revoke acceptance.
Guest execution may drain fuel after cancellation; no OS-thread termination or
unbounded queue is introduced. Snapshot preparation and acceptance validation
remain synchronous and bounded, not hard real-time operations.
Returned lines are unclipped semantic output, not ready-made pick targets or an
issued drawing. A display/output consumer must apply host crop/styles and use the
same visible representation for picking. This foundation validates structure and
provenance, not the physical correctness of arbitrary provider claims about cuts.

## Evidence and remaining acceptance

`os-plugin-host/tests/plan_graphics.rs` invokes a test provider through actual
generic catalog loading and the new service transport. It verifies scoped payload
use, read-only behavior, stale consumption after edit/undo/reopen/view/unload, and
15 malformed/version/context/geometry/unsupported failure variants. Invalid read
scope, provider, view and deadline reject before the provider is called.

`os-plugin-api::plan::tests` adds bounded deterministic byte-mutation/truncation
fuzz smoke across the response parser and geometry validator, plus nonfinite/count
checks. This is not a coverage-guided security audit. The test provider's single
line is a protocol fixture, not a qualified architectural column representation.

Three worker tests now load actual WAT-generated Wasm fixtures through directory
installation. They cover successful read-only single-consumption delivery, shared
single-flight rejection (including synchronous bypass), deadline/slot release,
cancellation, inactive view, edit/undo/reopen, unload, dropped jobs, traps,
fuel-exhausted loops, and invalid range/duplicate/malformed replies. Slot counts
drain to zero and failures preserve the pre-poll document revision/model.
These fixtures validate the runtime path, not an architectural provider's output.

The [separately built column outline edition](independent-column-plan.md) now
passes installed Wasm-worker/controller acceptance. There is no new claim of native graphics/picking,
cut compliance, pointer tools, previews, snapping or one-gesture commit via an
independent provider. Those remain required D work. Existing SDK/example build
instructions, generic APIs, Wasm ABI 1, container 2/model 4 and plan settings 1
remain unchanged. No dependency or license choice changed.

## Verification commands

On the current untracked Windows working tree, the following passed, including
the new service tests and contract-only mutation test. There is no commit, remote
or hosted CI result. Native UI inspection was not repeated: this change adds no
desktop control or render integration.

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 test -p os-plugin-api --no-default-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\tools\cargo.ps1 run -p os-app --locked --offline --target-dir work/completion-build '--' --smoke outputs/plan-service-foundation.osb
```

The smoke ran after the interrupted turn resumed and created the new named
artifact. It passed bundled plugin load, wall edits, level reassignment,
regeneration, undo/redo and native save/reopen. It does not exercise the new plan
provider service; the dedicated service tests above provide that evidence. Use
a new output filename when repeating the smoke.

Worker follow-up verification repeated the full all-feature workspace tests,
strict all-target Clippy, formatting and default workspace build successfully.
The three new Wasm worker tests pass (six total plan-service integration tests).
The wire contract is unchanged by this follow-up. Host Rust consumers matching
`JobOutcome` exhaustively must handle the new `PlanGraphics` variant. The default
build still does not enable external Wasm plugins or desktop provider rendering.

```powershell
.\tools\cargo.ps1 run -p os-app --locked --offline --target-dir work/completion-build '--' --smoke outputs/plan-worker-verified.osb
```

That fresh smoke passed bundled plugin load, wall editing, level reassignment,
regeneration, undo/redo and save/reopen. Worker-specific evidence is supplied by
the Wasm tests, not the bundled smoke. No new native visual inspection, independent
Rust guest installation, dependency audit or hosted CI run is claimed.

Next: host-clipped plan-scene integration; separately built Rust-provider geometry;
native linked workflow acceptance. D and E1-E4 remain incomplete; G1-G6 are not qualified.
Production targets, licensing and publication remain open owner decisions.
L1/L2 remain separately planned follow-on work.
