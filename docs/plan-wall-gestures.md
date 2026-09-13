# D: two-point native wall gestures

The active plan now offers **Draw wall in plan**. It snapshots Properties wall
defaults (including height/thickness), uses the plan's associated level, and waits
for start/end clicks. The canvas shows a transient axis preview and semantic
endpoint/midpoint/nearest snap markers. Dragging pans; scrolling zooms.

Optional **Length (m)** and **Angle (deg)** constrain the endpoint. Angle is
counterclockwise from the persisted view-plane +X axis. Blank fields use the
pointer/snapped geometry. Exact input takes precedence and is labeled as exact,
not falsely labeled as the overridden snap. Invalid text stays in the gesture
without a commit. This is an axis preview, not a full material/layer wall preview.

The second click prevalidates parameters and prism geometry, then routes creation
through the existing trusted bundled Wall command. One gesture creates one normal
history entry and regenerates the wall in plan and 3D. Previews and the first click
do not alter model/history/dirty state. Gestures bind document session/revision,
view/settings and provider activation; stale/replayed commits fail. Escape,
Cancel wall, selection/level/view changes and opening settings cancel. Model edits
and Undo invalidate the baseline; navigation retains the model-space start point.

## Evidence and limits

- `os-ui/tests/plan_gesture.rs`: independent exact length/angle expectation,
  read-only previews, one commit/Undo/Redo, replay rejection, invalid numeric
  input, inactive view and intervening model changes.
- Real egui frame/input test: draw one wall, acquire its endpoint from a nearby
  second start click, draw a second wall with exactly the shared endpoint, verify
  both 3D meshes, Undo only the second wall, then Escape another draft unchanged.
- Full locked/offline all-feature workspace suite: 197 tests including the
  doctest. Strict Clippy, formatting and default build pass. The subsequent
  [native pointer checkpoint](native-pointer-walls.md) verifies two snapped exact
  walls in split view, cancellation, save/reopen and selection on Windows.

The initial pointer route requires the bundled Wall provider. It does not invoke
Wasm synchronously or claim independent API-2 pointer-tool acceptance. The external
Wall numeric workflow remains unchanged. No format/API versions, dependencies or
unsafe-code policies changed.

D remains incomplete: grids and grid/axis constraints, move/resize, exact offsets,
richer previews, UI snap overrides, independent plugin plan tools and broader
native acceptance remain required.
