# Independent floor-plan annotation crops

Schema 79 / native Model API 60 adds optional `PlanSettings.annotation_crop`.
The rectangle uses the existing rotated view plane in metres. It is supported
on floor plans, including enlarged-plan callout targets. Family/package 10,
generic API 2 and container 2 are unchanged. RCP, Drafting, Section, Elevation
and Facade semantics are unchanged.

In **Plan settings**, enable a rectangular model crop, then **Enable annotation
crop**. Activation copies the staged model bounds; expand the annotation bounds
as needed and Apply. Both rectangles are validated in one transaction. Annotation
bounds must be finite, have positive spans, and contain the model crop. Removing
the model crop or expanding it beyond the annotation boundary fails atomically
with a containment message. No bounds are silently expanded or contracted.

**Edit Annotation Crop** shows a labelled boundary and eight handles. A 10-logical-
pixel hit radius and 6-logical-pixel handles remain DPI independent. Dragging
previews only the boundary. A valid release produces one UpdateView/history step;
collapse, containment failure, pointer leaving the canvas, Escape, changed view,
document session, revision, provider, drawing, camera or canvas cancels the drag.
An owned handle press blocks panning until release; other canvas drags still pan.

Model geometry, model picks/snaps and semantic anchor/source eligibility retain
the model crop. Eligible dimensions (including witness lines and label text),
opening-tag badges/leaders, independent detail lines and text notes use the
annotation crop. Dimensions and tags whose model references are outside the
model crop or unavailable are excluded. Independent lines and notes do not
require a model source. Other annotation categories retain their existing rules.
Crop graphics have no model identity and never enter picking or snapping.

Enabled annotation crops use one bounded paper-unit vector layout for native
paint, picking, sheet preview and vector PDF. Detail-line derivation and snapping
use the annotation boundary before paper adapters. Labels retain their placement
when partly clipped; strokes and glyph cells carry the same rectangular clip.
Each sheet viewport lays out paper-size text/strokes at its own scale. Native
navigation changes only projection. Label cells use the native UI font;
PDF uses Helvetica. Shared placement and clipping do not imply identical font
outlines or typography qualification. With the optional crop absent, the legacy
rendering and clipping branches are retained.

Migration 78→79 rejects any legacy `plan.annotation_crop` key, including null,
before changing input. Native headers advance; existing view parameters, IDs,
family/package data and opaque metadata are preserved. Frozen fixture:
`crates/os-storage/tests/fixtures/schema-78-annotation-crop.json`.

## Focused evidence

Commands use `./tools/cargo.ps1`, `--locked --offline`, PowerShell argument arrays,
and `-- --test-threads=1`. TMP/TEMP use workspace `work/issue-test-temp`.

- Model/storage/render `--test annotation_crop`: 3 passed. Optional legacy wire,
  strict migration/null rejection, finite/containment/type validation, atomic
  failure/history, save/reopen, clipped detail lines, annotation picking/snapping,
  and exact sheet/PDF vector placement at viewport scales 1:25 and 1:100.
- UI `--all-features --lib annotation_crop`: 4 passed. Staged settings, every
  boundary handle/hit radius, preview non-mutation, one-step history, invalid and
  stale cancellations, pan ownership, rotated plans and enlarged-plan targets,
  native glyph placement and model-source exclusion. Profiles: 1280×800/100% and
  1000×650/150%.
- UI `--all-features --lib crop_tests`: 6 passed, including the existing model-
  crop interaction regressions at both profiles.
- Render annotation/detail-line/drafting/plan/snapping focused tests: 33 passed.

Rustfmt check stopped with Windows `Access is denied`. Scoped UI-library Clippy
stopped during configuration discovery with the same error. Neither is verified;
no permission escalation was attempted. Cargo reports sandbox hard-link fallback
and path-canonicalization warnings.

No full-workspace suite, native visual acceptance, physical printing, polygonal
annotation crop, other view-kind annotation crop, rich typography or Revit parity
is claimed. Hatch work is outside this increment.
