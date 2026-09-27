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
General copy, multi-entity transforms, constraints, group editing,
general-entity gestures and independent callable plugin plan gestures remain
unimplemented. Plan-only
Shift-drag window/crossing selection is covered by [the linked workspace evidence](native-plan-workspace.md).
No dependency, API or file-format version changed in this checkpoint.

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

## Native wall rotate/mirror — 2026-09-27 automated evidence

The **Snaps → Wall transforms** controls rotate or mirror exactly one selected,
visible native straight wall. Rotate uses the wall midpoint as its pivot and
accepts either pointer direction or an exact angle in degrees. Mirror uses two
plan clicks to define the reflection axis. The disposable candidate includes
hosted door/window geometry; preview remains transient and does not mutate the
model, history, or 3D scene. A valid release commits one transaction and
regenerates shared plan/3D geometry. Wall and hosted-opening identities,
wall metadata, opening dimensions/host offsets, and window placement remain
stable. Mirroring also flips typed wall-layer side and swaps hosted door swing
handedness.

Escape, pointer loss, changed document/session/revision, selection, view,
drawing, provider signature, or pending provider work cancels the draft. The
installed Wall provider uses the existing bounded worker command path for
single-wall rotations and compatible mirrors. Mirrors needing companion door
swing or typed-layer-side edits are rejected for that provider because its
command contract cannot express the atomic batch. Joined walls, walls with
attached dimensions/opening tags, and mirrors of off-center typed window panes
are also rejected; no annotation relocation or join-graph policy is introduced.

`crates/os-ui/src/plan_workspace/transforms.rs` and
`crates/os-ui/src/plan_workspace/endpoint_tests.rs` cover pointer and exact
rotation, horizontal/vertical/diagonal mirror math, rotated plan basis at large
coordinates, hosted door/window and typed-layer preservation, preview
non-mutation, commit/undo/redo, stale and Escape cancellation, unsupported
geometry, menu/canvas precedence, and split 2D/3D regeneration at
1280×800/1.0 and 1000×650/1.5. The focused command
`cargo test -p os-ui --all-features wall_ -- --nocapture` passed 32 unit tests
plus its matching filtered integration tests. This is headless egui evidence;
native visual acceptance has not been performed. It is a single-wall editing
slice, not Revit parity or completion of milestone D.

## Native wall trim/extend — 2026-09-27 automated evidence

Select a visible native straight wall and choose **Trim/Extend start** or
**Trim/Extend end** in the wall toolbar, then click a visible straight boundary
wall on the same level. The edited endpoint is placed at the exact intersection
of the source wall axis and the finite boundary-wall axis. The opposite source
endpoint remains fixed; a result that reverses/collapses the wall, misses the
boundary segment, is parallel, falls outside the crop, or fails hosted-opening
validation does not commit. Joined source walls are rejected rather than
silently invalidating their explicit topology.

Preview changes only the highlighted wall axis. A moving start updates every
hosted opening offset by the signed along-wall displacement, keeping each
door/window at its existing world location; end edits leave offsets unchanged.
The complete `UpdateWall` plus any `UpdateOpening` commands are validated and
committed as one history step. If a door/window no longer fits, the whole edit
is rejected. Installed Wall end edits use the existing bounded command worker.
The installed API cannot atomically update the hosted-opening offsets needed by
a start edit, so installed start trim/extend is unavailable on walls with
openings. This slice does not trim both walls, chain multiple boundaries, or
create an automatic join.

`trim_extend_start_and_end_keep_hosted_openings_and_commit_once_at_both_dpis`
uses real egui toolbar and pointer input at 1280×800/1.0 and 1000×650/1.5. It
checks preview non-mutation, stable wall/opening identity and properties,
world-position preservation for typed doors/windows, split-scene regeneration,
and one-step undo/redo. The invalid-fit test confirms no partial model/history
change; the installed guest test covers a bounded-worker Trim/Extend-end commit
and Escape cancellation. These are headless UI assertions, not native-window
visual inspection. E01.18 remains partial; arbitrary entities, multi-wall trim,
auto-join and native visual acceptance remain open.

## Native wall Align — 2026-09-27 automated evidence

Select exactly one visible native straight wall, choose **Snaps → Wall transforms
→ Align wall**, then click a visible same-level reference wall. Its infinite
centerline determines the perpendicular translation; its visible footprint
determines picking. Parallel and antiparallel axes are accepted within 1e-8
radians. Shifts of 1e-6 metres or less, skew targets, self-picks and targets
outside the crop are rejected. The wall retains its direction, length, axial
position, UUID, properties and type assignment. Hosted doors/windows retain all
parameters and translate with it. The reference wall stays fixed.

Preview uses an isolated validated Document and does not change history or 3D.
Release commits once and regenerates the plan and scene. Active Align claims
pointer input before grips, selection and pan. Escape, pointer loss, stale
document/view/drawing/selection/provider context or pending plugin work cancels
the draft. Joined sources and sources with dimensions/opening tags are rejected.
The existing WallGesture adapter routes installed edits through the bounded
worker; preview never submits a guest command.

`cargo test -p os-ui --all-features --lib wall_ -- --nocapture` passed 36 tests
with one installed Align test ignored. Coverage includes both desktop DPI
profiles, menu/picking, preview isolation, full-model preservation apart from
the translated endpoints, one-step undo/redo, split regeneration, ordinary pan,
stale cancellation, rejected targets, angular tolerance and rotated plan basis
at large coordinates. The installed Align test was added but not executed in
this pass. Touched Rust files passed `rustfmt --check` with `skip_children=true`.
E01.16 remains partial: persistent locked alignment, a constraint solver, native
visual acceptance and Revit parity are not implemented.

## Native wall Split — 2026-09-27 automated evidence

Select one visible native straight wall and choose **Snaps → Wall transforms →
Split wall**, then click an interior station in its visible footprint. The pick
is projected onto the wall centerline using the plan basis; generic snaps do not
move the cut off axis. Both segments must exceed 1 mm. Picks and projected cuts
outside the crop are rejected. The start segment retains the original UUID;
exactly one new wall receives the end segment, copied header metadata, wall
parameters, and type assignment including `flipped`.

The isolated candidate validates the complete Document transaction. Openings
before the cut stay unchanged; openings after it retain their UUID and parameters
except for the new host and offset minus the split station. Cuts through openings
or their 1 mm end clearance are rejected. Opening tags retain their UUID target
and independent world position. Every source-End dimension reference, including
additional chain/baseline anchors, maps to new-wall End; source-Start references
remain. Affected dimensions must resolve before and after, retain physical
anchors, and have valid layouts; angular geometry is also compared.

For each affected room, the source directed edge becomes the two segment edges
in traversal order. Directions follow quantized coordinate order, not wall
start/end order. `FaceKey` canonicalizes the new signature. The seed must resolve
uniquely to the expected face before and after; face count, boundary geometry,
and area are checked within the room geometry lattice tolerance. An unresolved
or unsupported room/dimension remap rejects the whole candidate.

Preview paints both candidate segments without changing the document, history,
or scene. Release commits one native transaction and regenerates 2D/3D. The
transform claims input before grips, selection, and pan. Escape, pointer loss,
and stale selection/view/document/drawing/provider context cancel. Explicit
butt/corner/tee joins and all installed Wall providers are unsupported; their
single-command protocol cannot submit the compound split. No schema or protocol
changes were made. Multi-wall and general entity splits remain open.

`cargo test -p os-ui --all-features --lib wall_split -- --nocapture` passes six
tests. Headless egui input at 1280×800/1.0 and 1000×650/1.5 covers preview
isolation, pointer precedence, full-model dependency preservation, both wall
directions, all four dimension layouts, room area/signatures, invalid release,
stale/cancel behavior, undo/redo, save/reopen, and split 2D/3D regeneration.
Additional checks cover rotated plan basis at large coordinates, legacy walls,
source visibility/crop, joins, and the installed-provider rejection guard
without an external guest. E01.19 remains partial. Native-window visual
acceptance has not been performed.
