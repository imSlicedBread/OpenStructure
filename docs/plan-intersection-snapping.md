# Semantic finite-line intersection snapping

2026-09-12: shared host snap queries now support intersections of the finite
semantic segments supplied to the plan drawing. The bundled wall pointer tools
enable this query kind and display the Intersection marker. This is not an
alignment constraint, wall join, infinite-axis intersection or plugin wire API.

Both `(entity UUID, feature key)` references are retained, canonically ordered
for input-order-independent tie breaking. Priority is endpoint, intersection,
midpoint, grid-axis nearest, then ordinary nearest. Screen distance and semantic
keys break ties within a kind. Query settings, excluded entity, document/view
context and camera remain part of result validity. Excluding an edited entity
also excludes intersections involving it; crop boundaries never invent features.

## Numerical and resource policy

Intersection uses normalized directions and local differences, not squared
world coordinates. Parallel/collinear segments have no unique intersection.
Pairs with absolute unit-direction determinant at most `1e-12` are considered
ill-conditioned and omitted. Parameters must lie on both finite segments;
screen acquisition tolerance does not extend them. Finite-value failures reject
the query. Existing scene maximum is 10,000 segments.

A query first acquires segments within the pointer's model-space acquisition
radius, then checks at most 256 nearby segments (32,640 pairs). Overflow of that
local limit rejects the query with an explicit zoom-in diagnostic; candidates
are not truncated. API consumers can disable intersection queries. A native
snap-override UI, spatial index and sustained dense-model timing qualification
are still required; these bounds are not a performance acceptance claim.

Follow-up: [plan snap controls](plan-snap-controls.md) now expose the enable
switches in the desktop toolbar, including disabling intersections. Broader
priority customization and performance qualification remain open.

[Exact-input precedence](exact-input-snap-precedence.md) now bypasses acquisition
when an anchored gesture has a complete exact destination. The local segment
limit still applies whenever the draft requires pointer snapping.

## Evidence

- Renderer tests cover crossing lines, canonical pair identity, input ordering,
  crop exclusion, excluded editing targets, collinear/parallel/disjoint and
  near-parallel pairs, translated diagonal crossings at a million-metre origin,
  and explicit dense-query failure.
- Desktop egui input test `pointer_wall_acquires_semantic_intersection_of_two_walls`
  creates two crossing walls, clicks off their intersection to acquire it, then
  creates a third wall from that exact point and undoes it as one transaction.
- Full offline all-feature workspace tests, strict all-target/all-feature Clippy,
  formatting and default workspace build pass.

No native-window inspection of this new snap kind is claimed. Curves, projected
infinite intersections, independent plugin contributions and production snap
override/priority workflows remain open.
