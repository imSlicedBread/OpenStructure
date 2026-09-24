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
adapter builds supported rectangular native walls with their hosted door/window
openings and native floors, cuts the bounded meshes independently, then combines
coplanar contour intervals so adjacent wall cells do not leave internal seams.
Resulting contour lines retain source entity identity for selection. The plan
marker is shown in the associated level plan, and clicking it opens the linked
Section view.

Use `Place section` in a floor plan, then click two points to author the marker.
Endpoint/intersection/midpoint/grid snapping follows the active plan snap
settings. The first click is only a draft; the second creates one undoable view.
Escape, plan/document/provider changes, or an invalid marker discard the draft.
The vertical range is inferred from levels and native wall/floor extents in the
marker building, with 0.25 m padding; it can be inspected in the persisted view
definition but has no editing dialog yet.

Section drawings show contour lines only. Cut fills/poché, depth beyond the
marker plane, annotations, section view settings UI, and multi-viewport sheet
composition are not implemented. One section can be placed on its own A3 sheet
and exported as vector PDF. The renderer currently supports native rectangular
walls with segmented hosted openings and native floors; unsupported plugin
geometry is not guessed or included. See the remaining gaps in
`docs/2d-coverage.md`.
