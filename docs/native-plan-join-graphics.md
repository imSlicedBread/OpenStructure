# Native plan wall join graphics

Plan walls clean up only at persisted native Butt, Corner, and Tee contacts.
The checked wall model remains the source of adjusted station limits and contact
traces. Geometric contact alone never creates a join. This work changes the
derived outlines only; wall footprints, UUIDs, authored axes, openings,
quantities, picking interiors, snapping, and 3D geometry remain per wall.

The plan adapter resolves each contact in the active view basis and collects
uncropped material footprints before crop clipping. Cleanup is limited to
intervals supported by both original and visible contact boundaries. At each
interval, the renderer compares plan role, resolved surface color, and resolved
role stroke. Matching contacts in the same role lose their internal seam on
both walls. A role, material-color, or line-style discontinuity retains one
contact stroke: the lower UUID for Butt, the trimmed peer for Corner, and the
branch for Tee. Partial layer intervals and exposed residual edges stay
independent.

Phase filtering removes joins from the view-only source when a member is not
visible. Crop boundaries remain visible, and the interval algorithm has a
shared work budget that rejects excess complexity atomically. The same checked
outline stream feeds canvas, sheet, and vector PDF. Existing wall identity,
overlap selection, and authored snap features are unchanged; provider lines do
not acquire inferred connectivity.

Focused evidence:

- `crates/os-render/tests/plan_join_graphics.rs`: role/style/material
  compatibility, explicit-contact requirement, interval ownership, crop,
  surface identity, overhead boundary hits, stale context, and work budget.
- `crates/os-ui/tests/plan_join_graphics.rs`: Butt/Corner/Tee owners and
  directions, material overrides, phase filtering, rotated plans, pick/snap
  ownership, and shared canvas/sheet/PDF outlines.
- Existing `wall_joins` and `perpendicular_joins` integration suites continue
  to cover model geometry and join editing.

This is partial Revit parity. Revit-style Clean/Don't Clean/Use View Setting
controls, same-type cleanup policy, layer priority/wrapping and material
termination rules, curved or variable-profile joins, and native-window or
physical-print qualification remain open. Unequal wall dimensions, ambiguous
contacts, and generic provider connectivity are not added by this increment.

## Validation (2026-10-24)

- Renderer `plan_join_graphics`: 7 passed.
- Editor `plan_join_graphics`, `wall_joins`, and `perpendicular_joins`: 13 passed (4, 5, and 4 respectively).
- Production Clippy passed for `os-render` and `os-ui` libraries/binaries with warnings denied and four unrelated existing lint classes allowed: `suspicious-assignment-formatting`, `collapsible-if`, `large-enum-variant`, and `manual-checked-ops`.
- Scoped `rustfmt --check` passed for the new renderer join module and both new test files.