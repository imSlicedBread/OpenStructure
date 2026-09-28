# Native project phases and element lifecycles

Model schema 44 and native Model API 25 introduced the persisted phase/lifecycle
foundation. Each project starts with ordered `Existing` and `New Construction`
phases, referenced by stable UUID. Walls, hosted openings, floors, stairs,
roofs, ceilings, columns, rooms and room-separation lines can record a creation
phase and optional demolition phase. New document-command additions default to
the latest phase. Lifecycle changes are ordinary atomic document commands and
participate in undo/redo.

The model derives `Existing`, `New`, `Demolished`, `Temporary`, `Future` and
`PreviouslyDemolished` relative to a target phase. A first-phase element is
Existing; an element created in a later target phase is New. Same-phase creation
and demolition is Temporary. Native schema-43 files migrate atomically: every
existing phaseable element is assigned to Existing, preserving their visible
legacy behavior. The generated phase IDs are deterministic per project so
reopening an unsaved legacy file does not change identity. Strict validation
checks phase name/identity/order bounds, lifecycle references/order, and
explicit hosted-opening lifetimes against their wall. Generic plugin API 2 and `.osb`
container 2 are unchanged. Current full-model native plugins require API 32/schema 51.

Focused evidence:

- `crates/os-model/src/phases.rs` covers ordering, status derivation and invalid
  phase references.
- `crates/os-document/tests/phases.rs` covers latest-phase defaults, atomic
  invalid edits, lifecycle changes and undo/redo.
- `crates/os-storage/tests/phases.rs` covers schema-43-shaped migration,
  preservation and atomic rejection of ambiguous phase fields. Its fixture is
  assembled from a current model and downgraded, rather than stored as a frozen
  historical JSON artifact.
- IFC export reports custom phase/lifecycle loss and requires explicit
  acknowledgement.

## Desktop authoring

Manage → Manage phases opens an ordered, local phase draft. Add phase appends
at the end; selecting a phase exposes its name, adjacent Move up/Move down,
and Delete phase. Apply phases commits the draft as one document transaction
and one undo/redo step. An adjacent swap uses two `UpdatePhase` commands;
deleting an unused middle phase includes the required order renumbering in
the same batch. UUIDs remain stable. The first phase is pinned because
elements without an explicit lifecycle fall back to that phase. Referenced
phases cannot be deleted, and lifecycles are never silently remapped.

For a single selected phaseable element, expand Properties → Phasing to edit
Created in and optional Demolished in (`None` clears demolition). Same-phase
creation and demolition displays Temporary. Apply lifecycle submits exactly
one `SetElementLifecycle` command. Cancel lifecycle or Escape discards local
edits; selection, document revision, and session changes also discard them.
Invalid names, capacity, order, and element/host lifetimes leave the committed
model, revision, and history unchanged. Validation errors remain in the current
draft for correction.

Cancel phases or Escape discards the manager draft. A session/revision change
automatically closes it and reports that its edits were discarded. The modal
consumes keyboard input before document/plan shortcuts, including on the frame
Escape closes it. These controls do not change the schema or plugin protocol.

Authoring evidence run for this increment:

- `cargo test -p os-ui --lib desktop_tests::phase_tests -- --nocapture`:
  4 passed. Real egui input covers add/rename/adjacent moves/unused middle
  deletion, stable IDs, native save/reopen and single-step undo/redo; first and
  referenced phase deletion, invalid names/capacity/reorder; wall, hosted opening
  and room demolition assignment/clearing, Temporary and invalid lifetimes;
  stale selection/revision/session drafts, Cancel/Escape, and modal shortcut
  precedence with both undo and redo entries available. The phase-management
  persistence flow runs at 1280×800/1× and 1000×650/1.5×.
- `cargo test -p os-ui --lib -- --test-threads=1`: 287 passed, including the
  four phase tests and existing desktop interaction tests.
- `cargo test --workspace -- --test-threads=1`: passed, including doctests.
- `cargo clippy --workspace --all-features --all-targets -- -D warnings` and
  `cargo fmt --all -- --check`: passed.

## Per-plan phase display (schema 45)

Plan settings now includes Target phase and Phase filter in the existing local
draft. Apply makes one `UpdateView` transaction, with the existing revision/session
checks and undo/redo. Newly created floor/RCP views and migrated older plans pin
the latest project phase. The explicit `Latest phase` option follows future phase
changes. Deleting a phase referenced by a pinned plan is rejected atomically.

Version-3 settings strictly persist both `target_phase` and `phase_filter`.
The explicit 44→45 migration rejects partially upgraded data and preserves all
other plan settings, metadata and opaque plugin content. Existing schema-2 frozen
plan fixtures continue through the migration chain; the isolated schema-44 test
uses a deliberately downgraded migrated document, not a frozen schema-44 artifact.

Before native geometry is derived, visibility includes the chosen filter's
Existing/New/Demolished/Temporary statuses and excludes Future and
PreviouslyDemolished. This covers walls, doors/windows, floors, stairs, roofs,
ceilings, columns, rooms and room separators. Hidden openings are removed from
wall cutout inputs; openings cannot outlive a hidden host in the drawing. Joins
to hidden walls are excluded from the view geometry. Room/opening tags and
dimensions with phase-hidden references are omitted. Grids, detail lines,
section markers and generic plugin graphics retain their existing view behavior.
Door and Window category switches are separate from this lifecycle filter:
category-hidden openings keep their host-wall cutouts but omit opening symbols
and dependent plan annotations. A phase-hidden opening still removes its host
cutout in the view-only model. Plan category visibility does not affect saved
schedule rows.

The shared `PlanDrawing` resolves subdued gray Existing, dark blue New, dashed
muted red Demolished and dashed purple Temporary linework. Category weights are
retained where defined. Phase color/dashes override linked/local graphics;
selection remains a transient overlay. Canvas, sheet preview and vector PDF
consume these same appearances, including stair tread footprints and arrows,
columns, ceilings, room outlines and room separators. Stair items are flattened
into the common native footprint/line collections by `with_stairs`.

The existing `PlanContext` revision identity rejects stale geometry and provider
results. Provider authority is checked again when results are consumed. No
additional context fields, generic protocol changes or container changes were needed.

Focused evidence for this slice:

- `crates/os-ui/src/plan/phase_tests.rs`: all six statuses/five filters, all native
  categories, independent views, opening cutout/host/annotation parity, snapping,
  picking, stale snapshots, native/provider precedence, explicit phase-reference
  rejection, one-step history, save/reopen and stair sheet/PDF stroke identity.
- `crates/os-ui/src/desktop_tests/plan_phase_tests.rs`: real egui settings, drafts,
  stale edits, history, canvas/paper style parity, committed stair arrow/tread
  colors and unchanged split 3D, at 1280×800/100% and 1000×650/150%.
- `crates/os-ui/tests/provider_plans.rs`: phase edits reject prepared results;
  generic graphics remain unphased and unload invalidates composed drawings.
- `crates/os-storage/tests/plan_settings.rs`: migration defaults/preservation,
  missing/invalid/ambiguous phase fields, invalid references, file preservation
  on rejected open, and native save/reopen.

Room enclosure topology remains based on the full model: phase filtering changes
room visibility and appearance but does not reconstruct room boundaries or areas.
Room-bound ceiling resolution shares that limitation. Split 3D remains the full
committed model.

## Phase-aware opening schedules (schema 48)

Door, Window and All saved opening schedules own their phase target and filter;
they do not inherit a plan or sheet view's crop, range, category visibility or
phase settings. New schedules pin the current latest phase with ShowAll. A saved
schedule can instead pin any phase by UUID or follow Latest dynamically, and can
select ShowAll, ShowExisting, ShowNew, ShowDemolished or ShowTemporary. A row is
eligible only when both its opening and host wall pass the selected filter at the
target phase. This prevents a schedule from reporting an opening whose host is
not present in that phase.

Schema 48 adds the required `ScheduleParams.phase`. The 47→48 migration marks all
existing saved schedules `LegacyUnphased`, preserving their previous report rows
and bytes rather than silently changing issued schedules. Configure explicitly
switches those definitions to phase-aware behavior. RoomFinish remains unphased
because room enclosure/area is not phase-aware. A pinned phase is a model
reference; deleting it is rejected atomically. Latest resolves by phase ordering
each time rows are derived, while pinned identity survives reorder. UI, placed
sheet tables, PDF, instance CSV and quantity CSV share the same eligible rows.

Evidence: `os-model/tests/schedule_phases.rs` covers all six derived statuses,
all five filters, pinned-vs-Latest identity and UUID resolution;
`os-storage/tests/schedule_phases.rs` covers frozen schema-47 migration,
save/reopen, strict rejection, pinned-phase deletion and atomic history;
`os-ui/src/opening_schedule/csv_export/tests/phases.rs` checks opening/host
status disagreements and row parity across UI-derived rows, sheets, PDF and both
CSV reports, including legacy bytes and empty output. UI draft controls and
save/cancel/stale/undo have focused egui tests. Design options and linked-model
phase mapping remain deferred. P01 remains partial, not production-complete.
Migrations also remain bounded by the native 64 MiB serialized-model limit;
legacy files whose added lifecycle records exceed that limit are rejected
atomically rather than opened in a state that cannot be saved.
