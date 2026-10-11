# Native linked building sections

Schema 68 introduced `SectionViewSettings` version 2 for linked vertical
section views; current model schema is 70 / native Model API 51. The plan-space `start`→`end` line defines positive
station; its viewer side selects the looking direction without reversing that
axis. Bottom/top are absolute project elevations. A section also stores
`CutOnly` or `CutAndProjection`, finite far depth, an independent station crop
relative to the line start, and a scale denominator. All distances/elevations
are in metres. `ViewParams.level` identifies the marker's plan level and the
building scope. The section UUID remains the marker/view identity.

Schema 67→68 upgrades version-1 settings deterministically: Left side,
`CutOnly`, 30 m far depth, station crop `[0, marker length]`, and scale 1:100.
The migration preserves endpoints, elevations, view/entity UUIDs, opaque
metadata and extension payloads; malformed or ambiguous section definitions
fail atomically. Frozen migration and save/reopen evidence is in
`crates/os-storage/tests/section_v2.rs`.

`os-geometry::section::vertical_section` cuts a closed indexed triangle mesh by
the vertical plane and returns deterministic outer/hole contour rings in
`(station, absolute elevation)` coordinates. It handles exact vertex/edge
intersections and coplanar patches, and rejects open/non-manifold, touching, or
intersecting contours instead of repairing them. Native section derivation
includes supported walls/layers and hosted door/window openings and components,
floors, ceilings, supported roofs, stairs/ramps, columns, casework, curtain
components, and railing members. Components are sectioned independently to
preserve material and identity. Unknown extension/plugin geometry fails closed
with an explicit incomplete-section diagnostic. Geometry and visibility work
are bounded; exceeding those limits rejects the drawing rather than returning
partial geometry.

In `CutAndProjection`, the same native solids feed a finite-depth, shared
hidden-line solve. Original mesh edges are clipped to station/elevation/depth
bounds; synthetic crop-plane or far-plane edges are not created. Opaque cut
fills mask projected background strokes while section holes reveal them. Cut
and projected geometry retain source entity/material attribution. The checked
drawing feeds the canvas, linked sheets and vector PDF; transient selection
accents are not model geometry. The split 2D/3D model changes only on commit.

Use `Place section` in a floor plan and click two points to draft the marker.
Endpoint/intersection/midpoint/grid snapping follows current plan settings. The
second click opens a review draft rather than changing the model. Review/edit
viewer side, display mode, line endpoints, station crop, vertical extents, far
depth and scale, then Apply for one undoable view change. Existing sections
have a `Section settings` editor. Escape, document/session/provider changes,
view changes or invalid geometry discard/reject the draft; preview does not
write model/history state. The marker shows viewing direction and, when enabled,
the finite projection region.

Native Section horizontal/vertical dimensions now acquire visible semantic
straight-wall and rectangular hosted door/window extents. Their UUID references
resolve live in the section frame and share canvas, sheet and vector PDF
graphics. Preview is transient; commit/repair/offset edits use one transaction,
and stale or canceled drafts leave no model/history edits. See
[native dimensions](native-dimensions.md) for exact semantics, diagnostics,
schema-68→69 migration and two-DPI automated evidence.

Native Section cut fills also support project-owned, model-scale parallel
material hatches. The shared world origin, pattern assignment, crop/hole/mask
clipping, canvas/sheet/PDF vectors, package dependencies and limits are described
in [native material patterns](native-material-patterns.md). Schema 69→70 and
API 50→51 add this data without changing generic plugin protocols.

The section view remains a focused native workflow, not Revit parity. It does
not yet provide general section annotations, chains, surface or paper-scale
hatch patterns, transparency/monochrome controls, editable graphic templates, general
linked-model/plugin adapters, a complete category-visibility system, or
production visual/physical-print qualification. Unsupported geometry and
budget exhaustion remain visible as explicit limitations. Sections can be
placed through the sheet layout workflow alongside other views, with up to 64
viewports and one schedule in the current layout UI; viewport scales and
placement remain independent. See `docs/2d-coverage.md` for the
broader evidence and open requirements.

For face-on views and opening placement/editing, see
[linked straight-wall elevations](native-elevations.md). Elevations project
visible surfaces; sections preserve their cut-plane semantics.
