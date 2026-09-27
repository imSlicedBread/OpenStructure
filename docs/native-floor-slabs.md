# Native floor and slab authoring

OpenStructure has a native floor/slab vertical slice. In a named floor
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

Select a visible slab in plan to expose 6 px vertex handles. Drag a handle to
reshape one boundary vertex; the selected floor is excluded from its own snap
queries. The candidate boundary and fill are preview-only while dragging, and
release commits one `UpdateFloor` transaction if the resulting simple polygon
is valid. Escape, stale plan/document/provider state, pointer loss, crop exit,
or an invalid/collapsing release leaves the model unchanged. The committed slab
regenerates in 3D; the preview never mutates the 3D scene. Dragging the slab
body still pans the canvas; numeric inspector editing remains unavailable.

Select a visible slab and choose **Add slab opening…** to sketch a closed
inner loop. Click boundary vertices, then **Finish opening** or close near the
first point. The draft is preview-only and Escape cancels it. A valid loop must
be strictly inside the slab, remain clear of its outer edge and other openings,
and preserve positive net area. One `UpdateFloor` transaction adds the opening;
selected opening vertices have the same preview-only drag behavior, and the
Properties palette removes one opening per transaction. Plan fill and picking,
sheet/vector-PDF outlines, split 2D/3D meshes, and vertical sections carry the
void through. Stair geometry is not created automatically.

The authored XY ring accepts either winding and simple concave polygons with
3–256 vertices. Up to 16 simple opening rings are supported, with 3–256
vertices per ring and at most 1024 vertices total; winding is independent.
Openings cannot touch, overlap, or nest. Duplicate/near-duplicate points, self-intersection,
backtracking, negligible area, nonfinite values, and out-of-range coordinates
are rejected atomically. The model stores the boundary, thickness, top offset,
level, optional material, name, and opening loops. Net area, deterministic
hole-aware triangulation, and the closed outward-wound 3D mesh are derived. Top
elevation is level elevation plus top offset; the slab extrudes downward by
thickness.

Schema 8→9 adds the required `floors` map; schema 35→36 adds an empty opening
list to existing floors and advances native headers/root atomically. Existing
files preserve IDs and opaque extension data. Native full-model plugin API is
17/schema 36. IFC export refuses a model that contains floors because this
release does not yet provide a lossless floor exchange path.

Limitations: boundaries/openings are straight-edged and horizontal; slopes,
layered floor types, joins with walls, parametric constraints, floor-specific
visibility controls, numeric/property inspector editing, and IFC slab
import/export are not implemented. This adds slab voids, not Revit-equivalent
floor authoring.

Evidence:

- `crates/os-model/src/floors.rs`: semantic validation and area.
- `crates/os-geometry/src/floors.rs` and `floor_holes.rs`: deterministic
  concave/holed triangulation and closed outward mesh tests.
- `crates/os-document/src/tests/floors.rs`: transactions, invalid edit
  atomicity, dependency invalidation, undo/redo.
- `crates/os-storage/src/tests/floors.rs`: frozen schema-8 migration and real
  `.osb` save/reopen, plus schema-35 opening-ring migration.
- `crates/os-render/tests/plan.rs`: floor fill data, concave picking, crop and
  stale-context checks; slab voids excluded from fill and picking.
- `crates/os-ui/src/plan_workspace/floor_tests.rs`: egui interaction at
  1280×800 / 100% and 1000×650 / 150%, snap use, preview-only state, commit,
  split-scene mesh, undo/redo, Escape, stale context and invalid-boundary checks.
  Vertex-edit tests cover logical-pixel acquisition, crop clipping, unchanged
  model/history/3D state during preview, metadata and identity preservation,
  one-step update/undo/redo, invalid release, stale revision, handle-over-pan
  precedence, and body-drag canvas pan. Opening authoring covers two DPI profiles,
  preview immutability, one-step update/undo/redo, escape/invalid finish, provider
  invalidation, removal, and direct inner-loop vertex edits.
