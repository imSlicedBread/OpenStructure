# Architectural 2D coverage ledger

This is the required evidence ledger for BUILD_PROMPT and [the production specification](2d-production-spec.md). It is not a readiness claim. Requirement IDs below refine each specification capability into separately trackable items; no group is completed by one example. Reference workflow is the matching spec section (including its cited references); details and acceptance conditions there remain mandatory.

## Profile and current gate

2026-10-01 paired hinged doors: native straight-wall door types support a
single or paired layout with an active-leaf width stored proportionally to the
clear primary bay. Both leaves have independent static 0–90° poses. Plan and 3D
share resolved leaf geometry; windows, curved hosts, sculpted components, and
partial cuts remain outside the paired layout. Paired-door IFC import/export is
explicitly refused by the bounded IFC subset. Schema 58/API 39 and package v5;
container 2, generic API 2, and IFC dialect version are unchanged. This is a
bounded family increment, not Revit parity.

Evidence: model restrictions in `os-model/src/opening_family.rs`;
shared leaf geometry in `os-geometry/src/openings.rs`;
schema/package migration and reopen in `os-storage/tests/paired_doors.rs`;
real-egui leaf authoring in `opening_family_tests.rs`; explicit-reset and
one-step-history flows in `opening_type_assignment_tests.rs` and
`opening_type_package_tests.rs`; IFC fail-closed behavior in
`os-ifc/tests/exchange.rs`.

Validation passed: the full workspace test/doctest run and full `os-storage`
suite passed. The `os-ui` library suite reported 432 passed and 4 ignored; all
5 paired-specific UI tests passed, including plan/mesh coordinates, type edit,
assignment, package replacement, and both DPI profiles. Warning-denied
workspace Clippy, formatting, and `git diff --check` passed. Remaining paired-
specific coverage: copy/rehost, compound or joined-host cases, and downstream
tag/dimension/schedule/section/PDF consumers. Native visual/print QA is not
claimed.

Earlier 2026-10-01 per-instance opening-pose increment: selected eligible doors
expose a
0–90° leaf angle; rectangular Sliding windows expose a normalized sash fraction;
rectangular Casement windows expose a 0–90° opening angle. Default preserves
prior geometry (90° doors, closed windows). Straight-host eligibility gates the
feature; Fixed windows, curved hosts and unsupported component profiles retain
their previous geometry with feedback. Plan symbols and 3D use the same pose;
candidate preview does not mutate the live model, and Apply preflights and
commits one undoable transaction. UUID, host cut and wall quantities are
unchanged. Schema 57/API 38; container 2, package formats and generic API 2 are
unchanged.

Evidence: model validation and pose defaults in `os-model/src/openings.rs`,
strict schema migration/save-reopen in `os-storage/tests/opening_states.rs`,
state transforms and fallback coverage in `os-geometry/src/openings.rs`,
candidate preview/apply/history in `os-ui/src/opening_tools.rs`, and real egui
Cancel/Apply/Undo/Redo at 1280×800/100% and 1000×650/150% in
`os-ui/src/desktop_tests/opening_family_tests.rs`.
`opening_copy_preserves_instance_and_type_on_another_wall` covers copying
non-default door and Sliding poses.

This advances M01.18/M01.24/M02.01 while keeping readiness partial. Animated
motion, hardware, curved/profiled sash poses, broader opening families,
manufacturing semantics, IFC pose exchange and native visual/print QA remain
open. See [per-instance opening poses](native-hosted-openings.md#per-instance-opening-poses-schema-57).

Validation for this increment: the full workspace test and doctest run passed;
`os-ui` reported 427 passing library tests and 4 ignored. Warning-denied
workspace Clippy, `cargo fmt --all -- --check`, and `git diff --check` passed.

2026-10-01 static closed window sashes: the existing WindowOperation now drives
eligible straight-host rectangular 3D components. Fixed retains its prior mesh;
Sliding has two overlapping glazed sashes on separated depth tracks; Casement
has one closed perimeter-rail sash with inset glazing. The existing pane-depth
envelope and alignment are retained. Only the primary bay changes in side-lite
types. Materials, effective dimensions, instance pins, outer frame, wall cuts,
host quantities and plan symbols retain their existing semantics. Arc hosts and
nonrectangular authored profiles keep prior geometry with explicit plan-only
feedback in the editor. No persistence/protocol changes.

Evidence: `crates/os-geometry/src/openings.rs` closed_sash tests cover operation
distinction, positive solid volume, bounds/materials, rotations/reversals, all
pane positions, tiny legal dimensions, unchanged side lites and fallbacks.
`crates/os-ui/src/opening_profile_tests.rs` covers disposable preview,
all-instance Apply/regeneration, cancel/stale, one-step history, host quantities,
reopen and a model-valid geometry failure rejected before commit.
`crates/os-ui/src/desktop_tests/opening_family_tests.rs` drives the operation
selector with real egui frames at 1280×800/100% and 1000×650/150%, including
Casement cancel and Sliding apply/undo/redo with unchanged host geometry.
`crates/os-ui/tests/openings.rs` preserves existing plan-mark expectations.
This advances M01.18/M01.24/M01.30/M02.01 while leaving their partial status.
Dedicated package-transfer geometry checks, export/section consumer
qualification and native visual/print QA are not claimed.
Hardware, curved/profiled sash poses, manufacturing semantics,
sash quantities, general families and M03 formulas/reporting remain open. See
[static sashes](native-hosted-openings.md#static-closed-window-sashes-2026-10-01).

Focused validation for this sash increment passed:

- `cargo test -p os-geometry --all-features --locked --offline --quiet`: 73 tests.
- `cargo test -p os-ui --all-features --locked --offline --lib opening_ --quiet`:
  200 tests, including existing opening desktop coverage.
- `cargo test -p os-ui --all-features --locked --offline --test openings --quiet`:
  6 tests.
- `cargo test --workspace --all-features --locked --offline --quiet` passed;
  `os-ui` had 425 passing tests and 4 ignored. The later-added operation-
  selector desktop test passed separately at both stated DPI profiles.
- Warning-denied workspace Clippy passed.
- `cargo fmt --all -- --check` and `git -c core.safecrlf=false diff --check` passed.

Native shared opening Length parameters at that milestone: schema 56 / native API 37 / package
v4. This focused M03 increment provides project-owned named metre values bound
to multiple native door/window type widths, heights and window sills. Resolution
is instance pin → project parameter → type literal. The parameter manager stages
create/edit/rename/duplicate/delete, lists uses, previews effective changes and
commits once; type editing binds/unbinds and freezes effective values. Candidates
settle clearance locks and reject invalid placements atomically. Parameter-only
changes invalidate types, instances, hosts, drawings, tags, schedules and sheets.

Evidence: `os-document/tests/opening_lengths.rs` (all three dimensions, equal
pins/reset, locks, preview/history, invalid rollback, lifecycle, unplaced types),
`os-storage/tests/opening_lengths.rs` (save/reopen, frozen legacy chain, strict
fields and exact dependencies), `os-storage/tests/opening_type_package.rs`
(v1–v4 migration and strictness), and desktop `opening_length_tests.rs`,
`opening_type_package_tests.rs`, `opening_tag_tests.rs` (both DPI profiles,
preview/apply/cancel/stale/undo, copied package identities, scene/tag/sheet output).
Tests stage some draft fields directly while exercising real egui action buttons;
they do not establish native visual or print QA. Import always copies parameter
dependencies; explicit dependency reuse UI is deferred. Formulas, reporting
parameters, other categories and arbitrary instance bindings remain open. This
does not close the full M03 parameter-system requirement.

Final validation for this increment (2026-10-01):

- `cargo test --workspace --all-features --locked --offline --quiet` passed.
- `cargo test -p os-ui --all-features --locked --offline --quiet` passed separately;
  the UI library had 423 passing tests and 4 ignored, with integration tests passing.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  passed; `cargo fmt --all -- --check` and `git diff --check` passed.
- Initial layout regressions were fixed by keeping the Properties layout and
  profile editor geometry stable: parameter management is in Manage, bindings
  are on the type editor's `Lengths…` panel. All previously failing tests pass.
- The floor-pan test passed eight isolated repeats but raced async drawing
  completion during a full run. It now uses its existing 10-second-bounded
  readiness condition after the concurrent edit; the pan assertion is unchanged.
- Dedicated shared-parameter schedule/CSV assertions and native visual/print QA
  remain unverified; existing schedule/CSV suites passed in the workspace run.

Native opening host-end clearance locks: schema 55 / native API 36. M01.10,
M01.18 and M01.30 gain one persistent Start/End centerline distance per native
door/window on straight or circular-arc hosts. Document candidates settle
dependent offsets before validation/history; conflicting moves, overlap and
fit errors reject atomically. Properties and explicit endpoint spacing controls
edit/unlock the constraint. Neighbor-jamb spacing is never implicitly locked;
rehost/split require unlock. Focused evidence is in model/document/storage
`tests/opening_clearances.rs` and UI `opening_spacing_tests.rs` (test prefix
`clearance_lock`). Basic plan/3D/schedule propagation is covered; wall accent
previews do not display dependent opening motion, and native visual/print QA
remains open. No M02/M03 shared-parameter or general solver claim is added.
See [host-end clearance locks](native-hosted-openings.md#persistent-host-end-clearance-locks-schema-55).

2026-09-28 native circular wall paths (M01.03): schema 54 / native API 35.
Three-click start/bulge/end authoring, analytic stations/projection/snaps,
bounded annular 3D geometry, plan/section/sheet/PDF projection, Properties,
undo/redo, and schema-53 migration/save-reopen have model, geometry, storage,
and egui evidence in the files listed by M01.03. Display tessellation is derived
and capped at 4096 segments. Curved-wall joins, rooms, openings, endpoint
transforms, outer-face dimensions, installed generic API-2 authoring, and IFC
export remain unsupported and fail closed or stay outside room boundary inputs.

2026-09-28 associative outer wall faces (A01.01/A01.10/A01.14): schema 53 / native
API 34 adds wall UUID + Left/Right side + metric station anchors for Aligned,
Chain and Baseline. Current effective compound thickness drives model and plan
snapshot resolution. Endpoint/jamb picks keep priority; faces require visible
clipped wall bodies and respect foreground geometry, opening voids and phases.
Properties repair, immutable previews and one-step history are covered at both
desktop DPI profiles in `crates/os-ui/src/plan_workspace/face_tests.rs` (included
by `endpoint_tests.rs`). Model evidence: `crates/os-model/tests/dimension_faces.rs`;
frozen schema-52 migration and save/reopen: `crates/os-storage/tests/dimension_faces.rs`.
The focused endpoint suite passes 64 tests with 3 installed-guest cases ignored.
Angular remains endpoint-only. Curved walls, material-layer faces, split-child
face remapping, native-window visual QA and dedicated face sheet/PDF output
comparisons remain open. Generic API 2 and container 2 are unchanged.

2026-09-28 draw opening width: Door/Window placement has an opt-in **Draw opening
width** control. A press/drag/release on one visible native straight wall previews
the measured opening symbol and aperture, then creates one instance using the
existing atomic creation path. Typed widths are instance overrides. Preview
leaves the model/history/cached plan/scene unchanged; commit regenerates plan and
3D apertures. Invalid or stale gestures retain press ownership and create nothing.
Evidence: the three `draw_opening_width_*` tests in
`crates/os-ui/src/plan_workspace/opening_tests.rs`, at both desktop DPI profiles,
cover both kinds, horizontal/rotated and reversed axes, both drag directions,
painted preview apertures, atomic default types, typed overrides, undo/redo,
split-view regeneration, invalid release, visibility/crop, snapping, stale
contexts, and pan precedence. Existing click placement and jamb resize tests
remain the regression gate. Native-window visual/print qualification and
multi-wall/curved/plugin-hosted opening gestures remain open. No schema, storage,
protocol, or 3D operation changes. See
[native hosted openings](native-hosted-openings.md#draw-opening-width-in-plan).

Validation: `cargo test -p os-ui --all-features --lib plan_workspace::opening_tests`
passed 55 tests (0 failures). `rustfmt --edition 2024 --config skip_children=true
--check` passed for the three touched Rust files; scoped `git diff --check`
passed for those files and the two documentation files. No workspace-wide gate
or native-window visual acceptance is claimed for this increment.

2026-09-28 window operation symbols: schema 52/native API 33 adds persisted
type-level Fixed/Sliding/Casement metadata. The plan retains the prior Fixed
symbol and draws distinct Sliding/Casement marks; migration and `.osot` v1/v2
default old types to Fixed. Model/package/migration and rotated-host plan tests
originally asserted unchanged 3D meshes. The 2026-10-01 increment above adds static
closed 3D assemblies for eligible windows; actual open-state behavior and
native-window visual/print qualification remain open. See
[native hosted openings](native-hosted-openings.md#window-operation-plan-symbols-schema-52).

2026-09-27 instance window panes: schema 46/native API 27 adds explicit inheritance
or pinned Center/LeftFace/RightFace in Properties/Exact Edit, a selected off-center
window Side control, and atomic native wall mirror companion edits. Shared types
and unrelated windows remain unchanged. Evidence: `window_pane_tests.rs`,
`opening_sill_tests.rs`, `endpoint_tests.rs` and storage `window_panes.rs`.
Real egui tests cover both desktop DPI profiles; native-window visual QA is separate.

2026-09-27 per-plan phase display: schema 45/native API 26 persists target phase
and filter in strict version-3 plan settings. Native geometry, cutouts, snaps,
picks and referenced annotations respect the phase filter; shared line appearances
reach canvas, sheets and vector PDF. Real egui coverage runs at both desktop DPI
profiles. Generic API 2/container 2 remain unchanged. Room topology, schedules,
3D phase display and linked phase mapping remain deferred; see
[native phase evidence](native-phases.md#per-plan-phase-display-schema-45).

2026-09-27 native phase/lifecycle foundation: schema 44/API 25 adds stable
ordered Existing/New Construction phases, persisted creation/demolition
lifecycle for native architectural elements, derived phase statuses, atomic
commands/history, deterministic strict schema-43 migration and explicit IFC loss reporting.
Model, document and migration tests pass. Phase management and per-plan display
are implemented in subsequent slices. Phase-dependent room topology, schedules,
linked phase mapping and design options remain unimplemented; see [native phases](native-phases.md).

2026-09-27 associative opening-jamb dimensions: schema 43/native API 24 adds
live `OpeningJamb` references alongside wall endpoints. Typed/legacy width,
override, host reversal/rehost, live plan labels, real-egui door/window creation,
history, strict schema-42 migration and `.osb` roundtrip have focused evidence.
Face/material references, native-window/print acceptance and broader dimension
styles remain open. See [native dimensions](native-dimensions.md).

2026-09-27 persisted opening schedule filters: schema 42/native API 23 adds
bounded typed AND rules to saved Door/Window/All definitions. Live unrounded
text/dimension values drive the schedule window, sheet preview and searchable
vector PDF. Frozen schema-41 migration, strict validation/serde, draft controls,
source edits/history and persistence have focused coverage. RoomFinish filters,
groups, totals and styles remain unsupported; S01 stays partial. See
[native hosted openings](native-hosted-openings.md#persisted-opening-schedule-filters-schema-42).

2026-09-27 opening-tag label preset increment: schema 41/native API 22 adds
four closed live-data label choices without arbitrary templates. Schema 40 tags
migrate atomically to `Full`; Properties editing, move preservation, undo/redo,
plan/sheet/vector-PDF output and a two-DPI desktop path are covered. Automated
focused evidence is recorded in [native opening tags](native-opening-tags.md).
Native visual and physical-print qualification remain open.

2026-09-27 live room-bound ceiling association: schema 40/native API 21 adds an
optional source-room identity while retaining the saved boundary for recovery.
Room topology updates propagate to reflected plans, sections and 3D; unresolved
associations are omitted safely, edited openings validate against the live
boundary, and detach freezes the current outline when resolvable. The full
workspace suite, warning-denied Clippy and formatting checks pass. See
[native ceilings](native-ceilings.md).

2026-09-26 room material assignment increment: schema 35/native API 16 adds
independent floor/wall/ceiling material UUIDs alongside finish codes. Focused
checks cover atomic validation/removal/history, frozen schema-34 migration and
save/reopen, local property drafts/cancel/stale guards, live composite schedule
and sheet/PDF text, and subdued floor RGB fills with crop/selection/unresolved
checks at 1280×800/100% and 1000×650/150%. See [native rooms](native-rooms.md).
This increment adds no quantities, finish-face/net areas, wall/ceiling takeoffs,
legends, IFC associations, room solids or floor construction. Native-window and
physical-print qualification remain open. Full workspace checks are left to the
coordinator; earlier test totals below describe historical checkpoints.

2026-09-26 shared material color increment: schema 34/native API 15 adds editable
RGB for shared project materials. S01.02 and M01.10 record automated migration,
native render/family preview, geometry/quantity stability and history coverage;
see [shared material colors](native-material-colors.md). Earlier test counts below
describe their named historical checkpoints. No native-window or production
visual qualification is claimed for this increment.

2026-09-26 Room Finish acceptance: model schema 33/native API 14 adds validated
optional room finish codes and saved RoomFinish schedules with live topology
area, blank unresolved area/status, room selection, and the existing sheet/PDF
table route. P01.06, P01.07, S01.02 and S02.14 record the bounded scope below.
Focused Room Finish UI tests pass at 1280×800/100% and 1000×650/150%; model,
document and frozen migration/save-reopen tests are recorded in
[native rooms](native-rooms.md). Workspace all-feature tests: 597 passed,
0 failed, 3 ignored. Warnings-denied workspace Clippy passes. No native-window
or physical-print qualification is claimed.

2026-09-26 native door/window material acceptance: schema 32/API 13 adds
separate project-material assignment for panel/pane and frame geometry, with
explicit schema and family-format migration. Schema 31/API 12 introduced the
associative opening tags documented in [native-opening-tags.md](native-opening-tags.md);
material-assignment scope and current validation evidence are in
[native-opening-materials.md](native-opening-materials.md).
The full all-features workspace test suite and warnings-denied workspace Clippy
pass at this checkpoint; the material page has desktop input coverage at 100%
and 150% display scaling.
Native visual qualification, physical print acceptance and independent hosted
IFC viewer acceptance remain open; this does not change the broader production
limitations below.

D is now in progress after the [bounded B/C baseline audit](bc-baseline-audit.md).
The [callable plan-graphics foundation](callable-plan-graphics.md) adds optional
service-1 DTOs, a scoped validated host call and bounded Wasm worker dispatch.
This is not complete D provider acceptance or a production coverage claim.
The [provider-line render foundation](provider-line-rendering.md) now supplies
host clipping, semantic snap endpoints and matching line picking. Controller
composition now maps checked service results while retaining provider validity;
full independent pointer acceptance remains open.
The [independent Rust column outline](independent-column-plan.md) now passes a
prebuilt-host installation probe through the Wasm plan worker and controller.
Full independent pointer acceptance remains open;
outline-only service output is not qualified architectural cut graphics.
The [bounded provider scheduler](plan-provider-scheduler.md) now coordinates
explicit routes, cancellation, stale batches and per-target failure diagnostics
through the shared Wasm pool. [Native column plan integration](native-column-plan.md)
now wires this into background drawing, Fit plan, picking and shared selection.
Native form edits, undo/redo and save/reopen are verified for the installed column;
independent pointer authoring and the full D failure workflow remain open.
The [installed Wall gesture controller](installed-wall-gesture-adapter.md) now
freezes exact snapped creation drafts into scoped Wasm commands, verified against
the existing guest with cancellation and inactive-view rejection. Native pending
intent wiring now has [installed headless UI evidence](installed-wall-pointer-ui.md).
The [installed edit adapter](installed-wall-edits.md) now passes move, both endpoint
resizes and offset-copy with scoped worker commands and real-guest headless UI
tests. Native visual acceptance and broader D workflow gaps remain required.
Provider composition now has a transferable checked snapshot for off-thread
derivation, verified against edits/reload during work and the installed guest.
The [plan geometry foundation](plan-geometry-foundation.md) adds actual checked
horizontal cuts/projections, cropped semantic polygons, 2D coordinate/navigation
math and revision-bound native-wall controller derivation. [Named plan settings](persisted-plan-settings.md)
were introduced in model schema 4 and migrated through current model schema 43
with transactional commands and frozen migration
tests. The [native plan workspace](native-plan-workspace.md) adds actual plan
creation/selection, equal split, background derivation, navigation and shared
wall selection with native edit/undo/save/reopen evidence. [Plan-settings forms](plan-settings-form.md)
now edit the persisted settings with atomic Apply/Cancel and stale-draft rejection.
[Semantic snapping](semantic-plan-snapping.md) now supplies checked native-wall
endpoint/midpoint/nearest queries alongside background drawings. The initial
[two-point wall tool](plan-wall-gestures.md) adds snap feedback and exact drafts
through the bundled provider. The [native pointer checkpoint](native-pointer-walls.md)
verifies two snapped exact walls, cancellation, split display and save/reopen.
[Architectural grid persistence](architectural-grids.md) adds building-scoped
straight datums, validated commands/history and explicit migration 3→4.
[Grid plan integration](grid-plan-integration.md) adds clipped lines, shared grid
selection and grid-axis snapping with headless pointer-workflow tests. [Grid forms](grid-authoring-forms.md)
now support numeric create/edit, preview, cancellation and undo, including a
connected headless New grid → snapped wall test. [Native grid inspection](native-grid-authoring.md)
now verifies create/edit/undo/redo, invalid/cancel, snapped wall and save/reopen.
[Wall move/resize click tools](plan-wall-edit-gestures.md) now preserve identity,
support exact drafts and pass controller and real egui input tests. [Native edit
inspection](native-wall-edits.md) verifies split updates and save/reopen, while
exposing an untitled quick-Save path defect, now [corrected with destination
prompting](untitled-save-destination.md) and regression tests. Selected visible
native-wall endpoint handles now have [headless desktop evidence](plan-wall-edit-gestures.md#endpoint-handle-slice--2026-09-20-automated-evidence)
at 1280×800/1.0 and 1000×650/1.5, including rendering/hits, preview/commit/history,
cancellation/pan precedence and an explicitly run installed Wall worker test.
Native manual handle inspection, broader gesture acceptance and broader
installed-provider acceptance remain required; D is incomplete.

[Native hosted openings](native-hosted-openings.md) add straight-wall doors and
windows with checked host-local dimensions, one-step commands, schema 4→5
migration, actual segmented wall apertures shared by 3D and plan, fixture picking,
and exact-input create/edit/delete forms. Schema 10 adds per-door Start/End hinge
and Left/Right swing, matching 90° leaves and pickable 16-segment quarter-circle
arcs. Start/Left preserves existing geometry during schema-9 migration and remains
the new-door default. Headless tests cover both wall directions, all combinations,
crop/picking, preview/apply/cancel/stale drafts and undo/redo. Selected visible
doors also have separate Hinge and Swing plan buttons; typed windows have
type-eligible Side and Flip lite actions on straight and circular-arc hosts.
One-shot opening-center alignment projects between parallel straight hosts and
directionally onto finite circular-arc hosts; E01.16 records its disposable
preview, validation, cancellation and history.
M01.22 records direct flip acceptance, including grip/tool precedence,
cancellation, field preservation and plan/3D history at both DPI profiles.
These add no schema/protocol or native visual/physical-print qualification.
Family libraries, curved/plugin hosts,
and native visual inspection remain open; this does not close D.

[Synthetic snap-stage timings](plan-snap-stage-timings.md) now separate query
cost from source/scene setup and compare dense intersection overrides/overload.
Prepared immutable segment directions reduce repeated intersection-query work,
with large-coordinate/order/revision regression coverage and before/after timings.
These local microbenchmarks do not qualify event-to-paint latency, regeneration,
installed providers or production performance budgets.

[Exact-input precedence correction](exact-input-snap-precedence.md) verifies that
dense intersection acquisition cannot block a fully specified offset, while
partial pointer-dependent drafts retain their resource checks.

[Native offset/save inspection](native-offset-save.md) verifies exact parallel
copy preview/commit in split view, snap-menu visibility, corrected untitled Save
prompting and save/reopen with independently inspected wall coordinates/IDs.

[Finite-line intersection snapping](plan-intersection-snapping.md) now retains
both semantic references and is used by bundled wall gestures, with bounded
local acquisition and desktop input coverage. Curves and plugin contributions
remain outside this implemented subset.
[Plan snap controls](plan-snap-controls.md) expose session-only enable switches
for the implemented kinds without model/history edits; priority customization
and independent-provider aggregation remain open.

[Desktop plugin form view provenance](plugin-form-view-provenance.md) now binds
existing command workers to active plan/settings identity and revokes drafts on
view changes. The host also [validates the persisted settings revision](worker-persisted-view-validation.md)
at dispatch, polling and checked geometry consumption instead of accepting a
caller-only revision echo. Independent pointer/preview/graphics/snapping contracts remain
unimplemented; command-worker guards alone do not satisfy D.

The transaction foundation now has [bounded snapshot retention](history-retention.md):
128 entries / 256 MiB estimated owned data by default, whole-entry eviction,
atomic oversized-entry rejection, and desktop notice/dirty-state regression tests.
The measured synthetic cost motivated retention, not a delta rewrite. Actual
peak-process memory and production history/performance budgets remain unqualified.

Native independent Wall follow-up now verifies registered create/edit, two-parameter
undo/redo, save/open and shared viewport/browser selection on Windows. See
[Wall workflow evidence and explicit omissions](native-wall-workflow.md). This
adds concrete B/C evidence without declaring the joint gate complete.

Latest C follow-up: a separate candidate migration service now passes the installed
33-column probe, including cancellation after partial progress, rejection in the
final batch, stale-document rejection and a single live undo/redo transaction.
Four-element worker batches retain existing fuel/message limits. Editor/forms now
use staging for single-type owners, with native Apply/Undo/Redo/Save evidence.
Multi-type desktop review is implemented with a two-type controller test and
single-type native evidence. Independent mixed-type execution now passes distinct
transforms, final second-type failure isolation, geometry, history and storage;
native mixed-type review/Apply/Undo/Redo/Save/Open now has Windows evidence;
large-project performance and broader B/C completion remain pending;
this does not close B/C. See [migration follow-up](plugin-migration-service.md).
The installed migration probe also verifies same-version provider replacement,
reopened-document identity, overall deadline expiry and Editor view-revision
changes after partial progress, with no live/history mutation on rejection.
The all-type review checkpoint passed 151 tests; snapshot retention passed 160;
the plan-geometry checkpoint passed 172. The persisted-settings follow-up passes
the full workspace suite. Older counts are historical checkpoints, not broader
acceptance claims. Installed Wall/column Editor and mixed-migration probes also
pass with bounded history. See [retention evidence](history-retention.md).

Owner response on 2026-09-11: not sure of jurisdiction, project types/size, concurrent editors, OS, CAD/PDF versions or plot sizes. All remain open qualification decisions. Planning only: metric/imperial architecture, new construction and renovation, offline local use, eventual multi-user office use. Observed platform: Windows x64; Linux CI configured, not observed; macOS unverified. No production deployment profile is approved.

The [source-backed B/C audit](bc-baseline-audit.md) verifies the bounded development gate and advances implementation to D. This is not production plugin qualification. D and E1–E4 remain incomplete: no complete linked floor-plan workflow, issued reference project, physical plot or production qualification has passed. The working tree has no commit or remote; configured CI is not a hosted result. Older per-slice test counts are historical evidence only.

Allowed statuses: not started, partial, automated-tested, workflow-validated, owner-approved exclusion. No exclusions have been approved. A dash means missing evidence, never assumed success. Rows can be refined further without dropping parent requirements.

## Capability rows

Each row inherits its dependency from the milestone column: D requires B/C, E1 requires D, E2 requires E1, E3 requires E2 plus translator/team ADRs. All require the architecture commitments below. `Not implemented` means no supported public workflow or acceptance evidence; it is not a finding about every internal helper.

| ID | Requirement / reference workflow | Milestone | Status | Current behavior and limits | Source/test/review evidence |
| --- | --- | --- | --- | --- | --- |
| V01.01 | Named floor plans | D/E1 | partial | Desktop create/picker/browser, settings forms and persistence verified; complete plan authoring remains | native-plan-workspace.md; plan-settings-form.md; os-storage/tests/plan_settings.rs |
| V01.02 | Roof plans | D/E1 | partial | Native single-plane sloped-roof geometry, openings, plan-range/crop projection and section/sheet/vector-PDF integration; no dedicated roof-plan view, multi-slope/hip/valley system, roof-specific tags or visual/physical-print qualification | native-roofs.md; crates/os-geometry/src/roofs.rs; crates/os-ui/src/plan.rs; crates/os-render/tests/plan.rs; crates/os-ui/src/plan_workspace/roof_tests.rs |
| V01.03 | Reflected ceiling projection | D/E1 | partial | Saved upward-looking reflected plan type, persisted range/visibility, crop-aware native ceiling projection including live room-bound sources and sheet/vector-PDF rendering; preview/apply and source-boundary update acceptance at 1280×800/100% and 1000×650/150%; no grids, lights/MEP coordination, native-window/print qualification, or full visual acceptance | native-ceilings.md; crates/os-ui/src/plan.rs; crates/os-ui/src/plan_workspace/ceiling_tests.rs; crates/os-render/src/sheet.rs; crates/os-storage/src/tests/ceilings.rs |
| V01.04 | Site plans | D/E1 | not started | Not implemented | — |
| V01.05 | Area plans | D/E1 | not started | Not implemented | — |
| V01.06 | Architectural coordination plans | D/E1 | not started | Not implemented | — |
| V01.07 | Interior elevations | D/E1 | not started | Not implemented | — |
| V01.08 | Exterior elevations | D/E1 | not started | Not implemented | — |
| V01.09 | Building sections | D/E1 | partial | Versioned section plane/extents and migration; two-click snap-aware plan marker, linked native wall/door/window/floor contours, section canvas, source selection, standalone or paired Plan/Section A3 sheet and vector PDF verified; cut fills/poché, depth, and settings editor remain | native-sections.md; native-sheet-preview.md; os-ui/src/plan_workspace/section_tests.rs; os-ui/src/plan_workspace/sheet_tests/combined.rs; os-ui/tests/sections.rs; os-geometry/src/section.rs; os-storage/tests/sections.rs |
| V01.10 | Wall sections | D/E1 | not started | Not implemented | — |
| V01.11 | Enlarged plans | D/E1 | not started | Not implemented | — |
| V01.12 | Detail views | D/E1 | not started | Not implemented | — |
| V01.13 | Independent drafting views | D/E1 | not started | Not implemented | — |
| V01.14 | Cut/top/bottom/depth ranges | D/E1 | partial | Persisted range with numeric draft UI, checked native prism policy and level conversion; other categories remain | plan-settings-form.md; os-geometry/tests/plan.rs; os-ui/tests/plan.rs |
| V01.15 | Local plan regions | D/E1 | not started | Not implemented | — |
| V01.16 | Upward/downward underlays | D/E1 | not started | Not implemented | — |
| V01.17 | Far clipping | D/E1 | not started | Not implemented | — |
| V01.18 | Model crop boundaries | D/E1 | partial | Numeric and graphical rectangular crop editing; eight edge/corner handles, boundary-only preview, single view update, clipping/picking regeneration, undo/redo and save/reopen tested. Non-rectangular crops and native-window/print qualification remain | plan-settings-form.md; os-ui/src/plan_workspace/crop_tests.rs; os-geometry/tests/plan.rs; os-render/tests/plan.rs |
| V01.19 | Annotation crop boundaries | D/E1 | not started | Not implemented | — |
| V01.20 | Rotated crops | D/E1 | partial | Graphical crop handles operate in rotated view-plane coordinates; all eight handles and hit-radius behavior tested at 1280×800/1.0 and 1000×650/1.5. General orientations and native-window/print qualification remain | plan-settings-form.md; os-ui/src/plan_workspace/crop_tests.rs; os-geometry/tests/plan.rs; os-ui/tests/plan.rs |
| V01.21 | Scope boxes | D/E1 | not started | Not implemented | — |
| V01.22 | Project/true north | D/E1 | not started | Not implemented | — |
| V01.23 | Explicit view orientation | D/E1 | partial | Horizontal XY origin/yaw settings UI; general view orientations remain | plan-settings-form.md; os-ui/src/plan_settings.rs |
| V01.24 | Independent duplication | D/E1 | not started | Not implemented | — |
| V01.25 | Duplication with detailing | D/E1 | not started | Not implemented | — |
| V01.26 | Dependent view overrides | D/E1 | not started | Not implemented | — |
| V01.27 | Matchlines | D/E1 | not started | Not implemented | — |
| V01.28 | Callouts and reference-only callouts | D/E1 | not started | Not implemented | — |
| V01.29 | Placed-detail references | D/E1 | not started | Not implemented | — |
| V01.30 | Search/filter project browser | D/E1 | not started | Not implemented | — |
| V01.31 | View naming and organization | D/E1 | partial | Generated plan names, transactional rename and browser selection; organization beyond flat list remains | plan-settings-form.md; native-plan-workspace.md |
| V01.32 | View tabs | D/E1 | not started | Not implemented | — |
| V01.33 | Split linked 2D/3D | D/E1 | partial | Equal-width native plan/3D panes, linked numeric wall edit/undo; exact pointer authoring remains | native-plan-workspace.md; os-ui desktop_tests::named_plan_split_selects_the_same_wall_and_preserves_navigation_history |
| V01.34 | Saved navigation | D/E1 | not started | Not implemented | — |
| V01.35 | Shared cross-view selection | D/E1 | partial | Native walls/openings/rooms share plan/browser selection; room graphics are plan-only, extension plan providers and broader categories remain | native-plan-workspace.md; native-rooms.md; os-ui desktop input tests |
| V01.36 | Navigation from markers/annotations/schedules/sheets | D/E1 | partial | Linked Plan/Section source actions and sheet-picker return tested; broader annotation/schedule/reference navigation remains | native-sheet-preview.md; os-ui/src/plan_workspace/sheet_tests/combined.rs |
| V02.01 | Category/subcategory styles | E1 | not started | Not implemented | — |
| V02.02 | Per-element overrides | E1 | not started | Not implemented | — |
| V02.03 | Rule-based overrides | E1 | not started | Not implemented | — |
| V02.04 | Material cut/surface patterns | E1 | not started | Not implemented | — |
| V02.05 | Scale-aware line weights | E1 | not started | Not implemented | — |
| V02.06 | Line styles and dash patterns | E1 | not started | Not implemented | — |
| V02.07 | Hidden/overhead lines | E1 | not started | Not implemented | — |
| V02.08 | Join graphics | E1 | not started | Not implemented | — |
| V02.09 | Halftone | E1 | not started | Not implemented | — |
| V02.10 | Transparency | E1 | not started | Not implemented | — |
| V02.11 | Color/monochrome output | E1 | not started | Not implemented | — |
| V02.12 | Coarse/medium/fine representation | E1 | not started | Not implemented | — |
| V02.13 | Model-scale hatch patterns | E1 | not started | Not implemented | — |
| V02.14 | Paper-scale hatch patterns | E1 | not started | Not implemented | — |
| V02.15 | Pattern origin/rotation/spacing | E1 | not started | Not implemented | — |
| V02.16 | Compound-layer cuts | E1 | not started | Not implemented | — |
| V02.17 | Masking and draw order | E1 | not started | Not implemented | — |
| V02.18 | Linked view templates | E1 | partial | Persisted floor-plan graphics templates for walls, doors, windows, and slabs; create/edit/link/unlink, atomic Apply, deletion guard, linked-view invalidation, schema-27→28 migration, and resolved cut/projected color/weight/dash strokes in native plan canvas, sheet preview, and vector PDF. Apply-once, batch assignment, transfer standards, element/filter overrides, section graphics, and native/physical-print qualification remain | native-plan-graphics-templates.md; crates/os-model/src/plan_graphics.rs; crates/os-ui/src/plan_settings/graphics.rs; crates/os-storage/tests/plan_graphics.rs; crates/os-render/tests/plan_graphics.rs; crates/os-ui/src/plan_workspace/graphics_tests.rs; crates/os-ui/src/plan_settings.rs |
| V02.19 | Apply-once templates | E1 | not started | Not implemented | — |
| V02.20 | Batch template assignment | E1 | not started | Not implemented | — |
| V02.21 | Transfer of standards | E1 | not started | Not implemented | — |
| V02.22 | Override precedence | E1 | partial | Linked template styles take precedence over retained local view styles, that resolved result reaches native plan and sheet/PDF strokes, and unlink restores local styles. Per-element, filter/rule, and broader view override precedence remain unimplemented | native-plan-graphics-templates.md; crates/os-model/src/plan_graphics.rs; crates/os-ui/src/plan_settings.rs; crates/os-render/tests/plan_graphics.rs |
| V02.23 | Temporary hide/isolate | E1 | not started | Not implemented | — |
| V02.24 | Persistent hide | E1 | not started | Not implemented | — |
| V02.25 | Reveal-hidden diagnostics | E1 | not started | Not implemented | — |
| V02.26 | Invisibility explanation | E1 | not started | Not implemented | — |
| V02.27 | Issue-safe temporary states | E1 | not started | Not implemented | — |
| M01.01 | Straight rectangular walls | E1 | automated-tested | Numeric straight-wall authoring, rectangular prism only; no plan graphics | os-model WallParams; plugins/walls; os-ui/tests/vertical_slice.rs; baseline tests |
| M01.02 | Layered walls | E1 | partial | Straight native walls support reusable ordered compound types, stable layer IDs, per-wall assignment/flip, layer-aware geometry, openings, plan/section identity, quantities, history, and schema-21→22 migration. Circular paths derive bounded annular layer geometry with analytic quantities and per-layer hosted openings; curved joins and production visual qualification remain open | native-wall-types.md; crates/os-ui/tests/wall_types.rs; crates/os-ui/src/desktop_tests/wall_type_tests.rs; crates/os-storage/tests/wall_types.rs; crates/os-geometry/tests/wall_arcs.rs |
| M01.03 | Curved walls | E1 | partial | Analytic circular-arc paths with three-click authoring, exact station/tangent/projection, bounded annular mesh/plan footprints, analytic per-layer quantities, and radial hosted door/window apertures/components; schema-53→54 migration, save/reopen, plan snapping/picking, section/sheet/PDF and two-DPI egui preview/commit/cancel/history/arc-opening placement are tested. Curved joins, room topology, straight-only transforms and wall-face/angular dimensions, installed generic API-2 authoring, IFC/rectangular-Solid export and visual/production qualification remain unsupported | native-wall-types.md; native-hosted-openings.md; crates/os-model/tests/wall_arcs.rs; crates/os-geometry/tests/wall_arcs.rs; crates/os-storage/tests/wall_arcs.rs; crates/os-ui/src/plan_workspace/arc_tests.rs; crates/os-ui/src/plan_workspace/arc_opening_tests.rs; crates/os-ui/tests/wall_arcs.rs |
| M01.04 | Wall joins | E1 | partial | Explicit compatible-profile Butt/Corner/Tee joins for straight walls, including compound types; schema-20→21 migration, graph validation, shared mesh/plan/section derivation, openings/quantities, persistence, history and provider/API compatibility evidence. Native diamond-grip drag freezes the reachable graph and solves endpoint displacements atomically with deterministic minimum motion. Two-DPI headless egui evidence covers isolated regressions, Butt chains with resized-start opening compensation and hosted Tee stations, Corner/Tee propagation, a four-Corner loop, a Tee path reconnecting to its host, reversed directions/owners, exact contacts, preview/identity/history, invalid loop rejection, cancellation, visibility and pan precedence. Snap acquisition excludes all graph walls and filters legal axis candidates before ranking; checked identity and inconsistent-equation tests pass. Dense interactive solving is capped at 64 walls per connected component; performance at the cap and native visual qualification remain unqualified. Missing-provider guard is covered; installed-provider no-job test remains opt-in and unrun without its guest fixture. Installed-provider junction edits, arbitrary-angle/layer termination/curved junctions and joined IFC remain open | native-wall-joins.md; native-wall-types.md; crates/os-ui/src/plan_workspace/junction_tests.rs; crates/os-ui/tests/wall_joins.rs; crates/os-ui/tests/perpendicular_joins.rs; crates/os-ui/tests/wall_types.rs; crates/os-ui/src/desktop_tests/wall_join_tests.rs; crates/os-storage/tests/wall_joins.rs; crates/os-plugin-host/tests/wasm_transport.rs; plugins/walls/src/lib.rs |
| M01.05 | Curtain systems | E1 | partial | Persisted straight level-owned assembly/grid/panel/mullion topology with stable component UUIDs, atomic identity/customization validation, schema-58→59 migration and populated save/reopen. Separate material-aware rectangular panel/mullion solids generate the selected assembly's 3D scene on open/edit/type/material/level changes and history. Native building sections cut components independently and preserve material boundaries. Native two-click snapped plan placement uses preview-only draft state, preflights geometry, creates missing default types and the assembly in one transaction, selects the assembly, and derives crop/range-aware component outlines with native plan picking. Selected-assembly properties, vertical/horizontal numeric grid editing, panel/mullion type creation and per-component/default assignment, and explicit reset for customized removed bays are staged and preflighted before one undoable commit. Two-DPI egui tests cover staged non-mutation, grid identity, invalid/stale cancellation, save/reopen, undo/redo, plan selection, and split-scene regeneration. Direct grid/component grips, box selection, quantities UI, sheet/PDF qualification, operable panels, wall cuts, joins/curves, and IFC remain unsupported | native-curtain-systems.md; crates/os-model/tests/curtain_systems.rs; crates/os-geometry/tests/curtain_systems.rs; crates/os-document/tests/curtain_systems.rs; crates/os-storage/tests/curtain_systems.rs; crates/os-ui/src/plan/curtain_section_tests.rs; crates/os-ui/src/plan_workspace/curtain_tests.rs; crates/os-ui/tests/curtain_systems.rs |
| M01.06 | Floors/slabs | E1 | partial | Native horizontal concave slabs with metre thickness/offset, preview-only boundary and inner-opening loop sketch/vertex edits, and a staged name/level/material/thickness/top-offset editor with crop-aware availability, cached draft validation, isolated scene preflight on Apply, one-step UpdateFloor, identity/boundary/hole preservation, save/reopen and both-DPI undo/redo/cancel/stale/invalid/popup-dismiss coverage. Net area, hole-aware plan fill/pick, closed regenerated 3D mesh, sections, sheet/vector-PDF output and schema-35→36 migration are covered; slopes, assemblies, joins, parametric constraints, floor-specific visibility controls, and IFC slab exchange remain | native-floor-slabs.md; crates/os-ui/src/plan_workspace/floor_properties_tests.rs; crates/os-ui/src/plan_workspace/floor_tests.rs; crates/os-document/src/tests/floors.rs; crates/os-storage/src/tests/floors.rs; crates/os-render/tests/plan.rs; crates/os-geometry/src/floor_holes.rs |
| M01.07 | Roofs | E1 | partial | Native editable single-plane roofs with sketched outer/opening loops, snap-aware preview, numeric slope/dimension properties, atomic edit/history, closed sloped mesh, plan/section/sheet/PDF derivation, schema-37→38 migration and fail-closed IFC; no hips/valleys/multiple planes, layers/joins/hosted skylights, roof schedules/quantities, dedicated roof-plan view, or production visual/print qualification | native-roofs.md; crates/os-model/src/roofs.rs; crates/os-geometry/src/roofs.rs; crates/os-document/src/lib.rs; crates/os-ui/src/plan_workspace/roofs.rs; crates/os-ui/src/plan_workspace/roof_tests.rs; crates/os-storage/src/tests/roofs.rs; crates/os-ifc/tests/exchange.rs |
| M01.08 | Ceilings | E1 | partial | Level-relative horizontal ceiling shells with openings/material, transient snap-aware plan sketch and numeric edit, one-step history, scene/section/RCP/sheet derivation, schema-38→39 ceiling addition and schema-39→40 room-source migrations. Room-created ceilings follow the accepted same-level room topology in plan, section and 3D; unresolved sources are omitted without blocking room/wall edits and detach freezes the current room outline when resolvable, otherwise the retained outline. Edited openings validate against the live boundary. Preview/apply acceptance covers 1280×800/100% and 1000×650/150%. Slopes/assemblies, grids/fixtures, quantities/schedules, IFC and production visual/print qualification remain open | native-ceilings.md; crates/os-model/src/ceilings.rs; crates/os-geometry/src/ceilings.rs; crates/os-document/src/tests/ceilings.rs; crates/os-ui/src/plan_workspace/ceiling_tests.rs; crates/os-storage/src/tests/ceilings.rs; crates/os-ifc/tests/exchange.rs |
| M01.09 | Hosted openings | E1 | automated-tested | Checked openings on straight and circular-arc native walls, including typed compound layers and straight wall members with butt/corner/tee trims; arc apertures remove radial material per layer with analytic quantities and bounded plan/section geometry. Bounded IFC4 import/export remains straight/full-depth rectangular only; no extension-element hosts, custom IFC profiles, or joined IFC | native-hosted-openings.md; ifc-roadmap.md; crates/os-ifc/tests/exchange.rs; crates/os-ifc/tests/validate_hosted.py; native-wall-joins.md; native-wall-types.md; crates/os-ui/tests/openings.rs; crates/os-ui/tests/wall_arcs.rs; crates/os-ui/tests/wall_joins.rs; crates/os-ui/tests/wall_types.rs; crates/os-geometry/tests/wall_arcs.rs |
| M01.10 | Doors/windows | E1 | automated-tested | Typed hosted doors/windows with editable profiles/frames, hinge/swing and pane alignment; repeatable placement, center move, anchored jamb resize, Rehost, one-shot center alignment across compatible straight and circular-arc hosts, overrides, schedules and regeneration. Pending placement/copy adds preview-only hinge/swing/pane/lite flips and opt-in exact width in metres. Exact and drawn-width modes are exclusive; reset preserves the base type/copy width representation, including nullable pins. Click, drawn-width, repeat and signed-arc commits preserve source/type and one-step history. Native circular-arc hosts use radial panels/frames/lites, rigid tangent-frame door leaves, analytic dimensions and bounded two-DPI placement-preview/commit coverage without schema changes. Schema 30→31 adds associative tags; schema 40→41 adds four persisted live-data label presets (`Full`, instance, type+dimensions, dimensions only), defaults old tags to `Full`, and rejects ambiguity atomically. Tag identity/orphans, derived leaders, crop/clipping, plan/sheet/vector-PDF, label preset UI and one-step history/move preservation are covered. Schema 31→32 adds panel/pane and frame materials; schema 34/API 15 adds shared editable RGB with migration and rendering evidence. Single-type `.osot` v2 exchange (with v1 single-panel import) and a searchable non-recursive local package folder support reuse. Schema 49→50 adds optional fixed side-lite/two-bay doors and windows with leaf-bay hinge/swing, lite material, production preview, plan divider, atomic shared-type regeneration, frozen migration and package compatibility tests. Office/cloud libraries, arbitrary templates/nested families, IFC material/family round-trip, textures/transparency/PBR, and native visual/physical-print qualification remain open. Temporary editable Start/End spacing uses visible neighbor jambs or host endpoints and changes only instance offset in one undo step; exact/no-op/invalid/stale/precedence behavior has headless egui coverage at both display profiles. Schema 47→48 adds phase-aware saved opening schedules with legacy-unphased migration; schedules require both opening and host to pass their target filter. Schema 48→49 adds per-view Door/Window symbol visibility while retaining hosted wall cutouts. Existing same-kind project types can be assigned to 1–256 selected visible openings with disposable aperture/symbol previews, whole-batch geometry validation and one atomic undo step; typed overrides survive and legacy dimensions are pinned. Duplicate type names are distinguished by stable UUID. | crates/os-ui/src/plan_workspace/placement_orientation_tests.rs; crates/os-ui/src/plan_workspace/placement_width_tests.rs; crates/os-ui/src/plan_workspace/opening_type_assignment_tests.rs; crates/os-ui/src/plan_workspace/opening_spacing_tests.rs; native-material-colors.md; native-opening-materials.md; native-opening-tags.md; native-opening-type-packages.md; crates/os-ui/src/desktop_tests/opening_tag_tests.rs; crates/os-ui/src/desktop_tests/opening_type_library_tests.rs; crates/os-render/src/plan/opening_tags.rs; crates/os-render/src/sheet.rs; crates/os-ui/src/opening_profile_tests.rs; crates/os-ui/src/desktop_tests/opening_family_tests.rs; crates/os-geometry/src/openings.rs; crates/os-document/src/tests/openings.rs; crates/os-storage/src/tests/opening_materials.rs; native-hosted-openings.md; native-phases.md; ifc-roadmap.md; crates/os-ui/tests/openings.rs; crates/os-ui/src/plan_workspace/opening_tests.rs; crates/os-ui/src/plan_workspace/opening_align_tests.rs; crates/os-storage/src/tests/openings.rs; crates/os-storage/tests/two_bay_families.rs; crates/os-storage/tests/opening_type_package.rs; crates/os-ifc/tests/exchange.rs; crates/os-ifc/tests/validate_hosted.py |
| M01.11 | Stairs | E1 | partial | Native straight flights between same-building levels with preview-only repeatable two-click placement, snapping, Properties edit/delete and one-step undo/redo; generated bounded closed 3D mesh, plan tread/arrow graphics, section contours, sheet/vector-PDF projection, save/reopen and fail-closed IFC export. Landings/turns, slab joins/openings, quantities, code checks and native visual/print qualification remain unsupported | native-stairs.md; crates/os-model/src/stairs.rs; crates/os-geometry/src/stairs.rs; crates/os-document/src/tests/stairs.rs; crates/os-render/src/plan/stairs.rs; crates/os-ui/src/plan_workspace/stair_tests.rs; crates/os-storage/src/tests/stairs.rs; crates/os-ifc/tests/exchange.rs |
| M01.12 | Ramps | E1 | partial | Native same-building straight-run slabs with level-derived rise, two-click snapped plan placement, preview-only uphill-arrow/footprint, staged properties, closed 3D mesh, crop/range-aware plan projection, picking, boundary snapping, section contours, sheet/vector-PDF output, save/reopen, one-step history and fail-closed IFC. Landings/turns, rails/curbs, assemblies, quantities/schedules, code checks and IFC exchange remain unsupported | native-ramps.md; crates/os-model/src/ramps.rs; crates/os-geometry/src/ramps.rs; crates/os-document/src/tests/ramps.rs; crates/os-render/src/plan/ramps.rs; crates/os-ui/src/plan_workspace/ramps.rs; crates/os-ui/src/plan_workspace/ramp_tests.rs; crates/os-storage/src/tests/ramps.rs; crates/os-ifc/tests/exchange.rs |
| M01.13 | Railings | E1 | partial | Straight stair-hosted left/right railings with one shared rectangular top-rail/post type, bounded post count and generated-coordinate validation, stable host/type identities, staged type/instance edits, atomic stair deletion, checked independent member meshes, crop/range/phase-aware plan marks and picking, material-aware section contours, sheet/PDF output, save/reopen, undo/redo and fail-closed IFC. Exact cuts through a member triangulation edge remain ambiguous. Free-standing/slab-hosted rails, landings/turns, extensions, infill, custom profiles, schedules, code checks and production visual/print qualification remain unsupported | native-stairs.md; crates/os-model/src/railings.rs; crates/os-geometry/src/railings.rs; crates/os-document/src/tests/railings.rs; crates/os-ui/src/plan.rs; crates/os-ui/src/plan/railing_drawing_tests.rs; crates/os-ui/src/plan_workspace/railings.rs; crates/os-ui/src/plan_workspace/railing_tests.rs; crates/os-storage/src/tests/stairs.rs; crates/os-ifc/tests/exchange.rs |
| M01.14 | Architectural columns | E1 | partial | First-class vertical rectangular columns with stable native identity, level/base offset, dimensions and optional material; snapped plan placement preview, cut/projected plan footprint and pick, generated 3D prism, numeric Properties edit/delete, one-step history and schema-26→27 migration. Circular/slanted columns, reusable types, wall/slab joins, section graphics, schedules, IFC and visual/production qualification remain | native-columns.md; crates/os-model/src/columns.rs; crates/os-geometry/src/columns.rs; crates/os-document/src/tests/columns.rs; crates/os-render/tests/columns.rs; crates/os-storage/src/tests/columns.rs; crates/os-ui/src/plan_workspace/column_tests.rs |
| M01.15 | Furniture/casework/fixtures | E1 | partial | Native typed rectangular casework supports snapped plan placement, transient preview, selected shared-type/instance editing, plan selection, 3D regeneration, one-transaction placement, undo/redo, save/reopen, and schema-61→62 migration. The type is a simple solid envelope only; furniture/fixtures, cabinet doors/drawers/interiors, detailed families, section graphics, quantities, generic move/copy/mirror, and IFC remain unsupported | native-casework.md; crates/os-model/src/casework.rs; crates/os-geometry/src/casework.rs; crates/os-document/src/tests/casework.rs; crates/os-render/tests/casework.rs; crates/os-storage/src/tests/casework.rs; crates/os-ui/src/plan_workspace/casework_tests.rs; crates/os-ifc/tests/exchange.rs |
| M01.16 | Site coordinates | E1 | not started | Not implemented | — |
| M01.17 | Terrain/survey references | E1 | not started | Not implemented | — |
| M01.18 | Type/instance parameters | E1 | partial | Opening types share kind/dimensions, normalized component profile, extrusion depth, frame dimensions, optional Start/End side-lite width fraction and physical mullion width, plus nullable panel/pane, frame and optional lite materials; instances retain name/host/offset/hinge/swing and can independently inherit/pin/reset sill, width and height. Shared type changes preflight all instances, preserve instance pins, regenerate in one undoable transaction and retain stable material IDs; no formulas or general nested families. Temporary plan spacing edits preserve all type/instance fields except offset, re-resolve effective width/reference at commit, and add no persisted constraint. Exact placement width remains transient until commit, pins only the placed typed instance, and Copy/reset preserve the source's original nullable width representation. Existing-type reassignment supports 1–256 same-kind instances in one floor plan; inherited values follow the destination while pins, host, offset, identity and metadata survive. Legacy width/height/window sill become overrides. Candidate/commit preflight covers the full batch, with no-op and stale-context protection. The instance-only editor supports 1–256 same-kind openings: common/Mixed effective values and value sources; Keep/Set and typed-only Inherit for width/height/window sill; independent absolute door hinge/swing. Multi-selection summaries open it from Properties; a single opening uses the Architecture command. Mixed types and typed/legacy groups preserve unchosen fields and definitions. One invalid member rejects all; preview is disposable, apply is changed-only and undoable. | crates/os-ui/src/plan_workspace/opening_batch_edit_tests.rs; crates/os-ui/src/plan_workspace/placement_width_tests.rs; crates/os-ui/src/plan_workspace/opening_type_assignment_tests.rs; crates/os-ui/src/plan_workspace/opening_spacing_tests.rs; native-opening-materials.md; native-hosted-openings.md; crates/os-document/src/tests/openings.rs; crates/os-ui/src/opening_type_tools.rs; crates/os-ui/src/opening_profile_tests.rs; crates/os-ui/src/opening_sill_tests.rs; crates/os-storage/tests/opening_materials.rs; crates/os-storage/tests/window_sills.rs |
| M01.19 | Level relationships | E1 | partial | Straight walls only; no full architectural drawings or schedules | plugins/walls wall_solid; vertical_slice.rs; baseline tests |
| M01.20 | Host relationships | E1 | partial | Openings retain a native wall host and reject host deletion/shortening that invalidates them; no general host graph | native-hosted-openings.md; crates/os-ui/tests/openings.rs |
| M01.21 | Placement | E1 | partial | Straight and circular-arc walls; repeatable click-to-place and opt-in snapped two-jamb drag placement for native doors/windows, with transient measured-width symbol/aperture preview and instance width override. An exclusive Exact width mode accepts finite metre values, centers on the snapped host station, preserves type defaults and copy pins until reset/commit, and previews the same analytic centerline width on arcs. Curved placement uses centerline stations and bounded radial symbols; no plugin hosts or full architectural drawings. Selected native openings also support exact temporary clear-spacing edits on straight and circular-arc hosts to visible neighboring jambs or wall endpoints; arc values measure centerline travel and use a curved dimension graphic. Crop/phase/range/canvas filtering and guarded pointer ownership remain authoritative. | crates/os-ui/src/plan_workspace/placement_width_tests.rs; crates/os-ui/src/plan_workspace/opening_spacing_tests.rs; native-hosted-openings.md; crates/os-ui/src/plan_workspace.rs; crates/os-ui/src/plan_workspace/opening_tests.rs; crates/os-ui/src/plan_workspace/arc_opening_tests.rs |
| M01.22 | Flip/mirror | E1 | partial | Selected visible native straight or circular-arc doors have independent direct plan controls for Start/End hinge and Left/Right swing relative to stored host direction/local tangent. Typed windows expose Side for off-center panes and Flip lite for side-lites, using local host face and stored station semantics. One-field validated updates preserve other instance/type fields and regenerate plan/3D. Curved controls anchor to the analytic center grip and respect crop/canvas, grip hit areas and single-selection ownership. Headless egui acceptance covers both DPI profiles, sweep signs, typed/legacy instances, Center/legacy suppression, cancel/undo/redo, curved leaf and unequal pane/lite geometry; manual native visual QA remains open. General flip/mirror tools remain unimplemented | native-hosted-openings.md; straight opening flip tests in crates/os-ui/src/plan_workspace/opening_tests.rs and window_pane_tests.rs; curved acceptance in crates/os-ui/src/plan_workspace/arc_opening_tests.rs |
| M01.23 | Material layers | E1 | partial | Wall layers retain stable material references and density; per-layer quantities and identity flow through plan/section/mesh. Shared editable RGB now colors plan/sheet cut fills and 3D surfaces without changing geometry or quantities. Textures, transparency, PBR, hatch standards, and a full material library remain | native-wall-types.md; native-material-colors.md; crates/os-ui/tests/wall_types.rs |
| M01.24 | Geometry and symbolic representations | E1 | partial | Straight-wall cuts support rectangular or authored simple polygon elevation profiles; circular-arc openings sweep the authored elevation regions radially through each wall layer for 3D meshes, plan cuts, sections, and analytic layer quantities. Profile cuts reject rectangle-only Solid exports. Splines, multiple rings/holes and a full detail-level system remain open | native-hosted-openings.md; crates/os-geometry/src/walls/profiles.rs; crates/os-geometry/tests/host_cut_profiles.rs; crates/os-geometry/tests/wall_arcs.rs; crates/os-ui/src/plan.rs |
| M01.25 | View/detail visibility | E1 | partial | Independent per-plan Show doors/Show windows controls; category-hidden symbols, tags, referenced dimensions and direct opening interactions are omitted while openings continue to cut visible host walls. Phase filtering remains distinct and can hide the opening cut. Full visibility/detail controls for other native categories and linked models remain unimplemented; manual native-window/print qualification remains open | native-hosted-openings.md; crates/os-model/src/plans.rs; crates/os-ui/src/plan_settings.rs; crates/os-ui/src/plan.rs; crates/os-ui/src/plan/opening_visibility_tests.rs; crates/os-ui/src/plan_workspace/opening_visibility_tests.rs; crates/os-storage/tests/opening_visibility.rs |
| M01.26 | Consistent quantities | E1 | partial | Straight walls report net wall and per-layer volume; circular wall layer volume/mass is analytic and checked against bounded mesh volume. No full architectural quantity schedules or broader element takeoff | native-wall-types.md; crates/os-ui/tests/wall_types.rs; crates/os-geometry/tests/wall_arcs.rs; vertical_slice.rs |
| M01.27 | Model group membership | E1 | not started | Not implemented | — |
| M01.28 | Reusable assemblies | E1 | not started | Not implemented | — |
| M01.29 | Architectural part/layer identity | E1 | partial | Compound wall layers have stable identities carried by geometry, plan and section while wall UUID remains selection owner; no independently editable/hosted part entities or part schedules | native-wall-types.md; crates/os-ui/tests/wall_types.rs |
| M01.30 | Host/type/level regeneration across drawings and information | E1 | partial | Host and shared-type edits regenerate affected 3D/plan/section geometry; opening schedules and associative tags resolve live dimensions/names and preserve orphan diagnostics; layer quantities follow wall-type edits. Generalized schedule/tag fields, filters and styles remain Batch opening type reassignment preflights affected hosts/panels, regenerates committed plan/3D in one transaction and preserves selection through undo/redo; live tag/jamb resolution and save/reopen have focused acceptance evidence. | crates/os-ui/src/plan_workspace/opening_type_assignment_tests.rs; native-hosted-openings.md; native-opening-tags.md; native-opening-materials.md; native-wall-types.md; crates/os-document/src/tests/openings.rs; crates/os-ui/src/opening_schedule.rs; crates/os-ui/src/desktop_tests/opening_tag_tests.rs; crates/os-ui/tests/wall_types.rs |
| M02.01 | Parametric opening family authoring | E1 | partial | Versioned editor provides separate normalized component and host-cut profiles, draft-only preview, and type-shared atomic regeneration. Family v5 optionally composes a fixed side lite and mullion with Start/End placement, normalized lite width and optional lite material; the side-lite mode is bounded to rectangular profiles and at least 1 mm usable clear width per bay. Project materials independently assign to the door leaf/window pane, lite and generated frame rails; material references are checked, rendered as distinct surfaces with shared editable RGB, and migrate through family v4/schema32 to family v5/schema50. Native door/window type packages v2 export/import definitions and exact referenced material snapshots; v1 single-panel packages upgrade on import, with explicit conflict resolution, host/instance preflight and one-step undo. General host cuts remain bounded to simple 3–32 vertex rings; straight-host generated frame rails require rectangular cuts. Constrained sketches, arbitrary/nested forms beyond the fixed two-bay composition, additional authored solids, splines, formulas, textures, transparency, PBR, IFC family/material export, searchable catalogs and office libraries remain | native-opening-type-packages.md; native-opening-materials.md; native-material-colors.md; native-hosted-openings.md; crates/os-model/src/opening_family.rs; crates/os-geometry/src/openings.rs; crates/os-ui/src/opening_family_editor.rs; crates/os-ui/src/opening_profile_tests.rs; crates/os-ui/src/desktop_tests/opening_family_tests.rs; crates/os-ui/src/desktop_tests/opening_type_package_tests.rs; crates/os-storage/tests/opening_type_package.rs; crates/os-storage/tests/two_bay_families.rs |
| E01.01 | Lines | D/E2 | not started | Not implemented | — |
| E01.02 | Polylines | D/E2 | not started | Not implemented | — |
| E01.03 | Arcs | D/E2 | not started | Not implemented | — |
| E01.04 | Circles | D/E2 | not started | Not implemented | — |
| E01.05 | Ellipses | D/E2 | not started | Not implemented | — |
| E01.06 | Splines | D/E2 | not started | Not implemented | — |
| E01.07 | Rectangles | D/E2 | not started | Not implemented | — |
| E01.08 | Polygons | D/E2 | not started | Not implemented | — |
| E01.09 | Reference lines/planes | D/E2 | not started | Not implemented | — |
| E01.10 | Closed sketch/region validation | D/E2 | not started | Not implemented | — |
| E01.11 | Select/window/crossing selection | D/E2 | partial | Shift-drag supports left-to-right containment and right-to-left crossing for visible native walls/openings, columns, floors/ceilings and valid rooms; selected groups highlight in plan and same-kind opening groups have a Properties summary with common/Mixed effective dimensions and inherited/pinned/legacy states. Generic provider graphics, annotation marquee selection and general group edits remain open. Same-kind native opening groups now support existing project type reassignment (1–256 visible instances in one floor plan) with draft preview and atomic apply. Properties instance edits support width/height/window sill Keep/Set/typed-only Inherit and absolute door handing across mixed assigned types or typed/legacy groups, with atomic validation, draft preview and one-step undo/redo. Other entity group edits remain open. | crates/os-ui/src/plan_workspace/opening_batch_edit_tests.rs; crates/os-ui/src/plan_workspace/opening_type_assignment_tests.rs; native-plan-workspace.md; os-ui/src/plan_workspace/selection.rs; os-ui/src/plan_workspace/selection_tests.rs |
| E01.12 | Overlap cycling | D/E2 | partial | Click the plan to focus it, hover an overlap, Tab/Shift+Tab previews candidates in deterministic existing-pick order, click revalidates/commits; Escape/movement/stale context cancel without model/history mutation. Native doors/windows cycle separately from host walls; all current pick layers and UI fallbacks are enumerated. Candidate graphics are highlighted in-canvas. Native-window visual QA and broader category/tie-policy validation remain | native-plan-workspace.md; os-render/src/plan/overlap.rs; os-render/src/plan.rs (dimension_hits); os-ui/src/plan_workspace/overlap_selection.rs; os-ui/src/plan_workspace/selection_tests.rs |
| E01.13 | Selection category filters | D/E2 | partial | Session-only category toggles for walls, doors, windows, floors, ceilings, columns, roofs, stairs, rooms, grids, annotations and Other, exposed under Snaps → Selection filters. Filters affect ordinary click-through, overlap cycling and the existing crop-clipped native marquee set; previews/drafts cancel on change without pruning selection. Defaults reset on document-session change and persist across plan views. Does not hide geometry or affect snapping/editing/3D. Marquee still omits annotations and generic provider graphics; native-window visual QA remains | native-plan-workspace.md; crates/os-ui/src/plan_workspace/selection_filters.rs; crates/os-ui/src/plan_workspace/selection_tests.rs |
| E01.14 | Move/copy | D/E2 | partial | Bundled wall move preserves UUID and level; 2–64 visible same-level native straight walls move together by snapped or exact metric vector in one atomic undo step. Selected wall assemblies can also be copied with hosted openings, complete internal joins, and active-plan opening tags using stable remapped IDs; rooms, room tags, and dimensions are intentionally not duplicated. Preview/cancel/stale/snap/undo/redo/save-reopen and split redraw are covered at two DPI profiles. Partial joins, collisions, curves, mixed levels and installed Wall-provider copy are unsupported. Other entities remain | native-wall-edits.md; native-wall-set-copy.md; native-hosted-openings.md; crates/os-ui/src/plan_workspace/wall_set_move_tests.rs; crates/os-ui/src/plan_workspace/wall_set_copy_tests.rs; crates/os-ui/src/plan_workspace/opening_tests.rs (`opening_center_snap_*`, `opening_drag_*`, `opening_copy_preserves_instance_and_type_on_another_wall`); crates/os-ui/src/plan_workspace/arc_opening_tests.rs; plan-wall-edit-gestures.md; os-ui/tests/plan_gesture.rs |
| E01.15 | Rotate/mirror | D/E2 | partial | One visible selected native straight wall; midpoint pointer/exact-degree rotation and two-click-axis mirror, disposable preview, atomic commit/undo, hosted door/window updates including instance pane-side swaps, typed layer-side flip, stale/cancel checks, and split 2D/3D regeneration. Joined/annotated walls, multi-select, and incompatible installed-provider mirrors remain unsupported; native visual acceptance remains open | plan-wall-edit-gestures.md; native-wall-edits.md; crates/os-ui/src/plan_workspace/transforms.rs; crates/os-ui/src/plan_workspace/endpoint_tests.rs |
| E01.16 | Align | D/E2 | partial | One-shot parallel/antiparallel native wall centerline alignment plus selected door/window center projection across straight and finite circular-arc hosts. Opening alignment uses the analytic aperture center, supports mixed host geometry and signed major-arc travel, previews both centers/guide and candidate aperture, rejects invalid fit/overlap/out-of-sweep projections without clamping, and commits only offset in one undo step. Both workflows have two-DPI preview/history/cancel evidence; opening tests cover typed/legacy kinds, visibility/phase/crop, stale contexts and pointer ownership. General persistent alignment/equality constraints and a broader solver, nonparallel straight-to-straight alignment, native visual acceptance and full Revit parity remain open; installed wall Align acceptance remains separately limited | plan-wall-edit-gestures.md; native-wall-edits.md; native-hosted-openings.md; crates/os-ui/src/plan_workspace/transforms.rs; crates/os-ui/src/plan_workspace/endpoint_tests.rs; crates/os-ui/src/plan_workspace/opening_align_tests.rs |
| E01.17 | Offset | D/E2 | partial | Signed centreline parallel wall copies with preview/cancel and one-step undo; curves, general entities and independent tools remain | plan-wall-offset.md; os-ui/tests/plan_gesture.rs; desktop pointer offset test |
| E01.18 | Trim/extend | D/E2 | partial | One selected unjoined straight wall start/end trims or extends to the picked intersection with a visible finite same-level straight wall. Preview is transient; one atomic commit/undo. Moving the start adjusts hosted door/window offsets to preserve world location; invalid opening fit is rejected. Installed Wall End edits use the bounded worker; Start edits with hosted openings and joined walls remain unsupported. No multi-wall trim, chaining or auto-join; native visual QA remains open | native-wall-edits.md; plan-wall-edit-gestures.md; crates/os-ui/src/plan_gesture.rs; crates/os-ui/src/plan_workspace/endpoint_tests.rs (`trim_extend_*`) |
| E01.19 | Split | D/E2 | partial | One selected visible native straight wall; picked interior station, isolated preview and atomic split. Preserves metadata/types, rehosts openings, remaps dimension endpoints and room signatures with explicit geometry checks. Both DPI profiles, tags, cancellation/staleness, undo/redo, save/reopen and split 2D/3D tested. Joined walls and installed providers rejected; multi-wall/general entity splits and native visual acceptance remain open | plan-wall-edit-gestures.md; native-wall-edits.md; crates/os-ui/src/plan_workspace/transforms/split.rs; crates/os-ui/src/plan_workspace/split_tests.rs |
| E01.20 | Fillet | D/E2 | not started | Not implemented | — |
| E01.21 | Arrays | D/E2 | partial | Array along wall copies one selected visible native door/window on a straight or circular-arc host: count 2–256 including source, positive centre spacing in metres along the analytic centerline, stored Start/End direction, transient preview and clear gap. Apply validates the entire batch and creates independent instances in one undo step; optional tags copy into each opening's local host frame. Six focused headless egui/unit tests cover both DPI profiles, typed/legacy doors/windows, reversed/rotated straight hosts, property/tag preservation, collisions/limits, cancellation, save/reopen and split regeneration; the curved-host two-DPI acceptance also checks preview/commit/undo. Persistent array constraints, other entities/hosts, and native-window visual QA remain | native-hosted-openings.md; crates/os-ui/src/plan_workspace/opening_array.rs; crates/os-ui/src/plan_workspace/opening_array_tests.rs; crates/os-ui/src/plan_workspace/arc_opening_tests.rs |
| E01.22 | Group/ungroup | D/E2 | not started | Not implemented | — |
| E01.23 | Pin/unpin | D/E2 | not started | Not implemented | — |
| E01.24 | Copy/paste aligned with identity remapping | D/E2 | not started | Not implemented | — |
| E01.25 | Endpoint snapping | D/E2 | partial | Semantic wall endpoints and two-point feedback; selected native-wall handle drags reuse snap/exclusion with headless tests at two DPI profiles; broader independent plugin/native acceptance remains | plan-wall-gestures.md; plan-wall-edit-gestures.md; os-render/tests/snapping.rs; os-ui/src/plan_workspace/endpoint_tests.rs |
| E01.26 | Midpoint snapping | D/E2 | partial | Semantic midpoint queries and initial pointer-tool feedback; broader entities/providers remain | plan-wall-gestures.md; os-render/tests/snapping.rs |
| E01.27 | Intersection snapping | D/E2 | partial | Finite semantic line pairs, crop/exclusion checks, canonical pair references and bounded local search; curves, independent providers and native inspection remain | plan-intersection-snapping.md; os-render/tests/snapping.rs; desktop intersection input test |
| E01.28 | Perpendicular snapping | D/E2 | partial | Gesture-anchor projection onto finite semantic lines, screen acquisition, crop/exclusion and visible session toggle; curves, inferred anchors and independent providers remain | plan-perpendicular-snapping.md; os-render/tests/snapping.rs; desktop perpendicular input test |
| E01.29 | Tangent snapping | D/E2 | not started | Not implemented | — |
| E01.30 | Nearest snapping | D/E2 | partial | Cropped semantic-line query used by initial wall tool; curves/providers remain | plan-wall-gestures.md; os-render/tests/snapping.rs |
| E01.31 | Center snapping | D/E2 | not started | Not implemented | — |
| E01.32 | Grid snapping | D/E2 | partial | Persisted finite building grids supply endpoint/midpoint and GridAxis nearest snaps in bundled wall gestures; crop intersections are not endpoints. Intersections, overrides and external route remain | os-render/tests/grids.rs; os-ui/tests/grid_plans.rs; desktop grid pointer test; grid-plan-integration.md |
| E01.33 | Axis snapping | D/E2 | partial | Optional semantic line-axis extensions beyond finite endpoints, crop/exclusion/query checks and pointer workflow; inferred/world-axis guides and independent providers remain | plan-axis-extension-snapping.md; os-render/tests/snapping.rs; desktop axis extension test |
| E01.34 | Snap priorities/overrides | D/E2 | partial | Deterministic host priority with visible session enable switches for implemented kinds; custom priority/cycling and provider aggregation remain | plan-snap-controls.md; semantic-plan-snapping.md; os-render/tests/snapping.rs; desktop snap controls tests |
| E01.35 | Screen acquisition and geometric tolerances | D/E2 | not started | Not implemented | — |
| E01.36 | Exact coordinates | D/E2 | partial | Numeric wall Properties/commands only; no plan input gestures | os-ui palettes.rs and desktop_tests |
| E01.37 | Exact length | D/E2 | partial | Bundled wall creation/endpoint resize length and move distance drafts plus native door/window exact placement width in metres; broader tools remain | plan-wall-edit-gestures.md; crates/os-ui/src/plan_workspace/placement_width_tests.rs; os-ui/tests/plan_gesture.rs |
| E01.38 | Exact angle | D/E2 | partial | Bundled wall gesture degree input in active plan basis; broader tools remain | plan-wall-edit-gestures.md; os-ui/tests/plan_gesture.rs |
| E01.39 | Exact offset | D/E2 | partial | Signed metre input for parallel native wall copies, independent of plan rotation; broader reference/unit workflows remain | plan-wall-offset.md; os-ui/tests/plan_gesture.rs |
| E01.40 | Metric input | D/E2 | partial | Numeric wall Properties/commands and opt-in native door/window placement width in metres; broader plan input remains | os-ui palettes.rs; desktop_tests; crates/os-ui/src/plan_workspace/placement_width_tests.rs |
| E01.41 | Decimal imperial input | D/E2 | not started | Not implemented | — |
| E01.42 | Fractional imperial input | D/E2 | not started | Not implemented | — |
| E01.43 | Temporary dimensions | D/E2 | partial | Editable temporary Start/End spacing dimensions exist for one selected native door/window on straight and circular-arc hosts. Neighbor-jamb measurements remain one-shot offset edits; only an explicit action against a real host endpoint can create/edit a persistent clearance lock. Locks preserve one Start/End centerline distance through host length and effective-width edits, with exact input, cancel/stale protection and one-step undo. General temporary dimensions for other categories, neighbor/equality constraints, and native-window/physical-print qualification remain open | native-hosted-openings.md; crates/os-ui/src/opening_tools/spacing.rs; crates/os-ui/src/plan_workspace/opening_spacing_tests.rs; crates/os-document/tests/opening_clearances.rs |
| E01.44 | Live previews | D/E2 | partial | Transient wall axis/exact-input and hosted door/window symbol/aperture previews include pending hinge/swing/pane/lite orientation and exact width; exact-vs-drawn width, parse errors, immutable preview and fit/overlap feedback are covered. Live solid/3D and general tools remain | plan-wall-edit-gestures.md; native-hosted-openings.md; crates/os-ui/src/plan_workspace/placement_orientation_tests.rs; crates/os-ui/src/plan_workspace/placement_width_tests.rs; crates/os-ui/src/plan_workspace/opening_tests.rs |
| E01.45 | Repeat tools | D/E2 | partial | Door/window placement and copy remain active for repeated one-click commits until Escape; preview orientation and exact-width choices persist across repeat placements and each instance commits independently. General repeat tools remain | native-hosted-openings.md; crates/os-ui/src/plan_workspace/placement_orientation_tests.rs; crates/os-ui/src/plan_workspace/placement_width_tests.rs; crates/os-ui/src/plan_workspace/opening_tests.rs |
| E01.46 | Cancel without residue | D/E2 | partial | Escape/cancel wall gestures, opening placement, and grid/settings forms preserve model; broader tools remain | plan-wall-edit-gestures.md; native-hosted-openings.md; crates/os-ui/src/plan_workspace/opening_tests.rs |
| E01.47 | Keyboard focus preservation | D/E2 | partial | Numeric wall Properties/commands only; no plan input gestures | os-ui palettes.rs and desktop_tests |
| E01.48 | One gesture one undo | D/E2 | partial | Wall create/move/click resize, native endpoint drags, and each opening placement click commit once; bundled and explicitly run installed handle tests verify undo/redo; broader tools and native manual inspection remain | plan-wall-edit-gestures.md; native-hosted-openings.md; os-ui/tests/plan_gesture.rs; os-ui/src/plan_workspace/opening_tests.rs |
| E01.49 | Geometric/dimensional constraints | D/E2 | partial | One optional persistent Start/End clearance constraint per native door/window on straight or circular-arc hosts; candidate settlement propagates host-length and effective-width changes atomically and validates fit/overlap before one history entry. Direct conflicting moves, hidden openings, unlock/rehost/split policy, schema migration and basic plan/3D/schedule propagation have focused tests. General geometric constraints, neighbor-jamb chains, equality, an interactive/general solver and native visual/print QA remain open | docs/native-hosted-openings.md; crates/os-model/src/opening_clearances.rs; crates/os-model/tests/opening_clearances.rs; crates/os-document/tests/opening_clearances.rs; crates/os-storage/tests/opening_clearances.rs; crates/os-ui/src/plan_workspace/opening_spacing_tests.rs |
| E01.50 | Equality constraints | D/E2 | not started | Not implemented | — |
| E01.51 | Locked alignment | D/E2 | not started | Not implemented | — |
| E01.52 | Transactional conflict explanations | D/E2 | partial | Clearance-lock conflicts identify the affected opening and require editing/unlocking the lock; invalid fit or overlap rejects the entire host/type/batch transaction without model/history changes. Broader solver conflict graphs and resolution UI remain open | docs/native-hosted-openings.md; crates/os-model/src/opening_clearances.rs; crates/os-document/tests/opening_clearances.rs |
| A01.01 | Aligned/linear dimensions | E2 | partial | Native three-click dimensions from wall endpoints or live native opening jambs; typed/legacy effective width, visible/cropped jamb acquisition, live metric value and editable offset. Schema-53 outer wall-face references now track metric station and effective thickness; print-faithful sizing remains | native-dimensions.md; os-model/tests/dimension_faces.rs; os-ui/src/plan_workspace/face_tests.rs; os-model/tests/dimension_openings.rs; os-render/tests/plan.rs; os-ui/src/plan_workspace/endpoint_tests.rs |
| A01.02 | Chained/baseline dimensions | E2 | partial | Native straight-wall endpoint/opening-jamb chains and cumulative baselines; ordered collinear anchors, one semantic entity, one-step history; paper-scale formatting and broader references remain | native-dimensions.md; os-model/src/dimensions.rs; os-ui/src/plan_workspace/endpoint_tests.rs; os-storage/tests/dimensions.rs |
| A01.03 | Angular dimensions | E2 | partial | Native straight-wall axes, four non-reflex sectors, live degree reporting and orphan recovery; cropped arc/label picking. Headless authoring at both DPI profiles, single transaction and undo/redo; frozen schema-11 migration and Angular roundtrip. No native-window or print qualification | native-dimensions.md; dimensions model/storage tests; plan renderer tests; endpoint_tests.rs |
| A01.04 | Radial/diameter dimensions | E2 | not started | Not implemented | — |
| A01.05 | Arc-length dimensions | E2 | not started | Not implemented | — |
| A01.06 | Spot elevation/coordinate/slope | E2 | not started | Not implemented | — |
| A01.07 | Witness editing | E2 | not started | Not implemented | — |
| A01.08 | Prefixes/suffixes/tolerances | E2 | not started | Not implemented | — |
| A01.09 | Unit/rounding styles | E2 | not started | Not implemented | — |
| A01.10 | Semantic feature/material references | E2 | partial | Native opening jambs and schema-53 outer wall faces are semantic references resolved from live host geometry and effective dimensions. Curved-wall, material-layer and linked-object references remain open | native-dimensions.md; os-model/tests/dimension_faces.rs; os-ui/src/plan_workspace/face_tests.rs; os-model/tests/dimension_openings.rs; os-storage/tests/dimension_anchors.rs |
| A01.11 | Linked-object references | E2 | not started | Not implemented | — |
| A01.12 | Reference remapping | E2 | not started | Not implemented | — |
| A01.13 | Visible orphan warnings | E2 | partial | Missing/releveled/coincident wall or opening references persist and show an on-plan marker, Properties reason and Browser identity; deliberate reference repair remains | native-dimensions.md; os-ui/src/plan_workspace/endpoint_tests.rs |
| A01.14 | Deliberate reference repair | E2 | automated-tested | Properties selects an anchor; same-level visible wall endpoint/body replacement commits one UpdateDimension, preserving UUID and all other data. Multiple missing/wrong-level references repair sequentially with independent undo/redo. Aligned/Chain/Baseline/Angular, invalid/cancel/stale and rotated-plan checks at both DPI profiles. Opening jamb and outer wall-face anchors are available through the same visible-anchor picker; native-window/installed-provider qualification remains open | native-dimensions.md#deliberate-reference-replacement-2026-09-27; crates/os-ui/src/plan_workspace/endpoint_tests.rs: dimension_repair_*; focused all-feature endpoint suite |
| A01.15 | Reporting versus driving dimensions | E2 | partial | Values derive from live wall endpoints or opening jambs and never drive geometry; no constraints or value overrides | native-dimensions.md; os-document/tests/dimensions.rs; os-ui/src/plan_workspace/endpoint_tests.rs |
| A01.16 | Identifiable value overrides | E2 | not started | Not implemented | — |
| A01.17 | Rich text | E2 | not started | Not implemented | — |
| A01.18 | Wrapping/lists | E2 | not started | Not implemented | — |
| A01.19 | Symbols | E2 | not started | Not implemented | — |
| A01.20 | Leaders/arrowheads | E2 | not started | Not implemented | — |
| A01.21 | Text alignment | E2 | not started | Not implemented | — |
| A01.22 | Find/replace | E2 | not started | Not implemented | — |
| A01.23 | Language-qualified Unicode shaping | E2 | not started | Not implemented | — |
| A01.24 | Explicit font substitution | E2 | not started | Not implemented | — |
| A01.25 | Element/material/room/area tags | E2 | partial | Native view-owned room badges and door/window opening tags use stable target/view UUIDs, live label resolution and orphan diagnostics. Opening tags have four persisted live-data label presets, schema 40→41 default migration, plan/sheet/vector-PDF output, derived straight leaders and orphan badges without leaders; desktop tests cover preset selection and undo/redo at both profiles. Room tags retain room number/name and area at room seed. Generic arbitrary-element/material/area tags, leaders for non-opening tags, configurable leader/tag styles, annotation schedules and visual/physical print qualification remain | native-rooms.md; native-opening-tags.md; os-model/tests/room_tags.rs; os-document/tests/room_tags.rs; os-storage/tests/room_tags.rs; os-ui/src/desktop_tests/room_tag_tests.rs; crates/os-ui/src/desktop_tests/opening_tag_tests.rs |
| A01.26 | Multi-element leaders | E2 | not started | Not implemented | — |
| A01.27 | Keynote catalogs/legends | E2 | not started | Not implemented | — |
| A01.28 | General notes | E2 | not started | Not implemented | — |
| A01.29 | Annotation schedules | E2 | not started | Not implemented | — |
| A01.30 | Type/instance label binding | E2 | partial | Opening tags resolve instance name, type name and effective width×height live from door/window data; four persisted presets select full, instance-only, type+dimensions or dimensions-only output. Edits update labels without replacing the tag. Arbitrary parameter selection/templates and other element categories remain | native-opening-tags.md; crates/os-model/tests/opening_tags.rs; crates/os-ui/src/desktop_tests/opening_tag_tests.rs |
| A02.01 | Independent detail lines | E2 | partial | View-owned straight XY lines with stable UUIDs, validated plan reference/endpoints/identity, transactional create/edit/remove and history, crop/pick/snap, and 0.25 mm vector sheet/PDF strokes. Focused model, document, frozen 17→18 storage, render and headless egui tests at both DPI profiles cover these paths. No native visual or physical print QA; curves, line styles and standalone detail views remain open. PDF searchability applies to sheet text, not unlabeled line paths. | native-detail-lines.md; crates/os-model/tests/detail_lines.rs; crates/os-document/tests/detail_lines.rs; crates/os-storage/tests/detail_lines.rs; crates/os-render/tests/detail_lines.rs; crates/os-ui/src/plan_workspace/detail_line_tests.rs |
| A02.02 | Filled/masking regions | E2 | not started | Not implemented | — |
| A02.03 | Insulation and break lines | E2 | not started | Not implemented | — |
| A02.04 | Detail components | E2 | not started | Not implemented | — |
| A02.05 | Repeating details | E2 | not started | Not implemented | — |
| A02.06 | Detail groups | E2 | not started | Not implemented | — |
| A02.07 | Foreground/background ordering | E2 | not started | Not implemented | — |
| A02.08 | Parametric detail component editor | E2 | not started | Not implemented | — |
| A02.09 | Annotation symbol editor | E2 | not started | Not implemented | — |
| A02.10 | Tag editor | E2 | not started | Not implemented | — |
| A02.11 | Title-block editor | E2 | not started | Not implemented | — |
| A02.12 | Model-element 2D symbol editor | E2 | not started | Not implemented | — |
| A02.13 | Content reference planes | E2 | not started | Not implemented | — |
| A02.14 | Content constraints | E2 | not started | Not implemented | — |
| A02.15 | Type catalogs | E2 | not started | Not implemented | — |
| A02.16 | Safe expressions/formulas | E2 | not started | Not implemented | — |
| A02.17 | Nested components | E2 | partial | The native opening family supports one bounded fixed side-lite/mullion composition (two bays) for rectangular door/window components. Arbitrary, reusable, recursively nested or user-authored multiple components remain unsupported | native-hosted-openings.md; crates/os-geometry/src/openings.rs; crates/os-ui/src/opening_profile_tests.rs |
| A02.18 | Visibility parameters | E2 | not started | Not implemented | — |
| A02.19 | Scale/detail previews | E2 | not started | Not implemented | — |
| A02.20 | Searchable local/office libraries | E2 | partial | Manage exposes a session-local browser for one user-entered, non-recursive folder of `.osot` packages. A bounded background scan sorts valid packages deterministically, reports corrupt files without hiding valid entries, and searches type name/kind, file path and material names; selection re-reads the package and enters the existing conflict-aware preview. Persistent favorites/paths, recursion, office/cloud catalogs and shared indexing remain | native-opening-type-packages.md; crates/os-storage/src/opening_type_library.rs; crates/os-storage/tests/opening_type_library.rs; crates/os-ui/src/opening_type_package_ui.rs; crates/os-ui/src/desktop_tests/opening_type_library_tests.rs |
| A02.21 | Versioned content packages | E2 | partial | Strict version-2 `.osot` JSON packages export/import one native door/window type plus only its referenced panel/frame/lite material snapshots; bounded v1 imports upgrade as single-panel families. Input/output is capped at 1 MiB, duplicate/unknown fields are rejected, and writes replace atomically. Searchable catalogs and future package migrations remain | native-opening-type-packages.md; crates/os-storage/src/opening_type_package.rs; crates/os-storage/tests/opening_type_package.rs |
| A02.22 | Loading/updating/conflicts | E2 | partial | Import previews as a new type by default or an explicit update to the selected same-kind type. Identical materials are reused; differing same-name materials and type names receive visible unique names rather than overwriting project data. Update preserves target UUID and is blocked if any affected host/opening fails preflight. Multi-target conflict UI and selective field merge remain | native-opening-type-packages.md; crates/os-ui/src/opening_type_package_ui.rs; crates/os-ui/src/desktop_tests/opening_type_package_tests.rs |
| A02.23 | Cross-project content transfer | E2 | partial | Explicit file export/import transfers one type between projects, remapping exact material dependencies and resolving destination collisions. Shared/office catalogs, bulk transfer and remote synchronization remain | native-opening-type-packages.md; crates/os-storage/tests/opening_type_package.rs; crates/os-ui/src/desktop_tests/opening_type_package_tests.rs |
| A02.24 | Licensed original starter content | E2 | not started | Not implemented | — |
| A02.25 | Affected-instance update previews | E2 | partial | Preview reports affected instance and pinned-dimension counts and validates every affected host/component before enabling commit; no graphical side-by-side preview yet | native-opening-type-packages.md; crates/os-ui/src/desktop_tests/opening_type_package_tests.rs |
| A02.26 | Preserve project edits on update | E2 | partial | Explicit update keeps the destination type UUID and every instance parameter/pinned dimension; imported definition fields intentionally replace destination type fields. Selective merge of local type edits remains | native-opening-type-packages.md; crates/os-ui/src/desktop_tests/opening_type_package_tests.rs |
| A02.27 | Content rollback | E2 | partial | One document transaction covers new materials and type add/update, so undo/redo is atomic; cancellation, stale contexts and invalid preflight do not mutate the model. Failed regeneration rolls back the transaction. Savepoint recovery across process failure and catalog revisions remain | native-opening-type-packages.md; crates/os-ui/src/desktop_tests/opening_type_package_tests.rs; crates/os-storage/tests/opening_type_package.rs |
| A02.28 | No host-code expression execution | E2 | not started | Not implemented | — |
| P01.01 | Room/area boundaries | E1 | partial | Native straight-wall centerlines and level-owned separator segments form faces with persistent mixed-ID topology signatures; holes and finish offsets remain | native-rooms.md; os-geometry/tests/rooms.rs; os-storage/tests/room_separation_lines.rs |
| P01.02 | Separation lines | E1 | partial | Level-owned straight lines with snapped plan authoring, preview/commit, endpoint/body edits, crop-clipped graphics and selection; schema-18→19 migration and save/reopen covered. Native-window and print acceptance remain | native-rooms.md; os-storage/tests/room_separation_lines.rs; os-ui room separator pointer tests |
| P01.03 | Room/area identifiers | E1 | partial | Native room UUID, level, seed, unique-per-level number and editable name; type catalogs and general area entities remain | native-rooms.md; os-model/src/rooms.rs; os-ui desktop room test |
| P01.04 | Placement/enclosure diagnostics | E1 | partial | Click-to-place in a unique enclosed face; broken topology reports without blocking wall edits; room repair workflow remains limited to delete/re-place | native-rooms.md; os-geometry/tests/rooms.rs; os-ui desktop room test |
| P01.05 | Area computation rules | E1 | partial | Recomputed centerline polygon area in m², independent of crop; finish-face/net area conventions remain | native-rooms.md; os-geometry/tests/rooms.rs; os-ui desktop room test |
| P01.06 | Finish parameters | E1 | partial | Independent floor/wall/ceiling material UUIDs and finish codes; local Properties drafts commit in one UpdateRoom transaction. Reference validation, atomic removal rejection, history and frozen 34→35 migration/save-reopen. Resolved active-plan floor RGB fills respect crop and selection; unresolved rooms retain intent with no fill. Finish quantities and finish-face/net areas remain | native-rooms.md; crates/os-model/tests/room_finishes.rs; crates/os-document/tests/rooms.rs; crates/os-storage/tests/room_materials.rs; crates/os-ui/src/plan_workspace/room_materials.rs |
| P01.07 | Room/area tags | E1 | partial | Native view-owned room tags have stable UUID references, live number/name, placement/move, orphan diagnostics and history; derived area remains at room seed. General area tags, styles and print qualification remain | native-rooms.md; crates/os-ui/src/desktop_tests/room_tag_tests.rs; crates/os-model/tests/room_tags.rs; crates/os-storage/tests/room_tags.rs |
| P01.08 | Color fills/legends | E1 | partial | Active native plan paints subdued floor-material RGB inside resolved room polygons, clipped to crop beneath walls, room outlines/text/tags and selection. Live color edits refresh the fill; unresolved rooms retain codes/refs with no stale polygon/fill. Headless checks at 1280×800/100% and 1000×650/150%, including concave rings. Legends, rule-based color schemes, wall/ceiling fills and physical-print qualification remain | native-rooms.md; crates/os-ui/src/plan_workspace/room_materials.rs (room_material_plan_fill_crop_selection_live_color_and_unresolved_at_both_dpis; room_material_concave_and_large_rings_have_exact_fill_area) |
| P01.09 | Opening and linked-boundary treatment | E1 | partial | Hosted doors/windows remain boundaries through native host centerlines; linked/plugin boundary providers remain unavailable | native-rooms.md; os-ui/tests/openings.rs; os-ui desktop room test |
| P01.10 | Phase-dependent enclosures | E1 | not started | Not implemented | — |
| P01.11 | Existing/new/demolished/temporary states | E1 | partial | Persisted lifecycles and derived states; single-selection Properties authoring with Apply/Cancel/Escape, stale-draft discard and one-command undo; real-egui wall/opening/room assignment, demolition clearing, Temporary and invalid-lifetime tests pass; plan graphics are phase-filtered and opening schedules filter both opening and host; room topology/area remains unphased | native-phases.md; crates/os-model/src/phases.rs; crates/os-document/tests/phases.rs; crates/os-model/tests/schedule_phases.rs; crates/os-ui/src/desktop_tests/phase_tests.rs |
| P01.12 | Ordered phases | E1 | partial | Manage modal adds/renames/moves/deletes phases in one Apply batch with stable UUIDs, pinned first phase, reference protection and atomic renumbering; 4 focused real-egui phase tests and 62 desktop tests pass, including save/reopen, undo/redo, invalid edits, stale sessions and shortcut precedence; phase view context remains | native-phases.md; crates/os-storage/tests/phases.rs; crates/os-document/tests/phases.rs; crates/os-ui/src/desktop_tests/phase_tests.rs |
| P01.13 | Phase view filters/graphics | E1 | partial | Persisted per-plan phase/filter; native geometry/cutout/snap/pick and annotation exclusion; four status appearances shared by canvas/sheet/PDF; saved opening schedules independently phase-filter opening plus host. Separate Door/Window category visibility retains category-hidden wall cutouts, unlike phase-hidden openings. Full-model room enclosure and split 3D remain unphased. | `plan/phase_tests.rs`, `desktop_tests/plan_phase_tests.rs`, `provider_plans.rs`, storage `plan_settings.rs`, `os-ui/src/opening_schedule/csv_export/tests/phases.rs`; [evidence](native-phases.md#per-plan-phase-display-schema-45) |
| P01.14 | Phase schedules/tags | E1 | not started | Not implemented | — |
| P01.15 | Linked-phase mapping | E1 | not started | Not implemented | — |
| P01.16 | Isolated option sets | E1 | not started | Not implemented | — |
| P01.17 | Option views/schedules | E1 | not started | Not implemented | — |
| P01.18 | Option membership | E1 | not started | Not implemented | — |
| P01.19 | Accept/merge alternatives | E1 | not started | Not implemented | — |
| P01.20 | Reject cross-option references | E1 | not started | Not implemented | — |
| P01.21 | Configurable reporting conventions | E1 | not started | Not implemented | — |
| S01.01 | Door/window schedules | E2 | partial | Saved model-owned Door/Window/All definitions, named picker, ordered configurable columns and ascending sort; live derived rows, transactional instance-name and eligible dimension edits, live placed sheet tables, filtered read-only quantity summary and separate instance/quantity CSV reports. Phase target/filter is schedule-owned; existing definitions migrate legacy-unphased and new schedules pin Latest. No custom fields or pagination | `os-model/tests/schedule_phases.rs`; `os-storage/tests/schedule_phases.rs`; `os-ui/src/opening_schedule/csv_export/tests/phases.rs`; `docs/native-phases.md`; `docs/native-hosted-openings.md`; `docs/native-sheet-preview.md` |
| S01.02 | Room/finish/material schedules | E2 | partial | Saved RoomFinish definitions retain eight columns/default order, room sorts, live centerline area, blank unresolved area/status, selection/navigation and sheet/PDF output. Schema 35/API 16 adds independent material refs; each finish cell composes the live catalog name with its code. Rename/color changes, unassigned codes, shared sheet/PDF text and existing overflow checks have focused coverage. Material quantities, custom fields, CSV and pagination remain | native-material-colors.md; native-rooms.md; crates/os-ui/src/opening_schedule/rooms/tests.rs; crates/os-ui/src/opening_schedule/definitions/tests.rs; crates/os-ui/src/plan_workspace/sheet_tests.rs |
| S01.03 | Key schedules | E2 | not started | Not implemented | — |
| S01.04 | Annotation note blocks | E2 | not started | Not implemented | — |
| S01.05 | Sheet/view indexes | E2 | not started | Not implemented | — |
| S01.06 | Revision schedules | E2 | not started | Not implemented | — |
| S01.07 | Reusable legends | E2 | not started | Not implemented | — |
| S01.08 | Reusable model illustrations | E2 | not started | Not implemented | — |
| S01.09 | Typed shared/project fields | E2 | not started | Not implemented | — |
| S01.10 | Unit-aware calculated fields | E2 | not started | Not implemented | — |
| S01.11 | Filters | E2 | partial | Saved Door/Window/All category selection followed by up to 32 typed AND rules: Name/Type/Level/Host text and full-precision Width/Height/Sill metre comparisons. Rust lowercase matching without accent/Unicode normalization. Draft add/edit/remove/reorder, validation/history, persistence and live schedule-window/sheet/PDF propagation; RoomFinish filters and OR/nested predicates remain | `crates/os-model/tests/schedule_filters.rs`; `crates/os-storage/tests/schedule_filters.rs`; `crates/os-ui/src/opening_schedule/definitions/tests/filters.rs`; `crates/os-ui/src/plan_workspace/sheet_tests/filters.rs` |
| S01.12 | Sort/group | E2 | partial | Saved Door/Window/All support up to two distinct ordered keys (Level, Kind, Type identity, Width, Height, Sill), exact effective numbers and deterministic ordering with saved instance sort/UUID ties inside groups. Shared summary in UI, sheet/vector-PDF and quantity CSV; empty keys preserve existing behavior. No descending sort or RoomFinish grouping | `selected_keys_exact_values_identity_legacy_and_stable_order`; `grouping_definition_save_cancel_stale_and_one_step_undo` |
| S01.13 | Totals | E2 | partial | Read-only group counts, first-key subtotals for two keys and grand count after saved category/filters, including zero for empty grouped results. Same-name UUIDs and Legacy kinds remain distinct for Type grouping. No arbitrary subtotals, dimension sums, RoomFinish grouping or full Revit parity | `filtered_committed_rows_and_paper_pdf_share_counts_and_report_overflow`; `configurable_summary_ui_paper_csv_parity_and_instance_bytes_unchanged`; `docs/native-hosted-openings.md` |
| S01.14 | Conditional formatting | E2 | not started | Not implemented | — |
| S01.15 | Key-driven defaults | E2 | not started | Not implemented | — |
| S01.16 | Linked-model rows | E2 | not started | Not implemented | — |
| S01.17 | Phase/option/view/sheet filters | E2 | partial | Saved opening schedules choose pinned or dynamic-Latest target plus five lifecycle filters; opening and host must both match, with common UI/sheet/PDF/instance-CSV/quantity-CSV rows. Plan phase filters remain view-owned and do not implicitly filter schedules. Design options and linked-model phase mapping remain unimplemented | `os-model/tests/schedule_phases.rs`; `os-storage/tests/schedule_phases.rs`; `os-ui/src/opening_schedule/csv_export/tests/phases.rs`; `docs/native-phases.md` |
| S01.18 | Transactional editable cells | E2 | partial | Existing instance-name action plus saved Door/Window schedule Width/Height and Window Sill cell edits submit one validated `UpdateOpening`; typed values pin/reset inheritance, legacy values update in place, drafts cancel on stale context and committed values flow to live sheet/PDF rows. All-category/RoomFinish and other nonnumeric cells plus Door sill remain read-only | `opening_schedule.rs` (`numeric_schedule_cell_pointer_edit_opens_editor_and_enter_commits`, `typed_schedule_dimensions_pin_equal_inherit_and_follow_type_edits`, `legacy_window_schedule_edits_fields_and_door_sill_is_suppressed`, `invalid_numeric_edit_rolls_back_retains_draft_and_allows_correction`, `numeric_edit_cancel_escape_and_stale_schedule_or_session_never_commit`); `plan_workspace/sheet_tests.rs` (`saved_schedule_sheet_live_preview_pdf_history_overflow_and_stale_export`) |
| S01.19 | Read-only computed cells | E2 | partial | Room area/enclosure status, type dimensions, host level/wall and stable ID are derived from the current model and cannot be edited in the schedule | crates/os-ui/src/opening_schedule/rooms/tests.rs; `os-ui::opening_schedule::tests::resolved_rows_follow_type_history_and_archive_without_cached_data` |
| S01.20 | Source selection/highlighting | E2 | partial | Native opening and room schedule cells select source and navigate to a source-level plan; missing-plan guidance retains selection; existing crop/visibility controls highlighting | crates/os-ui/src/opening_schedule/rooms/tests.rs; `row_click_selects_and_navigates_without_model_or_history_changes` at 1280×800/100% and 1000×650/150% |
| S01.21 | Schedule undo/validation | E2 | partial | Schedule definition create/configure/delete, instance-name edits and eligible opening dimension edits use validated document transactions; invalid drafts retain values without partial changes, no-op edits add no history, and Escape/Cancel/stale session/revision discard drafts | `os-document/tests/schedules.rs`; `os-ui::opening_schedule::definitions::tests`; `os-ui::opening_schedule::tests` |
| S01.22 | Column/header/border styling | E2 | not started | Not implemented | — |
| S01.23 | Repeated headings | E2 | not started | Not implemented | — |
| S01.24 | Split tables | E2 | not started | Not implemented | — |
| S01.25 | Pagination after row changes | E2 | not started | Not implemented | — |
| S01.26 | CSV encoding/units/loss reports | E2 | partial | Flat instance CSV bytes/order unchanged by grouping. Quantity CSV shares exact readable group/subtotal/grand labels and counts with UI/paper; empty grouped output has grand zero, empty ungrouped output is header-only. UTF-8, CRLF, quoting, formula mitigation, 16 MiB cap and preview/cancel/stale/no-clobber/explicit replacement remain. No CSV import, pagination, lossless interchange, RoomFinish or live-All export | `os-ui::opening_schedule::csv_export::tests`; `docs/native-hosted-openings.md` |
| S02.01 | Custom title blocks | E3 | not started | Not implemented | — |
| S02.02 | Custom sheet sizes | E3 | not started | Not implemented | — |
| S02.03 | Project/sheet metadata | E3 | partial | Persisted sheet number/name and basic title block; project metadata, custom title blocks and editable issue metadata remain | native-sheet-preview.md; os-model/src/sheets.rs; os-ui/src/plan_workspace/sheet_tests.rs |
| S02.04 | Unique drawing numbers | E3 | automated-tested | Trimmed ASCII case-insensitive uniqueness is model-validated; UI assigns the next free A-series number; numbering policy and atomic renumbering remain | native-sheet-preview.md; os-model/src/sheets.rs; os-model validation tests |
| S02.05 | Placeholder sheets | E3 | not started | Not implemented | — |
| S02.06 | Sheet browser organization | E3 | partial | Basic flat sheet picker; folders, sorting, filtering and index views remain | native-sheet-preview.md; os-ui/src/plan_workspace.rs |
| S02.07 | Sheet indexes | E3 | not started | Not implemented | — |
| S02.08 | Batch sheet edits | E3 | not started | Not implemented | — |
| S02.09 | Place/align/rotate scaled viewports | E3 | partial | One linked Plan plus one Section on A3 with independent numeric scale/center, overlap rejection, undo/redo and save/reopen; single-source sheets retained; rotation and graphical placement remain | native-sheet-preview.md; os-ui/src/plan_workspace/sheet_tests/combined.rs; os-render/src/sheet.rs |
| S02.10 | Viewport cropping | E3 | partial | Source Plan/Section bounds and each independent viewport rectangle are clipped in the shared preview/PDF page; independent crop editing remains | native-sheet-preview.md; os-render/src/sheet.rs |
| S02.11 | Viewport titles | E3 | partial | Source view name or persisted title override appears with the scale; title editing controls and full annotation styles remain | native-sheet-preview.md; os-model/src/sheets.rs; os-render/src/sheet.rs |
| S02.12 | Guide grids | E3 | not started | Not implemented | — |
| S02.13 | Mixed scales | E3 | partial | Two linked viewports with independent 1:50/1:100 paper mapping and clipping tested; shared preview/vector-PDF page; external viewer and physical plot qualification remain | native-sheet-preview.md; os-render/src/sheet.rs; os-ui/src/plan_workspace/sheet_tests/combined.rs |
| S02.14 | Sheet schedules/legends | E3 | partial | One saved door/window or Room Finish schedule table with explicit combined layout, live rows, searchable preview/PDF marks, fit diagnostics and undoable Add/Remove; no numeric draft editor, legends, multiple-table UI or pagination | crates/os-ui/src/plan_workspace/sheet_tests.rs (room_finish_sheet_live_area_preview_pdf_and_explicit_overflow_at_both_dpis); `os-ui::plan_workspace::sheet_tests::saved_schedule_sheet_live_preview_pdf_history_overflow_and_stale_export`; `os-render::sheet::tables::tests`; `os-storage/tests/sheet_tables.rs`; `docs/native-sheet-preview.md` |
| S02.15 | Sheet duplication/view-copy rules | E3 | not started | Not implemented | — |
| S02.16 | Automatic cross-references | E3 | not started | Not implemented | — |
| S02.17 | Placed/unplaced status | E3 | not started | Not implemented | — |
| S02.18 | Sheet/source navigation | E3 | partial | Flat sheet picker and separate linked Plan/Section source actions; combined-sheet return activates checked Plan and background Section; cross-reference navigation remains | native-sheet-preview.md; os-ui/src/plan_workspace/sheet_tests/combined.rs |
| S02.19 | Atomic renumbering | E3 | not started | Not implemented | — |
| S02.20 | Ordered sheet sets | E3 | not started | Not implemented | — |
| S02.21 | Package grouping | E3 | not started | Not implemented | — |
| S02.22 | Issue/revision metadata | E3 | not started | Not implemented | — |
| S02.23 | Superseded/withdrawn sheet history | E3 | not started | Not implemented | — |
| I01.01 | Revision sequences | E3 | not started | Not implemented | — |
| I01.02 | Project/sheet numbering policy | E3 | not started | Not implemented | — |
| I01.03 | View/sheet revision clouds | E3 | not started | Not implemented | — |
| I01.04 | Revision tags | E3 | not started | Not implemented | — |
| I01.05 | Revision descriptions/dates/recipients | E3 | not started | Not implemented | — |
| I01.06 | Revision schedules | E3 | not started | Not implemented | — |
| I01.07 | Revision inclusion/exclusion | E3 | not started | Not implemented | — |
| I01.08 | Review/approve/issue transitions | E3 | not started | Not implemented | — |
| I01.09 | Immutable issued revisions | E3 | not started | Not implemented | — |
| I01.10 | Explicit superseding issues | E3 | not started | Not implemented | — |
| I01.11 | Frozen model/view/template/link/plugin publish snapshot | E3 | not started | Not implemented | — |
| I01.12 | Issue drawing/revision manifest | E3 | not started | Not implemented | — |
| I01.13 | File hashes and publish settings | E3 | not started | Not implemented | — |
| I01.14 | Recorded warnings/acknowledgments | E3 | not started | Not implemented | — |
| I01.15 | Immutable issued artifact archive | E3 | not started | Not implemented | — |
| I01.16 | Historic issue reproduction | E3 | not started | Not implemented | — |
| I01.17 | Broken-reference preflight | E3 | not started | Not implemented | — |
| I01.18 | Stale-result preflight | E3 | not started | Not implemented | — |
| I01.19 | Missing font/link/plugin preflight | E3 | not started | Not implemented | — |
| I01.20 | Duplicate sheet number preflight | E3 | not started | Not implemented | — |
| I01.21 | Unplaced required view preflight | E3 | not started | Not implemented | — |
| I01.22 | Temporary-state preflight | E3 | not started | Not implemented | — |
| I01.23 | Dimension-override preflight | E3 | not started | Not implemented | — |
| I01.24 | Clipping/overflow preflight | E3 | not started | Not implemented | — |
| I01.25 | Blocking scale/geometry/reference faults | E3 | not started | Not implemented | — |
| X01.01 | Vector-first PDF | E3 | partial | Confirmed atomic single-page vector PDF from the shared paper-space drawing; fixed A3 landscape only and independent viewer/physical-scale checks remain | native-sheet-preview.md; os-render/src/sheet.rs; os-ui/src/plan_workspace/sheet_tests.rs |
| X01.02 | Explicit raster fallback | E3 | not started | Not implemented | — |
| X01.03 | Searchable lawful embedded text | E3 | partial | Helvetica/WinAnsi text is searchable and unsupported characters fail closed; fonts are not embedded and licensing review remains | native-sheet-preview.md; os-render/src/sheet.rs |
| X01.04 | Font substitution | E3 | not started | Not implemented | — |
| X01.05 | PDF weights/fills/transparency | E3 | partial | Basic vector weights and fills are emitted; transparency and office style mapping remain | native-sheet-preview.md; os-render/src/sheet.rs |
| X01.06 | PDF hyperlinks/bookmarks | E3 | not started | Not implemented | — |
| X01.07 | Page boxes/orientation/mixed sizes | E3 | partial | One A3 landscape MediaBox is tested; alternate page boxes and mixed orientations/sizes remain | native-sheet-preview.md; os-render/src/sheet.rs; os-ui/src/plan_workspace/sheet_tests.rs |
| X01.08 | Single/combined exports | E3 | not started | Not implemented | — |
| X01.09 | Saved ordered export sets | E3 | not started | Not implemented | — |
| X01.10 | Parameter-based output naming | E3 | not started | Not implemented | — |
| X01.11 | Background export progress/cancel | E3 | not started | Not implemented | — |
| X01.12 | Atomic complete-package publishing | E3 | partial | One PDF is written through a synced same-directory temporary and no-clobber/explicit-replace path; multi-sheet package publishing remains | native-sheet-preview.md; os-ui/src/plan_workspace/sheet_tests.rs |
| X01.13 | Native platform printing | E3 | not started | Not implemented | — |
| X01.14 | Print size/margins/orientation | E3 | not started | Not implemented | — |
| X01.15 | Print color/monochrome | E3 | not started | Not implemented | — |
| X01.16 | Print scale and preview | E3 | not started | Not implemented | — |
| X01.17 | Fit-to-page warning | E3 | not started | Not implemented | — |
| X01.18 | PDF/image underlays | E3 | not started | Not implemented | — |
| X01.19 | Underlay page/calibration/rotation/crop/reload | E3 | not started | Not implemented | — |
| X01.20 | Vector underlay snapping | E3 | not started | Not implemented | — |
| X01.21 | Raster limitation diagnostics | E3 | not started | Not implemented | — |
| X01.22 | Declared DWG versions/entities | E3 | not started | Not implemented | — |
| X01.23 | Declared DXF versions/entities | E3 | not started | Not implemented | — |
| X01.24 | CAD layers/colors/line types/hatches | E3 | not started | Not implemented | — |
| X01.25 | CAD blocks/text/font mapping | E3 | not started | Not implemented | — |
| X01.26 | CAD units/coordinates/paper/model space | E3 | not started | Not implemented | — |
| X01.27 | CAD external-reference packaging | E3 | not started | Not implemented | — |
| X01.28 | Independent CAD validation | E3 | not started | Not implemented | — |
| X01.29 | Native/IFC/CAD live links | E3 | not started | Not implemented | — |
| X01.30 | Relative link paths/shared coordinates | E3 | not started | Not implemented | — |
| X01.31 | Pin/reload/unload/replace links | E3 | not started | Not implemented | — |
| X01.32 | Nested-link/cycle policy | E3 | not started | Not implemented | — |
| X01.33 | Link visibility/missing diagnostics | E3 | not started | Not implemented | — |
| X01.34 | Portable linked packages | E3 | not started | Not implemented | — |
| X01.35 | Stable references across reload | E3 | not started | Not implemented | — |
| X01.36 | Snapshot versus live-link versions | E3 | not started | Not implemented | — |
| X01.37 | Translator licensing/redistribution approval | E3 | not started | Not implemented | — |
| X01.38 | Two independent PDF viewers | E3 | not started | Not implemented | — |
| X01.39 | Physical plot scale measurement | E3 | not started | Not implemented | — |
| X01.40 | Permissioned third-party roundtrip fixtures | E3 | not started | Not implemented | — |
| T01.01 | Single-writer ownership locks | E3 | not started | Not implemented | — |
| T01.02 | Lock expiry policy | E3 | not started | Not implemented | — |
| T01.03 | Read-only open | E3 | not started | Not implemented | — |
| T01.04 | External-change detection | E3 | not started | Not implemented | — |
| T01.05 | Deliberate writer handoff | E3 | not started | Not implemented | — |
| T01.06 | Recoverable backups | E3 | not started | Not implemented | — |
| T01.07 | Self-hosted worksharing ADR | E3 | not started | Not implemented | — |
| T01.08 | Editor identity | E3 | not started | Not implemented | — |
| T01.09 | Ownership/borrowing | E3 | not started | Not implemented | — |
| T01.10 | Atomic synchronization | E3 | not started | Not implemented | — |
| T01.11 | Conflict resolution | E3 | not started | Not implemented | — |
| T01.12 | Team permission checks | E3 | not started | Not implemented | — |
| T01.13 | Audit history | E3 | not started | Not implemented | — |
| T01.14 | Recoverable local work | E3 | not started | Not implemented | — |
| T01.15 | Concurrent model/annotation/view/sheet/content edits | E3 | not started | Not implemented | — |
| T01.16 | Host/dependent conflicts | E3 | not started | Not implemented | — |
| T01.17 | Undo after another user sync | E3 | not started | Not implemented | — |
| T01.18 | Interrupted-connection recovery | E3 | not started | Not implemented | — |
| T01.19 | Stale ownership recovery | E3 | not started | Not implemented | — |
| T01.20 | Crash/restart/central restore | E3 | not started | Not implemented | — |
| T01.21 | Version mismatch handling | E3 | not started | Not implemented | — |
| T01.22 | Federation distinct from shared editing | E3 | not started | Not implemented | — |
| T01.23 | Read-only review | E3 | not started | Not implemented | — |
| T01.24 | Portable project/issue archives | E3 | not started | Not implemented | — |
| T01.25 | Offline installer | E3 | not started | Not implemented | — |
| T01.26 | Qualified OS/hardware matrix | E3 | not started | Not implemented | — |
| T01.27 | Controlled upgrades | E3 | not started | Not implemented | — |
| T01.28 | Project-preserving uninstall | E3 | not started | Not implemented | — |
| T01.29 | Admin content/plugin versions | E3 | not started | Not implemented | — |
| T01.30 | Keyboard shortcuts | E3 | partial | Existing desktop keyboard/DPI checks; not full platform qualification | os-ui desktop_tests; docs/ui-design-guide.md |
| T01.31 | High-DPI/multi-monitor qualification | E3 | partial | Existing desktop keyboard/DPI checks; not full platform qualification | os-ui desktop_tests; docs/ui-design-guide.md |
| T01.32 | Accessible focus/contrast | E3 | partial | Existing desktop keyboard/DPI checks; not full platform qualification | os-ui desktop_tests; docs/ui-design-guide.md |

## Cross-cutting architecture and acceptance obligations

| ID | Requirement | Status | Evidence / dependency |
| --- | --- | --- | --- |
| ARCH.01 | Persistent object distinctions and stable schemas/dependencies | partial | Model schema 4 adds building-scoped straight grid entities with validated references, commands/history and frozen migration tests; typed plan settings remain version 1. See architectural-grids.md; native grid tools and drafting/content/library distinctions remain |
| ARCH.02 | Model/view/paper coordinate transforms and tolerance policy | partial | Horizontal world/view/screen transforms and pan/zoom tested; paper transforms and production precision qualification remain |
| ARCH.03 | Shared deterministic semantic 2D representation | partial | Bounded UUID-bearing polygon/line IR, room faces/labels and matching room picking; screen/output adapters, curves and style system remain |
| ARCH.04 | Cut/projected/symbolic graphics and override precedence | partial | Native prism cut/projected/depth roles and deterministic ordering; symbolic/category/template behavior remains |
| ARCH.05 | Revision/style/font/content/plugin/link-aware cache keys | partial | Native plan consumption checks session/model/view/settings and derives room faces from revision-bound same-level walls; style/font/content/plugin/link participation remains |
| ARCH.06 | Targeted regeneration and bounded background work | partial | Opt-in Editor uses bounded v2 workers and invalidates plugin meshes conservatively across revisions; transitive dependencies are tested, but targeted plugin caching, visible desktop controls and plan regeneration remain |
| ARCH.07 | Discard stale document/view replies | partial | V1/v2 Wasm worker session/model/view and generation checks tested in wasm_transport.rs and generic_commands.rs; desktop/plan pipeline not yet integrated |
| ARCH.08 | Frozen validated issue snapshots | not started | Existing model/transaction foundations do not establish this full requirement |
| ARCH.09 | Transactional annotation/sheet/content/schedule edits | partial | Saved schedule definitions, opening instance-name/dimension edits and schedule-table placement/removal use validated undoable document commands; annotation/content transactions and full runtime acceptance remain | `os-document/tests/schedules.rs`; `os-storage/tests/sheet_tables.rs`; `os-ui/src/plan_workspace/sheet_tests.rs` |
| ARCH.10 | Unknown plugin payload preservation and migration | partial | Model/storage extension tests, frozen envelope fixture, read-only UI, atomic core migration proposals; plugin-owned executable migration dispatch and full B/C installation acceptance remain |
| ARCH.11 | Callable content/tag/snap/graphics/schedule/export/preflight providers | not started | Existing model/transaction foundations do not establish this full requirement |
| ARCH.12 | Independent Rust SDK and Wall/column installation workflow | partial | Separately locked Rust Wasm Wall and column copied to installation directories; probes verify descriptors, checked worker geometry, native Wall/extension create/edit/delete/history and absent-plugin reopen. See native-wall-plugin.md. General SDK packaging and complete installed desktop authoring acceptance remain |
| ARCH.13 | Plugin deadline/cancellation/disable lifecycle | partial | Worker cancellation/result deadlines and unload/reload tested; compiler/desktop manager integration and full runtime acceptance missing |
| ARCH.14 | 2D plugin pointer/work-plane/preview/cancel/commit contracts | partial | plugin-plan-contract-design.md specifies context/gesture/snap/graphics validation and D acceptance; no callable 2D provider or wire DTO yet |
| ARCH.15 | Two-wall snapped exact-input split-view acceptance | partial | Bundled-provider native 4 m/3 m perpendicular snapped walls, cancellation, save/reopen and shared selection verified; independent plugin route and broader D workflow remain (native-pointer-walls.md) |
| ARCH.16 | Cross-view shared identity/selection and single-action undo | partial | Native room UUID is shared by plan/browser selection and each room edit is one history action; 3D room graphics and broader cross-view categories remain |
| ARCH.17 | Persist plan cut ranges and verify late inactive-view replies | not started | Existing model/transaction foundations do not establish this full requirement |

## Production release gates

| Gate | Status | Missing required evidence |
| --- | --- | --- |
| G1 | not started | New multi-storey and renovation reference projects; create/check/issue/revise/archive native workflows; no architect sign-off required |
| G2 | partial | Existing prism/IFC/depth tests only; architectural geometry, annotations, independent outputs, physical plots and approved tolerances missing |
| G3 | partial | Atomic save/corruption tests exist; checkpoint/restore UI, crash/disk/network/sync/publish failure matrix and clean-machine archive tests missing |
| G4 | not started | Approved hardware/project budgets, p95 measurements, memory bounds, five eight-hour scripted sessions per profile |
| G5 | not started | Agreed topology/concurrency, conflict/restore and declared CAD/PDF/plot/second-workstation checks |
| G6 | partial | Unsafe lint, bounded Wasm spike, dependency audit; expanded fuzzing, dependency redistribution review, installer integrity, SDK/support/maintenance owner and full documentation remain |

Current hosted-opening evidence increment: schema 50→51 adds nullable per-instance
Start/End side-lite inheritance/pinning. `lite_handedness_tests.rs` covers Door
and Window plan flips, draft-only properties, commit/undo/redo, and rendering at
both tested display profiles. The plan opening/endpoint tests cover copy,
rehost, and mirror preservation. Storage migration and model validation
evidence is listed in `native-hosted-openings.md`.

## Work sequence and next evidence

1. Complete B/C without enabling data-losing external elements: preservation, versioned generic SDK including 2D contexts, worker lifecycle, extension envelopes and independently built examples.
2. Complete D's actual plan/split-view workflow and independent 2D plugin acceptance before E1–E4.
3. Implement and verify every E1–E4 capability and G1–G6 gate; resolve owner profile/license/translator/support choices before qualification or publication.

[Opaque file decision](decisions/0007-opaque-container-files.md) addresses container bytes; [semantic envelope decision](decisions/0009-semantic-extension-envelopes.md) now adds bounded plugin payload preservation. Keep source-backed evidence current as each workflow lands. Initial classifications above are conservative and do not replace requirement-by-requirement completion audits.
