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

Focused validation:

- `cargo test -p os-ui --lib --all-features --locked --offline wall_set -- --test-threads=1` — 9 passed. The installed-provider test was ignored in that run and passed separately with `OPENSTRUCTURE_WALL_TEST_PLUGIN` pointing to `outputs/rust-wall-install` and `--ignored`.
- The document opening-clearance suite passed all 5 tests.
- `cargo test --workspace --all-features --locked --offline --target-dir work/completion-build -- --test-threads=1` passed after integration.
- Workspace Clippy with all targets/features and warnings denied, formatting, and `git diff --check` passed.

Native-window visual or production workflow qualification is not claimed.
