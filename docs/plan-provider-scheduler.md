# Bounded plan-provider scheduler

Follow-up: [native column plan integration](native-column-plan.md) now connects
this scheduler to the native workspace and records installed-column inspection.
The controller-only evidence and next-work statements below describe this step's
original state; full independent pointer/failure acceptance remains open.

2026-09-12. Opt-in controller work for D, not native workspace acceptance.

`PlanProviderBatch::begin` captures current document/view/settings and provider
activation IDs. Callers supply explicit `(plugin, PlanRequest)` routes; ambiguous
metadata does not silently select a provider. Bounds: 10k unique targets, one view,
Wasm only, positive overall deadline <=300 seconds, and 10k retained segments.
Each request still passes the host's permission/scope/version and output checks.

`poll(editor, active_view)` accepts at most one reply and admits at most one
request per call, with one outstanding ticket per batch. Shared host worker limits
apply across batches and unrelated command/geometry jobs. Busy providers wait
without another queued worker, subject to the overall deadline. Guest execution
is asynchronous; bounded snapshot preparation and reply validation are still
synchronous, not a hard frame-time guarantee.

Cancel, document/view changes, inactive view, activation changes and expiry revoke
the batch and clear retained results. Dropped tickets may leave guest work draining
under fuel; slots are not released prematurely. Old views cannot revive revoked
work. `into_results` checks authority and completion again before consumption.

Per-target failures remain separate from successful checked graphics. Callers must
show those diagnostics and retain failed targets as unavailable when composing a
drawing. Partial success is not a complete drawing or permission to issue. Stale
batch authority and cumulative output overflow reject the whole batch.

## Verification

The rebuilt `column_plan_probe` passed against the unchanged independently built
`outputs/rust-column-plan-install` guest. It now uses the scheduler for plan
queries and additionally checks cancellation/actual slot draining and a two-request
batch with one valid result plus one invalid-target failure diagnostic. It retains
cut/projected composition, context invalidation, unload and save/reopen checks.
This is a headless follow-up, not a fresh guest installation or native inspection.

Two unit tests cover empty completion, cancellation, inactive-view/expiry
revocation, non-revival, late consumption after reopen, model changes and invalid
deadlines. The full all-feature workspace suite, strict Clippy and formatting pass:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example column_plan_probe --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\examples\column_plan_probe.exe outputs/rust-column-plan-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
```

New public Rust controller APIs are gated by external-plugins. Wire/native formats,
dependencies and runtime trust boundaries are unchanged. Native frame-loop routing,
scheduling, drawing and selection are next, then independent pointer authoring and
full linked native acceptance. D/E1-E4/G1-G6 remain incomplete; L1/L2 later.
Production targets, license and publication remain open. No commit/remote or hosted
CI result is claimed for this untracked Windows working tree.
