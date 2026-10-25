# Native model groups

This is the first native plan-workspace group workflow. It is deliberately
narrower than Revit: groups currently root at native straight walls on one
level, with doors, windows, joins, wall-type assignments, clearance locks and
active-plan opening tags carried as dependencies.

## Model and transaction semantics

A `GroupDefinition` owns a typed canonical template. Its wall, hosted opening,
join and active-plan opening-tag template IDs are stable slots, not live model
entity IDs. Every `GroupInstance` maps those slots to the ordinary live entity
UUIDs used by geometry, schedules, references and existing editing commands.
Model validation enforces a one-to-one mapping, unique identities, valid
dependencies and exact equality with materialization from the shared
definition. Template-slot relationships are scoped to their own definition;
they cannot alias another definition or enter the live identity namespace.

The document transaction commands are:

- `CreateWallGroup`: capture 1–64 selected same-level straight walls, hosted
  doors/windows, complete wall joins, active-plan opening tags, wall-type
  assignments and opening-clearance locks. Existing member UUIDs are retained.
- `DuplicateGroupInstance`: create fresh live member identities and apply a
  translation from the source instance's current placement. The definition is
  shared; phase lifecycles remain per live member.
- `SetGroupInstanceTransform`: apply an absolute rigid XY transform from the
  canonical template, so repeated edits do not accumulate numeric drift.
- `UpdateGroupDefinition`: propagate an edited template to every instance in
  one validated transaction. Slot IDs and member categories must remain stable;
  optional type assignments and clearance locks can be added or removed.
- `MakeGroupInstanceUnique`: fork one instance to a new independent definition.
- `UngroupInstance`: remove group ownership while retaining all member model
  elements; an unused definition is removed automatically. The workspace leaves
  the released wall roots selected.

## Plan workflow

On a native FloorPlan, select one or more visible, ungrouped straight walls on
the same level and enter a name to create a group. Picking a wall, hosted door
or window, or active-plan opening tag selects the owning group; Tab and marquee
selection normalize and deduplicate members. Group members do not expose
individual editing grips or accept direct wall moves/resizes, and new doors or
windows cannot be hosted on grouped walls.

The group toolbar supports Duplicate group, Make unique and Ungroup. Duplicate
uses the native wall snap query and a transient 2D preview with stable member
IDs; the document/history and 3D scene remain unchanged until release. Geometry,
opening panels, collisions and existing room boundaries are preflighted before
one transaction. The installed Wall provider route is rejected. Preview is
canceled when its document, selection, provider, view, camera, canvas or
selection filters become stale. Ordinary canvas dragging still pans.

Failed operations leave the model, history, revision and events unchanged.
Successful operations are one undo/redo step, and changed-member UUIDs are
included in the document event for downstream invalidation. Member lifecycle
state is per-instance metadata, not shared by a definition.

## Persistence

Schema 81 adds typed `group_definitions` and `group_instances` model maps.
Storage migration 80→81 initializes both maps and advances native headers;
existing group-free files retain their model data and opaque extension payloads.

## Deliberate limits

This increment does not provide generic groups for floors, ceilings, roofs,
columns, casework, stairs, ramps, railings, curtain systems, rooms or annotation
sets. It does not group rooms, dimensions, other-view tags, or arbitrary
dependencies. All walls must be native straight walls on one level. Instance
transforms are rigid XY only; moving between levels, vertical transforms,
mirroring, nested groups, arrays with shared definitions, and editable
constraint graphs are not implemented. Existing wall/opening members cannot
be edited independently while grouped; make the instance unique or ungroup it
first. Grouped walls cannot accept new hosted openings. Individual member
property editing, editable shared definitions, transform/rotation handles,
other-root element groups, group dialogs and the broader Revit interaction
model remain future work.

## Evidence

- `cargo test -p os-model --all-features --lib --locked --offline` — 69 passed.
- `cargo test -p os-document --all-features --locked --offline` — passed,
  including all 6 group acceptance tests.
- `cargo test -p os-storage --all-features --locked --offline` — passed; the
  focused group migration suite has 4 tests.
- `cargo test -p os-storage --test group_migration --locked --offline` — 4 passed.
- `cargo check -p os-model -p os-storage --all-features --locked --offline` — passed.
- `cargo check -p os-ui --all-features --locked --offline` — passed.
- Plan-workspace group acceptance covers group selection, duplicate preview,
  collision/room preflight, cancellation/staleness, pan, one-step history and
  ungrouped wall-root selection at both DPI profiles.
- `cargo clippy -p os-model -p os-document --lib --all-features --locked --offline -- -D warnings` — passed.
- Scoped `rustfmt --check` for the isolated group implementation/test files —
  passed; the already-dirty plan-workspace module graph has formatting drift and
  was not bulk-reformatted.

The focused tests cover door/window capture, stable IDs, opening hosting,
complete joins, rigid transforms, shared updates, make-unique isolation,
ungroup, invalid-transaction rollback, history, schema migration and template
identity/metadata validation. The workspace test command reaches `os-ui` but
fails there (552 passed, 41 failed, 7 ignored); a serial `os-ui` rerun reports
554 passed, 39 failed, 7 ignored. All group UI tests pass in both runs; the
failures are in other opening, tag, snapping and endpoint suites. Strict
`os-ui` Clippy is blocked by existing warnings in storage, render and other UI
code; it passes with those four existing lint classes allowed. Native-window
visual QA has not been performed.
