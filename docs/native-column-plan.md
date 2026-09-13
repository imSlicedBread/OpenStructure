# Native installed column plan integration

2026-09-12, Windows untracked working tree. D remains partial.

The opt-in workspace routes extension entities to their already loaded owner's
unique Wasm ViewProvider. Missing/ambiguous providers produce diagnostics; hidden
extensions are not requested. Opening files never installs or activates providers.
Host permissions, scope, ownership, version and result checks remain mandatory.

The batch runs guest calls outside the frame loop. Checked results compose on a
dedicated worker: at most one native drawing worker and one provider-composition
worker per workspace, each draining retired work before replacement. One batch
ticket uses the shared Wasm pool. Bounded synchronous routing, snapshot preparation
and reply validation remain; this is not a qualified frame-time guarantee.
Document/view/settings/activation changes cancel batches and invalidate output;
returning to a view cannot revive retired composition. Current lines participate
in drawing, Fit plan, semantic picking and snapping; failures remain unavailable.

## Native evidence

Computer-use skill inspection used `plugin_desktop` with the unchanged independent
`outputs/rust-column-plan-install` guest. The harness explicitly loads its directory
with developer-owned grants and unloads bundled Wall, not a permission-manager test.
Installed Wasm SHA256:
`813323450C1F47F16A880F00AC5C3C698203318FE9069C59A0ECF79210DA237C`.

1. Applied `org.example.columns.create`: width 0.4, depth 0.6, height 3 metres.
2. Created Floor plan 1, fitted its outline, enabled Split 2D / 3D.
3. Clicked a plan edge: properties identified the column and both views highlighted
   it. Outline interiors are not selectable in this service version.
4. Applied width 0.8 via the registered edit form. Both views updated. Undo restored
   0.4 in both; Redo restored 0.8 in both.
5. Saved the new `outputs/native-column-plan.osb` and closed the app.
6. Rebuilt after the correction below, reopened the saved project, opened its plan,
   enabled split and fitted. Selecting the 3D column highlighted the plan outline.
   The project remained clean; the window closed and its absence was confirmed.

Read-only ZIP/JSON inspection confirms model schema 4, one schema-1 extension
`8b8f2c87-5617-45ac-9b94-653db85a37ef`, dimensions 0.8 × 0.6 × 3 metres, its level
dependency and persisted Floor plan 1: cut 1.2, top 2.5, bottom 0, depth -1 metres,
default basis, no crop, scale 1:100. Navigation/split state remains session-local.
Screenshots were inspected in tool results, not saved separately.

Inspection found an obsolete empty-native-plan message over valid provider lines.
The corrected canvas includes provider lines in its empty-state condition; the
rebuilt native inspection confirmed the message absent. A real egui shape-output
test covers provider-only and empty canvases. Two provider-state tests cover
missing-provider diagnostics/visibility changes and composition retirement.

## Reproduction and checks

Use the [independent column SDK/build instructions](independent-column-plan.md).
No guest rebuild was needed for this follow-up. Build and run the host harness:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_desktop --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\examples\plugin_desktop.exe outputs/rust-column-plan-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
```

Harness build, full all-feature tests, strict Clippy, formatting and the default
workspace build pass. The rebuilt `column_plan_probe` also passes against the
unchanged guest. The bundled wall smoke passes at the fresh path below; it tests
the baseline, not independent-provider native behavior. Use fresh save paths.

```powershell
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/native-column-plan-verified.osb
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example column_plan_probe --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\examples\column_plan_probe.exe outputs/rust-column-plan-install
```

No SDK, file-format, dependency, permission or isolation change. Wasm remains
bounded in-process execution. Unknown payload preservation remains; IFC still
has no extension mapping. This is outline-only, not filled architectural cuts.

Next: independent pointer/preview authoring; full D linked-wall/failure acceptance;
then E1 architectural modeling and views. This run does not verify two snapped
independent walls, drag handles, arbitrary placement, cut fills, multi-level scope,
or full native cancellation/late-result workflows. Headless tests are complementary.
D/E1–E4/G1–G6 remain incomplete; L1/L2 later. Production profile choices remain
open at the owner's request. No commit, remote, GitHub URL or hosted CI result;
licensing and publication authority remain pending.
