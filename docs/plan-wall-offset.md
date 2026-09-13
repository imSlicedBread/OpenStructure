# Signed parallel wall offset

2026-09-12: select a visible native wall in a plan and choose **Offset wall**.
The pointer determines perpendicular distance to the source centreline. Exact
signed Offset (m) overrides the pointer: positive is left of start-to-end and
negative is right, independent of plan rotation. The preview shows the parallel
axis and signed distance. Cancel wall or Escape discards it.

One destination click creates a fresh wall UUID through the bundled provider,
leaving the source unchanged. Name, level, height, thickness and material
parameters are copied, but arbitrary header properties/relationships are not.
This is centreline offset, not face clearance, a wall join or an associative
constraint. The source is excluded from snap acquisition during the gesture.

The controller projects the pointer onto the source's unit left normal in the
plan basis, then translates both original endpoints by the same world vector.
Exact offsets avoid a large-origin endpoint roundtrip. Nonfinite, zero/tiny,
below-coordinate-precision and stale drafts reject without mutation. Geometry
is checked before committing one history entry. Previews consume no history.

## Verification

- Controller test covers signed offsets of a diagonal wall at million-metre
  coordinates in a rotated plan, invalid input, source/parameter preservation,
  new identity, undo/redo, replay rejection and save/reopen.
- Real egui input test at 1280×800/100% and 1000×650/150% verifies pointer
  preview, cancellation, exact negative offset overriding the positive pointer
  side, unchanged source, two shared meshes and undo/redo.
- Full offline all-feature workspace tests, strict Clippy, formatting and
  default workspace build are checked for this checkpoint.

Native-window inspection, curve/general-entity offsets, repeated offsets,
face-relative references and independent plugin gestures remain open. No file
format, dependency or plugin API change; D and E01 remain incomplete.

Follow-up [native inspection](native-offset-save.md) verifies negative exact
offset preview/commit, split display and save/reopen of both wall identities.
