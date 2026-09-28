# Native hosted doors and windows

Doors and windows currently host only on straight native walls. Circular wall
paths are first-class native geometry, but opening placement and validation
reject them; curved jamb cuts, frames, panes and symbols are not approximated
from chords. See [native wall paths](native-wall-types.md#circular-wall-centerlines-schema-54).

## Draw opening width in plan

Activate Door or Window in a native floor plan, then check **Draw opening width**
beside Fit plan. Press at the first jamb, drag along one visible native straight
wall, and release at the second jamb. Either drag direction is supported. The
preview displays the measured width, opening symbol, and replacement wall
aperture. Both jambs use the existing host-axis opening snap preferences.

Release recomputes the candidate and uses the existing host-fit, family,
end-clearance, overlap, and atomic creation validation. A typed opening receives
an instance width override; its shared type remains unchanged. If a default type
is needed, it and the opening are created in the same undoable transaction.
The model, history, cached plan, and 3D scene remain unchanged during preview;
commit regenerates the plan and scene through the existing creation path.

Escape, pointer loss, or stale document/view/provider/drawing context cancels the
draft. Camera or canvas changes also discard the active drag. The press remains
owned through release, including invalid acquisition, so cancellation cannot
turn into canvas panning or selection. An invalid final position creates nothing.
Unchecking the control restores ordinary click placement; existing jamb resize
and click-to-copy retain their behavior.

Evidence: `draw_opening_width_preview_commit_history_and_apertures`,
`draw_opening_width_invalid_release_and_stale_press_ownership`, and
`draw_opening_width_visibility_snapping_and_return_to_pan` in
`crates/os-ui/src/plan_workspace/opening_tests.rs`. Real egui frames cover doors
and windows at 1280×800/100% and 1000×650/150%, horizontal and rotated hosts in
both axis directions, both drag directions, preview immutability and painted
apertures, shared-type preservation, default-type atomic creation, one-step
undo/redo, split-view scene regeneration, short spans, overlap, clearance,
hidden/cropped hosts, stale cancellation, snapping, and return to ordinary pan.

Limits: native straight walls on one host only; the pointer must remain within
the existing wall acquisition tolerance and visible crop at the final jamb.
This is egui interaction evidence, not native-window screenshot or print QA.
No schema, storage, plugin protocol, or 3D operation geometry changes are involved.

## Window operation plan symbols (schema 52)

Reusable window types carry `Fixed`, `Sliding`, or `Casement` operation metadata.
The type editor exposes the choice only for windows; legacy windows and migrated
types default to Fixed. Sliding adds opposing overlap/track marks, while Casement
adds a diagonal opening mark using the host's stored Start-side convention.
Fixed retains the existing plan symbol. These are documentation symbols only:
the generated window pane remains static in 3D, and the selected operation does
not alter wall cuts, meshes, opening instances, schedules, or export geometry.

Schema 51→52 adds the required type field and advances all native headers.
`.osot` v3 persists the enum; v1/v2 packages import as Fixed. Native full-model
plugins require API 33/schema 52, while generic API 2 and container 2 are
unchanged. Focused model, package, migration, and plan-symbol tests cover the
default, compatibility, distinct marks, rotated hosts, and unchanged 3D meshes.

## Fixed side-lite two-bay families (schema 50)

Reusable door/window types can optionally split their full rectangular opening
into a primary panel/pane and a fixed side lite at the type's Start or End. The
lite width is a normalized fraction of the overall opening width; the physical
mullion is 1–300 mm. Any outer frame width is reserved first, and validation
requires at least 1 mm of usable clear width in each bay. Both bays share the
type's height and the existing rectangular host-wall cut. The authored
component/cut profiles must remain rectangular while the two-bay option is on.

For doors, only the primary bay is the leaf: the existing Start/End hinge and
Left/Right swing orient that leaf within its bay, while the lite and mullion
remain fixed. Windows show both panes and the divider. The lite can reference a
separate project material; when unset it inherits the panel/pane material.
Existing frame material behavior is unchanged. The type editor exposes side,
lite-width, mullion-width and lite-material controls with the production 3D
preview. Plan symbols include the divider. Editing is draft-only until Apply;
the shared-type transaction preflights all affected instances, preserves their
UUIDs and pinned instance dimensions, and regenerates the committed 3D/plan
representation in one undoable step. The full outer opening width, jambs, host
cut, schedule/tag dimensions and outer-jamb dimension references do not change.

Schema 49→50 strictly advances each opening family from version 4 to 5 and
adds required nullable `side_lite`; existing families migrate to null, preserving
their previous single-panel geometry. Migration validates the whole copy before
adoption and advances all native headers. At schema 50, native full-model
plugins used API 31/schema 50; schema 51 used API 32. Current plugins require
API 33/schema 52. Current native full-model plugins use API 35/schema 54.
Generic API 2 and container 2 are unchanged. `.osot` package
version 2 carries the optional lite material dependency; version 3 adds window
operation metadata; version-1 packages import as single-panel version-5 families.
Evidence includes `two_bay_families.rs`, `two_bay_packages_v1_v2_upgrade_to_v3_strict_and_three_materials`,
`two_bay_preview_all_instances_history_host_cut_and_leaf_hinges`, and
`two_bay_controls_apply_cancel_and_history_at_both_dpis`.

This is a fixed two-bay composition, not arbitrary/nested family authoring. IFC
does not preserve the added lite/mullion family data: export rejects such a
family without partial output or model mutation. See the [IFC roadmap](ifc-roadmap.md).

## Per-instance lite side (schema 51)

A typed two-bay door or window can inherit its family's Start/End lite side or
pin either side independently. The selected opening exposes **Flip lite** in
the plan; the control pins the opposite effective side without editing the
reusable type. The Edit opening form can also inherit, select Start/End, or
reset to the type default. Equal-to-default pins are retained as explicit intent.
Copy and Rehost preserve the instance setting. Wall reflection retains the
endpoint-relative Start/End identity while pane-side reflection remains a
separate operation.

Schema 50→51 adds required nullable `OpeningParams.lite_side_override` to every
opening. Existing instances migrate to null; malformed, ambiguous, or
type-incompatible values reject atomically. Schema 51 used API 32/schema 51 and
`.osot` version 2; schema 52 uses API 33 and package version 3 for window
operation symbols. Generic API 2 and container 2 remain unchanged. Focused coverage is in
`crates/os-storage/tests/lite_handedness.rs`,
`crates/os-model/src/openings.rs`,
`crates/os-ui/src/plan_workspace/lite_handedness_tests.rs`, and the plan copy,
rehost, and mirror tests.

## Phase-aware saved schedules (schema 48)

Saved Door, Window and All opening schedules carry their own optional pinned
phase UUID or dynamic Latest target, plus one of the five phase filters. Newly
created schedules pin the latest phase with ShowAll. A row appears only when
both the opening and its host wall pass the filter at that target. Schedule
phase is independent of view/sheet phase, crop, range and category visibility.
RoomFinish schedules remain unphased.

The required `ScheduleParams.phase` field is migrated by 47→48. Existing
definitions migrate as `LegacyUnphased`, preserving historical rows and flat
instance CSV bytes until explicitly switched to phase-aware mode in Configure.
Pinned UUID references block phase deletion atomically; Latest follows phase
ordering changes. UI rows, placed sheet/PDF tables and instance/quantity CSV
share the same filtered row derivation. See [native phase coverage](native-phases.md).

## Per-plan door/window visibility (schema 49)

Plan settings expose independent **Show doors** and **Show windows** controls.
They are saved per view, default on for existing and new plans, and apply to the
same plan drawing used by the canvas and placed sheet/vector-PDF views. Hiding a
category hides its symbol, tag and dimensions that reference those openings; its
symbols are not pickable/snappable, and opening-specific grips/actions are
suppressed. Wall centerline snaps and unrelated annotations remain available.
Schedules keep their own category/phase rules and are not filtered by a plan's
visibility settings.

Category visibility does not remove the opening from the host-wall cut model:
the aperture remains in a visible wall when its door or window symbol is hidden.
Phase filtering is different: phase-hidden openings are removed from the
view-only wall model, along with their cutouts. Hiding walls still hides their
hosted openings regardless of the door/window toggles. Apply saves both controls
in the plan's existing single undoable settings transaction; Cancel, Escape and
stale drafts do not change the view.

Plan-settings schema 3→4 adds required `visibility.doors` and
`visibility.windows` booleans, both true. Native model schema 48→49 performs a
strict atomic migration and advances all native headers; native Model API 30
requires schema 49. Generic API 2 and container 2 are unchanged. Focused tests
cover independent plan settings, wall-cut preservation, hidden interaction and
sheet/PDF parity at 1280×800/100% and 1000×650/150%. This is limited to native
door/window categories; it is not a complete Revit visibility/detail system.

## Opening schedule CSV reports

Schema 47 adds saved `group_by` keys to Door/Window/All schedules. Configure up to
two distinct ordered keys: Level, Kind, Type, Width, Height or Sill; check/uncheck
to add/remove and use **Swap grouping order** to reorder. Save/cancel, stale-draft
guards and one-step undo use the existing definition transaction. RoomFinish
rejects grouping and has no grouping controls. The explicit 46→47 migration adds
an empty list to every saved schedule and advances native headers.

With keys selected, the read-only summary, sheet/vector-PDF table and quantity CSV
share the same Group rows and counts, first-key Subtotal rows for two keys, and a
Grand count (including zero for empty results). Only selected keys split groups:
Level uses its UUID, Type uses its UUID, and Legacy Door/Legacy Window are separate
Type buckets. Legacy dimensions split only when a dimension key is selected.
Numeric keys compare exact effective f64 values; signed zero compares equal.
Group order is ascending UUID, Door before Window, Legacy before typed UUIDs, or
numeric value, in saved key order. Instance rows retain the saved sort and UUID
tie-break inside groups and remain editable as before. UUID-bearing labels and
round-trip dimension text distinguish groups despite short instance formatting.

Grouped sheet placements show the count summary. Without grouping, sheet placements
keep instance rows and the UI/quantity CSV keep the fixed exact-variant summary
described below. Flat instance CSV always preserves its existing order, columns
and byte formatting, independently of grouping. Paper retains the eight-column,
10,000-row, rectangle/text/mark limits and reports overflow; long identity labels
may require a larger placement. Rows are never silently dropped. No pagination,
arbitrary subtotals, dimension sums or RoomFinish grouping is provided.

Saved Door, Window and All schedules also show a read-only **Quantity summary**
after the instance table and offer a separate **Export quantity CSV** action.
Both use the current committed `defined_rows` after category and AND filters.
When no configurable keys are selected, groups key on kind, stable opening-type UUID (or an explicit Legacy bucket), and
exact effective width, height and sill. Distinct same-name types stay separate;
dimension overrides split groups when their effective values differ. Equal-value
pins share a group until a type edit changes inherited dimensions. Legacy rows
combine only for matching kind and dimensions. Order is Door then Window, Legacy
then ascending type UUID, then ascending width/height/sill using f64 total order.
No cached model data or history is written; edits, undo/redo and reopen refresh it.

Quantity columns are Kind, Type, Type ID, Width (m), Height (m), Sill (m), Count.
Dimensions use three decimal places when that represents the exact f64 value;
otherwise they use shortest round-trip decimals, so nearby exact variants remain
distinguishable. Grouping never rounds values. The summary and quantity CSV share
these cells and deterministic order independently of the instance sort/columns.
Empty ungrouped quantity results export headers only. Quantity export uses the same preview,
cancel/stale checks, formula protection, 16 MiB cap and atomic replacement flow
described below. Preview/status identify quantity groups separately from instances.
Neither summary nor export is available for unsaved All-openings-live or RoomFinish.
This is a count report only: no dimension sums, descending group sort or lossless interchange.

Saved Door, Window and All opening schedules expose **Export CSV**. All contains
door/window openings only. The unsaved “All openings (live)” view and RoomFinish
schedules do not export. The dialog previews the saved name, ordered columns and
matching row count, and accepts a `.csv` path. Rows come from the same committed
flat instance derivation: configured filters, sort, effective dimensions and
stable IDs are preserved. Pending name, dimension and definition edits are absent.
Zero matches still writes the single header row; no title row is included.

Both reports use UTF-8 without BOM, comma delimiters, CRLF record endings, and
double-quote escaping for commas, quotes and embedded line breaks. Width, Height
and Sill headers carry `(m)`. Instance values retain three locale-independent
decimals; quantity values use the exact-value formatting described above.
This is a report, not lossless model interchange. For spreadsheet safety, text
whose first non-whitespace character is `=`, `+`, `-` or `@` receives an apostrophe
prefix before CSV quoting. Numeric dimensions are unchanged. The dialog discloses
both transformations. Encoded output is limited to 16 MiB; no CSV import is provided.

Cancel, Escape or closing the dialog/schedule writes nothing. Pending exports bind
to the document session/revision and selected schedule ID/definition; stale
contexts cancel before writing. Export leaves model, history and dirty state
unchanged. Existing destinations require explicit replacement consent, cleared
by any path edit. Writes use a synced temporary sibling and atomic persistence;
without consent, a destination appearing during export is never overwritten.
Failures retain the dialog for correction/consent and preserve prior destination
contents. Temporary files are cleaned up on failure.

Evidence: `cargo test -p os-ui --all-features opening_schedule -- --test-threads=1`;
CSV tests in `crates/os-ui/src/opening_schedule/csv_export/tests.rs` cover parsing,
Unicode, escaping, formula safety, units, empty results, category/filter/sort/report
parity, committed values, stale/cancel guards and atomic filesystem behavior.
Broader schedule coverage remains partial (no custom fields or pagination).

Quantity increment validation: 37 focused schedule tests passed with
`cargo test -p os-ui --all-features --locked --offline opening_schedule -- --test-threads=1`.
Targeted `cargo clippy -p os-ui --all-features --all-targets --locked --offline -- -D warnings`
and `cargo fmt --all -- --check` passed. Quantity tests cover exact nearby variants,
same-name type identities, legacy buckets, filtered counts, UI ordering/suppression,
CSV parity, history/reopen, read-only state and shared export safeguards. UI evidence
is headless egui; native-window visual acceptance remains open.

## Instance window pane position (schema 46)

Typed windows store required nullable `pane_position_override`. Null inherits
the shared type's pane position; Center, LeftFace and RightFace pin the instance,
including a value equal to the current default. Doors and legacy openings require
null. Type pane edits affect inheriting windows; pinned windows keep their position.
Edit opening in Properties/Exact Edit offers the three positions, inheritance,
effective/default values and **Reset pane to type default**.

A selected typed window with an effective LeftFace or RightFace exposes **Side**
in plan. It pins the opposite face in one undoable instance update. Center has no
flip control. Preview, Escape, pointer loss, drag and stale context retain the
existing cancellation rules. Copy and rehost retain the stored override.

Native straight-wall mirror swaps each hosted off-center typed window's effective
face into an instance override in the same transaction as the wall. This reflects
pane/frame geometry without changing shared types or windows on other walls.
Centered and legacy windows retain their parameters. Installed Wall providers
reject mirrors requiring these companion edits. Rotate, schedules and IFC are
outside this change.

Schema 45→46 validates native collections and header versions, then adds explicit
null to every opening. Missing current fields, malformed values and partial
migrations fail atomically. Native full-model API is 27; generic API 2 and container
2 are unchanged. Evidence: model pane override tests, storage `window_panes.rs`
with frozen schema-45 fixture, and real egui form/side-flip/mirror tests at both
desktop DPI profiles. These are headless tests, not native-window visual QA.

## Instance window sill

Typed windows inherit their type's **Default sill** unless **Override sill** is
selected in Edit opening. The form displays the effective value and offers
**Reset sill to type default**. An override equal to the default remains pinned
across later type edits. Width and height independently inherit or override type defaults. Legacy
windows retain their own sill; converting one to a type uses it as the default.
Duplicating a type from an overridden instance retains the source type default.
New typed windows inherit. Selected schedule windows expose **Edit window
properties** through the same instance form; schedule values and sorting use
the effective sill. Preview and cancellation leave model/history unchanged;
commit regenerates the host through the existing atomic opening transaction.

Native schema 29 introduced explicit `sill_override: null` or a number. IFC v3
preserves inheritance and pinned values in the bounded OpenStructure dialect;
see [IFC roadmap](ifc-roadmap.md). Native visual QA remains separate from the
headless egui tests.

This is a bounded native-model slice, not a production architectural profile.
Doors and windows are `core.opening` instances hosted by straight native walls.
Instances keep their UUID, name, host, first-jamb offset, hinge and swing; project-owned
`core.opening_type` entities own the kind, width, height and default sill, measured in
metres. Typed instances reference the type and can independently pin width,
height and (windows only) sill. Schema 30 adds required nullable width/height
overrides; atomic 29→30 migration leaves existing openings inheriting. The form
shows effective/default dimensions, independent Inherit/Override selectors and
reset actions. Disabled fields are not parsed. New typed openings inherit;
legacy conversion uses legacy dimensions as defaults. Deriving/duplicating a
type from a pinned instance uses source type defaults, not effective dimensions.
Effective dimensions drive geometry, schedules/sorting and quantity consumers.
Type edits update inherited values on assigned openings. Schema 7 projects migrate
to schema 8 by wrapping each existing opening as an independent legacy definition;
its identity and geometry remain unchanged.

Schema 9→10 adds required `hinge` (`Start`/`End`) and `swing` (`Left`/`Right`)
instance fields. Both migrated and new openings use **Start / Left**. This retains
the existing door leaf exactly: its local hinge is `(offset, -thickness/2)` and
its open tip is `(offset, width-thickness/2)`. Migration advances every native
entity header to 10, preserving IDs, type references, dimensions, metadata and
opaque extensions. Earlier schema migrations chain through this step.

Hinge Start is the opening jamb at `offset`; End is at `offset+width`. Swing is
relative to the wall's stored start→end direction: Left is the positive local
normal and Right is negative. The right-side leaf mirrors the transverse hinge
position to `+thickness/2` and extends toward decreasing local y. The leaf opens
90° toward the selected side. Reversing host endpoints reverses this local frame:
the offset is measured from the new start and Left/Right follow the new direction;
orientation is not automatically adjusted to preserve a world-space swing.
Windows require Start/Left and retain their existing pane and plan symbol.

Selected doors also expose [direct plan controls](#direct-plan-door-controls)
for each orientation field.

Schema 22→23 adds `pane_position` to reusable opening types. Existing types
migrate to **Center**, which preserves their previous centered pane. Window types
can instead place the pane flush to the host wall's **Left face** or **Right
face**, named relative to the host's stored start→end direction. The wall opening,
jambs, and host cells do not move; plan and 3D pane geometry share the same
calculated offset. Legacy windows remain independent and centered. Door types
require Center. The window type editor exposes the three choices; a shared type
edit updates inheriting instances in one transaction, with Cancel and stale
document guards unchanged. At schema 23, native full-model plugins used API 5 /
schema 23; the
generic DTO protocol is unchanged.

Schema 23→24 adds a version-1 declarative `OpeningFamily` to each reusable
opening type. Existing types migrate to the full rectangular profile extrusion,
preserving their previous leaf/pane geometry, stable IDs, metadata and opaque
extensions. The family currently supports one simple closed elevation profile
(3–32 vertices expressed as width/height fractions) and one positive extrusion;
depth is bounded to 1–200 mm and further capped to 20% of host thickness and
opening width. The hosted wall void remains rectangular. The type editor supports
dragging, inserting/removing vertices, numeric vertex edits, reset-to-rectangle,
depth edits, plus draft 3D and plan previews through the production evaluators.
Invalid/self-intersecting profiles cannot apply; shared type changes validate all
placed instances and commit once. Native full-model plugins use API 6 / schema 24.

Schema 24→25 advances `OpeningFamily` to version 2 with optional frame width and
depth. Existing schema-24 families migrate with frame width zero, retaining their
exact prior panel/pane geometry. **Add frame** creates separate closed jamb/head
solids for doors, and jamb/head/sill solids for windows; the editable panel/pane
is inset to meet those members. Frame width must leave at least 1 mm of component
clearance and is shared by every instance of the type. Frame depth is capped by
the host thickness. The type editor previews the combined assembly; normal plan
cuts keep the host jamb convention and use the same inset panel spans as 3D.
The wall void remains rectangular and the frame members are generated from fixed
profiles rather than independently sketched family components. Native full-model
plugins at this stage use API 7 / schema 25.

Schema 25→26 advances `OpeningFamily` to version 3 and adds a separate normalized
`cut_profile`, independent of the positive leaf/pane profile. Existing types
migrate to the full rectangle, so their wall mesh, plan and section geometry are
unchanged. `Rectangular` keeps the legacy cut; `Profile` uses the authored simple
polygon to cut the straight host wall. The exact piecewise-linear wall partition
feeds 3D meshes, plan cut/projection footprints and picking, vertical sections,
net volume, per-layer quantities and density-based mass. No boolean kernel or
stair-step approximation is used. The 3–32-vertex polygon must pass the same
bounded topology checks as the component, and the component must fit inside the
cut. Generated rectangular frame rails are currently supported only with a
rectangular host cut; incompatible edits are rejected before transaction commit.
Profiled cuts cannot be represented by the legacy vertical rectangular `Solid`
API, so those consumers return an explicit unsupported-geometry error instead
of exporting a false rectangle. Current native full-model plugins use API 24 /
schema 43.

Linear plan dimensions may reference either jamb of a native door or window.
The reference is associative: the jamb resolves from the current host, offset,
and effective typed/legacy width, so rehost, resize, type edits and undo/redo do
not leave cached measurement coordinates. Dimension pick/repair only accepts
opening jambs whose native symbol lines are visible in the current plan range
and crop. See [native dimensions](native-dimensions.md).

Use an active floor plan on the host wall's level. The Architecture ribbon's Door
or Window action starts a repeatable pointer tool: choose a saved type from its
type picker, hover a visible straight wall for a transient symbol/offset preview,
then click to place. If no type exists for that kind, the first valid placement
creates a default reusable type and its first instance in one history transaction.
Each click is independently undoable; continue on other walls or press Escape to
exit. Default dimensions are 0.90 × 2.10 m for a door and 1.20 × 1.20 m with a
0.90 m sill for a window. Invalid overlap or clearance previews in error color
and cannot commit.

The ribbon type menus create or select reusable types. The type editor supports
rename, dimension edits, duplicate and deletion when unused; dimension changes
validate all assigned instances atomically and regenerate their plan and 3D host
geometry. `Exact new…` retains numeric placement; its instance form edits name,
host and offset, while type dimensions are edited through the assigned type.
For doors, `Edit opening` / `Edit opening properties` and `Exact new…` also expose
Hinge (Wall start/end) and Swing (Left/right of wall). A validated plan-symbol
preview is shown in the form without changing the document. Apply updates the
instance through `UpdateOpening` in one undoable entry; Cancel/Escape discards the
draft. Session, revision, view, selection or provider activation changes invalidate
the draft. Invalid edits leave model and history unchanged.
Existing legacy openings remain independent until explicitly converted with
“Create reusable type from this opening”. Apply, edit, delete, conversion, and
host-with-openings deletion each use one history transaction. Invalid dimensions,
insufficient clearances, overlaps, non-floor doors, referenced type deletion, and
host edits that no longer fit are rejected atomically.

Select a visible door or window in its host-level floor plan to show one center
grip. Drag that grip to move the opening along its current straight native wall.
Pointer motion is projected onto the wall's stored start-to-end direction,
including reversed hosts; the press position is retained so grabbing near the
grip does not jump the opening. The offset is the only edited value: UUID, name,
host, type/family, width, height, sill, hinge and swing are preserved. Enabled
semantic snaps are acquired at the candidate opening center after correcting
for the press offset. Use the separate Rehost action below to change its host.

At sufficient zoom, the selected opening also shows two diamond jamb resize
handles. **Resize opening** moves the grabbed jamb along the host axis while
keeping the opposite jamb fixed. The initial press station is retained, so an
off-center grab does not jump. Typed resizing pins width; legacy resizing edits
the legacy width. Height, sill, identity, host, type/family and orientation remain
unchanged. A no-op does not create a pin or history entry. Crossing, sub-millimetre
width, frame/profile fit, clearance and overlap failures cancel the release.
Acquisition uses a 10 logical-pixel radius. Both jambs are
suppressed when their acquisition discs overlap each other or the center grip.
Active tools, wall handles and Rehost precede jambs; jambs precede the center
grip and selection/pan. Axis-oriented resize cursors and diamond hover/active
labels distinguish resize from the center grip's fixed-width move.

Select a visible hosted door or window and choose **Copy opening** in Properties
to start a repeatable plan placement tool. Click any other visible wall on the
same level to create a distinct opening instance. It preserves the source name,
legacy or typed definition, type/family assignment, dimension and sill overrides,
and door hinge/swing; only host and first-jamb offset change. The source remains
the copy template for subsequent clicks. Existing host-fit and overlap checks
apply, including rejection of placement over the source. Each valid click is a
single AddOpening history transaction; preview is disposable and does not update
3D until commit. Escape exits; stale view/document/provider context cancels.
General-purpose copy/array tools and copying non-opening entities are not implied.

`opening_copy_preserves_instance_and_type_on_another_wall` exercises doors and
windows, typed and legacy sources, both desktop DPI profiles, preview immutability,
overlap rejection, cross-wall placement, property/type preservation, regeneration,
and one-step undo/redo. Native-window visual qualification remains open.

Center and jamb snapping share the existing snap solver with enabled endpoint, midpoint,
intersection and nearest preferences, restricted to the host axis and other
visible same-host jamb segments. The edited symbol and aperture are excluded.
The resulting whole opening must still pass clearance and overlap validation.
Both grip paths use disposable previews and one changed valid release transaction;
session/revision, drawing, view/settings, selection and provider checks cancel
stale edits. Pointer loss cancels even if the pointer later returns before release.
Cancelled presses remain claimed until pointer-up.

Focused `opening_jamb_*` egui tests extend center-grip coverage to both jambs,
doors/windows, typed/legacy definitions, reversed hosts and both DPI profiles,
including preview/history/regeneration, invalid final release, cancellation,
snapping, hit radius, collision suppression, visibility and tool precedence.
These are headless tests; manual native visual qualification remains open.

Focused `opening_center_snap_*` tests cover acquisition at the corrected center,
edited-aperture exclusion, snap preferences, cropped targets, final-release
clearance/overlap validation, immutable preview/model/history/scene, and one-step
undo/redo. They exercise doors/windows, typed/legacy instances and reversed hosts
at 1280×800/100% and 1000×650/150%. Existing `opening_drag_*` tests cover
cancellation after snap acquisition and selection/pan precedence. This uses the
existing same-host snap references and adds no snap kinds or schema changes.

The separate **Rehost** action in the Architecture ribbon moves the selected
door or window to a different visible native straight wall on the active plan
level. It uses the same wall hits and crop checks as pointer placement. Hover
centers the opening at the projected wall station and previews both host
apertures and the selected symbol; the old host closes and the new host opens.
Only the paint stream is replaced. Model, history, cached drawing, picking and
3D scene remain unchanged until commit.

Click recomputes and validates the final candidate, then submits one
`Command::UpdateOpening`. Only host and first-jamb offset change; UUID, name,
typed or legacy definition, hinge/swing and all other instance properties remain.
Undo/redo restores both host walls and the opening in plan and 3D. An absent
target, same host, invalid fit or overlap displays an error and leaves Rehost
active after a click. Escape, lost pointer, outside release or changed document
session/revision, view/settings, selection, provider activation/signature or
drawing identity cancels the tool. A claimed press stays owned until pointer-up,
preventing delayed pan or selection; pointer-up clears ownership even outside
the canvas or after leaving the plan.

Headless egui Rehost evidence in `crates/os-ui/src/plan_workspace/opening_tests.rs`:
`opening_rehost_preview_commit_history_and_both_host_regeneration`,
`opening_rehost_invalid_absent_target_and_final_click_revalidation`,
`opening_rehost_cancel_stale_and_pointer_ownership`, and
`opening_rehost_target_visibility_level_crop_and_fit`. These exercise doors and
windows at 1280x800/100% and 1000x650/150%, including typed/legacy definitions,
reversed target walls, painted preview geometry, unchanged picking/cache/scene,
one-entry history, both-host regeneration, fit/overlap, and cancellation guards.
This adds no schema/protocol changes or plugin-host support. Native manual
visual qualification remains open.

A valid drag previews both the symbol and the host aperture using disposable
native plan geometry. It does not change the document, history, cached drawing
or 3D scene. A changed valid release inside the canvas submits exactly one
`UpdateOpening`; undo/redo restores/reapplies the move and regenerated geometry.
A click or a return to the original offset creates no history entry. End
clearance violations, overlaps and sub-millimetre/collapsing wall piers show an
error and cannot commit, including when the last hover was valid but release
is invalid. Escape, changed document session/revision, view/settings, selection,
provider activation or drawing identity, and release outside the canvas discard
the draft. Pointer ownership persists until release after cancellation, preventing
the same press from becoming a pan or selection. Active placement and wall
endpoint handles retain precedence; ordinary canvas drags still pan.

Focused headless egui evidence in `crates/os-ui/src/plan_workspace/opening_tests.rs`:
`opening_drag_preview_aperture_and_single_update_history`,
`opening_drag_rejects_clearance_overlap_and_collapsing_pier`,
`opening_drag_cancel_stale_outside_and_click_leave_no_residue`, and
`opening_drag_visibility_and_pointer_precedence`. These cover doors/windows,
typed/legacy instances, rotated forward/reversed hosts, preview paint geometry,
one-entry history, identity/property retention, invalid release and cancellation,
and 1280×800/100% plus 1000×650/150% profiles. Native manual visual inspection
and installed custom provider host support are not claimed.

Rectangular apertures retain the historical disjoint-cell path. Profiled cuts
partition wall elevation into convex linear regions; the same resolved family
drives plan footprints, section contours, wall meshes and layer quantities. A
thin leaf/pane has its own semantic entity for 3D and plan picking. This is not a
welded boolean B-rep: internal partition faces remain in the combined mesh.
`WallIfc` now exchanges the exact default rectangular door/window family through
IFC4 host voids and linked fillings. It preserves instance/type UUIDs and names,
typed versus Legacy definitions, host, offset, sill, dimensions and all four door
orientations. IFC host geometry is the full uncut source prism; void relationships
subtract full-thickness vertical-profile openings. Shared types requiring
conflicting IFC operation values are rejected, never split or merged. Custom
profiles/families, frames, noncenter pane alignment and unused types remain
unsupported. Fillings have no IFC component Body; export requires acknowledgement
of omitted panels/panes, family details and materials, and import reports default
component regeneration. See the [IFC contract and orientation mapping](ifc-roadmap.md#bounded-hosted-opening-contract).

IFC evidence: `os-ifc/tests/exchange.rs` covers semantic round trips, type identity,
reversed/rotated/elevated hosts, regenerated holes, malformed relationships and
transforms, unsupported profiles/families and all-or-nothing model validation.
The independent `validate_hosted.py` gate passed IFC4 EXPRESS and Open CASCADE
geometry checks on 20 fixtures / 80 openings with IfcOpenShell 0.8.5. This does
not extend the existing wall-only viewer qualification to doors/windows.

Door symbols add a quarter-circle from the closed leaf direction to the open tip,
using 16 fixed line segments. Jamb features remain 0/1, the leaf is 2, and arc
features are 3–18, all carrying the opening ID. Symbols respect view range and
crop, including when only the swing extends into the crop; drawing and picking
use the same clipped segments. Clicking the leaf/arc selects the opening, while
active placement retains precedence. Hinge/swing alter neither host apertures nor
shared type dimensions. No animation, plugin protocol change, installed-plugin-host
support or general mirror-tool behavior is introduced.

Automated evidence: `crates/os-document/src/tests/openings.rs` validates shared
type assignment, atomic edit, conversion and history; `crates/os-storage/src/tests/openings.rs`
covers schema 7→10, frozen 9→10, 22→23, 23→24, 24→25 and 25→26 migrations, atomic
rejection, opaque-data preservation and archive round trips. `crates/os-model/src/opening_family.rs`
validates both profile topologies and component containment. `crates/os-geometry/src/openings.rs`
checks component extrusion and plan spans; `crates/os-geometry/tests/host_cut_profiles.rs`
checks triangular/arched cuts, orientation/winding, plan cut/projection crop and hit
geometry, section contours, layer volume/mass, joined wall members and exact default
rectangle equivalence. `crates/os-ui/src/opening_type_tools.rs` and
`crates/os-ui/src/opening_profile_tests.rs` cover shared-instance preflight, invalid
rollback, preview-only drafts, plan/mesh agreement, history, stale rejection and
archive reopen.
`crates/os-ui/tests/openings.rs` covers missing volume, plan gaps, picking,
save/reopen, regeneration, all four orientations for typed/legacy doors in both
wall directions, matching 3D leaves, both-direction window pane placement in plan
and 3D, shared type history/save/reopen, and cropped-arc picking. Desktop egui
tests exercise pointer preview/place, repeat placement, cancellation, collision
rejection, exact create/edit/delete, orientation preview/apply/cancel/stale
rejection, type-level pane-position cancel/stale/apply, component and host-cut
profile tabs, vertex dragging, cut-from-component, preview-only drafts, invalid
rejection, undo/redo, and both supported DPI profiles. These are headless egui
tests, not manual native visual acceptance.

The **View** ribbon's **Door/window schedule** button opens a live
native schedule with separate Door and Window tables. Rows show instance name,
resolved type (or legacy instance dimensions), host level and wall, width, height,
sill in metres, and the complete stable instance ID. Rows sort by level name,
kind, instance name, then ID. Values resolve from the current model every frame;
type edits, undo/redo, and reopened projects require no stored schedule copies.
Click any cell to select its opening and open a configured floor plan on its
host level (lowest stable view ID when multiple plans exist). With no such plan,
selection remains active and the schedule explains how to enable navigation.
Existing view visibility and crop settings still govern viewport highlighting.

Each row has a separate **Edit name** action. Typing changes only a local draft;
**Apply** submits one `Command::UpdateOpening` transaction and **Cancel**, Escape,
or closing the schedule discards the draft. That name editor leaves type,
dimensions, host, and level read-only. A changed document session/revision cancels the editor, including
changes caused by undo/redo. Invalid names display validation errors while retaining
the draft and leaving model/history unchanged. A successful rename is one undo step.

Focused evidence in `crates/os-ui/src/opening_schedule.rs`:
`resolved_rows_follow_type_history_and_archive_without_cached_data` covers typed
and legacy values, host/level lookup, type changes, undo/redo, and archive reopen;
`deterministic_order_and_invalid_references_are_explicit` covers stable ID ties
and missing-host diagnostics; `row_click_selects_and_navigates_without_model_or_history_changes`
exercises actual egui row clicks with and without a plan at 1280×800/100% and
1000×650/150%, checking unchanged model/history and visible missing-plan guidance.
`view_ribbon_opens_schedule_window` preserves View-ribbon access. The focused tests
`instance_name_editor_remains_draft_only_undoable_and_stale_safe`,
`invalid_name_draft_retains_value_and_model_until_correction`, and
`name_editor_cancel_and_escape_discard_without_history` cover egui editing/Apply,
undo/redo, validation rollback and correction, cancellation, and stale rejection.
For saved Door and Window schedules, clicking Width or Height opens a transient
metre-value cell editor; Window schedules also allow Sill, while Door sill stays
zero and read-only. The existing instance-name action remains available. All,
RoomFinish and other nonnumeric cells remain read-only. Typed edits pin the
instance override even when it equals the type value; **Inherit type value / reset
override** clears it. Legacy openings update their own stored dimensions and have
no inheritance control. Invalid or non-finite input and host-fit/clearance failures
retain the draft and leave model/history unchanged; a no-op adds no history.
Escape, Cancel, closing, or a stale document/opening/schedule context discards the
draft. Only committed model values feed live schedule rows and the existing sheet
preview/PDF table. Focused evidence is in
`numeric_schedule_cell_pointer_edit_opens_editor_and_enter_commits`,
`typed_schedule_dimensions_pin_equal_inherit_and_follow_type_edits`,
`legacy_window_schedule_edits_fields_and_door_sill_is_suppressed`,
`invalid_numeric_edit_rolls_back_retains_draft_and_allows_correction`,
`numeric_edit_cancel_escape_and_stale_schedule_or_session_never_commit`, and the
schedule/PDF assertions in `saved_schedule_sheet_live_preview_pdf_history_overflow_and_stale_export`.
The same window now includes a saved-definition picker, **New Door schedule**,
**New Window schedule**, **Configure schedule**, and **Delete schedule**.
Creation opens a draft with a unique suggested name and all eight columns.
Configuration edits the name, Door/Window/All category, included columns, column
order (left arrows), and ascending sort (level/kind/name, name, type, width, height,
or sill). Stable instance ID breaks sort ties. **Save definition** commits one
transaction; Cancel, Escape, close, or a stale document discards the draft.
Invalid definitions retain the draft with an error. Deletion is undoable.
Saved definitions are model-owned `core.schedule` entities in schema 16 and
survive save/reopen; rows and selection remain derived/transient. The picker also
retains the original **All openings (live)** tables. Configured schedules display
one table so sorting applies across both categories when All is selected.

Evidence: `os-model/tests/schedules.rs` validates identities, bounded unique names,
columns and retention accounting; `os-document/tests/schedules.rs` covers atomic
commands and one-step history; `os-storage/tests/schedules.rs` covers representative
15→16 migration, atomic rejection, opaque data preservation and multiple-definition
archive roundtrip. `os-ui/src/opening_schedule/definitions.rs` tests category/column/
sort derivation, live edits/history, draft guards and actual create/configure/picker/
navigation clicks at both supported profiles. These are headless egui tests, not
manual native visual acceptance.

### Persisted opening schedule filters (schema 42)

Configure a saved Door, Window or All schedule to add, edit, remove or reorder
up to 32 filters. Empty filters include all rows in the selected category;
otherwise every rule must match (AND). Text fields Name, Type, Level and Host
support Equals, NotEquals, Contains and StartsWith. Matching uses Rust lowercase
mapping on both strings; accents and canonical Unicode forms are not normalized.
Text must be trimmed, nonempty, at most 256 UTF-8 bytes and contain no controls.
Numeric Width, Height and Sill rules support Equals, NotEquals, Less, LessOrEqual,
Greater and GreaterOrEqual with finite f64 metre thresholds. Comparisons use
live full-precision values, without tolerance or formatted-cell rounding.

Category selection and filters run in `defined_rows` before the configured sort.
That shared path drives the schedule window and `paper_table`, which supplies
sheet preview and searchable vector-PDF tables. Instance/type names, host/level
names and effective dimensions are resolved again after source edits and history
changes. Rules stay in the existing definition draft until Save; validation,
Cancel, Escape, close, stale revision/session and one-step history retain their
existing behavior. Switching to RoomFinish clears opening filters through the
category reset. Filters cannot be saved on RoomFinish schedules.

Schema 41→42 requires the schedules map, inserts empty filters into each legacy
definition, rejects an already-present filters field, and advances all native
headers atomically. The dedicated frozen schema-41 fixture verifies preservation
of existing data, archive round trips, ambiguity rejection and atomic failure.

Evidence: `crates/os-model/tests/schedule_filters.rs` (validation, strict tagged
serde, operators and memory); `crates/os-storage/tests/schedule_filters.rs`
(frozen migration and persistence);
`crates/os-ui/src/opening_schedule/definitions/tests/filters.rs` (AND/category,
case, exact boundaries, live source edits, history, visible rows and draft controls);
`crates/os-ui/src/plan_workspace/sheet_tests/filters.rs` (included/excluded
searchable PDF text, visible sheet rows, live dimensions, history and reopen).
UI acceptance runs at 1280×800/100% and 1000×650/150% using headless egui.

Remaining schedule scope: RoomFinish filters, OR/nested predicates, custom fields,
totals beyond the bounded opening group counts/subtotals, styles/conditional formatting,
RoomFinish CSV and pagination. One live saved
table can already be placed on a sheet. This increment does not complete S01 or
claim native-window visual or physical-print qualification.

Still limited: one profile-extruded panel/pane with generated frame members and a
rectangular or simple polygon host cut; arcs/splines, multiple rings/holes,
arbitrary/multiple/nested components, constrained sketches, formulas, material
assignment, libraries, general mirror/flipping
tools, tags, curved/plugin-defined hosts, general IFC family/component round-trip,
or production readiness.
Generated frames require a rectangular host cut. Pointer
placement is limited to visible native straight walls. Native manual visual
inspection has not been performed.

## Sill override acceptance — 2026-09-26

Verified on the combined uncommitted jamb/Rehost + sill-override worktree:

- Full workspace: `cargo test --workspace --all-features --locked --offline --quiet`
  passed, 559 tests passed, 0 failed, 3 ignored (including doc tests).
- `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings`
  passed. Formatting and `git diff --check` passed.
- Focused sill UI tests: 3 passed. Instance controls and schedule editing run at
  1280×800/100% and 1000×650/150%. They cover inherited/override/reset behavior,
  disabled invalid text, invalid apply, Cancel/Escape/stale rejection, history,
  host regeneration, effective plan/section/3D geometry and consistent volume.
- Storage: 75 passed, including frozen schema 28, explicit null insertion,
  ambiguity/missing-field rejection, earlier migration chains and save/reopen.
  The chain now iterates explicit steps to avoid overflowing the default thread
  stack; existing snapshot assertions still compare all prior fields exactly.
- IFC exchange: 18 passed, 1 ignored. V2 preserves differing instance sills and
  explicit `Some(default)`; the frozen v1 fixture imports as inherited. Missing,
  inconsistent and malformed v2 metadata fail. The ignored independent
  IfcOpenShell gate was not run.
- Plugin host: 69 passed, including bounded workers with the API-10 probe fixture.
- All pre-existing added jamb/Rehost source lines remain present. No commit was
  created. Native manual visual acceptance remains open.

Exact files changed for this increment, relative to the repository root.
Existing six-file work was retained; shared files received additive sill changes,
required constructor initialization or documentation updates.

```text
README.md
crates/os-document/src/tests/openings.rs
crates/os-geometry/src/openings.rs
crates/os-ifc/src/reader.rs
crates/os-ifc/src/writer.rs
crates/os-ifc/tests/exchange.rs
crates/os-model/src/lib.rs
crates/os-model/src/memory.rs
crates/os-model/src/openings.rs
crates/os-plugin-api/src/lib.rs
crates/os-plugin-host/src/lib.rs
crates/os-plugin-host/tests/wasm_transport.rs
crates/os-storage/src/lib.rs
crates/os-storage/src/tests/openings.rs
crates/os-storage/tests/fixtures/schema-28-window-sills.json
crates/os-storage/tests/rooms.rs
crates/os-storage/tests/wall_types.rs
crates/os-storage/tests/window_sills.rs
crates/os-ui/src/desktop_tests/opening_family_tests.rs
crates/os-ui/src/opening_family_editor.rs
crates/os-ui/src/opening_profile_tests.rs
crates/os-ui/src/opening_schedule.rs
crates/os-ui/src/opening_schedule/definitions/tests.rs
crates/os-ui/src/opening_sill_tests.rs
crates/os-ui/src/opening_tools.rs
crates/os-ui/src/opening_type_tools.rs
crates/os-ui/src/plan_settings.rs
crates/os-ui/src/plan_workspace.rs
crates/os-ui/src/plan_workspace/opening_tests.rs
crates/os-ui/src/plan_workspace/sheet_tests.rs
crates/os-ui/tests/openings.rs
crates/os-ui/tests/perpendicular_joins.rs
crates/os-ui/tests/sections.rs
crates/os-ui/tests/wall_joins.rs
crates/os-ui/tests/wall_types.rs
docs/2d-coverage.md
docs/file-format.md
docs/ifc-roadmap.md
docs/native-hosted-openings.md
docs/native-wall-types.md
docs/plugin-api.md
fixtures/wasm-probe/plugin.toml
fixtures/wasm-probe/probe.wat
fixtures/window-sill-v1.ifc
plugins/walls/plugin.toml
plugins/walls/src/lib.rs
```

## Dimension overrides and anchored resize acceptance — 2026-09-26

This increment builds on the preceding uncommitted sill and jamb/Rehost work.
It intentionally changes jamb positioning to anchored width resizing; center
move and Rehost remain separate. No commit was created.

- Required nullable width/height overrides resolve through the shared model
  resolver, validate effective family/host dimensions and preserve explicit
  Some(default) pins. Type edits, invalid changes, invalidation and undo/redo
  are exercised in model/document tests.
- Frozen schema-29 migration tests preserve every prior field, sill pin,
  identity and opaque payload, reject ambiguous/missing fields atomically,
  and save/reopen inherited/equal/different pins. Earlier schema-28 sill
  fixtures/tests remain intact and run through the current chain.
- Real headless egui forms at 1280×800/100% and 1000×650/150% cover independent
  inherit/override/reset, invalid enabled input, ignored disabled input,
  preview non-mutation, cancel/stale contexts, commit/history and host regeneration.
  Existing form assertions now scroll to offscreen controls/preview, retaining
  visible-control checks. Schedule rows and sorting use effective dimensions;
  selected-window schedule editing uses the existing instance route.
- Both jambs are tested across typed/legacy doors/windows and reversed hosts:
  opposite jamb fixed, off-center press correction, no-op release without pinning,
  snapping/self-exclusion, transient preview, one-step undo/redo, frame-fit,
  crossing/sub-mm/host/pier/overlap rejection, cancellation and pan/tool precedence.
- IFC v3 roundtrips shared defaults and inherited/equal/different pins, including
  all-pinned types, for doors and windows. Malformed/mixed/inconsistent metadata
  fails. Frozen v1 and v2 window imports, retained v2 sill tests and one-occurrence
  sill override acceptance remain covered. This is a bounded OpenStructure dialect.

Final gates (all locked/offline; final workspace run started after the frame-fit
and left-side overlap tests were added):

- `cargo test -p os-model -p os-document -p os-storage -p os-plugin-api -p os-plugin-host -p os-ifc -p os-ui --all-features --locked --offline --quiet`: exit 0.
- `cargo test --workspace --all-features --locked --offline --quiet`:
  **569 passed, 0 failed, 3 ignored**, including doc tests.
- UI: **161 unit tests passed, 2 ignored; 43 integration tests passed**.
  IFC exchange: **21 passed, 1 ignored**.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`: exit 0.
- `cargo fmt --all -- --check` and `git diff --check`: exit 0.
- Native manual visual qualification and independent hosted IFC viewer
  qualification remain open. No generic plugin gesture expansion or Revit
  parity is claimed. Generic API 2 and container 2 remain unchanged.

Exact files changed in this dimension increment (47; relative to repository
root). Constructor-only changes initialize the two required override fields.
The pre-existing frozen schema-28 and IFC-v1 files were not changed here.

```text
README.md
crates/os-document/src/tests/openings.rs
crates/os-geometry/src/openings.rs
crates/os-ifc/src/reader.rs
crates/os-ifc/src/writer.rs
crates/os-ifc/tests/exchange.rs
crates/os-model/src/lib.rs
crates/os-model/src/memory.rs
crates/os-model/src/openings.rs
crates/os-plugin-api/src/lib.rs
crates/os-plugin-host/src/lib.rs
crates/os-plugin-host/tests/wasm_transport.rs
crates/os-storage/src/lib.rs
crates/os-storage/src/tests/openings.rs
crates/os-storage/tests/fixtures/schema-29-opening-dimensions.json
crates/os-storage/tests/opening_dimensions.rs
crates/os-storage/tests/rooms.rs
crates/os-storage/tests/wall_types.rs
crates/os-storage/tests/window_sills.rs
crates/os-ui/src/desktop_tests/opening_family_tests.rs
crates/os-ui/src/opening_family_editor.rs
crates/os-ui/src/opening_profile_tests.rs
crates/os-ui/src/opening_schedule.rs
crates/os-ui/src/opening_schedule/definitions/tests.rs
crates/os-ui/src/opening_sill_tests.rs
crates/os-ui/src/opening_tools.rs
crates/os-ui/src/opening_type_tools.rs
crates/os-ui/src/plan_settings.rs
crates/os-ui/src/plan_workspace.rs
crates/os-ui/src/plan_workspace/opening_tests.rs
crates/os-ui/src/plan_workspace/sheet_tests.rs
crates/os-ui/tests/openings.rs
crates/os-ui/tests/perpendicular_joins.rs
crates/os-ui/tests/sections.rs
crates/os-ui/tests/wall_joins.rs
crates/os-ui/tests/wall_types.rs
docs/2d-coverage.md
docs/file-format.md
docs/ifc-roadmap.md
docs/native-hosted-openings.md
docs/native-wall-types.md
docs/plugin-api.md
fixtures/wasm-probe/plugin.toml
fixtures/wasm-probe/probe.wat
fixtures/window-sill-v2.ifc
plugins/walls/plugin.toml
plugins/walls/src/lib.rs
```

## Direct plan door controls

When a native straight-wall door is selected and visible on its host-level
floor plan, two labeled controls appear above its jambs. **Hinge** toggles
Start/End; **Swing** toggles Left/Right relative to the host wall's stored
start→end direction. Hovering either control explains that reference direction.
Both have accessible action labels and distinct hover/pressed states.
The controls are hidden for windows, unselected or hidden/cropped doors, active
tools, and whenever their hit areas would overlap each other or the center/jamb
grip acquisition areas. Their placement is checked independently of jamb-grip
availability, so they remain available at ordinary 35 px/m zoom when their own
hit areas fit. Both jambs must be inside the crop and canvas; each button must
fit the canvas with padding.

Each click clones the selected instance parameters, changes exactly one
orientation field, and submits the existing validated `Command::UpdateOpening`.
The opening identity, host, offset, definition, dimensions, overrides, type/family,
material assignments and the other orientation field are preserved. Plan symbol and door geometry regenerate
through the existing evaluators; undo and redo each restore the complete change
in one history step. No model schema or API change is involved. The press owns
the pointer until release: dragging, Escape, pointer loss, or changed document,
revision, selection, drawing, view/settings, provider, camera or canvas cancels
the flip without starting move, resize, selection or pan.

Headless egui tests in `crates/os-ui/src/plan_workspace/opening_tests.rs`
exercise both controls at 1280×800/1× and 1000×650/1.5×, at the fixture's
35 px/m zoom, for forward/reversed hosts and typed/legacy doors. They cover
independent toggles, preserved material assignments and dimension pins, plan/scene regeneration, one-step
undo/redo, tool and visibility suppression, hit-target separation from move and
jamb grips, hover/pressed feedback, tooltips, accessible labels, stale/cancelled
presses and clicks whose press/release arrive in one frame. Native visual
inspection and physical-print qualification remain open.

## Array along wall

Select one visible native hosted door or window, then choose **Architecture →
Array along wall**. Count is 2–256, including the unchanged source. Spacing is
positive centre-to-centre distance in metres. Start and End refer to the host's
stored endpoints, including when the wall is rotated or reversed. The panel
shows the clear gap (spacing minus effective opening width), proposed copies
appear in plan, and invalid batches report the first invalid copy index.
Nonfinite input and distance/offset overflow are rejected.

Each copy receives a new UUID, the same host, and source offset plus or minus
its index times spacing. Name, typed or legacy definition, type/family references,
dimension overrides, hinge and swing are preserved. These are independent
instances; there is no persistent array relationship.

**Copy tag in this plan** is off by default and offered only when the source has
an active-plan tag. New tags receive new IDs and opening targets, retain the
label preset, and translate their world positions along the host axis by the
copy's displacement. Other views' tags are not copied. Creation checks resolve
each new target, and whole-model validation enforces per-view uniqueness and
the 10,000-tag limit.

Preview changes neither model, history nor 3D. Apply rebuilds the batch from the
current document, checks host end/head clearances, family fit and at least 1 mm
separation from existing and proposed openings, and preflights native wall and
opening meshes. The existing native document command path submits all openings
and optional tags in one transaction. Validation failure leaves the document,
history and scene unchanged; a successful commit regenerates plan and 3D and
can be undone or redone in one step.

The draft uses the existing Rehost context guard. Escape, pointer loss, selection,
view/settings, drawing, document session/revision or provider changes discard it.
It owns canvas input ahead of grips, selection and pan; a consumed press stays
consumed through release. No schema, storage format or plugin gesture protocol
changes are required.

Evidence: `cargo test -p os-ui --all-features opening_array -- --test-threads=1`
passed **6 tests**. Tests live in
`crates/os-ui/src/plan_workspace/opening_array_tests.rs`, registered under the
existing opening interaction harness. Real headless egui frames exercise
1280×800 at 100% and 1000×650 at 150%. Coverage includes typed/legacy doors and
windows, rotated/reversed hosts, both directions, transient preview, preserved
instance data, translated tag targets/positions/presets, one-step undo/redo,
split plan/scene regeneration, save/reopen, numeric bounds/overflow, existing and
proposed opening collisions, host/family fit, tag limits, visibility, selection,
provider/revision guards, Escape and pointer-loss ownership.

Validation for this increment:

- `cargo test -p os-ui --all-features -- --test-threads=1`: 289 unit tests and
  46 integration tests passed; 4 installed-Wall-guest tests ignored; no doctests.
- `cargo clippy -p os-ui --all-features --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed (existing line-ending warnings only).

Limits: one native straight host and one selected source; no multi-host, radial,
constrained or general entity arrays. Preview remains clipped to the active
plan/canvas. Automated evidence is headless egui, not native-window visual or
physical-print acceptance. Independent installed-provider acceptance is not
newly exercised by these array tests.

## Temporary opening spacing dimensions

Select exactly one visible native door or window in its host-level plan. Two
temporary metre values measure from its start/end jamb along the wall's stored
Start → End axis to the nearest qualifying other opening jamb on that side,
or to the corresponding wall endpoint. Rotated and reversed hosts use the same
axis convention. References must be present in the current phase/range drawing
and inside the crop and canvas. A partially cropped neighbor can supply its
visible jamb. A side is suppressed if its reference or label cannot fit visibly
without overlapping the other label, flip buttons or opening grips.

Click a value, enter an exact distance in metres, and press Enter. Escape cancels.
The draft starts with the full precision distance; the plan label displays three
decimal places. Numeric edits do not snap or round. Only the selected opening's
offset changes: UUID, host, name, typed/legacy definition, dimension and pane
overrides, hinge/swing, and type/family/material assignments are preserved.
The exact neighbor+jamb or endpoint is resolved again at commit using the current
effective width. Disposable model and geometry preflight precede the existing
`UpdateOpening` command. A successful change is one undo/redo step; a no-op has
no history entry. Invalid text, insufficient clearance and overlap remain in the
draft without changing the model, history, scene or cached plan drawing.

Placement, Rehost, arrays, wall handles, flip controls and center/jamb grips take
precedence. A spacing press remains consumed through release even when cancelled,
so it cannot become a pan or selection. Escape, pointer loss, selection,
document session/revision, view/settings, drawing identity, provider activation
or signature, camera/canvas, and reference changes cancel stale edits. The
installed-provider bounded command path is unchanged. These values create no
persisted dimension or constraint and require no schema, storage or plugin
protocol change.

Automated evidence lives in
`crates/os-ui/src/plan_workspace/opening_spacing_tests.rs`. Headless egui frames
cover 1280×800 at 100% and 1000×650 at 150%, typed/legacy doors and windows,
both sides, neighboring jambs and wall endpoints, forward/reversed and rotated
hosts, exact keyboard entry, no-op/history and preserved fields, invalid drafts,
changed references and stale contexts, phase/range/visibility/crop/canvas,
precedence, Escape and pointer loss. These tests do not qualify native-window
appearance, physical printing, or independent installed-provider operation.
This increment is limited to temporary spacing edits on straight native hosts.
Door/window category visibility is implemented separately above; general
view/detail visibility across other model categories and full Revit workflows
remain open.

Validation for temporary spacing:

- `cargo test -p os-ui --all-features --lib opening_spacing -- --test-threads=1`:
  8 passed, including short-gap label separation at both display profiles.
- `cargo test -p os-ui --all-features --lib -- --test-threads=1`:
  352 passed, 4 installed-Wall-guest tests ignored.
- `cargo clippy --workspace --all-features --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed (existing line-ending warnings only).
