# Provider-line drawing foundation

Follow-up: [native column plan integration](native-column-plan.md) now wires the
background pipeline into the canvas and records installed-column inspection.
The earlier unwired status below is historical, not the current native status.

Background-composition follow-up (2026-09-12): `prepare_provider_plan` now
validates/maps bounded results and copies native plan inputs into a transferable
`PreparedProviderPlan`. Its `derive` runs without Editor, Document or runtime
handles and returns the same validity-retaining `ProviderPlanDrawing`. The
synchronous convenience method delegates to these stages; it is not intended
for native frame-loop use. This preserves the existing background wall-derivation
architecture rather than moving full prism derivation onto the UI thread.

A channel-coordinated test runs actual drawing derivation on another thread and
checks current output plus stale output after document edit or provider reload
while the worker is suspended. No sleeps or timing guesses coordinate that test.
The rebuilt installed-column probe also derives on a separate host thread and
passes against the unchanged independent Wasm artifact. Native frame-loop
admission, result handling and painting still require wiring and inspection.

2026-09-12, renderer portion of D; no native plugin-rendering acceptance yet.

`PlanDrawing::with_provider_lines` resolves checked provider output for entities
previously marked unavailable. It takes host-mapped `PlanLine` values, not plugin
runtime handles. Callers must consume the worker result with the current host,
document and view before mapping service roles/coordinates to these render values.
The controller adapter is now implemented as described below; its native
scheduling/render loop remains unfinished.

Each entity can supply up to 256 finite nonzero lines with unique semantic feature
keys. Wrong target IDs, replacement of native/grid identities, duplicate features,
nonfinite/degenerate geometry and excess input reject the drawing build wholly.
Provider/grid line inputs share a 10,000-segment bound, including cropped-away
inputs and repeated attachment batches. Final combined snap-source counts also
pass the existing 10k snap-scene validation. No partial result is returned on error.

Clipping uses the same finite-line host algorithm as grids. Display/picking uses
clipped extents; snap candidates retain the original feature endpoints and true
midpoint. Crop intersections therefore do not become invented endpoint snaps.
Fully invisible lines have no drawing/picking/snap entries. An explicitly valid
empty representation resolves the unavailable marker without inventing graphics.

Composition is fixed for this development subset: grids behind provider lines,
then native polygon bodies. Provider lines sort by depth/projected/cut role,
entity UUID and feature; line picking checks reverse draw order with the existing
logical-pixel radius. Polygon interiors retain priority, then provider lines,
then grids. The drawing adapter must follow that same order. This is not general
office style, masking, fill, text or output support.

`provider_lines(current_context)`, existing picking and snap accessors retain
full document/view/settings context checks. Provider lines join the normal snap
scene alongside native axes and datums; no provider-controlled snap priority is
introduced. Selection IDs remain semantic element IDs, not transient line indices.

## Evidence

`os-render/tests/provider_lines.rs` covers crop/display endpoints versus original
snap endpoints, clipped nearest acquisition, matching screen picking, exclusion,
stale settings, deterministic role ordering/reverse picking, unavailable handling,
invalid batches and an oversized entirely cropped-away source batch. All renderer
tests pass. Existing grid/snap tests retain large-coordinate and crop boundaries.

No new dependency, plugin wire change or persistence migration. Public Rust drawing
APIs gain PlanLine/with_provider_lines/provider_lines; existing constructors retain
empty provider-line collections. No desktop interaction changes in this step.
This is a checked render representation, not evidence that an independently built
provider draws correct architecture, nor evidence of native UI integration.

## Verification and next work

`Editor::plan_with_provider_graphics` now maps completed checked service results
into the same native-wall/grid derivation pipeline. It validates every result
against the current document/view/provider, rejects duplicate entity results,
bounds cumulative mapped segments before allocation, and does not transform
already plane-space provider coordinates again. Unresolved entities keep their
unavailable markers. No guest code runs during this composition method.

The returned `ProviderPlanDrawing` retains the original checked results alongside
the drawing. Call `drawing(editor, active_view)` for each current use; it rechecks
document/view and provider activation rather than caching bare copied lines after
discarding their authority. Do not retain an accessor's drawing reference as an
independent validity guarantee across subsequent host/document changes.

`os-ui/tests/provider_plans.rs` verifies mapped provider -> host clip -> semantic
snap/pick through an actual loaded service fixture, using a rotated plane with a
large origin to catch double transforms. It checks remaining unavailable IDs,
unchanged model/history, duplicate results, stale input/composition and unload/
reload revocation. This test fixture is not an independent architectural provider.

All commands below passed on the current untracked Windows working tree. The
smoke's new output passed bundled wall edits, level reassignment, regeneration,
undo/redo and save/reopen; provider-line behavior is verified by the dedicated
renderer tests, not by that smoke. Repeat the smoke with a new output filename.

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\tools\cargo.ps1 run -p os-app --locked --offline --target-dir work/completion-build '--' --smoke outputs/provider-lines-verified.osb
```

Next: integrate native provider scheduling/rendering; extend a separately built
Rust provider; verify its full native linked-plan workflow.
Existing SDK/example instructions and trusted-bundled/bounded-Wasm distinction
remain unchanged. No independent guest reinstall, native inspection or hosted CI
result is claimed. There is no Git commit/remote; license/publication and production
profile decisions remain open. D and E1-E4/G1-G6 remain incomplete, L1/L2 later.

Controller-composition follow-up: both new `provider_plans` tests and the full
all-feature workspace suite passed, as did strict Clippy, formatting and the
workspace build using the commands above. The fresh bundled smoke below also
passed its edit/regeneration/history/save/reopen checks; it is not native provider
integration evidence. No wire/file-format/dependency changes were made.

```powershell
.\tools\cargo.ps1 run -p os-app --locked --offline --target-dir work/completion-build '--' --smoke outputs/provider-plan-composition.osb
```
