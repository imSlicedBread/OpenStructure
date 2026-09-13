# Architectural grids in plans

Follow-up: [grid forms](grid-authoring-forms.md) now create/edit datums through
desktop controls. The grid-to-wall input test now creates its grid through those
controls; the earlier checkpoint below used an injected fixture.

The [schema-4 grid foundation](architectural-grids.md) now feeds native plan
drawing, selection and the bundled wall gesture. Native grid creation/editing
forms and independently installed plan/pointer providers remain unfinished.

## Contract and limits

The bounded host snapshot copies each grid in the associated level's building,
then transforms its XY axis and bounded name on the existing background worker.
The 10,000-element limit counts walls, extensions and grids before filtering.
Vertical datums do not use wall cut/depth classification; hiding walls does not
hide grids. A separate grid category visibility control remains unimplemented.
No native schema, plan-settings or independent plugin protocol changes here.

`PlanDrawing` stores deterministic datum lines separately from wall polygons.
Crop clipping uses finite model extents. Outside parallel lines and isolated
corner contacts supply no drawing or snaps; hidden source IDs still participate
in bounds and collision checks. Native Fit includes clipped grid extents. Datums
draw behind walls, share selection with Browser and show read-only Properties.
They are not fabricated 3D solids. Initial labels at the clipped start are UI
text, not qualified paper-scale bubbles, fonts, styles or plotted output.

Picking checks foreground wall interiors first, then clipped grid lines within
6 logical pixels. Grid snap features retain their original endpoints and midpoint;
crop intersections never become fake endpoints. The nearest switch also enables
finite `GridAxis` acquisition. Priority is Endpoint, Midpoint, GridAxis, Nearest,
then screen distance and stable IDs. All drawing/picking/snap access is guarded
by document/view/settings context. Wall gestures display GridAxis feedback and
use that point; exact second-point inputs retain precedence.

Intersections, infinite axes, wall-axis-specific snapping, snap-mode UI/overrides,
per-view datum extents and public plugin providers remain pending. This extends
D without claiming its complete workflow or any E1–E4/G1–G6 qualification.

## Evidence

Windows verification: 208 tests passed with `test --workspace --all-features
--locked --offline --target-dir work/completion-build --quiet`. Strict Clippy
passed with `--workspace --all-targets --all-features --locked --offline
--target-dir work/completion-build -- -D warnings`. Commands use `tools/cargo.ps1`.
These results extend the 203-test persistence checkpoint, not production coverage.

- `os-render/tests/grids.rs`: expected crop coordinates, fixed-pixel picks at
  1/100/10,000 pixels per metre, original-feature versus crop distinction, stale
  accesses, hidden/corner/parallel lines, malformed input, identity collisions,
  10,001-input rejection and deterministic ordering/ties.
- `os-ui/tests/grid_plans.rs`: independently calculated rotated large-coordinate
  endpoints, read-only derivation/snaps, no grid solids, edit/undo staleness,
  save/reopen and exclusion of another building's datum.
- `desktop_tests::plan_grid_selection_and_wall_axis_snap_use_semantic_datum`:
  actual egui inputs at 1280×800/100% and 1000×650/150%, grid picking 5 logical
  pixels off-line, correct Properties identity, GridAxis feedback, stored wall
  endpoints (1,0)→(1,2), and whole-gesture undo preserving the grid. The fixture
  supplies its grid through Document commands, not a nonexistent creation UI.
  This is headless input evidence, not native-window or production DPI validation.

Next: native grid create/edit controls and the remaining D move/resize/offset
and independently installed plan/pointer workflows. Production targets stay open.
