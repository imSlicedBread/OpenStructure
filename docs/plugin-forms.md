# Host-rendered plugin forms (partial desktop integration)

Local Windows verification, 2026-09-12. Behind `os-ui/external-plugins`, DesktopApp
now displays a Plugin tools window when an explicitly loaded API-2 catalog exists.
Commands, field labels, numeric units/bounds, choices, booleans and text byte limits
come from validated descriptors. No column-specific command or field enum was added.
Draft input remains separate from the model. Invalid partial numeric text stays
visible and disables Apply; Discard drops only the draft. Apply uses bounded Editor
workers; frames poll command/geometry results and expose pending, cancel, geometry
failure and retry controls. Pending work participates in New/Open/Close unsaved-work
checks; entering a confirmation cancels an outstanding command before further polling.

## Developer workflow

Install the independently compiled guests using the [column guide](../examples/rust-column/README.md)
or [Wall guide](../examples/rust-wall/README.md). Then:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_desktop --locked --offline
.\target\debug\examples\plugin_desktop.exe outputs/rust-column-geometry-install
# Or pass outputs/rust-wall-install to exercise the other registered descriptor.
```

The example explicitly replaces the bundled provider and grants model read/write
and UI tool access to the supplied local installation. This is a developer acceptance
harness, not automatic project loading or a production permission dialog. With no
argument it selects the repository's column geometry installation under outputs/.
Choose a registered create command, review/edit its fields and Apply. For edits,
choose an existing element in Target element, then its registered edit command.
Selecting another command or target discards the former draft. Stale document or
activation context is rejected at Apply; select the command again to review fresh data.

## Verification

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/plugin-forms-verified.osb
```

139 workspace tests pass, including a new real-egui-input test at 100% and 150%
scale: open the registered column form, confirm Width and metre bounds are visible,
type invalid numeric text, attempt Apply, verify no work/model/history change, and
Discard. This is automated frame verification, not native visual acceptance.
Strict lint, formatting, default build and the named smoke pass. Both installed
Editor probes also pass after the UI changes; they exercise workers and staged
reopening but do not operate the form widgets. The desktop harness builds.

The computer-use skill was used to attempt native verification. Its launch returned
`Computer Use app approval timed out`; no native form screenshot or interaction
was observed. That visual check remains pending; no alternate UI automation or
approval bypass was attempted. Other field kinds are rendered but do not yet have
native interaction acceptance. Use a new smoke filename when rerunning.

## Limits and next three tasks

1. Complete native input/visual acceptance for both guests. Shared selection and
   external Wall form routing now have follow-up implementation below; complete
   installed desktop authoring acceptance is still outstanding.
2. Add explicit production plugin management/loading/grants and connect staged Open
   to desktop file controls. Implement plugin migration delivery and qualify the full
   independent installation, authoring, persistence and absent-plugin workflow.
3. Complete D linked plans and independent providers, then E1–E4 and G1–G6.
   L1/L2 remain later milestones, not completed scope.

No wire/API schema, file version, dependency or unsafe-policy changes. The new
window is opt-in; the ordinary os-app build still uses its bundled plugin. Wasm
remains bounded in-process execution, not process isolation. IFC restrictions and
opaque preservation guarantees are unchanged. The [coverage ledger](2d-coverage.md)
continues to show incomplete architectural/companion BIM capabilities and production
gates. No approved jurisdiction, deployment/project size/concurrency, CAD/PDF or plot
profile; no issued reference-project or physical plotting qualification. Windows
only observed, Linux CI configured but not observed, macOS unverified.

All source remains an untracked scaffold with no HEAD, remote, GitHub URL or hosted
CI result. No publication performed; project license decision remains pending.

## Shared selection follow-up

The form target picker now uses DesktopApp's selection rather than a second stored
ID. Viewport picking/highlighting, the new Plugin elements browser group and the
form refer to that same semantic ID. Changing selection discards the reviewed form
and cancels any pending command. Refresh/history retains an existing extension
selection and clears it when the entity is removed. Unknown extensions remain
selectable for read-only inspection. Native Wall mutation controls cannot mutate
a selected extension, and its properties do not show Wall instance fields.

For an explicitly loaded API-2 Wall provider, legacy Wall Apply/Delete actions now
open its registered create/edit/delete form for review. The legacy property inputs
are replaced with a pointer to that form; no guest is invoked by this routing step.
The user must still Apply the reviewed plugin command. Bundled v1 Wall behavior is
unchanged. The incomplete-view warning now counts missing extension meshes, not
every extension in the model.

Re-run the workflow above: select a plugin element in the browser or rendered
viewport, choose its registered edit command, and review parameters. Clearing or
changing selection discards the draft. Automated tests use actual egui pointer
input at a rendered extension mesh sample, then check browser selection, history,
deletion clearing, native-control protection and form targeting. The mesh-picking
test deliberately supplies a host test mesh; it is not independent Wasm geometry
acceptance. The separate installed probes retain that controller-level evidence.

Current full suite: **141 passing tests**, up from 139. Strict Clippy, formatting,
default build, both installed Editor probes, and the desktop harness build pass.
Smoke passes at `outputs/shared-plugin-selection-verified.osb`. Use the commands
above with that new smoke path (or another unused name). Native external Wall
routing and visual acceptance remain unverified; the earlier launch-approval timeout
was not bypassed. Public contracts, file versions, dependencies, trust boundaries,
publication status and production-profile decisions are unchanged. The full
BUILD_PROMPT and all remaining B/C, D, E1–E4/G1–G6 and later L1/L2 work remain active.
