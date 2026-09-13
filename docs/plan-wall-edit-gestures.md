# Wall move and endpoint resize checkpoint

Implemented 2026-09-12. This extends the bundled-provider two-point wall tool;
it does not complete milestone D or the production editing capability group.

Select a visible native wall in a floor plan, then choose **Move wall**,
**Resize start** or **Resize end**. Move takes a base-point click followed by a
destination click. Resize takes one destination click and preserves the opposite
endpoint exactly. Distance/length is in metres; angle is in degrees relative to
the active plan basis. Escape or Cancel wall discards the draft.

Previews are transient plan axes, not committed walls or live 3D solids. The
original wall remains until commit-by-click. Snapping excludes the edited wall
after choosing the move base, and throughout resize. The exclusion is part of
the snap query identity, so a result cannot be reused under another exclusion.

Commit goes through the existing bundled Wall provider's EditWall request.
UUID, header, level, material, height and thickness are retained. Move rotates
the displacement without a large-origin roundtrip; it does not assign the wall
to the current plan's level. Each non-noop commit is one transaction/history
entry and regenerates shared geometry. Raw zero-displacement move is a noop.
Document, view, provider activation and session changes invalidate drafts.
Hidden/out-of-crop targets and invalid or collapsing endpoints are rejected.

## Verification

- `os-ui/tests/plan_gesture.rs`: exact move/resize, preserved identity/properties,
  fixed endpoints, rotated large-coordinate plans, level preservation,
  non-mutating previews, single history steps, undo/redo, save/open, invalid and
  stale drafts, hidden targets and noop history.
- `os-render/tests/snapping.rs`: excluded targets are not acquired; changing
  exclusion invalidates a previous snap result.
- `os-ui` desktop input test `pointer_wall_edits_preserve_identity_preview_and_undo`:
  actual egui pointer/button events create and select a wall, move it, resize its
  end, undo/redo each edit and cancel another move. Runs at 1280×800/100% and
  1000×650/150%, with preview/model and shared-scene assertions.
- Full all-feature offline workspace tests passed; strict all-target/all-feature
  Clippy, formatting and default workspace build passed.

[Native edit inspection](native-wall-edits.md) now covers move, exact resize-end,
undo/redo, resize-start cancellation, split updates and save/reopen. It also
records an open untitled quick-Save path defect. Drag handles, copy/rotate,
constraints, multi-selection, general entities and independent callable plugin
plan gestures remain unimplemented. No dependency, API or file-format version
changed in this checkpoint.
