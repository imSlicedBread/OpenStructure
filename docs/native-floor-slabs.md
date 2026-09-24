# Native floor and slab authoring

OpenStructure has a first native floor/slab vertical slice. In a named floor
plan, use **Draw floor**, click straight-edged boundary vertices, then click
**Finish floor** or close the outline by clicking near its first point. Escape
cancels. The draft is view/session/revision/provider-bound, does not mutate the document,
and uses the plan's current semantic snap settings. Thickness and top offset are
entered in metres before commit.

Finishing creates one `core.floor` entity on the plan's level in one history
transaction. The plan displays a filled, pickable boundary; split 2D/3D and the
normal 3D view use the same committed floor identity. Undo/redo and save/reopen
preserve it. Floors can also be created, updated, and removed through native
document commands.

The authored XY ring accepts either winding and simple concave polygons with
3–256 vertices. Duplicate/near-duplicate points, self-intersection,
backtracking, negligible area, nonfinite values, and out-of-range coordinates
are rejected atomically. The model stores the boundary, thickness, top offset,
level, optional material and name. Area, deterministic triangulation, and the
closed outward-wound 3D mesh are derived. Top elevation is level elevation plus
top offset; the slab extrudes downward by thickness.

Schema 8→9 adds the required `floors` map. Existing schema-8 files migrate
without changing IDs or opaque extension data. IFC export refuses a model that
contains floors because this release does not yet provide a lossless floor
exchange path.

Limitations: boundaries are straight-edged and horizontal; holes, slopes,
layered floor types, joins with walls, parametric constraints, floor-specific
visibility controls, property editing in the desktop inspector, and IFC slab
import/export are not implemented. This is a basic architectural slab object,
not Revit-equivalent floor authoring.

Evidence:

- `crates/os-model/src/floors.rs`: semantic validation and area.
- `crates/os-geometry/src/floors.rs`: deterministic concave triangulation and
  closed mesh tests.
- `crates/os-document/src/tests/floors.rs`: transactions, invalid edit
  atomicity, dependency invalidation, undo/redo.
- `crates/os-storage/src/tests/floors.rs`: frozen schema-8 migration and real
  `.osb` save/reopen.
- `crates/os-render/tests/plan.rs`: floor fill data, concave picking, crop and
  stale-context checks.
- `crates/os-ui/src/plan_workspace/floor_tests.rs`: egui interaction at
  1280×800 / 100% and 1000×650 / 150%, snap use, preview-only state, commit,
  split-scene mesh, undo/redo, Escape, stale context and invalid-boundary checks.
