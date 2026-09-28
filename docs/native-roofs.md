# Native single-plane roofs

Status: bounded native authoring support; not a general architectural roof
system and not Revit parity.

## Model and geometry

Model schema 38 adds `Model.roofs`, keyed by stable entity UUID. `RoofParams`
stores a name, level, optional material, a simple outer plan ring, zero or more
strictly contained opening rings, vertical thickness and top offset, and a
slope anchor/direction with signed rise per horizontal run. The anchor's top
elevation is `level elevation + top_offset`; arrow length sets direction only.
Ring, hole, dimension, reference, coordinate and derived elevation limits are
validated before a command is accepted.

The 3D representation reuses the bounded perforated-slab triangulation and
applies an affine shear for the roof slope. It is a closed, deterministic solid;
thickness remains vertical, not normal to the sloped face. Plan output clips the
actual sloped solid against the view's depth/bottom/cut/top range and crop. The
same mesh feeds native section derivation, sheet preview and vector-PDF
projection. Roof plan fills and openings therefore come from the roof geometry,
not from a rendered screenshot or a footprint-only approximation.

## Desktop workflow

Open a floor plan and choose **Build → Roof**. Click the outer boundary
vertices, then use **Finish roof loop** in Properties. Place the slope anchor
and direction point; the plan shows a transient roof fill and slope arrow.
Use **Sketch roof opening** to add a hole ring. Properties exposes the exact
coordinates and dimensions, level, material, thickness, offset and signed
rise/run for review or numeric editing. **Apply roof** creates or updates the
whole element in one history transaction. Selection Properties also supports
redrawing the boundary, adding/removing openings and changing the slope; Delete
is explicit. The transient preview does not mutate the model or live 3D scene.
Escape, pointer loss, a document revision, provider change, view/settings
change or stale edit context discards the draft. Existing floor-plan snaps are
used for authored points.

## Interchange and compatibility

The atomic schema-37→38 migration adds an empty roof map and advances native
headers; a pre-existing map in schema 37 is rejected as ambiguous. Native
full-model plugin API 32 requires schema 51. Generic plugin API 2 and the `.osb`
container version do not change. IFC import rejects `IfcRoof`, and IFC export
rejects native roofs instead of silently dropping them.

## Limits and evidence

This slice supports one affine plane per roof, one optional material, a simple
boundary and up to the shared slab triangulator's bounded opening rings. It
does not support hips/valleys, multiple slopes, ridges, dormers, roof joins or
wall attachment, layered assemblies, hosted skylight families, roof-specific
views/schedules/quantities, IFC roof exchange, generic plugin pointer gestures,
or production visual/physical-print qualification. Plan graphics are drawn in
ordinary floor plans; this does not add a dedicated roof-plan view.

Evidence is in `crates/os-model/src/roofs.rs`,
`crates/os-geometry/src/roofs.rs`,
`crates/os-ui/src/plan_workspace/roofs.rs`,
`crates/os-ui/src/plan_workspace/roof_tests.rs`,
`crates/os-storage/src/tests/roofs.rs`, and
`crates/os-ifc/tests/exchange.rs`. See the exact run results in the project
coverage ledger; UI-frame tests do not claim native-window or plotter acceptance.
