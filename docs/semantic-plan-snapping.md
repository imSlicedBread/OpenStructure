# D: semantic plan snapping foundation

Later [grid plan integration](grid-plan-integration.md) adds persistent grid
endpoints/midpoints and finite GridAxis snapping. The older checkpoint below
describes wall-only snapping before that addition.

Follow-up: [the two-point wall tool](plan-wall-gestures.md) now consumes these
queries for desktop snap feedback. This page records the earlier foundation
checkpoint; independent external providers remain unimplemented.

Status: implemented in the render/controller path and tested; not yet a desktop
pointer-authoring or callable external-plugin workflow.

`os-render::snapping` accepts checked, context-bound semantic line segments in
view-plane metres. Native plan derivation attaches each visible wall's authoritative
axis as feature 0, with start/end endpoint keys 0/1. It converts world coordinates
using the persisted horizontal basis after resolving plan visibility. It does not
infer axes from tessellated pixels, crop vertices or opaque extension payloads.
The existing background plan worker produces both the drawing and snap scene.

`PlanDrawing::snap(current_context, query)` returns a context/query-bound result.
Queries specify the camera, viewport size, pointer in logical screen pixels,
acquisition radius in (0,64] pixels and enabled endpoint/midpoint/nearest kinds.
Results provide semantic entity/feature, kind, point in view-plane metres and screen
distance. Convert the point through the context basis for a model-space commit.
The consumer must supply the current context and query when retrieving a candidate;
changed model/view/settings, pointer, camera, viewport or options reject old results.

## Initial policy

- Candidates outside the screen acquisition radius are ignored. Logical pixels,
  not world-unit tolerances, make acquisition consistent through zoom and DPI.
- Enabled endpoint snaps outrank midpoint snaps, then nearest snaps. Screen
  distance breaks same-kind ties, followed by stable entity/feature/endpoint keys.
  Input order cannot change a tie. No caller-supplied numeric priority is accepted.
- Cropping suppresses actual endpoints/midpoints outside the crop. Nearest queries
  use the clipped interval of the axis; a crop intersection remains a nearest
  point, never a newly invented semantic endpoint.
- Hidden/unavailable elements cannot be attached to a drawing's snap scene.
  Native hidden walls keep their model/3D data but supply no plan snap candidates.
- Degenerate/nonfinite/duplicate features, invalid contexts/queries, over-budget
  sources and arithmetic overflow reject wholly. Input is capped at 10,000
  segments. Queries scan this bounded set and retain one best result, not an
  unbounded candidate array or quadratic pair-intersection search. Large-model
  interaction performance and spatial-index thresholds remain unmeasured.

This is an internal host API, not permission to accept arbitrary provider geometry.
The future external adapter must additionally enforce provider authorization,
geometry/feature scope and response bounds. API-2 operations remain unchanged.
Intersection/perpendicular/tangent/center/grid/axis snapping, user-facing overrides,
gesture previews, exact-input precedence and one-commit pointer tools remain D/E work.

## Evidence

Five `os-render/tests/snapping.rs` tests cover priority, exact pixel-radius
boundaries across zooms, deterministic input-order ties, clip-generated nearest
points, stale navigation/context, malformed/bounded inputs and independent nearest
point expectations around coordinates 1,000,000 m. `os-ui/tests/plan.rs` verifies
wall IDs, rotated basis round trips, read-only querying, hidden-wall preservation,
and stale results after settings changes/Undo against the actual controller.

The complete locked/offline all-feature workspace suite passes: 194 tests including
the doctest. No dependencies, unsafe code or native-format versions changed.
No desktop input was added or native snapping observation claimed in this slice.
Next integrate these results into visible snap feedback and transactional wall
gestures, alongside level/grid authoring and the independent plugin plan contract.
