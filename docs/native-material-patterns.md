# Native Section material cut patterns

Schema 69→70 adds project-owned `MaterialPattern` definitions and an optional
pattern reference on each material. Existing materials migrate to no pattern
and retain solid cut fills. Native full-model providers now require API 51 / model
schema 70; generic plugin API 2 and container 2 are unchanged. See
[`file-format.md`](file-format.md) and [`plugin-api.md`](plugin-api.md).

The first bounded pattern is one parallel line family. Angle is measured in the
Section station/elevation plane, counterclockwise from station toward elevation;
spacing is a perpendicular model distance in metres. A shared world XYZ origin
is projected into each section plane, so phase does not restart at a component,
triangle, crop, or viewport boundary. Angle remains section-plane-relative.
Definitions validate their name, angle, spacing, origin and UUID; project
patterns are capped at 256.

The **Project materials / Section hatches…** editor is available from the
wall-type/material-layers workflow. It edits shared material colors/density,
pattern definitions and material assignments in a revision-bound draft. Apply
commits one undoable transaction; Cancel, Escape, stale documents or invalid
parameters do not mutate the project. Removing a pattern explicitly returns
its assigned materials to solid fill.

Patterns apply only to actual native vertical Section cut fills carrying the
assigned material identity, including supported wall layers and hosted
door/window components. The checked hatch vectors are derived after section
fill tessellation, merge triangle seams, respect holes, crop and compositing
masks, and use bounded work/segment budgets. Budget exhaustion rejects the
drawing rather than returning partial output. Projected surfaces are not
hatch-filled. Canvas, sheet and vector PDF share the same model-space line
segments; zoom and viewport scale do not change hatch spacing. Hatch strokes
currently use a fixed black 0.10 mm line weight.

Opening type package v7 carries exactly the material patterns referenced by its
door/window material snapshots and remaps them on import. Package v6 and earlier
upgrade with no pattern assignments. See
[`native-opening-type-packages.md`](native-opening-type-packages.md).

## Evidence and limits

- `crates/os-model/tests/material_patterns.rs`: bounds, UUID and material-reference
  validation.
- `crates/os-storage/tests/material_patterns.rs`: schema-69 migration, round trip
  and atomic ambiguity rejection.
- `crates/os-render/tests/section_hatches.rs`: triangle seam merging, holes,
  components, masking, rotated section/origin projection, crop, scale independence,
  budget rejection and exact shared sheet/PDF vectors.
- `crates/os-storage/tests/opening_type_package.rs` and
  `crates/os-ui/src/desktop_tests/opening_type_package_tests.rs`: package
  dependency transfer, legacy defaults, conflict remapping and history.
- `crates/os-ui/src/material_graphics.rs`: staged editor, commit/cancel, stale
  rejection, history and derived section hatches at 1280×800/100% and
  1000×650/150%.

This does not provide surface hatches, multiple/crossing line families, paper-
scale patterns, per-material hatch color/weight, general section graphic
templates, monochrome controls or full material appearance standards. Manual
native-window and physical-print qualification remains open.
