# Native linked building sections

The native model schema 15 adds a versioned definition for a linked vertical
section view. It persists the view plane as a plan-space line and the
vertical extents as absolute project elevations; all values are in metres.
The line's `start` to `end` direction establishes the positive section-space
horizontal axis and is also the intended plan-marker baseline. `ViewParams.level`
identifies the level on which the marker belongs.

`SectionViewSettings` rejects non-finite or degenerate lines, unsupported
settings versions, inverted/zero vertical ranges, and dimensions beyond the
document's bounded geometry envelope. A newly created `ViewKind::Section` must
include both a marker level and settings. Previously saved unconfigured Section
views remain loadable so older documents can still be inspected and repaired;
they are not considered usable linked sections.

Schema 14→15 preserves entity IDs, view settings revisions, headers' other
metadata, extension payloads, and every existing view parameter. The new
optional `section` field is absent on legacy views and defaults to no section
settings. A schema-14 document that already contains a `section` field is
rejected as ambiguous instead of being silently reinterpreted. See
`crates/os-storage/tests/sections.rs` for migration evidence.

`os-geometry::section::vertical_section` cuts a closed indexed triangle mesh by
the vertical plane and returns deterministic outer/hole contour rings in
`(distance along section, absolute elevation)` coordinates. It handles exact
vertex/edge intersections and coplanar patches, and rejects open/non-manifold,
touching, or intersecting cut contours instead of repairing them. The section
adapter builds supported native walls with hosted door/window cuts and native
floors, cuts the bounded meshes independently, then combines coplanar contour
intervals so adjacent wall cells do not leave internal seams. Door and window
fillings are also sectioned: leaves, sashes, panes, side-lites, and frame members
use the same positive component solids as 3D scene generation. Touching
components are cut separately; their contours retain the opening UUID and
component material, so they remain independently selectable and crop-aware in
the linked drawing, sheet, and vector PDF. Host reveals stay attributed to the
wall. See [native hosted openings](native-hosted-openings.md). The plan marker is
shown in the associated level plan, and clicking it opens the linked Section
view.

Use `Place section` in a floor plan, then click two points to author the marker.
Endpoint/intersection/midpoint/grid snapping follows the active plan snap
settings. The first click is only a draft; the second creates one undoable view.
Escape, plan/document/provider changes, or an invalid marker discard the draft.
The vertical range is inferred from levels and native wall/floor extents in the
marker building, with 0.25 m padding; it can be inspected in the persisted view
definition but has no editing dialog yet.

Section drawings now render opaque, material-colored cut fills for native wall
layers, floors, ceilings, supported roofs, stairs and ramps, independently cut
curtain and railing members, and hosted door/window components. Fill regions
preserve section holes and nested islands, are clipped to the section crop, and
are shared by the canvas, linked sheet, and vector PDF. Existing contour lines
remain the selection and snapping geometry; fill triangles add no pick targets
or strokes. Each independent solid is tessellated separately, so touching
components retain their own material and identity. See the section-fill tests
for area, void, crop, material, and canvas/export evidence.

This is solid-color cut poché only: material hatch patterns, transparency,
monochrome controls, editable section graphics standards, depth beyond the
marker plane, annotations, and section view settings UI are not implemented.
A section can be placed on its own A3 sheet or combined with one Plan viewport;
each viewport has independent scale and placement, with shared vector PDF output.
The combined-sheet UI does not yet support arbitrary multi-view layouts or a
schedule table alongside the two viewports. Evidence:
`crates/os-ui/src/plan_workspace/sheet_tests/combined.rs`,
`combined_sheet_scales_history_navigation_live_edit_reopen_and_pdf`.
The renderer currently supports native straight and
bounded circular-arc walls, segmented hosted openings and their native door or
window fillings, and native floors; unsupported plugin geometry is not guessed
or included. See the remaining gaps in
`docs/2d-coverage.md`.

For face-on views and opening placement/editing, see
[linked straight-wall elevations](native-elevations.md). Elevations project
visible surfaces; sections retain their cut-plane semantics.
