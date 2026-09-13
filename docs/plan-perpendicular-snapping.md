# Perpendicular snapping from a gesture anchor

2026-09-12: the shared snap query accepts an optional finite view-plane anchor.
For each semantic segment it projects the anchor onto the supporting line and
offers the foot only when it lies within that finite segment, inside the crop,
outside excluded entities and within the pointer's screen acquisition radius.
It does not clamp an out-of-range projection to an endpoint. Coincident
anchor/foot within 1e-9 m is not offered as a perpendicular direction.

The target retains entity UUID and feature key. The anchor is part of query
identity, so changing it invalidates an earlier result. Unit-direction/local
arithmetic avoids squared world coordinates; nonfinite anchors/projection
arithmetic reject the query. Work is linear in the existing bounded scene.
Priority is endpoint, intersection, perpendicular, midpoint, grid-axis nearest,
then ordinary nearest. This is acquisition, not an associative constraint.

The wall tool supplies its current start/base/fixed endpoint as the anchor.
The Snaps menu exposes **Perpendicular from anchor**, enabled by default and
subject to the master snapping switch. No anchor means no perpendicular query.
Exact numeric input remains authoritative. The setting is session-only.

## Evidence

- Renderer tests: expected foot and semantic identity, anchor-change staleness,
  outside-segment/collinear rejection, crop/exclusion, nonfinite anchors and
  diagonal features at million-metre coordinates.
- Desktop egui input: start a wall at `(1,2)`, click off the perpendicular foot
  near the existing wall's centreline, observe Perpendicular, commit at `(1,0)`,
  and undo once with the original model intact.
- Full all-feature offline workspace tests, strict Clippy, formatting and default
  workspace build pass.

Native-window inspection, curves, inferred anchors before the first click,
independent plugin snap contributions and persistent geometric constraints
remain open. D and the production snapping group are not complete.
