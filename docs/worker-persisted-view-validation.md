# Worker persisted-view validation

2026-09-12, D prerequisite correction, not independent plan-provider acceptance.

The original worker contract accepted an existing view UUID with any caller-owned
settings counter. With persisted view settings now present, this permitted a job
to start against a counter that did not describe the document at all. Repeating
that same counter at poll/consumption passed the old echo-only check. Normal model
edits already advanced the document revision and rejected old replies; this is
an admission/consumption boundary gap, not evidence that ordinary edits bypassed
document revision checks.

`ViewContext.settings_revision` now means exactly the persisted view parameter's
revision. Legacy command, generic command (including migration batch), and generic
geometry start paths validate it before dispatch and worker-slot reservation.
The common stamp validator checks it again on poll and checked geometry access.
Missing views and incorrect counters are rejected; None remains valid for
view-independent jobs. Session, document revision, caller view identity, plugin
generation, cancellation and deadline checks remain in force.

This tightens the public host Rust API's semantics without changing its shape.
Callers using a navigation counter must instead supply the persisted settings
revision. Desktop forms already do so. No JSON API-1/API-2, Wasm ABI, native
container/model/settings schema, SDK guest code or dependency changed. Navigation
and future gesture/query identity must be handled separately rather than encoded
in this field. No plan wire operation is advertised by this correction.

## Regression evidence

Before the fix, `geometry_worker_requires_the_persisted_view_revision_before_dispatch`
failed with "accepted a view revision absent from the document" against the actual
Wasm worker path. Its document contains stored settings revision 7; requests for
0, 6, 8 and u64::MAX must fail without reserving a worker slot. Revision 7 must
return consumable checked geometry with unchanged model, revision and history.

Existing legacy/generic command view tests additionally reject wrong persisted
counters before invocation. The private stamp regression verifies that agreeing
ticket/caller counters still fail consumption if they contradict the document.
Existing tests retain view switching, edit/undo/reopen, cancellation, unload,
late replies and single-consumption coverage. These WAT fixtures test the host
boundary; they are not a newly installed independent Rust plan provider.

Verification passed on the current untracked Windows x64 working tree, with no
Git commit/remote or hosted CI result. Commands:

```powershell
.\tools\cargo.ps1 test -p os-plugin-host --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
```

No desktop interaction changed, so native visual inspection was not repeated.
Existing independent Wall/column SDK installation instructions remain valid;
their prior acceptance is historical, not a new installation result for this step.
Trusted bundled plugins and bounded, no-import Wasm retain their existing trust
distinction. Native storage remains container 2/model 4, with documented size and
unknown-field limits. D remains partial; E1-E4, production targets and G1-G6 remain
open. L1/L2 are later milestones. Next dependencies: callable plan wire services,
their host/desktop integration, then installed independent linked-view acceptance.
