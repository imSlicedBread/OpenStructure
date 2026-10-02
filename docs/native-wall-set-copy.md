# Native wall-set copy

## Workflow

On a floor plan, select 1–64 visible native straight walls on the same level and
choose **Copy walls**. Pick a base point, then place the copy using plan snaps or
an exact metric distance and angle. The selected source stays in place; the
committed copy becomes the current selection. Split 2D/3D refreshes after the
single undoable commit, not during preview.

## Copied assembly

- Selected wall geometry and metadata, including wall-type assignments and
  effective lifecycle.
- Every hosted opening, including one not currently visible in the plan. Typed
  definitions, instance overrides, swing/hinge, pane settings, open state, and
  clearance locks are retained.
- Wall joins only when every member is selected. Join IDs and wall references
  are remapped to the copied assembly.
- Opening tags associated with the copied openings in the active plan. Their
  IDs, opening references, label presets, and positions are remapped/translated.

IDs are allocated once when the draft begins, so repeated pointer previews use
the same identities. Preview candidates are isolated and disposable. Commit
preflights the document, native wall/opening geometry, collision envelopes,
room-boundary preservation, and plan regeneration before publishing one
transaction.

## Limits

Copy is deliberately narrower than general CAD/BIM duplication:

- Only native straight walls on one active floor-plan level are supported.
- The source walls must be visible and inside the plan crop; joined walls must
  be selected as a complete group.
- Copies that overlap existing walls or one another are rejected. Hosted
  openings must remain valid on the translated hosts.
- Rooms, room tags, dimensions, and opening tags belonging to other views are
  not duplicated. Existing room boundaries must remain valid.
- Curved walls, mixed-level selections, arbitrary plugin entities, and copies
  routed through an installed Wall worker are unsupported.
- At most 4096 dependent openings, joins, and active-plan tags are accepted.
  Coordinates are bounded to ±1,000,000 m.

## Automated evidence

`crates/os-ui/src/plan_workspace/wall_set_copy_tests.rs` exercises real egui
frames at 1280×800/100% and 1000×650/150%. It verifies preview isolation and
stable IDs; joined geometry; typed doors/windows, type assignments, tags,
clearance locks, and implicit lifecycle; collision/partial-join rejection;
snapping; cancellation and stale-selection revocation; one-step undo/redo; and
save/reopen. These tests do not constitute native-window visual or production
workflow qualification, and the feature does not imply Revit parity.
