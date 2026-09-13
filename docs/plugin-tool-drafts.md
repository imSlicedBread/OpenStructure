# Descriptor-driven tool draft controller

The follow-up [Editor worker integration](editor-plugin-workers.md) now connects
these drafts to command polling and scene regeneration; visible widgets remain pending.

`os_ui::plugin_tools::ToolDraft` now bridges validated plugin descriptors to
reviewable command inputs without mutating the document. This is controller
implementation, not activated desktop widgets or completed plugin-manager UI.

`begin(host, document, owner, command_id, ToolContext { level, selection })`
reads registered fields/defaults. Create uses command defaults; edit/delete require
one owned selected element. Matching edit fields read committed extension payload
or the native Wall projection. Fields absent from that payload use command defaults;
this is a command-input convention, not an arbitrary property-binding language.
Material/header fields are not exposed by the native projection.

`set` changes only a registered draft value. Invalid values stay in the draft so
widgets can explain errors; `validate` checks number limits, choices, text byte
limits, booleans and enabled state. No silent string/number/bool coercion occurs.
The immutable descriptor accessor supplies labels, units, choices and constraints
for the forthcoming form renderer. Unknown fields are rejected.

Invocation checks document session/revision, reviewed selection/level and the
host's new ephemeral `activation_id`. Reloading the same owner/version with the
same descriptors invalidates an old draft. It scopes only the selected element
and target level; it does not infer grants from all related objects. Create reserves
one identity; edit/delete authorize one selected target. The host still checks
permissions, payloads, current registry and the complete candidate graph.

With `os-ui/external-plugins`, `start` sends the reviewed inputs to a bounded Wasm
worker. It never calls the synchronous plugin convenience service. Consumers must
poll the returned job and cancel it when editing intent changes; stale document/
view/plugin replies remain rejected by the host. `Editor::regenerate`, loader
orchestration and DesktopApp controls have not been switched to this route yet.

## Reproduce

Use the installed examples from [Wall](../examples/rust-wall/README.md) and
[column](../examples/rust-column/README.md), then:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_tool_probe --locked --offline
.\target\debug\examples\plugin_tool_probe.exe outputs/rust-wall-install
.\target\debug\examples\plugin_tool_probe.exe outputs/rust-column-geometry-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
```

Both real installed Rust Wasm probes pass default-driven creation, selected-value
prefill, typed invalid-input rejection before dispatch, worker commit, edit refresh
and undo invalidation. The probe uses the same controller code for Wall and column,
discovering commands by descriptor mode rather than their concrete tool names.
Four new unit tests cover numeric/boolean/text/choice and disabled states, draft
isolation, context changes/reopen/undo and identical-descriptor plugin reload.

Current local result: 131 workspace tests pass on Windows x64/Rust 1.98.1. No wire
or native file-format change; the activation ID is host-only and not persisted.
No new dependency versions, unsafe exceptions or automatically loaded plugins.
The default desktop feature set remains unchanged. Native UI inspection was not
repeated because no frame/control interaction changed. These headless/controller
checks do not establish a usable desktop property panel or full B/C acceptance.

## Next three tasks

1. Integrate controller drafts and pending command/geometry results with Editor
   regeneration, selection/level changes and a host-rendered desktop form.
2. Add safe plugin-manager loading/grants/unload and migrations; run full installed
   Wall then column desktop acceptance, including native interaction checks.
3. Complete D linked floor plans and independent plan providers, then all E1–E4
   and G1–G6. L1/L2 remain after qualification.

Production targets and licensing remain undecided. The workspace is still an
untracked scaffold with no HEAD, remote or hosted CI result. CI has explicit
controller probes for both installed examples; only local Windows execution is
observed. No architectural or production gate is closed by this controller slice.
