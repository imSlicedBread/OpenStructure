# Wall move and endpoint resize checkpoint

Implemented 2026-09-12. This extends the bundled-provider two-point wall tool;
it does not complete milestone D or the production editing capability group.

Select a visible native wall in a floor plan, then choose **Move wall**,
**Resize start** or **Resize end**. Move takes a base-point click followed by a
destination click. Resize takes one destination click and preserves the opposite
endpoint exactly. Distance/length is in metres; angle is in degrees relative to
the active plan basis. Escape or Cancel wall discards the draft.

Previews are transient plan axes, not committed walls or live 3D solids. The
original wall remains until a valid click-tool commit or endpoint-drag release. Snapping excludes the edited wall
after choosing the move base, and throughout resize. The exclusion is part of
the snap query identity, so a result cannot be reused under another exclusion.

Click tools and endpoint handles use the existing provider-routing entry points:
the bundled Wall provider's EditWall request or the installed Wall command worker.
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
records the original untitled quick-Save path defect (subsequently corrected).
General copy/rotate,
constraints, multi-selection, general entities and independent callable plugin
plan gestures remain unimplemented. No dependency, API or file-format version
changed in this checkpoint.

## Endpoint handle slice — 2026-09-20 automated evidence

Selected native straight walls expose start/end circles only when the wall is in
the checked visible drawing and that endpoint is inside both crop and canvas.
Circle radius is 6.0 logical points; hit testing uses a 10.0 logical-point
Euclidean radius, choosing the nearest endpoint and then start on a tie.

A primary handle press starts the existing `begin_plan_wall_edit` route. Dragging
shares `WallGesture` snapping, source exclusion, exact input, validation and stale
checks. Only the accent preview changes during the drag. A moved, valid release
inside the canvas calls `commit_plan_wall`; the opposite endpoint, identity and
wall properties are preserved. Installed submission ends the local drag and the
existing worker lifecycle owns completion/cancellation; handles never invoke the
bundled provider directly.

Input precedence is Escape/stale cancellation, active endpoint drag, new handle
press, existing click wall gesture, pan, then selection. Clicks without movement
(including stationary long presses), invalid drafts and releases outside cancel.
One cancellation helper clears the endpoint draft and wall gesture. A claimed
press stays reserved until button-up after cancellation so it cannot become a
pan or selection. Ordinary body/empty-canvas drags still pan; body clicks select.

`crates/os-ui/src/plan_workspace/endpoint_tests.rs` feeds real egui events through
the desktop frame at 1280×800/1.0 and 1000×650/1.5. Assertions cover rendered
circle shapes; exact Euclidean hit boundaries and nearest/start ties; selected,
visible, cropped, off-canvas and checked-drawing eligibility; both endpoint
previews/commits; unchanged model/history/scene during preview; preserved header,
name, material, level, height, thickness and fixed endpoint; single-step undo/redo;
snap and exact-input precedence; click/outside/invalid/Escape, revision/settings,
view, selection, provider and session cancellation; cleared interaction state;
and body/empty-canvas pan precedence.

The installed endpoint test was explicitly run against the existing unchanged
`outputs/rust-wall-install` guest. Both endpoints and profiles passed preview,
pending submission without immediate model mutation, worker completion, preserved
properties, undo/redo and pending Escape cancellation.

Validation commands (from the project root):

```powershell
cargo test -p os-ui --all-features --locked --offline --target-dir work/endpoint-handles-astra
$env:OPENSTRUCTURE_WALL_TEST_PLUGIN = 'C:\fKey Labs\OpenStructure\outputs\rust-wall-install'
cargo test -p os-ui --all-features --locked --offline --target-dir work/endpoint-handles-astra --lib -- --ignored --nocapture
```

The first command passes 86 tests; two installed-guest tests are ignored by
default. The second explicitly passes both installed tests (the new endpoint
test and the existing click-tool regression), so default skips are not counted
as evidence. egui input/response/options APIs were
checked against the pinned 0.33.3 local crate source. No endpoint-handle native
manual inspection was performed. This bounded slice does not complete D or
general graphical editing, and changes no schema or protocol.
