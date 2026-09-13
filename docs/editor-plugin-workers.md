# Editor plugin command and geometry orchestration

With `os-ui/external-plugins`, Editor now owns pending descriptor commands and
geometry jobs. The default desktop feature set and visible controls are unchanged;
this is opt-in controller integration, not completed desktop authoring acceptance.

`start_plugin_tool(draft, context, view)` starts one reviewed command;
`cancel_plugin_tool` revokes it. Repeated `poll_plugin_work(current_context, view)`
commits checked results and updates the editor scene through geometry workers.
Selection/level changes cancel the pending command; host stamps reject stale
document/view/plugin results. Poll returns newly settled errors, commit status
and whether work is still pending. It does not wait for or execute Wasm code.
Host snapshot construction, validation, bounded prism tessellation and transaction
commit still run on the caller and require future performance qualification.

The scheduler shares host single-flight/four-slot limits. It scopes geometry to
the element and its prerequisite levels, not arbitrary related objects. Missing
geometry capability, unsupported worker adapters and failed replies become retained
errors, not per-frame automatic retries. `retry_plugin_geometry` is explicit.
Cancellation/drop lets revoked threads drain under their existing fuel bounds.

For correctness, current invalidation is conservative: any document revision or
activation change drops all managed plugin jobs/meshes and queues current plugin
elements. This is not yet targeted per-dependency caching or a production performance
claim. Undo never leaves the later mesh visible. View changes discard pending
geometry jobs; already accepted model geometry is view-independent, not plan output.
Plugin unload removes managed meshes while preserving authoritative entities.

Saving refuses pending commands/geometry or retained geometry failures, leaving
the destination untouched. After an optional plugin is unloaded, native save can
again preserve its opaque data without pretending geometry is available. Ordinary
bundled Wall regeneration and default-build save/open semantics remain unchanged.

## Verified workflow

After building/installing the [Wall](../examples/rust-wall/README.md) and
[column](../examples/rust-column/README.md) artifacts:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_editor_probe --locked --offline
.\target\debug\examples\plugin_editor_probe.exe outputs/rust-wall-install
.\target\debug\examples\plugin_editor_probe.exe outputs/rust-column-geometry-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
```

Both real installed Rust Wasm probes pass Editor command commit, asynchronous scene
generation, doubled-height geometry, undo/model-and-mesh restoration, cancellation,
pending-save rejection, dirty-state/save checks, native reopen comparison and unload
invalidation. Two new unit tests prove unsupported adapters do not execute
synchronously or retry continuously, and failed geometry prevents saving until
the optional plugin is unloaded. Full workspace suite: 133 passing tests locally
on Windows x64/Rust 1.98.1. Clippy, formatting, build and smoke are rerun.
They pass; the new smoke output is `outputs/editor-plugin-workers-verified.osb`.
Workspace license metadata remains valid for 347 packages with publication disabled.

## Remaining limits and next tasks

1. Render and connect descriptor forms and generic selection in DesktopApp, add
   visible pending/error/retry states and inspect actual native interactions.
2. Integrate plugin-manager loading/grants/unload and staged asynchronous external
   project opening, then migrations and complete installed Wall/column B/C acceptance.
   Follow-up: [staged opening](staged-plugin-open.md) now verifies the opt-in
   candidate/worker/adoption path for both installed guests. `Editor::open` still
   uses the v1 route; the visible desktop Open controls must adopt the new API.
3. Complete D linked plans and independent 2D providers, then E1–E4/G1–G6. L1/L2
   remain later milestones.

No public wire or native file version change, new dependency versions or unsafe
exceptions. Wasm remains bounded in-process execution, not a process sandbox.
Existing desktop extension warnings and IFC restrictions are not removed. No
production platform/project/concurrency/output profile or license is approved.
The untracked scaffold has no HEAD, remote or hosted CI result; CI now repeats
the two Editor probes but only local Windows execution is observed. No native
visual check is claimed because the visible desktop path was not changed.
