# Native compound wall types and layers

Status: bounded native implementation, automated-tested; not a claim of Revit
parity or production qualification.

Schema 22 adds project-owned `core.wall_type` entities and a per-wall assignment
map. A type contains an ordered set of one to 32 layers. Every layer has its own
stable UUID, name, thickness, function, and optional project material reference.
Thicknesses are finite and positive, and a complete assembly is limited to 10 m.
Each wall assignment references one type and stores a per-instance layer flip.
The wall retains its UUID, level, name, height, and other identity; its effective
thickness and material profile come from the assigned type.

The shared `Model::resolve_wall` resolver is the source of effective wall
parameters and layer offsets. Legacy walls resolve as independent one-layer
walls and are not automatically shared. The “Create type from this wall” editor
starts from that resolved profile. Type, material, assignment, and wall edits use
validated document transactions and normal undo/redo. The revision-bound type
editor commits its staged type/material changes and optional wall assignment as
one history entry; Cancel and stale-document saves do not apply the draft.

The native straight-wall geometry kernel derives layer cells from the effective
profile. Hosted openings cut each layer, layer volumes reconcile with net wall
volume, and mass is computed only when a referenced material supplies a density.
Plan footprints and section lines carry layer/material identity while selection
and picking remain owned by the original wall UUID. Shared type changes
invalidate dependent walls and views. Explicit butt/corner/tee joins use the
resolved compound profile and require compatible joined profiles; arbitrary
layer termination and compound-profile junction design remain unresolved.

The built-in native Wall plugin path uses API 9 / model schema 28. The older
single-Solid wall export and IFC wall export reject assigned compound walls
instead of silently flattening or losing their layer data. API-2 generic DTO
contracts and envelopes are unchanged; this feature does not grant arbitrary
plugin pointer gestures or compound-type authoring by plugins.

Automated evidence is in `crates/os-ui/tests/wall_types.rs`,
`crates/os-ui/src/desktop_tests/wall_type_tests.rs`, and
`crates/os-storage/tests/wall_types.rs`. These cover shared type edits and
invalidation, reorder/flip, materials and density-based quantities, openings,
plan/section/mesh identity, equal-profile joins, save/reopen, migration,
atomic rejection, UI cancel/stale/save/history, and refusal of lossy exports.

Remaining scope includes curved walls, variable/compound junction semantics,
material appearance and hatch standards, independent architectural parts,
wall-type schedules, full IFC layer-set exchange, and visual/production
qualification. Material identity and density are implemented; rendered material
textures, colors, and drafting hatch libraries are not.
