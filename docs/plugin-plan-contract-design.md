# Plan interaction and graphics contract design

Status: design for B; callable implementation and independent plugin acceptance
are required in D. The [optional plan-graphics service foundation](callable-plan-graphics.md)
now has synchronous/worker host calls and bounded semantic-line validation. Generic API-2
operations remain unchanged. Native integration and the other operations
below remain required; this design is broader than the implemented service.

Implementation prerequisite: [desktop form view provenance](plugin-form-view-provenance.md)
now connects existing worker view tickets to active desktop plans. This does not
implement or advertise the gesture/graphics/snapping operations below.
The [worker revision boundary](worker-persisted-view-validation.md) additionally
checks supplied settings revisions against the persisted view before dispatch
and at consumption; it is not a navigation or gesture sequence counter.

## Context and coordinates

The host owns active view/level, selection, screen-to-plane conversion and gestures.
A provider receives project/session/model revision, view UUID/settings revision,
active level UUID/elevation, projection direction and an f64 work plane: world
origin and orthonormal right/up/normal axes. Reject nonfinite/degenerate bases.
Model coordinates use metres; paper sizes and screen acquisition radii have
separate explicit units. Navigation is not a model edit. Selection and history
are shared across views, not owned independently by each plugin.

Plan context includes persisted level-relative top/cut/bottom/depth, finite ordered
ranges, crop boundary, scale/detail, visibility and style revision. Reflected
ceiling projection requires explicit upward semantics. Providers echo the full
context; cache keys include these settings, provider version and model revision.
Unsupported views/ranges/crops/representations return an unavailable diagnostic.

## Gesture lifecycle

1. Begin: host reserves a gesture ID and prospective entity IDs, authorized command,
   scope, baseline revision and view. Send bounded plane-space input, not OS handles.
2. Update: pointer/modifiers/exact inputs produce transient previews, validation
   feedback and optional snap queries only. Coalesce updates, allow one live request
   per gesture, discard superseded replies. No dirty state, persisted ID or undo
   entry from previews. Exact numeric inputs remain separate uncommitted drafts.
3. Commit: provider proposes a final batch against the same baseline. Host checks
   gesture/view/session/revision/permissions, validates geometry/references, and
   commits exactly once through Document. One drawing gesture/drag is one undo step.
4. Cancel: Escape, tool/view/document switch, closure or failure clears previews
   and revokes the gesture. Executing Wasm may drain fuel, but cannot publish late
   results. Failure preserves committed model/history/saved state and input drafts.

Model edits or undo during a gesture invalidate its baseline; never silently rebase
plugin edits. Selection changes that alter authorization revoke the gesture.
Pan/zoom can retain model-space gesture state but invalidate screen snap acquisition.

## Snapping

Providers receive authorized nearby objects and a model/view-space query. Return
position, supported snap kind, semantic entity/feature reference, stable local
feature key and context stamp. Validate finite positions and scope; changed/deleted
features invalidate candidates. Start with at most 256 candidates per query.

The host computes screen distance and applies deterministic priorities/tie-breaking
and user overrides across providers. Plugins cannot inflate priority or control
acquisition tolerances. Cover endpoint, midpoint, intersection, perpendicular,
tangent, nearest, center, grid and axis snaps as implemented; never silently
substitute an unsupported kind. Exact coordinate/length/angle input is authoritative.

## Graphics and picking

Return a deterministic semantic 2D representation shared by screen/output, not
guest pixels or egui callbacks: bounded paths/symbols, draw order, cut/projected/
overhead roles, style/material references and semantic hit-test references. Validate
coordinates, indices, curve parameters, clips, nesting, counts and response bytes.
Numeric limits and golden fixtures must land with the drawing IR. Reject invalid
output wholly, rather than presenting partial output as a complete drawing.

Resolve cut/projection/symbol/visibility rules before caching. Test provider
compliance against independent expected fixtures. Picking uses the same clipped
visible representation; no invisible hit targets. Model-linked graphics reference
the same semantic UUIDs as 3D. View-owned drafting is separate, never duplicate
wall geometry. The host owns category/style/visibility policy.

Supported straight wall prisms may use a documented, kernel-validated host cut/
projection fallback with identical range/visibility rules. Never guess geometry
for opaque extension payloads. Missing providers show unavailable state; stale
cached previews are derived data and cannot qualify an issued drawing. A top-view
mesh is not plan support.

## Negotiation and required acceptance

Advertise operations only after adapters and validation exist. Version new required
fields/operations explicitly or negotiate optional extensions; do not silently
change API-2 required fields. Older hosts/plugins must fail actionably. Registration
metadata alone cannot enable a tool/provider. Extend checked worker tickets with
view/gesture stamps; unload revokes callbacks without freeing executing code.
No filesystem/network/UI/kernel handles cross the boundary.

D must run installed Rust Wall/column tools without host source changes: two
snapped exact-input walls, split 2D/3D selection/editing, level/cut changes, correct
cuts/picking and save/reopen of settings/IDs. Verify cancel, invalid batches/
graphics/snaps, wrong scope, stale model/view/gesture, late inactive-view replies,
undo/redo and missing-provider preservation. Measure pointer feedback separately
from regeneration. The API-2 command and WAT creation tests do not satisfy D.
