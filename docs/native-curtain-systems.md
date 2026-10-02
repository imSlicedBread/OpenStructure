# Native curtain systems (partial)

Curtain systems now have persisted native topology, derived geometry, native
plan placement, assembly picking, and a staged selected-assembly editor for
properties, numeric grids, and component assignments. This is an incremental
workflow, not production acceptance or Revit-level curtain-wall parity.

## Persisted model and identity

A straight assembly stores its level, start/end baseline, base offset, height,
normal flip, default panel/mullion types, and ordered vertical and horizontal
grids. Grid, panel, and mullion records have stable UUIDs. Panels refer to four
grid boundaries and a panel type; mullions refer to a grid/span and a mullion
type. `reconcile()` derives topology after a staged grid edit, preserving IDs
and component type assignments where the topology is unchanged. Committed
updates validate the old-to-new identity transition, and disappearing customized
components cannot be silently discarded.

Default panel and mullion types seed newly reconciled components. Changing an
assembly's default type does not implicitly reassign existing panels/mullions;
an editor must make that reassignment explicit. Type changes can update all
components assigned to that shared type.

The authored baseline runs between perimeter grid centerlines. Perimeter
vertical mullions are clipped to half-width at each end; horizontal mullions
stop at vertical-mullion faces; panels fill the remaining clear bays. Quantities
are analytic and independent of tessellation: panel area, member length, solid
volume, and material-derived mass when density is available.

## Derived scene and section geometry

Each resolved panel and mullion is an independent closed rectangular prism with
its own component UUID, material identity, and quantities. The display scene
combines these disjoint component meshes under the assembly UUID for selection.
The model validates all transformed world-space corners against the section
kernel's finite coordinate envelope before a transaction can commit. Geometry
regenerates on open, edits, type/material/level changes, removal, undo, and redo.
Preview geometry does not mutate the document or 3D scene.

Section snapshots retain the separate component meshes and cut them individually
because the section kernel intentionally rejects touching contours presented as
one union. Resulting cut lines remain owned by the assembly UUID, use unique
feature IDs, and retain each component's material as line-surface metadata.
Missed cuts and zero-area tangencies produce no lines. The component contours
are not a boolean union and do not yet provide hatch/poché or a full curtain
elevation drawing.

Schema 58→59 adds the required curtain-system, panel-type, and mullion-type
collections. Migration creates empty collections only when all three are
absent, advances native headers, and validates before adoption. Populated
assemblies, custom assignments, materials, save/reopen, and schema migration
are covered by `crates/os-storage/tests/curtain_systems.rs`. Native full-model
plugins use API 40/schema 59; generic API 2 and container 2 are unchanged.

## Plan placement

Architecture → Build → Curtain starts a two-click baseline tool in a ready
floor plan. Both endpoints use the active plan snap settings. The toolbar lets
the user set height and base offset in metres; the initial defaults are a 3 m
assembly, 20 mm glazing, and a 50 × 100 mm frame. Missing default types are
created in the same transaction as the assembly. Preview and invalid endpoints
do not change the document, history, or scene. A valid second click preflights
the exact transaction and geometry on an isolated editor, then commits one
undoable transaction and selects the new assembly.

The plan draws clipped component outlines at the applicable cut/projected/depth
role. Lines retain the assembly UUID and deterministic feature IDs, so ordinary
plan picking selects the curtain as one assembly. Escape, pointer loss, plan or
document changes, provider changes, and displayed-drawing changes cancel a
placement draft. Provider work is never called synchronously by the curtain
tool.

## Selected assembly editing

The Properties palette exposes “Edit curtain properties…” for one selected,
visible curtain in a ready floor plan. The scrollable staged dialog edits name,
height, base offset, normal orientation, and default panel/mullion types. The
level and baseline length are read-only. A default change affects newly
reconciled components only; separate bulk actions and per-component selectors
reassign existing panels and mullions. New panel and mullion types can be
created in the same staged transaction, with material assignment.

Vertical station values measure from the baseline start; horizontal stations
measure above the assembly base. Perimeter rows are locked and keep their UUIDs.
Interior stations can be added, removed, or moved numerically; the model's
18-grid bound, minimum spacing, clear-bay geometry, and type validity remain
authoritative. Reconciliation retains identities and assignments for unchanged
topology. If a proposed removal would discard customized assignments, the
dialog identifies the affected components and requires an explicit reset.
That reset and the resulting topology replacement are ordered commands in one
undoable transaction.

Drafts do not mutate the model, history, or scene. Apply previews the exact
command batch, regenerates an isolated scene, and derives the active native plan
drawing before publishing the transaction. Errors retain the draft; Escape,
selection/document/view/provider changes, and stale displayed drawings discard
it. The selected assembly UUID is retained, and split 3D regenerates only after
commit. The default selection filter still classifies curtains under “Other”;
there is not yet a dedicated curtain filter or box-selection footprint.

## Not implemented yet

- Direct component/grid grips and component-aware area selection.
- Filled 2D footprints, curtain-specific schedule or quantity report UI, and
  production sheet/PDF appearance.
- Wall-host cuts, joins, curved or segmented assemblies, operable panels,
  glazing transparency, and IFC exchange. IFC currently fails closed rather
  than silently dropping curtain data.
- Hatch/poché, full curtain elevations, and visual qualification.

Horizontal grids remain numeric properties because a floor-plan drag cannot
distinguish their elevation. Shared type management, component grips, and
reusable curtain-assembly types remain later increments.

## Focused evidence

- `cargo test -p os-model -p os-geometry -p os-ui --all-features --locked --offline --test curtain_systems`
- `cargo test -p os-ui --all-features --locked --offline --lib curtain_section_tests`
- `cargo test -p os-ui --all-features --locked --offline --lib curtain_`
- `cargo test -p os-storage --test curtain_systems --locked --offline`

These tests cover stable topology/invalidation, world bounds, diagonal and
reversed baselines, normal orientation, materials, analytic-vs-mesh volume,
disjoint component envelopes, scene history/open, section cuts, and migration
plus populated persistence. The egui tests also cover two-profile preview-only
placement and editing, numeric vertical/horizontal grids, stable grid
identities, staged per-component assignments and new types, explicit reset for
customized removed bays, invalid and stale drafts, native plan picking,
split-scene regeneration, save/reopen, and one-step undo/redo. They do not imply
production acceptance.
