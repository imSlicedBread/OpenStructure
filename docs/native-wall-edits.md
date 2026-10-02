# Native wall edit inspection

2026-09-12, current default desktop build, native window 11535656, 1280×800
client area. Computer-use skill drove actual pointer and keyboard input; no
synthetic controller calls were used for this checkpoint.

## Observed workflow

1. Created the default 5 m wall and Floor plan 1. Selected-wall toolbar exposed
   Move wall, Resize start and Resize end.
2. Move acquired the original start endpoint, then a destination 65 logical
   pixels right and up. The wall moved without changing its 5 m length.
   Undo restored its original position; Redo restored the moved position.
3. Enabled Split 2D / 3D. Resize end accepted exact length 3 and angle 90.
   The destination click committed the exact perpendicular wall; both views
   changed together and Properties showed 3 m length and 1.800 m³ volume.
4. Undo restored the moved 5 m wall in both views; Redo restored the 3 m wall.
   Started Resize start and pressed Escape; the committed wall stayed intact.
5. Saved a new `outputs/native-wall-edits.osb`, reopened it through File/Open,
   selected the saved floor plan and observed the same selected perpendicular
   wall in both views with the same Properties values. Closed the clean window.

## Independently inspected saved data

Read `model.json` directly from the saved ZIP after native Save:

- Model schema 4, one wall, no grids or extensions.
- Wall UUID `77008e45-cfd6-48be-b5b0-77717c7bd539`.
- Start `(1, 1)`, end `(1.0000000000000002, 4)` metres.
- Height 3, thickness 0.2, material null.
- Ground level `4bb1ffba-690f-4fb4-876e-4d53588bf763`.
- Plan `0f8f63d4-fcc1-479d-ba0e-f1c40df62a71`, associated with that level,
  default basis/range, settings version 1 and revision 0.

Identity across edits is proven by the controller/input regression tests, not
by comparing before/after ZIPs in this native run. Resize-start commit, invalid
drafts and rotated plans remain automated-test evidence rather than native
inspection evidence. This checkpoint does not qualify production editing.

## Discovered save-path defect

The initial quick Save on an untitled project used relative `project.osb`.
Launching through the desktop tool inherited the Codex application directory,
so the attempted temporary save in Program Files failed with access denied.
The application retained the unsaved model and displayed the error. Entering
the explicit workspace output path in File and saving succeeded. No existing
artifact was overwritten.

Untitled Save should request a destination instead of silently relying on the
process working directory. This remains an open usability defect for the next
implementation pass. The skill-based inspection exposed it; no permissions or
system settings were changed to work around it.

Follow-up: [untitled quick-Save correction](untitled-save-destination.md) now
implements destination prompting and routing with passing desktop input tests.
The observations above describe the original inspected build, not the correction.

## Endpoint handles — automated follow-up, 2026-09-20

The [endpoint handle slice](plan-wall-edit-gestures.md#endpoint-handle-slice--2026-09-20-automated-evidence)
adds direct start/end dragging for selected visible native straight walls.
Headless egui desktop tests at 1280×800/1.0 and 1000×650/1.5 verify 6-point
circles, 10-point Euclidean hits, nearest/start tie handling, transient previews,
preserved identity/properties/fixed endpoints, one-step undo/redo, cancellation
and ordinary pan/selection precedence. Visibility and stale-drawing guards, snap
and exact-input reuse, invalid/outside release and cleared state are asserted.
An explicitly executed test with the existing installed Rust Wall guest also
verifies deferred worker submission/completion and pending Escape cancellation
for both endpoints at both profiles.

The all-feature locked offline `os-ui` tests pass using
`--target-dir work/endpoint-handles-astra`; commands and counts are recorded in
the linked slice document. These are automated input/shape/model assertions,
not native visual inspection. The earlier 2026-09-12 observations do not verify
the new handles. Native manual endpoint-handle acceptance remains open.

## Rotate/mirror — automated follow-up, 2026-09-27

The [wall transform slice](plan-wall-edit-gestures.md#native-wall-rotatemirror--2026-09-27-automated-evidence)
adds midpoint rotation and two-click-axis mirroring for one selected, visible
native straight wall. Headless egui tests exercise preview isolation,
hosted-door/window preservation, mirrored door/layer handedness, atomic
history, stale/cancel behavior, and regenerated split 2D/3D at both supported
DPI profiles. Joined walls, attached dimensions/opening tags, off-center typed
window panes, and installed-provider mirrors requiring companion edits are
explicitly rejected. This is not native visual inspection and does not imply
Revit parity.

## Trim/Extend — automated follow-up, 2026-09-27

The [wall gesture evidence](plan-wall-edit-gestures.md#native-wall-trimextend--2026-09-27-automated-evidence)
adds start/end trim or extend to a picked same-level wall-axis intersection.
Hosted doors/windows keep their world position when the start moves, and an
opening-fit failure rejects the complete edit. Headless UI tests cover both
desktop DPI profiles, preview isolation, one-step history and split 2D/3D
regeneration; the installed Wall guest exercises End through its bounded worker.
Joined walls and installed Start edits with hosted openings are rejected. This
is partial drafting coverage, not native visual acceptance or Revit parity.

## Align — automated follow-up, 2026-09-27

The [Align evidence](plan-wall-edit-gestures.md#native-wall-align--2026-09-27-automated-evidence)
covers one-shot parallel wall centerline alignment from the Wall transforms
menu. The source and hosted doors/windows translate together; identity,
properties, opening offsets and type assignments remain stable. Headless tests
at both DPI profiles cover transient preview, one-step history, split 2D/3D
regeneration and pointer precedence. Geometry and stale-context checks pass.
The installed-worker test is present but was not executed in this pass.
Joined/annotated sources are unsupported. Persistent locked alignment and a
constraint solver are not implemented; this does not establish Revit parity.

## Split — automated follow-up, 2026-09-27

The [Split evidence](plan-wall-edit-gestures.md#native-wall-split--2026-09-27-automated-evidence)
covers one picked interior station on a selected visible native straight wall.
The original UUID remains on the start segment; one new wall receives the end
segment and copied metadata/type assignment. Hosted openings, dimension anchors,
and room signatures are preserved or remapped in one validated transaction.
Invalid opening clearances or unresolved dimension/room dependencies reject the
entire split. Tags keep their opening target and world position.

Six focused tests pass, including both desktop DPI profiles, preview isolation,
stale/cancel behavior, one-step undo/redo, save/reopen, and split 2D/3D updates.
Joined walls and installed Wall providers are rejected. E01.19 remains partial:
multi-wall/general entity splitting and native visual acceptance are open.

## Multi-wall translation — 2026-09-29 automated evidence

Select 2–64 visible native straight walls on the active floor-plan level and
choose **Move walls**. A snapped or exact metric base-to-destination vector
translates the selection in one validated transaction. Selected entities are
excluded from snap candidates so the set cannot snap back to itself. Every wall
keeps its UUID, level, dimensions, material, and metadata; hosted doors/windows
keep their instance parameters. Opening tags translate with their hosted
openings. Associative dimensions continue resolving from moved wall anchors.
Room seeds and room tags translate only when every entity in the room boundary
signature is selected; partial room boundaries reject rather than silently
opening the room. Explicit joins likewise require every joined wall.

The isolated candidate validates the full document and meshes only the moved
walls/openings plus affected room faces during pointer preview. Preview does
not mutate the live document, 3D scene, history, or plan camera. Invalid
release, Escape, pointer loss, stale document/view/drawing/selection/provider
context, or pending plugin work cancels without model changes. Commit is one
undo step and regenerates plan/3D. Headless egui tests at 1280×800/1.0 and
1000×650/1.5 cover exact vectors, semantic snap/exclusion,
tags/dimensions/rooms/joins, stale and invalid cancellation, split redraw,
ordinary pan, and undo/redo.

Validation: `cargo test -p os-ui --all-features --locked --offline wall_set_move
--lib -- --test-threads=1` passes five tests. Native-window visual acceptance
has not been performed. This bounded native translation does not provide
general multi-entity transforms, attached constraint solving, or Revit parity.

## Copy wall assembly — 2026-10-02 automated evidence

Select 1–64 visible native straight walls on one active floor-plan level and
choose **Copy walls**. The operation copies every hosted door/window (including
hosted openings outside the current view), complete internal wall joins, and
opening tags attached in the active plan. IDs and intra-assembly references are
remapped once at draft creation; wall type assignments, instance overrides,
open states, clearance locks, lifecycle, and source metadata are retained.
Snapped or exact metric displacement is previewed without changing the model,
scene, history, or camera. A validated commit adds the entire assembly in one
undo step and regenerates the plan/3D scene.

Copy requires complete join membership and rejects wall collisions, invalid
opening geometry, stale contexts, and out-of-bounds coordinates. It does not
duplicate rooms, room tags, dimensions, or tags in other views. Curved/mixed-
level walls, partial join groups, and an installed Wall worker provider are not
supported. No schema, storage, or plugin protocol changed.

`wall_set_copy_tests.rs` covers real-egui placement at 1280×800/1.0 and
1000×650/1.5, rotated-plan exact displacement, stable draft IDs, model/scene/
history isolation, typed door/window instance data, internal joins, tag and
clearance remapping, implicit lifecycle preservation, collision and partial-join
rejection, snapping, Escape/stale selection, one-step undo/redo, and save/reopen.
These are automated assertions, not native-window visual qualification. See
[native wall-set copy](native-wall-set-copy.md) for behavior and limits.
