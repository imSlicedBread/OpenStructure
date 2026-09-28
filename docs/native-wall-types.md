# Native compound wall types and layers

Status: bounded native implementation, automated-tested; not a claim of Revit
parity or production qualification.

## Circular wall centerlines (schema 54)

Native walls store one analytic `WallPath`: a straight segment or a bounded
circular arc defined by center, radius, start angle and signed sweep. Plan
authoring uses three clicks (start, a point on the curve, end); arc length,
station, tangent and projection remain analytic model values. Rendering derives
an adaptive annular shell and plan footprints with a 1 mm sagitta tolerance and
a hard 4096-segment cap. Per-layer arc volume and mass remain analytic. The same
committed wall UUID appears in plan, 3D, section and sheet projections; preview
does not mutate the document or 3D scene. Center/radius/sweep are editable in
Properties. Schema 53 straight walls migrate losslessly to `Straight` paths.

Circular-wall joins, hosted doors/windows and room-boundary participation are
not implemented. Those consumers reject or omit arcs rather than treating
chords as semantic paths. Straight-only endpoint transforms, outer-face
dimensions, generic API-2 wall commands, rectangular-Solid export and IFC export
also do not convert arcs. Arc authoring currently requires the bundled native
Wall provider; installed API-2 Wall gestures cannot express analytic paths.
Evidence: `crates/os-model/tests/wall_arcs.rs`,
`crates/os-geometry/tests/wall_arcs.rs`,
`crates/os-storage/tests/wall_arcs.rs`,
`crates/os-ui/src/plan_workspace/arc_tests.rs`, and
`crates/os-ui/tests/wall_arcs.rs`. This bounded increment is not Revit parity or
visual/production qualification.

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

The native wall geometry kernel derives straight layer cells or segmented
annular shells from the effective profile. Straight-wall hosted openings cut
each layer, while straight and circular layer quantities reconcile with net wall
volume, and mass is computed only when a referenced material supplies a density.
Plan footprints and section lines carry layer/material identity while selection
and picking remain owned by the original wall UUID. Shared type changes
invalidate dependent walls and views. Explicit butt/corner/tee joins use the
resolved compound profile and require compatible joined profiles; arbitrary
layer termination and compound-profile junction design remain unresolved.

The current built-in native Wall plugin path uses API 35 / model schema 54. The older
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

Remaining scope includes curved-wall joins/hosts, variable/compound junction semantics,
textures, transparency, PBR and hatch standards, independent architectural parts,
wall-type schedules, full IFC layer-set exchange, and visual/production
qualification. Material identity, density and editable shared RGB are implemented;
[material-color evidence](native-material-colors.md) covers 3D layers, opening
panels/frames, family preview, plan/sheet fills, migration and history. Rendered
textures and drafting hatch libraries remain absent.
