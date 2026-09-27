# Native door and window tags

Model schema 31 adds persistent, view-owned tags for hosted native doors and
windows. Schema 41 adds four persisted label presets. This is an incremental
annotation feature, not a claim of Revit parity.

## Behavior

- A tag associates one opening UUID with one floor-plan view UUID and a world-XY
  label position. At most one tag for an opening may exist in a given view.
- To place one, select a door or window hosted on the active plan's level and
  invoke **Opening Tag**, then click its label position. Existing tags can be
  selected and dragged; the Properties position field also moves a tag.
- The label is derived at render time from current opening data. The Properties
  selector offers `Full` (instance · type/Legacy · dimensions), `Instance name`,
  `Type and dimensions`, and `Dimensions only`; these are closed presets rather
  than arbitrary text templates. Renaming the instance or type, or changing
  inherited/overridden dimensions, updates the relevant label without changing
  the tag identity.
- A removed opening, missing host, invalid opening, or opening rehosted to a
  different level leaves the tag in place and displays an orphan diagnostic.
  Tags are not silently rebound to another opening. An orphan can still be
  selected, inspected, moved, or deleted; it has no leader.
- Leaders are derived from each model snapshot: the host wall centerline at
  the first-jamb offset plus half the resolved opening width, transformed into
  the owning plan plane. The saved world-XY label position remains the target.
  Opening moves, effective width/type edits and Rehost update the source without
  editing the tag. The leader itself remains derived and does not add persisted
  geometry.
- Committed tags and placement/drag previews draw one straight leader to the
  nearest side of the visible label box. A source inside or effectively touching
  the box produces no leader. Leaders are clipped to the crop and canvas; an
  anchor outside the crop hides both label and leader. Picking remains badge-only,
  and leaders add no snap targets or changes to opening center move/snapping.
- Placement and movement are crop-checked and preview before commit. Drafts are
  guarded by view context, document session/revision and provider signature;
  Escape and stale contexts do not apply a partial edit. A committed operation
  is one undoable document transaction.
- Tags appear in the plan drawing and sheet output. Sheets use a centered,
  bounded paper label box with an explicitly positioned baseline and inset text
  clip. The leader stops at that box, outside the text clip, and is emitted as a
  clipped vector path before vector text in PDF. Long labels remain bounded.
  A tag-only plan remains renderable when its opening geometry is gone.

## Storage and plugin compatibility

The required `opening_tags` model map was added in schema 31. Schema 41 adds the
required `label_preset` enum. The explicit 40→41 migration sets existing tags
to `Full`, rejects an ambiguous preexisting preset, advances native entity
headers and validates atomically. Tests cover frozen schema-40 migration,
ambiguity, malformed input, and non-default preset save/reopen. The tag
introduction used API 12 / schema 31; current native full-model plugins use API
24 / schema 43. Generic plugin API 2 and container 2 are unchanged. See
[file format](file-format.md) and [plugin API](plugin-api.md).

## Evidence and limitations

Focused coverage includes model resolution/uniqueness/serialization and all
four label presets for typed/legacy doors and windows; document
atomicity/history/invalidation/orphan behavior; explicit 30→31 and 40→41
storage migrations and preset save/reopen; and nine desktop-egui acceptance
tests. Desktop cases run
at 1280×800/100% and 1000×650/150% and cover placement, live labels, select and
drag, ordinary canvas panning, cancellation, invalid release, stale drafts,
crop, duplicate rejection, Browser/Properties access, orphan retention,
overlap with a wall endpoint, and one-step history. Leader checks include exact
painted placement/drag previews, badge-only picking, opening move, effective
width/type change, Rehost, rotated plan coordinates, an oblique host, crop, and
orphan badge retention with no painted leader. The typed-window test asserts
source positions `(1.45, 0)`, `(1.5, 0)` after widening, and `(2.5, 3)` after
Rehost while preserving the saved tag. Three render tests cover all four box
sides, corners, overlap/touch omission, crop/canvas clipping, orphan behavior,
sheet text clipping, and exact vector-PDF path coordinates before text.

Leader increment checks (2026-09-27; no full-workspace test claim):

- `cargo test -p os-ui --lib opening_tag --locked --offline --quiet`:
  9 passed, 0 failed, 0 ignored; all nine exercise both DPI profiles.
- `cargo test -p os-model -p os-document -p os-storage --test opening_tags
  --locked --offline --quiet`: 6 passed, 0 failed, 0 ignored (2 per crate).
- `cargo test -p os-render --locked --offline --quiet`: 64 passed,
  0 failed, 0 ignored, including the three leader geometry/sheet tests.
- `cargo test -p os-ui --lib plan_workspace::opening_tests --locked --offline
  --quiet`: 33 passed, 0 failed, 0 ignored, including existing center move,
  snapping, jamb resize and Rehost acceptance.
- `cargo clippy -p os-render -p os-ui --all-targets --all-features --locked
  --offline -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

Label-preset increment checks (2026-09-27):

- `cargo test -p os-model --test opening_tags --locked --offline`: 4 passed.
- `cargo test -p os-storage --test opening_tags --locked --offline`: 3 passed.
- `cargo test -p os-ui --lib opening_tag --locked --offline`: 9 passed at
  1280×800/100% and 1000×650/150%; selection, preview, identity, move,
  undo/redo and plan/sheet/vector-PDF labels are checked.
- `cargo test --workspace --all-features --locked --offline`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked --offline
  -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

This increment supplies four fixed label presets and automatic straight
leaders. Arbitrary element/material tags, custom label templates, configurable leaders and tag
families/styles, annotation schedules,
native visual qualification and physical print acceptance remain open. Native
review still needs to confirm leader/text appearance at both DPIs, crop-edge
labels, and sheet/PDF font appearance and physical print output. Automated egui
shapes and PDF operator assertions do not replace that review. This increment
does not add generic plugin-authored pointer gestures or change IFC behavior.
