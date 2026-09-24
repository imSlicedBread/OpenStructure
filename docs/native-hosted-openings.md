# Native hosted doors and windows

This is a bounded native-model slice, not a production architectural profile.
Doors and windows are `core.opening` instances hosted by straight native walls.
Instances keep their UUID, name, host, first-jamb offset, hinge and swing; project-owned
`core.opening_type` entities own the kind, width, height and sill, measured in
metres. Typed instances store only a type reference, so changing a type updates
every assigned opening without duplicating dimensions. Schema 7 projects migrate
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
of exporting a false rectangle. Native full-model plugins use API 9 / schema 28.

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
host, type/family, width, height, sill, hinge and swing are preserved. There is no
rehosting, resizing, snapping, schema change or storage change in this gesture.

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

No other editable row fields, custom fields, arbitrary filters, grouping, totals,
CSV exchange, or schedule sheet placement/pagination are implemented.

Still limited: one profile-extruded panel/pane with generated frame members and a
rectangular or simple polygon host cut; arcs/splines, multiple rings/holes,
arbitrary/multiple/nested components, constrained sketches, formulas, material
assignment, libraries, per-instance dimension overrides, general mirror/flipping
tools, tags, curved/plugin-defined hosts, general IFC family/component round-trip,
or production readiness.
Generated frames require a rectangular host cut. Pointer
placement is limited to visible native straight walls. Native manual visual
inspection has not been performed.
