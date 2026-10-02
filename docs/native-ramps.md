# Native straight-run ramps

## Supported element

`core.ramp` represents one straight rectangular slab joining two distinct
levels in the same building. The lower and upper elevations are read from the
referenced levels; rise and rise/run are derived and are not stored on the
ramp. The authored plan endpoints mark the centres of its lower and upper
walk-surface edges. Width and structural thickness are in metres. Thickness
extends vertically below the sloped walk surface, so the slab has vertical end
faces and volume `run × width × thickness`.

These are geometry semantics and modeling bounds, not slope, accessibility,
egress, or building-code checks.

## Plan workflow and generated views

From a floor plan, choose **Architecture → Ramp**, select an upper level in the
same building, then click the lower walk edge and the upper walk edge. Existing
plan snapping positions both endpoints. Width, structural thickness, name, and
optional material are editable in Properties before placement. The footprint
and uphill-arrow preview is transient: it does not modify the model, undo
history, or 3D scene. Escape cancels. A valid second click commits one ramp
transaction; the repeat-placement tool remains active until Escape. Selected
ramps expose staged Properties editing and Delete.

The committed ramp generates one deterministic closed, outward-wound slab mesh.
Plan projection, range/crop clipping, picking, boundary snapping, vertical
section contours, and sheet/vector-PDF graphics derive from the native ramp.
Level, material, or ramp edits regenerate dependent views and the 3D scene;
preview never mutates the live scene.

## Persistence, exchange, and limits

Schema 60→61 adds the required `ramps` map. Migration rejects an ambiguous
pre-existing key, inserts an empty map, advances native headers, and validates
before adoption. Save/reopen preserves ramp, level, material, and entity
identities. Native full-model plugins require API 43/schema 62; generic API 2
and the `.osb` container version are unchanged. IFC export fails closed when a
project contains ramps rather than silently omitting them.

Only one straight slab is supported: there are no landings, turns, curbs,
handrails, edge conditions, attached ramps, ramp assemblies, quantities or
schedules, code checks, or IFC ramp exchange. The app does not claim Revit
feature parity or physical-print/production visual qualification.

## Evidence

- `crates/os-model/src/ramps.rs`: constraints, derived dimensions, and level /
  building / material validation.
- `crates/os-geometry/src/ramps.rs`: closed-shell, orientation, material,
  volume, and coordinate-bound tests.
- `crates/os-document/src/tests/ramps.rs`: atomic commands, invalidation,
  stable identity, history, and level-dependency tests.
- `crates/os-render/src/plan/ramps.rs`: clipped plan projection, direction
  arrow, picking, snapping boundaries, stale contexts, and section-line data.
- `crates/os-storage/src/tests/ramps.rs`: schema migration and populated
  save/reopen identity coverage.
- `crates/os-ui/src/plan_workspace/ramp_tests.rs`: preview isolation,
  placement, snapping, properties, cancellation, stale state, invalid input,
  split-scene regeneration, picking, and undo/redo at 1280×800 / 100% and
  1000×650 / 150%.
- `crates/os-ifc/tests/exchange.rs`: fail-closed export coverage.
