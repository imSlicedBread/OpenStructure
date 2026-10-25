# Native wall-set copy

## Workflow

On a floor plan, select 1–64 visible native straight walls on the same level and
choose **Copy walls**. Pick a base point, then place the copy using plan snaps or
an exact metric distance and angle. The selected source stays in place; the
committed copy becomes the current selection. Split 2D/3D refreshes after the
single undoable commit, not during preview.

### Copy aligned to another level

With the same selection, choose **Copy to level…** in the plan toolbar. In the
modal, choose an existing floor plan on a different level in the same building.
The dependency summary lists walls, hosted openings, internal joins and clearance
locks. The destination preview uses that plan's basis, range, visibility and crop;
accent lines show the copies and gray lines show existing geometry. The visible
wall count makes hidden or cropped copies apparent before applying.

**Apply copy to level** preserves world XY, height, thickness, material, name,
wall-type assignment and lifecycle, and changes the copied walls' level. Doors,
windows, instance overrides, clearance locks and complete internal joins receive
new IDs and remapped hosts/members. Source-level header references on copied
entities follow the destination level. After one transaction, the destination
plan opens and the new wall set is selected, including in split 2D/3D.

Opening tags, rooms, room tags and dimensions are **not copied across levels**;
the modal and completion status state this explicitly. Same-level **Copy walls**
continues to copy active-plan opening tags as described below.

The draft and its cached destination preview are model-neutral. Apply publishes
only after candidate validation, collision and room-boundary checks, full native
3D regeneration and destination drawing preflight in a disposable editor.
Cancel, Escape, source selection/view/document/provider changes revoke the draft.
Same-level targets, other buildings, missing/invalid views and installed Wall
providers are rejected. Vertical overlap with walls on adjacent levels is also
a collision, even when their level IDs differ. No schema, storage or plugin
protocol changes are involved.

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

- Only native straight walls selected on one active floor-plan level are supported.
  Aligned cross-level copy targets one existing floor plan at a time; there is no
  system clipboard, multi-destination paste or general entity duplication.
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

`crates/os-ui/src/plan_workspace/wall_level_copy_tests.rs` adds two real-egui
acceptance tests at both profiles. These use the visible toolbar action, chooser,
destination preview and Apply/Cancel controls with a joined enclosure, typed door
and window, instance overrides and clearance locks. They check exact source
preservation, aligned geometry, destination selection and plan picking, split
scene regeneration, remapped dependencies, excluded annotations, history and
save/reopen. Rejection coverage includes invalid/missing/same-level/other-building
targets, reflected ceiling plans, partial joins, occupied destination geometry,
vertical overlap, Escape and changed selection/view/document. An opt-in installed
guest test checks provider rejection and revocation of an already prepared copy.

The source-preservation test exposed clearance settlement rewriting an untouched,
valid source offset due to floating-point rounding. Settlement now preserves
unchanged accepted offsets; host-length, width and lock edits still solve their
constraints. `crates/os-document/tests/opening_clearances.rs` covers that regression.

## Linear wall-assembly arrays — 2026-10-23

Choose **Array walls** with 1–64 selected visible native straight walls on one
floor-plan level. The total instance count includes the source and is limited to
2–32, further bounded so no more than 64 walls or 4096 dependent openings, joins,
active-plan tags and clearance locks are added. Two clicks define the per-instance
spacing vector; the existing exact distance/angle fields and plan snaps apply.
Repeat *n* is translated by *n* times that vector.

Each copy receives independent IDs and remapped wall/opening/join/tag references.
The source stays unchanged; typed opening data, assignments, lifecycle and
clearance locks are retained. Preview draws every copy without mutating the model,
scene or history. A checked isolated regeneration and drawing preflight precede
one undoable transaction. Collisions between repeats or with existing geometry,
partial joins, room-boundary changes, invalid spacing and stale/cancelled contexts
reject the whole array. Copied tee branches are derived from the copied host's
exact station point. Installed Wall-provider batches are not supported; no schema
or protocol changed.

`crates/os-ui/src/plan_workspace/wall_assembly_array_tests.rs` covers the two
desktop DPI profiles, typed doors/windows, active-plan tags, locks, stable preview,
one-step undo/redo, save/reopen, split 2D/3D regeneration, exact non-cardinal tee
contacts, aggregate budgets, late-copy collision, room-boundary protection,
snapping, Escape/stale-selection cancellation and ordinary canvas panning. This
is automated headless evidence, not native-window visual qualification. The array
is static rather than a persistent associative Revit array, and other entity kinds
remain outside its scope.

## Static radial wall-assembly arrays — 2026-10-23

Choose **Array walls → Radial**, set the total count and signed **Angular step
(°, signed)**, then click a snapped pivot in the plan. The next canvas click
applies the preview. Positive angles rotate counterclockwise in world XY;
negative angles rotate clockwise. Switching Linear/Radial clears the base/pivot
so the next click defines it again. Escape cancels; array input owns the pointer
until completion or cancellation, after which ordinary canvas dragging pans.

Repeat *i* rotates the original assembly by *i × step* around the pivot. The
step must be finite and nonzero, and `(count − 1) × abs(step) < 360°`. Existing
eligibility and budgets apply: 1–64 visible native straight walls on one level,
complete joins, 2–32 total instances, at most 64 added walls and 4096 added
openings/joins/active-plan opening tags/clearance locks. Each repeat has stable
fresh IDs throughout preview; each transformation starts from the source rather
than accumulating the previous rotation. Active-plan opening tag positions
rotate too. Tee branch contacts use the transformed host's exact stored station.

The full candidate retains the copy dependency policy: source elements and
metadata remain intact, rooms/room tags/dimensions/other-view tags are not
cloned, and accepted room boundaries must remain unchanged. Existing-wall and
repeat-to-repeat collisions, including a collision in the last repeat, reject
the entire candidate. Preview is isolated from the live model, scene and history.
Commit preflights regeneration and drawing budgets, applies one transaction and
regenerates committed 2D/3D. Installed Wall-provider batches fail closed, including
revocation of a draft after provider replacement. No schema, storage or plugin
protocol changed. These are static independent copies; associative array
constraints, curved/mixed-level hosts and arbitrary entity arrays remain
outside this array increment. Native same-level wall assembly groups now have
separate, partial document/model support; see [native model groups](native-model-groups.md).

`wall_assembly_radial_array_tests.rs` adds headless real-egui acceptance at
1280×800/100% and 1000×650/150% for signed steps, a snapped pivot in a rotated
plan, preview isolation/stable IDs, typed doors/windows and overrides, wall
assignments/lifecycle, clearance locks, tag rotation/exclusion, source preservation,
pointer ownership, cancellation/stale context, invalid release, one-step history,
save/reopen and split scene regeneration. Additional cases cover large coordinates,
invalid angles/counts, overlapping copies, late-repeat collision, exact non-cardinal
Tee contacts and room protection. An opt-in installed-guest test exercises batch
refusal and provider-change revocation. Native-window visual QA remains open.

Prior copy/level-copy validation (before this radial increment):

- `cargo test -p os-ui --lib --all-features --locked --offline wall_set -- --test-threads=1` — 9 passed. The installed-provider test was ignored in that run and passed separately with `OPENSTRUCTURE_WALL_TEST_PLUGIN` pointing to `outputs/rust-wall-install` and `--ignored`.
- The document opening-clearance suite passed all 5 tests.
- `cargo test --workspace --all-features --locked --offline --target-dir work/completion-build -- --test-threads=1` passed after integration.
- Workspace Clippy with all targets/features and warnings denied, formatting, and `git diff --check` passed.

Native-window visual or production workflow qualification is not claimed.
