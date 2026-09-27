# Native hosted doors and windows

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
edit updates every placed instance in one transaction, with Cancel and stale
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
or closing the schedule discards the draft. Type, dimensions, host, and level
remain read-only. A changed document session/revision cancels the editor, including
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
`name_edit_is_draft_only_and_commits_one_undo_step`,
`invalid_name_preserves_draft_model_and_history`, and
`stale_context_cancel_and_escape_never_commit` cover actual egui editing/Apply,
undo/redo, validation rollback and correction, cancellation, and stale rejection.
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
groups, totals, styles/conditional formatting, CSV and pagination. One live saved
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
