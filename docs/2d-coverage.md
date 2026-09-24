# Architectural 2D coverage ledger

This is the required evidence ledger for BUILD_PROMPT and [the production specification](2d-production-spec.md). It is not a readiness claim. Requirement IDs below refine each specification capability into separately trackable items; no group is completed by one example. Reference workflow is the matching spec section (including its cited references); details and acceptance conditions there remain mandatory.

## Profile and current gate

D is now in progress after the [bounded B/C baseline audit](bc-baseline-audit.md).
The [callable plan-graphics foundation](callable-plan-graphics.md) adds optional
service-1 DTOs, a scoped validated host call and bounded Wasm worker dispatch.
This is not complete D provider acceptance or a production coverage claim.
The [provider-line render foundation](provider-line-rendering.md) now supplies
host clipping, semantic snap endpoints and matching line picking. Controller
composition now maps checked service results while retaining provider validity;
full independent pointer acceptance remains open.
The [independent Rust column outline](independent-column-plan.md) now passes a
prebuilt-host installation probe through the Wasm plan worker and controller.
Full independent pointer acceptance remains open;
outline-only service output is not qualified architectural cut graphics.
The [bounded provider scheduler](plan-provider-scheduler.md) now coordinates
explicit routes, cancellation, stale batches and per-target failure diagnostics
through the shared Wasm pool. [Native column plan integration](native-column-plan.md)
now wires this into background drawing, Fit plan, picking and shared selection.
Native form edits, undo/redo and save/reopen are verified for the installed column;
independent pointer authoring and the full D failure workflow remain open.
The [installed Wall gesture controller](installed-wall-gesture-adapter.md) now
freezes exact snapped creation drafts into scoped Wasm commands, verified against
the existing guest with cancellation and inactive-view rejection. Native pending
intent wiring now has [installed headless UI evidence](installed-wall-pointer-ui.md).
The [installed edit adapter](installed-wall-edits.md) now passes move, both endpoint
resizes and offset-copy with scoped worker commands and real-guest headless UI
tests. Native visual acceptance and broader D workflow gaps remain required.
Provider composition now has a transferable checked snapshot for off-thread
derivation, verified against edits/reload during work and the installed guest.
The [plan geometry foundation](plan-geometry-foundation.md) adds actual checked
horizontal cuts/projections, cropped semantic polygons, 2D coordinate/navigation
math and revision-bound native-wall controller derivation. [Named plan settings](persisted-plan-settings.md)
were introduced in model schema 4 (current model schema 28) with transactional commands and frozen migration
tests. The [native plan workspace](native-plan-workspace.md) adds actual plan
creation/selection, equal split, background derivation, navigation and shared
wall selection with native edit/undo/save/reopen evidence. [Plan-settings forms](plan-settings-form.md)
now edit the persisted settings with atomic Apply/Cancel and stale-draft rejection.
[Semantic snapping](semantic-plan-snapping.md) now supplies checked native-wall
endpoint/midpoint/nearest queries alongside background drawings. The initial
[two-point wall tool](plan-wall-gestures.md) adds snap feedback and exact drafts
through the bundled provider. The [native pointer checkpoint](native-pointer-walls.md)
verifies two snapped exact walls, cancellation, split display and save/reopen.
[Architectural grid persistence](architectural-grids.md) adds building-scoped
straight datums, validated commands/history and explicit migration 3→4.
[Grid plan integration](grid-plan-integration.md) adds clipped lines, shared grid
selection and grid-axis snapping with headless pointer-workflow tests. [Grid forms](grid-authoring-forms.md)
now support numeric create/edit, preview, cancellation and undo, including a
connected headless New grid → snapped wall test. [Native grid inspection](native-grid-authoring.md)
now verifies create/edit/undo/redo, invalid/cancel, snapped wall and save/reopen.
[Wall move/resize click tools](plan-wall-edit-gestures.md) now preserve identity,
support exact drafts and pass controller and real egui input tests. [Native edit
inspection](native-wall-edits.md) verifies split updates and save/reopen, while
exposing an untitled quick-Save path defect, now [corrected with destination
prompting](untitled-save-destination.md) and regression tests. Selected visible
native-wall endpoint handles now have [headless desktop evidence](plan-wall-edit-gestures.md#endpoint-handle-slice--2026-09-20-automated-evidence)
at 1280×800/1.0 and 1000×650/1.5, including rendering/hits, preview/commit/history,
cancellation/pan precedence and an explicitly run installed Wall worker test.
Native manual handle inspection, broader gesture acceptance and broader
installed-provider acceptance remain required; D is incomplete.

[Native hosted openings](native-hosted-openings.md) add straight-wall doors and
windows with checked host-local dimensions, one-step commands, schema 4→5
migration, actual segmented wall apertures shared by 3D and plan, fixture picking,
and exact-input create/edit/delete forms. Schema 10 adds per-door Start/End hinge
and Left/Right swing, matching 90° leaves and pickable 16-segment quarter-circle
arcs. Start/Left preserves existing geometry during schema-9 migration and remains
the new-door default. Headless tests cover both wall directions, all combinations,
crop/picking, preview/apply/cancel/stale drafts and undo/redo. Family libraries, curved/plugin hosts,
and native visual inspection remain open; this does not close D.

[Synthetic snap-stage timings](plan-snap-stage-timings.md) now separate query
cost from source/scene setup and compare dense intersection overrides/overload.
Prepared immutable segment directions reduce repeated intersection-query work,
with large-coordinate/order/revision regression coverage and before/after timings.
These local microbenchmarks do not qualify event-to-paint latency, regeneration,
installed providers or production performance budgets.

[Exact-input precedence correction](exact-input-snap-precedence.md) verifies that
dense intersection acquisition cannot block a fully specified offset, while
partial pointer-dependent drafts retain their resource checks.

[Native offset/save inspection](native-offset-save.md) verifies exact parallel
copy preview/commit in split view, snap-menu visibility, corrected untitled Save
prompting and save/reopen with independently inspected wall coordinates/IDs.

[Finite-line intersection snapping](plan-intersection-snapping.md) now retains
both semantic references and is used by bundled wall gestures, with bounded
local acquisition and desktop input coverage. Curves and plugin contributions
remain outside this implemented subset.
[Plan snap controls](plan-snap-controls.md) expose session-only enable switches
for the implemented kinds without model/history edits; priority customization
and independent-provider aggregation remain open.

[Desktop plugin form view provenance](plugin-form-view-provenance.md) now binds
existing command workers to active plan/settings identity and revokes drafts on
view changes. The host also [validates the persisted settings revision](worker-persisted-view-validation.md)
at dispatch, polling and checked geometry consumption instead of accepting a
caller-only revision echo. Independent pointer/preview/graphics/snapping contracts remain
unimplemented; command-worker guards alone do not satisfy D.

The transaction foundation now has [bounded snapshot retention](history-retention.md):
128 entries / 256 MiB estimated owned data by default, whole-entry eviction,
atomic oversized-entry rejection, and desktop notice/dirty-state regression tests.
The measured synthetic cost motivated retention, not a delta rewrite. Actual
peak-process memory and production history/performance budgets remain unqualified.

Native independent Wall follow-up now verifies registered create/edit, two-parameter
undo/redo, save/open and shared viewport/browser selection on Windows. See
[Wall workflow evidence and explicit omissions](native-wall-workflow.md). This
adds concrete B/C evidence without declaring the joint gate complete.

Latest C follow-up: a separate candidate migration service now passes the installed
33-column probe, including cancellation after partial progress, rejection in the
final batch, stale-document rejection and a single live undo/redo transaction.
Four-element worker batches retain existing fuel/message limits. Editor/forms now
use staging for single-type owners, with native Apply/Undo/Redo/Save evidence.
Multi-type desktop review is implemented with a two-type controller test and
single-type native evidence. Independent mixed-type execution now passes distinct
transforms, final second-type failure isolation, geometry, history and storage;
native mixed-type review/Apply/Undo/Redo/Save/Open now has Windows evidence;
large-project performance and broader B/C completion remain pending;
this does not close B/C. See [migration follow-up](plugin-migration-service.md).
The installed migration probe also verifies same-version provider replacement,
reopened-document identity, overall deadline expiry and Editor view-revision
changes after partial progress, with no live/history mutation on rejection.
The all-type review checkpoint passed 151 tests; snapshot retention passed 160;
the plan-geometry checkpoint passed 172. The persisted-settings follow-up passes
the full workspace suite. Older counts are historical checkpoints, not broader
acceptance claims. Installed Wall/column Editor and mixed-migration probes also
pass with bounded history. See [retention evidence](history-retention.md).

Owner response on 2026-09-11: not sure of jurisdiction, project types/size, concurrent editors, OS, CAD/PDF versions or plot sizes. All remain open qualification decisions. Planning only: metric/imperial architecture, new construction and renovation, offline local use, eventual multi-user office use. Observed platform: Windows x64; Linux CI configured, not observed; macOS unverified. No production deployment profile is approved.

The [source-backed B/C audit](bc-baseline-audit.md) verifies the bounded development gate and advances implementation to D. This is not production plugin qualification. D and E1–E4 remain incomplete: no complete linked floor-plan workflow, issued reference project, physical plot or production qualification has passed. The working tree has no commit or remote; configured CI is not a hosted result. Older per-slice test counts are historical evidence only.

Allowed statuses: not started, partial, automated-tested, workflow-validated, owner-approved exclusion. No exclusions have been approved. A dash means missing evidence, never assumed success. Rows can be refined further without dropping parent requirements.

## Capability rows

Each row inherits its dependency from the milestone column: D requires B/C, E1 requires D, E2 requires E1, E3 requires E2 plus translator/team ADRs. All require the architecture commitments below. `Not implemented` means no supported public workflow or acceptance evidence; it is not a finding about every internal helper.

| ID | Requirement / reference workflow | Milestone | Status | Current behavior and limits | Source/test/review evidence |
| --- | --- | --- | --- | --- | --- |
| V01.01 | Named floor plans | D/E1 | partial | Desktop create/picker/browser, settings forms and persistence verified; complete plan authoring remains | native-plan-workspace.md; plan-settings-form.md; os-storage/tests/plan_settings.rs |
| V01.02 | Roof plans | D/E1 | not started | Not implemented | — |
| V01.03 | Reflected ceiling projection | D/E1 | not started | Not implemented | — |
| V01.04 | Site plans | D/E1 | not started | Not implemented | — |
| V01.05 | Area plans | D/E1 | not started | Not implemented | — |
| V01.06 | Architectural coordination plans | D/E1 | not started | Not implemented | — |
| V01.07 | Interior elevations | D/E1 | not started | Not implemented | — |
| V01.08 | Exterior elevations | D/E1 | not started | Not implemented | — |
| V01.09 | Building sections | D/E1 | partial | Versioned section plane/extents and migration; two-click snap-aware plan marker, linked native wall/door/window/floor contours, section canvas, source selection, one-view A3 sheet and vector PDF verified; multi-view sheets, cut fills/poché, depth, and settings editor remain | native-sections.md; native-sheet-preview.md; os-ui/src/plan_workspace/section_tests.rs; os-ui/src/plan_workspace/sheet_tests.rs; os-ui/tests/sections.rs; os-geometry/src/section.rs; os-storage/tests/sections.rs |
| V01.10 | Wall sections | D/E1 | not started | Not implemented | — |
| V01.11 | Enlarged plans | D/E1 | not started | Not implemented | — |
| V01.12 | Detail views | D/E1 | not started | Not implemented | — |
| V01.13 | Independent drafting views | D/E1 | not started | Not implemented | — |
| V01.14 | Cut/top/bottom/depth ranges | D/E1 | partial | Persisted range with numeric draft UI, checked native prism policy and level conversion; other categories remain | plan-settings-form.md; os-geometry/tests/plan.rs; os-ui/tests/plan.rs |
| V01.15 | Local plan regions | D/E1 | not started | Not implemented | — |
| V01.16 | Upward/downward underlays | D/E1 | not started | Not implemented | — |
| V01.17 | Far clipping | D/E1 | not started | Not implemented | — |
| V01.18 | Model crop boundaries | D/E1 | partial | Numeric and graphical rectangular crop editing; eight edge/corner handles, boundary-only preview, single view update, clipping/picking regeneration, undo/redo and save/reopen tested. Non-rectangular crops and native-window/print qualification remain | plan-settings-form.md; os-ui/src/plan_workspace/crop_tests.rs; os-geometry/tests/plan.rs; os-render/tests/plan.rs |
| V01.19 | Annotation crop boundaries | D/E1 | not started | Not implemented | — |
| V01.20 | Rotated crops | D/E1 | partial | Graphical crop handles operate in rotated view-plane coordinates; all eight handles and hit-radius behavior tested at 1280×800/1.0 and 1000×650/1.5. General orientations and native-window/print qualification remain | plan-settings-form.md; os-ui/src/plan_workspace/crop_tests.rs; os-geometry/tests/plan.rs; os-ui/tests/plan.rs |
| V01.21 | Scope boxes | D/E1 | not started | Not implemented | — |
| V01.22 | Project/true north | D/E1 | not started | Not implemented | — |
| V01.23 | Explicit view orientation | D/E1 | partial | Horizontal XY origin/yaw settings UI; general view orientations remain | plan-settings-form.md; os-ui/src/plan_settings.rs |
| V01.24 | Independent duplication | D/E1 | not started | Not implemented | — |
| V01.25 | Duplication with detailing | D/E1 | not started | Not implemented | — |
| V01.26 | Dependent view overrides | D/E1 | not started | Not implemented | — |
| V01.27 | Matchlines | D/E1 | not started | Not implemented | — |
| V01.28 | Callouts and reference-only callouts | D/E1 | not started | Not implemented | — |
| V01.29 | Placed-detail references | D/E1 | not started | Not implemented | — |
| V01.30 | Search/filter project browser | D/E1 | not started | Not implemented | — |
| V01.31 | View naming and organization | D/E1 | partial | Generated plan names, transactional rename and browser selection; organization beyond flat list remains | plan-settings-form.md; native-plan-workspace.md |
| V01.32 | View tabs | D/E1 | not started | Not implemented | — |
| V01.33 | Split linked 2D/3D | D/E1 | partial | Equal-width native plan/3D panes, linked numeric wall edit/undo; exact pointer authoring remains | native-plan-workspace.md; os-ui desktop_tests::named_plan_split_selects_the_same_wall_and_preserves_navigation_history |
| V01.34 | Saved navigation | D/E1 | not started | Not implemented | — |
| V01.35 | Shared cross-view selection | D/E1 | partial | Native walls/openings/rooms share plan/browser selection; room graphics are plan-only, extension plan providers and broader categories remain | native-plan-workspace.md; native-rooms.md; os-ui desktop input tests |
| V01.36 | Navigation from markers/annotations/schedules/sheets | D/E1 | not started | Not implemented | — |
| V02.01 | Category/subcategory styles | E1 | not started | Not implemented | — |
| V02.02 | Per-element overrides | E1 | not started | Not implemented | — |
| V02.03 | Rule-based overrides | E1 | not started | Not implemented | — |
| V02.04 | Material cut/surface patterns | E1 | not started | Not implemented | — |
| V02.05 | Scale-aware line weights | E1 | not started | Not implemented | — |
| V02.06 | Line styles and dash patterns | E1 | not started | Not implemented | — |
| V02.07 | Hidden/overhead lines | E1 | not started | Not implemented | — |
| V02.08 | Join graphics | E1 | not started | Not implemented | — |
| V02.09 | Halftone | E1 | not started | Not implemented | — |
| V02.10 | Transparency | E1 | not started | Not implemented | — |
| V02.11 | Color/monochrome output | E1 | not started | Not implemented | — |
| V02.12 | Coarse/medium/fine representation | E1 | not started | Not implemented | — |
| V02.13 | Model-scale hatch patterns | E1 | not started | Not implemented | — |
| V02.14 | Paper-scale hatch patterns | E1 | not started | Not implemented | — |
| V02.15 | Pattern origin/rotation/spacing | E1 | not started | Not implemented | — |
| V02.16 | Compound-layer cuts | E1 | not started | Not implemented | — |
| V02.17 | Masking and draw order | E1 | not started | Not implemented | — |
| V02.18 | Linked view templates | E1 | partial | Persisted floor-plan graphics templates for walls, doors, windows, and slabs; create/edit/link/unlink, atomic Apply, deletion guard, linked-view invalidation, schema-27→28 migration, and resolved cut/projected color/weight/dash strokes in native plan canvas, sheet preview, and vector PDF. Apply-once, batch assignment, transfer standards, element/filter overrides, section graphics, and native/physical-print qualification remain | native-plan-graphics-templates.md; crates/os-model/src/plan_graphics.rs; crates/os-ui/src/plan_settings/graphics.rs; crates/os-storage/tests/plan_graphics.rs; crates/os-render/tests/plan_graphics.rs; crates/os-ui/src/plan_workspace/graphics_tests.rs; crates/os-ui/src/plan_settings.rs |
| V02.19 | Apply-once templates | E1 | not started | Not implemented | — |
| V02.20 | Batch template assignment | E1 | not started | Not implemented | — |
| V02.21 | Transfer of standards | E1 | not started | Not implemented | — |
| V02.22 | Override precedence | E1 | partial | Linked template styles take precedence over retained local view styles, that resolved result reaches native plan and sheet/PDF strokes, and unlink restores local styles. Per-element, filter/rule, and broader view override precedence remain unimplemented | native-plan-graphics-templates.md; crates/os-model/src/plan_graphics.rs; crates/os-ui/src/plan_settings.rs; crates/os-render/tests/plan_graphics.rs |
| V02.23 | Temporary hide/isolate | E1 | not started | Not implemented | — |
| V02.24 | Persistent hide | E1 | not started | Not implemented | — |
| V02.25 | Reveal-hidden diagnostics | E1 | not started | Not implemented | — |
| V02.26 | Invisibility explanation | E1 | not started | Not implemented | — |
| V02.27 | Issue-safe temporary states | E1 | not started | Not implemented | — |
| M01.01 | Straight rectangular walls | E1 | automated-tested | Numeric straight-wall authoring, rectangular prism only; no plan graphics | os-model WallParams; plugins/walls; os-ui/tests/vertical_slice.rs; baseline tests |
| M01.02 | Layered walls | E1 | partial | Straight native walls support reusable ordered compound types, stable layer IDs, per-wall assignment/flip, layer-aware geometry, openings, plan/section identity, quantities, history, and schema-21→22 migration; no curved walls, advanced layer junctions, or production visual qualification | native-wall-types.md; crates/os-ui/tests/wall_types.rs; crates/os-ui/src/desktop_tests/wall_type_tests.rs; crates/os-storage/tests/wall_types.rs |
| M01.03 | Curved walls | E1 | not started | Not implemented | — |
| M01.04 | Wall joins | E1 | partial | Explicit compatible-profile butt/corner/tee joins for straight walls, including compound types; schema-20→21 migration, graph validation, shared mesh/plan/section derivation, openings/quantities, persistence, history and provider/API compatibility evidence; no arbitrary-angle/layer termination/curved junctions, connected-node drag handles, joined IFC, or native visual qualification | native-wall-joins.md; native-wall-types.md; crates/os-ui/tests/wall_joins.rs; crates/os-ui/tests/perpendicular_joins.rs; crates/os-ui/tests/wall_types.rs; crates/os-ui/src/desktop_tests/wall_join_tests.rs; crates/os-storage/tests/wall_joins.rs; crates/os-plugin-host/tests/wasm_transport.rs; plugins/walls/src/lib.rs |
| M01.05 | Curtain systems | E1 | not started | Not implemented | — |
| M01.06 | Floors/slabs | E1 | partial | Native straight-edged, horizontal concave slabs with metre thickness/offset, transactional sketching, plan fill/pick and regenerated 3D mesh; no holes, slopes, assemblies, joins, inspector editing or IFC slab exchange | native-floor-slabs.md; crates/os-ui/src/plan_workspace/floor_tests.rs; crates/os-document/src/tests/floors.rs; crates/os-storage/src/tests/floors.rs; crates/os-render/tests/plan.rs |
| M01.07 | Roofs | E1 | not started | Not implemented | — |
| M01.08 | Ceilings | E1 | not started | Not implemented | — |
| M01.09 | Hosted openings | E1 | automated-tested | Checked openings on straight rectangular native walls, including typed compound layers and wall members with butt/corner/tee trims; bounded IFC4 import/export uses full-depth rectangular void relationships; no curved or extension-element hosts, custom IFC profiles, or joined IFC | native-hosted-openings.md; ifc-roadmap.md; crates/os-ifc/tests/exchange.rs; crates/os-ifc/tests/validate_hosted.py; native-wall-joins.md; native-wall-types.md; crates/os-ui/tests/openings.rs; crates/os-ui/tests/wall_joins.rs; crates/os-ui/tests/wall_types.rs |
| M01.10 | Doors/windows | E1 | automated-tested | Reusable typed doors/windows with editable profile-extruded panel/pane and optional 3-side door/4-side window frame, per-door hinge/swing, clipped arcs, repeatable placement, configurable window pane alignment, and selected-opening same-host grip drag with transient symbol/aperture preview, validated one-command release and residue-free cancellation (doors/windows, forward/reversed hosts, both DPI profiles); bounded IFC4 exchange preserves semantic fills/types but omits filling Body geometry; rectangular wall voids only, no family libraries or plugin hosts | native-hosted-openings.md; ifc-roadmap.md; crates/os-ui/tests/openings.rs; crates/os-ui/src/desktop_tests/opening_family_tests.rs; crates/os-ui/src/plan_workspace/opening_tests.rs; crates/os-storage/src/tests/openings.rs; crates/os-ifc/tests/exchange.rs; crates/os-ifc/tests/validate_hosted.py |
| M01.11 | Stairs | E1 | not started | Not implemented | — |
| M01.12 | Ramps | E1 | not started | Not implemented | — |
| M01.13 | Railings | E1 | not started | Not implemented | — |
| M01.14 | Architectural columns | E1 | partial | First-class vertical rectangular columns with stable native identity, level/base offset, dimensions and optional material; snapped plan placement preview, cut/projected plan footprint and pick, generated 3D prism, numeric Properties edit/delete, one-step history and schema-26→27 migration. Circular/slanted columns, reusable types, wall/slab joins, section graphics, schedules, IFC and visual/production qualification remain | native-columns.md; crates/os-model/src/columns.rs; crates/os-geometry/src/columns.rs; crates/os-document/src/tests/columns.rs; crates/os-render/tests/columns.rs; crates/os-storage/src/tests/columns.rs; crates/os-ui/src/plan_workspace/column_tests.rs |
| M01.15 | Furniture/casework/fixtures | E1 | not started | Not implemented | — |
| M01.16 | Site coordinates | E1 | not started | Not implemented | — |
| M01.17 | Terrain/survey references | E1 | not started | Not implemented | — |
| M01.18 | Type/instance parameters | E1 | partial | Opening types share kind/dimensions, normalized component profile, extrusion depth and optional frame width/depth; instances retain name/host/offset/hinge/swing, with atomic type edits and explicit legacy conversion; no formulas, nested families or per-instance dimension overrides | native-hosted-openings.md; crates/os-document/src/tests/openings.rs; crates/os-ui/src/opening_type_tools.rs |
| M01.19 | Level relationships | E1 | partial | Straight walls only; no full architectural drawings or schedules | plugins/walls wall_solid; vertical_slice.rs; baseline tests |
| M01.20 | Host relationships | E1 | partial | Openings retain a native wall host and reject host deletion/shortening that invalidates them; no general host graph | native-hosted-openings.md; crates/os-ui/tests/openings.rs |
| M01.21 | Placement | E1 | partial | Straight walls and repeatable click-to-place native doors/windows with transient host preview; no curved/plugin hosts or full architectural drawings | native-hosted-openings.md; crates/os-ui/src/plan_workspace.rs; crates/os-ui/src/plan_workspace/opening_tests.rs |
| M01.22 | Flip/mirror | E1 | partial | Bounded native straight-wall door hinge/swing flip slice only: per-instance Start/End hinge and Left/Right swing; general flip/mirror tools remain unimplemented | native-hosted-openings.md; crates/os-ui/tests/openings.rs; crates/os-ui/src/plan_workspace/opening_tests.rs |
| M01.23 | Material layers | E1 | partial | Wall layers retain stable material references and density; per-layer quantities and material identity flow through plan/section/mesh. No material appearance, hatch standards, or full material library workflow | native-wall-types.md; crates/os-ui/tests/wall_types.rs |
| M01.24 | Geometry and symbolic representations | E1 | partial | Straight-wall host cuts support rectangular or authored simple polygon elevation profiles; 3D meshes, plan cut/projection footprints, sections, and layer quantities use the same partition. Profile cuts reject rectangle-only Solid exports. No curved/profiled hosts or full detail-level system | native-hosted-openings.md; crates/os-geometry/src/walls/profiles.rs; crates/os-geometry/tests/host_cut_profiles.rs; crates/os-ui/src/plan.rs |
| M01.25 | View/detail visibility | E1 | not started | Not implemented | — |
| M01.26 | Consistent quantities | E1 | partial | Straight native walls report net wall and per-layer volume; density-backed per-layer mass reconciles with compound openings. No full architectural quantity schedules or broader element takeoff | native-wall-types.md; crates/os-ui/tests/wall_types.rs; vertical_slice.rs |
| M01.27 | Model group membership | E1 | not started | Not implemented | — |
| M01.28 | Reusable assemblies | E1 | not started | Not implemented | — |
| M01.29 | Architectural part/layer identity | E1 | partial | Compound wall layers have stable identities carried by geometry, plan and section while wall UUID remains selection owner; no independently editable/hosted part entities or part schedules | native-wall-types.md; crates/os-ui/tests/wall_types.rs |
| M01.30 | Host/type/level regeneration across drawings and information | E1 | partial | Host edits, shared opening-type changes, and shared wall-type changes invalidate affected native 3D, plan, section and layer quantities; level elevation follows host; no complete schedules/tags | native-hosted-openings.md; native-wall-types.md; crates/os-document/src/tests/openings.rs; crates/os-ui/tests/openings.rs; crates/os-ui/tests/wall_types.rs |
| M02.01 | Parametric opening family authoring | E1 | partial | Versioned editor provides separate normalized component and host-cut profiles; shared-type edits preflight all instances and commit atomically. Host cuts are bounded to simple 3–32 vertex rings; generated frame rails require rectangular cuts. No constrained sketches, arbitrary multiple/nested forms, arcs, formulas, material assignment, content packages or cross-project libraries | native-hosted-openings.md; crates/os-model/src/opening_family.rs; crates/os-geometry/src/walls/profiles.rs; crates/os-ui/src/opening_family_editor.rs; crates/os-ui/src/opening_profile_tests.rs; crates/os-storage/src/tests/openings.rs |
| E01.01 | Lines | D/E2 | not started | Not implemented | — |
| E01.02 | Polylines | D/E2 | not started | Not implemented | — |
| E01.03 | Arcs | D/E2 | not started | Not implemented | — |
| E01.04 | Circles | D/E2 | not started | Not implemented | — |
| E01.05 | Ellipses | D/E2 | not started | Not implemented | — |
| E01.06 | Splines | D/E2 | not started | Not implemented | — |
| E01.07 | Rectangles | D/E2 | not started | Not implemented | — |
| E01.08 | Polygons | D/E2 | not started | Not implemented | — |
| E01.09 | Reference lines/planes | D/E2 | not started | Not implemented | — |
| E01.10 | Closed sketch/region validation | D/E2 | not started | Not implemented | — |
| E01.11 | Select/window/crossing selection | D/E2 | not started | Not implemented | — |
| E01.12 | Overlap cycling | D/E2 | not started | Not implemented | — |
| E01.13 | Selection category filters | D/E2 | not started | Not implemented | — |
| E01.14 | Move/copy | D/E2 | partial | Bundled wall base/destination move preserves UUID and level, with native move/undo/redo evidence; copy and other entities remain | native-wall-edits.md; plan-wall-edit-gestures.md; os-ui/tests/plan_gesture.rs; desktop pointer edit test |
| E01.15 | Rotate/mirror | D/E2 | not started | Not implemented | — |
| E01.16 | Align | D/E2 | not started | Not implemented | — |
| E01.17 | Offset | D/E2 | partial | Signed centreline parallel wall copies with preview/cancel and one-step undo; curves, general entities and independent tools remain | plan-wall-offset.md; os-ui/tests/plan_gesture.rs; desktop pointer offset test |
| E01.18 | Trim/extend | D/E2 | not started | Not implemented | — |
| E01.19 | Split | D/E2 | not started | Not implemented | — |
| E01.20 | Fillet | D/E2 | not started | Not implemented | — |
| E01.21 | Arrays | D/E2 | not started | Not implemented | — |
| E01.22 | Group/ungroup | D/E2 | not started | Not implemented | — |
| E01.23 | Pin/unpin | D/E2 | not started | Not implemented | — |
| E01.24 | Copy/paste aligned with identity remapping | D/E2 | not started | Not implemented | — |
| E01.25 | Endpoint snapping | D/E2 | partial | Semantic wall endpoints and two-point feedback; selected native-wall handle drags reuse snap/exclusion with headless tests at two DPI profiles; broader independent plugin/native acceptance remains | plan-wall-gestures.md; plan-wall-edit-gestures.md; os-render/tests/snapping.rs; os-ui/src/plan_workspace/endpoint_tests.rs |
| E01.26 | Midpoint snapping | D/E2 | partial | Semantic midpoint queries and initial pointer-tool feedback; broader entities/providers remain | plan-wall-gestures.md; os-render/tests/snapping.rs |
| E01.27 | Intersection snapping | D/E2 | partial | Finite semantic line pairs, crop/exclusion checks, canonical pair references and bounded local search; curves, independent providers and native inspection remain | plan-intersection-snapping.md; os-render/tests/snapping.rs; desktop intersection input test |
| E01.28 | Perpendicular snapping | D/E2 | partial | Gesture-anchor projection onto finite semantic lines, screen acquisition, crop/exclusion and visible session toggle; curves, inferred anchors and independent providers remain | plan-perpendicular-snapping.md; os-render/tests/snapping.rs; desktop perpendicular input test |
| E01.29 | Tangent snapping | D/E2 | not started | Not implemented | — |
| E01.30 | Nearest snapping | D/E2 | partial | Cropped semantic-line query used by initial wall tool; curves/providers remain | plan-wall-gestures.md; os-render/tests/snapping.rs |
| E01.31 | Center snapping | D/E2 | not started | Not implemented | — |
| E01.32 | Grid snapping | D/E2 | partial | Persisted finite building grids supply endpoint/midpoint and GridAxis nearest snaps in bundled wall gestures; crop intersections are not endpoints. Intersections, overrides and external route remain | os-render/tests/grids.rs; os-ui/tests/grid_plans.rs; desktop grid pointer test; grid-plan-integration.md |
| E01.33 | Axis snapping | D/E2 | partial | Optional semantic line-axis extensions beyond finite endpoints, crop/exclusion/query checks and pointer workflow; inferred/world-axis guides and independent providers remain | plan-axis-extension-snapping.md; os-render/tests/snapping.rs; desktop axis extension test |
| E01.34 | Snap priorities/overrides | D/E2 | partial | Deterministic host priority with visible session enable switches for implemented kinds; custom priority/cycling and provider aggregation remain | plan-snap-controls.md; semantic-plan-snapping.md; os-render/tests/snapping.rs; desktop snap controls tests |
| E01.35 | Screen acquisition and geometric tolerances | D/E2 | not started | Not implemented | — |
| E01.36 | Exact coordinates | D/E2 | partial | Numeric wall Properties/commands only; no plan input gestures | os-ui palettes.rs and desktop_tests |
| E01.37 | Exact length | D/E2 | partial | Bundled wall creation/endpoint resize length and move distance drafts in metres; broader tools remain | plan-wall-edit-gestures.md; os-ui/tests/plan_gesture.rs |
| E01.38 | Exact angle | D/E2 | partial | Bundled wall gesture degree input in active plan basis; broader tools remain | plan-wall-edit-gestures.md; os-ui/tests/plan_gesture.rs |
| E01.39 | Exact offset | D/E2 | partial | Signed metre input for parallel native wall copies, independent of plan rotation; broader reference/unit workflows remain | plan-wall-offset.md; os-ui/tests/plan_gesture.rs |
| E01.40 | Metric input | D/E2 | partial | Numeric wall Properties/commands only; no plan input gestures | os-ui palettes.rs and desktop_tests |
| E01.41 | Decimal imperial input | D/E2 | not started | Not implemented | — |
| E01.42 | Fractional imperial input | D/E2 | not started | Not implemented | — |
| E01.43 | Temporary dimensions | D/E2 | not started | Not implemented | — |
| E01.44 | Live previews | D/E2 | partial | Transient wall axis/exact-input and hosted door/window plan-symbol previews; live solid/3D and general tools remain | plan-wall-edit-gestures.md; native-hosted-openings.md; crates/os-ui/src/plan_workspace/opening_tests.rs |
| E01.45 | Repeat tools | D/E2 | partial | Door/window pointer placement remains active for repeated one-click commits until Escape; general repeat tools remain | native-hosted-openings.md; crates/os-ui/src/plan_workspace/opening_tests.rs |
| E01.46 | Cancel without residue | D/E2 | partial | Escape/cancel wall gestures, opening placement, and grid/settings forms preserve model; broader tools remain | plan-wall-edit-gestures.md; native-hosted-openings.md; crates/os-ui/src/plan_workspace/opening_tests.rs |
| E01.47 | Keyboard focus preservation | D/E2 | partial | Numeric wall Properties/commands only; no plan input gestures | os-ui palettes.rs and desktop_tests |
| E01.48 | One gesture one undo | D/E2 | partial | Wall create/move/click resize, native endpoint drags, and each opening placement click commit once; bundled and explicitly run installed handle tests verify undo/redo; broader tools and native manual inspection remain | plan-wall-edit-gestures.md; native-hosted-openings.md; os-ui/tests/plan_gesture.rs; os-ui/src/plan_workspace/opening_tests.rs |
| E01.49 | Geometric/dimensional constraints | D/E2 | not started | Not implemented | — |
| E01.50 | Equality constraints | D/E2 | not started | Not implemented | — |
| E01.51 | Locked alignment | D/E2 | not started | Not implemented | — |
| E01.52 | Transactional conflict explanations | D/E2 | not started | Not implemented | — |
| A01.01 | Aligned/linear dimensions | E2 | partial | Native three-click endpoint dimensions, live metric value, selectable annotation and editable signed offset; wall-face references and print-faithful sizing remain | native-dimensions.md; os-render/tests/plan.rs; os-ui/src/plan_workspace/endpoint_tests.rs |
| A01.02 | Chained/baseline dimensions | E2 | partial | Native straight-wall endpoint chains and cumulative baselines; ordered collinear anchors, one semantic entity, and one-step history; paper-scale formatting and broader references remain | native-dimensions.md; os-model/src/dimensions.rs; os-ui/src/plan_workspace/endpoint_tests.rs; os-storage/tests/dimensions.rs |
| A01.03 | Angular dimensions | E2 | partial | Native straight-wall axes, four non-reflex sectors, live degree reporting and orphan recovery; cropped arc/label picking. Headless authoring at both DPI profiles, single transaction and undo/redo; frozen schema-11 migration and Angular roundtrip. No native-window or print qualification | native-dimensions.md; dimensions model/storage tests; plan renderer tests; endpoint_tests.rs |
| A01.04 | Radial/diameter dimensions | E2 | not started | Not implemented | — |
| A01.05 | Arc-length dimensions | E2 | not started | Not implemented | — |
| A01.06 | Spot elevation/coordinate/slope | E2 | not started | Not implemented | — |
| A01.07 | Witness editing | E2 | not started | Not implemented | — |
| A01.08 | Prefixes/suffixes/tolerances | E2 | not started | Not implemented | — |
| A01.09 | Unit/rounding styles | E2 | not started | Not implemented | — |
| A01.10 | Semantic feature/material references | E2 | not started | Not implemented | — |
| A01.11 | Linked-object references | E2 | not started | Not implemented | — |
| A01.12 | Reference remapping | E2 | not started | Not implemented | — |
| A01.13 | Visible orphan warnings | E2 | partial | Missing/releveled/coincident references persist and show an on-plan marker, Properties reason and Browser endpoint names; deliberate reference repair remains | native-dimensions.md; os-ui/src/plan_workspace/endpoint_tests.rs |
| A01.14 | Deliberate reference repair | E2 | not started | Not implemented | — |
| A01.15 | Reporting versus driving dimensions | E2 | partial | Values derive from current wall endpoints and never drive geometry; no constraints or value overrides | native-dimensions.md; os-document/tests/dimensions.rs; os-ui/src/plan_workspace/endpoint_tests.rs |
| A01.16 | Identifiable value overrides | E2 | not started | Not implemented | — |
| A01.17 | Rich text | E2 | not started | Not implemented | — |
| A01.18 | Wrapping/lists | E2 | not started | Not implemented | — |
| A01.19 | Symbols | E2 | not started | Not implemented | — |
| A01.20 | Leaders/arrowheads | E2 | not started | Not implemented | — |
| A01.21 | Text alignment | E2 | not started | Not implemented | — |
| A01.22 | Find/replace | E2 | not started | Not implemented | — |
| A01.23 | Language-qualified Unicode shaping | E2 | not started | Not implemented | — |
| A01.24 | Explicit font substitution | E2 | not started | Not implemented | — |
| A01.25 | Element/material/room/area tags | E2 | partial | Native view-owned room-number/name badges use stable room/view UUIDs, live label resolution and orphan diagnostics; move by drag or Properties. Area remains at the room seed. Six headless egui tests cover preview non-mutation, place/select/move, crop, cancel/stale release, Browser/Properties, pan and overlapping-wall-handle precedence, and one-step undo/redo at 1280×800/1.0 and 1000×650/1.5; frozen 12→13 migration and live/orphan save/reopen are covered. Generic element/material/area tags, leaders, tag styles/schedules, native visual inspection and print acceptance remain | native-rooms.md; os-model/tests/room_tags.rs; os-document/tests/room_tags.rs; os-storage/tests/room_tags.rs; os-render/tests/plan.rs; os-ui/src/desktop_tests/room_tag_tests.rs |
| A01.26 | Multi-element leaders | E2 | not started | Not implemented | — |
| A01.27 | Keynote catalogs/legends | E2 | not started | Not implemented | — |
| A01.28 | General notes | E2 | not started | Not implemented | — |
| A01.29 | Annotation schedules | E2 | not started | Not implemented | — |
| A01.30 | Type/instance label binding | E2 | not started | Not implemented | — |
| A02.01 | Independent detail lines | E2 | partial | View-owned straight XY lines with stable UUIDs, validated plan reference/endpoints/identity, transactional create/edit/remove and history, crop/pick/snap, and 0.25 mm vector sheet/PDF strokes. Focused model, document, frozen 17→18 storage, render and headless egui tests at both DPI profiles cover these paths. No native visual or physical print QA; curves, line styles and standalone detail views remain open. PDF searchability applies to sheet text, not unlabeled line paths. | native-detail-lines.md; crates/os-model/tests/detail_lines.rs; crates/os-document/tests/detail_lines.rs; crates/os-storage/tests/detail_lines.rs; crates/os-render/tests/detail_lines.rs; crates/os-ui/src/plan_workspace/detail_line_tests.rs |
| A02.02 | Filled/masking regions | E2 | not started | Not implemented | — |
| A02.03 | Insulation and break lines | E2 | not started | Not implemented | — |
| A02.04 | Detail components | E2 | not started | Not implemented | — |
| A02.05 | Repeating details | E2 | not started | Not implemented | — |
| A02.06 | Detail groups | E2 | not started | Not implemented | — |
| A02.07 | Foreground/background ordering | E2 | not started | Not implemented | — |
| A02.08 | Parametric detail component editor | E2 | not started | Not implemented | — |
| A02.09 | Annotation symbol editor | E2 | not started | Not implemented | — |
| A02.10 | Tag editor | E2 | not started | Not implemented | — |
| A02.11 | Title-block editor | E2 | not started | Not implemented | — |
| A02.12 | Model-element 2D symbol editor | E2 | not started | Not implemented | — |
| A02.13 | Content reference planes | E2 | not started | Not implemented | — |
| A02.14 | Content constraints | E2 | not started | Not implemented | — |
| A02.15 | Type catalogs | E2 | not started | Not implemented | — |
| A02.16 | Safe expressions/formulas | E2 | not started | Not implemented | — |
| A02.17 | Nested components | E2 | not started | Not implemented | — |
| A02.18 | Visibility parameters | E2 | not started | Not implemented | — |
| A02.19 | Scale/detail previews | E2 | not started | Not implemented | — |
| A02.20 | Searchable local/office libraries | E2 | not started | Not implemented | — |
| A02.21 | Versioned content packages | E2 | not started | Not implemented | — |
| A02.22 | Loading/updating/conflicts | E2 | not started | Not implemented | — |
| A02.23 | Cross-project content transfer | E2 | not started | Not implemented | — |
| A02.24 | Licensed original starter content | E2 | not started | Not implemented | — |
| A02.25 | Affected-instance update previews | E2 | not started | Not implemented | — |
| A02.26 | Preserve project edits on update | E2 | not started | Not implemented | — |
| A02.27 | Content rollback | E2 | not started | Not implemented | — |
| A02.28 | No host-code expression execution | E2 | not started | Not implemented | — |
| P01.01 | Room/area boundaries | E1 | partial | Native straight-wall centerlines and level-owned separator segments form faces with persistent mixed-ID topology signatures; holes and finish offsets remain | native-rooms.md; os-geometry/tests/rooms.rs; os-storage/tests/room_separation_lines.rs |
| P01.02 | Separation lines | E1 | partial | Level-owned straight lines with snapped plan authoring, preview/commit, endpoint/body edits, crop-clipped graphics and selection; schema-18→19 migration and save/reopen covered. Native-window and print acceptance remain | native-rooms.md; os-storage/tests/room_separation_lines.rs; os-ui room separator pointer tests |
| P01.03 | Room/area identifiers | E1 | partial | Native room UUID, level, seed, unique-per-level number and editable name; type catalogs/tags remain | native-rooms.md; os-model/src/rooms.rs; os-ui desktop room test |
| P01.04 | Placement/enclosure diagnostics | E1 | partial | Click-to-place in a unique enclosed face; broken topology reports without blocking wall edits; room repair workflow remains limited to delete/re-place | native-rooms.md; os-geometry/tests/rooms.rs; os-ui desktop room test |
| P01.05 | Area computation rules | E1 | partial | Recomputed centerline polygon area in m², independent of crop; finish-face/net area conventions remain | native-rooms.md; os-geometry/tests/rooms.rs; os-ui desktop room test |
| P01.06 | Finish parameters | E1 | not started | Not implemented | — |
| P01.07 | Room/area tags | E1 | not started | Not implemented | — |
| P01.08 | Color fills/legends | E1 | not started | Not implemented | — |
| P01.09 | Opening and linked-boundary treatment | E1 | partial | Hosted doors/windows remain boundaries through native host centerlines; linked/plugin boundary providers remain unavailable | native-rooms.md; os-ui/tests/openings.rs; os-ui desktop room test |
| P01.10 | Phase-dependent enclosures | E1 | not started | Not implemented | — |
| P01.11 | Existing/new/demolished/temporary states | E1 | not started | Not implemented | — |
| P01.12 | Ordered phases | E1 | not started | Not implemented | — |
| P01.13 | Phase view filters/graphics | E1 | not started | Not implemented | — |
| P01.14 | Phase schedules/tags | E1 | not started | Not implemented | — |
| P01.15 | Linked-phase mapping | E1 | not started | Not implemented | — |
| P01.16 | Isolated option sets | E1 | not started | Not implemented | — |
| P01.17 | Option views/schedules | E1 | not started | Not implemented | — |
| P01.18 | Option membership | E1 | not started | Not implemented | — |
| P01.19 | Accept/merge alternatives | E1 | not started | Not implemented | — |
| P01.20 | Reject cross-option references | E1 | not started | Not implemented | — |
| P01.21 | Configurable reporting conventions | E1 | not started | Not implemented | — |
| S01.01 | Door/window schedules | E2 | partial | Saved model-owned Door/Window/All definitions, named picker, ordered configurable columns and ascending sort; live derived rows, transactional instance-name edits and live placed sheet tables; no custom fields, CSV or pagination | `os-model/tests/schedules.rs`; `os-document/tests/schedules.rs`; `os-storage/tests/schedules.rs`; `os-storage/tests/sheet_tables.rs`; `os-ui::opening_schedule`; `docs/native-hosted-openings.md`; `docs/native-sheet-preview.md` |
| S01.02 | Room/finish/material schedules | E2 | not started | Not implemented | — |
| S01.03 | Key schedules | E2 | not started | Not implemented | — |
| S01.04 | Annotation note blocks | E2 | not started | Not implemented | — |
| S01.05 | Sheet/view indexes | E2 | not started | Not implemented | — |
| S01.06 | Revision schedules | E2 | not started | Not implemented | — |
| S01.07 | Reusable legends | E2 | not started | Not implemented | — |
| S01.08 | Reusable model illustrations | E2 | not started | Not implemented | — |
| S01.09 | Typed shared/project fields | E2 | not started | Not implemented | — |
| S01.10 | Unit-aware calculated fields | E2 | not started | Not implemented | — |
| S01.11 | Filters | E2 | partial | Saved Door/Window/All category filter only; arbitrary predicates remain | `saved_definition_derives_category_columns_sort_and_live_edits` |
| S01.12 | Sort/group | E2 | partial | Configurable deterministic ascending sort with stable ID ties; no grouping or descending sort | `saved_definition_derives_category_columns_sort_and_live_edits` |
| S01.13 | Totals | E2 | not started | Not implemented | — |
| S01.14 | Conditional formatting | E2 | not started | Not implemented | — |
| S01.15 | Key-driven defaults | E2 | not started | Not implemented | — |
| S01.16 | Linked-model rows | E2 | not started | Not implemented | — |
| S01.17 | Phase/option/view/sheet filters | E2 | not started | Not implemented | — |
| S01.18 | Transactional editable cells | E2 | partial | Instance name edits submit one validated `UpdateOpening` transaction; other cells remain read-only | `os-ui::opening_schedule::tests::name_edit_is_draft_only_and_commits_one_undo_step` |
| S01.19 | Read-only computed cells | E2 | partial | Type dimensions, host level/wall and stable ID are derived from the current model and cannot be edited in the schedule | `os-ui::opening_schedule::tests::resolved_rows_follow_type_history_and_archive_without_cached_data` |
| S01.20 | Source selection/highlighting | E2 | partial | Native opening schedule cells select source and navigate to a host-level plan; missing-plan guidance retains selection; existing crop/visibility controls highlighting | `row_click_selects_and_navigates_without_model_or_history_changes` at 1280×800/100% and 1000×650/150% |
| S01.21 | Schedule undo/validation | E2 | partial | Schedule definition create/configure/delete and instance-name edits use validated document transactions; invalid drafts retain values without partial changes; Escape/Cancel and stale session/revision discard drafts | `os-document/tests/schedules.rs`; `os-ui::opening_schedule::definitions::tests`; `os-ui::opening_schedule::tests` |
| S01.22 | Column/header/border styling | E2 | not started | Not implemented | — |
| S01.23 | Repeated headings | E2 | not started | Not implemented | — |
| S01.24 | Split tables | E2 | not started | Not implemented | — |
| S01.25 | Pagination after row changes | E2 | not started | Not implemented | — |
| S01.26 | CSV encoding/units/loss reports | E2 | not started | Not implemented | — |
| S02.01 | Custom title blocks | E3 | not started | Not implemented | — |
| S02.02 | Custom sheet sizes | E3 | not started | Not implemented | — |
| S02.03 | Project/sheet metadata | E3 | partial | Persisted sheet number/name and basic title block; project metadata, custom title blocks and editable issue metadata remain | native-sheet-preview.md; os-model/src/sheets.rs; os-ui/src/plan_workspace/sheet_tests.rs |
| S02.04 | Unique drawing numbers | E3 | automated-tested | Trimmed ASCII case-insensitive uniqueness is model-validated; UI assigns the next free A-series number; numbering policy and atomic renumbering remain | native-sheet-preview.md; os-model/src/sheets.rs; os-model validation tests |
| S02.05 | Placeholder sheets | E3 | not started | Not implemented | — |
| S02.06 | Sheet browser organization | E3 | partial | Basic flat sheet picker; folders, sorting, filtering and index views remain | native-sheet-preview.md; os-ui/src/plan_workspace.rs |
| S02.07 | Sheet indexes | E3 | not started | Not implemented | — |
| S02.08 | Batch sheet edits | E3 | not started | Not implemented | — |
| S02.09 | Place/align/rotate scaled viewports | E3 | partial | One linked Plan viewport can be positioned numerically and assigned a scale; rotation, multiple viewports and graphical placement remain | native-sheet-preview.md; os-ui/src/plan_workspace/sheet_tests.rs; os-render/src/sheet.rs |
| S02.10 | Viewport cropping | E3 | partial | Source plan crop and viewport rectangle clipping are honored; independent viewport crop editing remains | native-sheet-preview.md; os-render/src/sheet.rs |
| S02.11 | Viewport titles | E3 | partial | Source view name or persisted title override appears with the scale; title editing controls and full annotation styles remain | native-sheet-preview.md; os-model/src/sheets.rs; os-render/src/sheet.rs |
| S02.12 | Guide grids | E3 | not started | Not implemented | — |
| S02.13 | Mixed scales | E3 | not started | Not implemented | — |
| S02.14 | Sheet schedules/legends | E3 | partial | One saved door/window schedule table with explicit combined layout, live rows, searchable preview/PDF marks, fit diagnostics and undoable Add/Remove; no numeric draft editor, legends, multiple-table UI or pagination | `os-ui::plan_workspace::sheet_tests::saved_schedule_sheet_live_preview_pdf_history_overflow_and_stale_export`; `os-render::sheet::tables::tests`; `os-storage/tests/sheet_tables.rs`; `docs/native-sheet-preview.md` |
| S02.15 | Sheet duplication/view-copy rules | E3 | not started | Not implemented | — |
| S02.16 | Automatic cross-references | E3 | not started | Not implemented | — |
| S02.17 | Placed/unplaced status | E3 | not started | Not implemented | — |
| S02.18 | Sheet/source navigation | E3 | partial | Flat sheet picker opens the linked Plan and an Edit source plan action returns to it; cross-reference navigation remains | native-sheet-preview.md; os-ui/src/plan_workspace.rs |
| S02.19 | Atomic renumbering | E3 | not started | Not implemented | — |
| S02.20 | Ordered sheet sets | E3 | not started | Not implemented | — |
| S02.21 | Package grouping | E3 | not started | Not implemented | — |
| S02.22 | Issue/revision metadata | E3 | not started | Not implemented | — |
| S02.23 | Superseded/withdrawn sheet history | E3 | not started | Not implemented | — |
| I01.01 | Revision sequences | E3 | not started | Not implemented | — |
| I01.02 | Project/sheet numbering policy | E3 | not started | Not implemented | — |
| I01.03 | View/sheet revision clouds | E3 | not started | Not implemented | — |
| I01.04 | Revision tags | E3 | not started | Not implemented | — |
| I01.05 | Revision descriptions/dates/recipients | E3 | not started | Not implemented | — |
| I01.06 | Revision schedules | E3 | not started | Not implemented | — |
| I01.07 | Revision inclusion/exclusion | E3 | not started | Not implemented | — |
| I01.08 | Review/approve/issue transitions | E3 | not started | Not implemented | — |
| I01.09 | Immutable issued revisions | E3 | not started | Not implemented | — |
| I01.10 | Explicit superseding issues | E3 | not started | Not implemented | — |
| I01.11 | Frozen model/view/template/link/plugin publish snapshot | E3 | not started | Not implemented | — |
| I01.12 | Issue drawing/revision manifest | E3 | not started | Not implemented | — |
| I01.13 | File hashes and publish settings | E3 | not started | Not implemented | — |
| I01.14 | Recorded warnings/acknowledgments | E3 | not started | Not implemented | — |
| I01.15 | Immutable issued artifact archive | E3 | not started | Not implemented | — |
| I01.16 | Historic issue reproduction | E3 | not started | Not implemented | — |
| I01.17 | Broken-reference preflight | E3 | not started | Not implemented | — |
| I01.18 | Stale-result preflight | E3 | not started | Not implemented | — |
| I01.19 | Missing font/link/plugin preflight | E3 | not started | Not implemented | — |
| I01.20 | Duplicate sheet number preflight | E3 | not started | Not implemented | — |
| I01.21 | Unplaced required view preflight | E3 | not started | Not implemented | — |
| I01.22 | Temporary-state preflight | E3 | not started | Not implemented | — |
| I01.23 | Dimension-override preflight | E3 | not started | Not implemented | — |
| I01.24 | Clipping/overflow preflight | E3 | not started | Not implemented | — |
| I01.25 | Blocking scale/geometry/reference faults | E3 | not started | Not implemented | — |
| X01.01 | Vector-first PDF | E3 | partial | Confirmed atomic single-page vector PDF from the shared paper-space drawing; fixed A3 landscape only and independent viewer/physical-scale checks remain | native-sheet-preview.md; os-render/src/sheet.rs; os-ui/src/plan_workspace/sheet_tests.rs |
| X01.02 | Explicit raster fallback | E3 | not started | Not implemented | — |
| X01.03 | Searchable lawful embedded text | E3 | partial | Helvetica/WinAnsi text is searchable and unsupported characters fail closed; fonts are not embedded and licensing review remains | native-sheet-preview.md; os-render/src/sheet.rs |
| X01.04 | Font substitution | E3 | not started | Not implemented | — |
| X01.05 | PDF weights/fills/transparency | E3 | partial | Basic vector weights and fills are emitted; transparency and office style mapping remain | native-sheet-preview.md; os-render/src/sheet.rs |
| X01.06 | PDF hyperlinks/bookmarks | E3 | not started | Not implemented | — |
| X01.07 | Page boxes/orientation/mixed sizes | E3 | partial | One A3 landscape MediaBox is tested; alternate page boxes and mixed orientations/sizes remain | native-sheet-preview.md; os-render/src/sheet.rs; os-ui/src/plan_workspace/sheet_tests.rs |
| X01.08 | Single/combined exports | E3 | not started | Not implemented | — |
| X01.09 | Saved ordered export sets | E3 | not started | Not implemented | — |
| X01.10 | Parameter-based output naming | E3 | not started | Not implemented | — |
| X01.11 | Background export progress/cancel | E3 | not started | Not implemented | — |
| X01.12 | Atomic complete-package publishing | E3 | partial | One PDF is written through a synced same-directory temporary and no-clobber/explicit-replace path; multi-sheet package publishing remains | native-sheet-preview.md; os-ui/src/plan_workspace/sheet_tests.rs |
| X01.13 | Native platform printing | E3 | not started | Not implemented | — |
| X01.14 | Print size/margins/orientation | E3 | not started | Not implemented | — |
| X01.15 | Print color/monochrome | E3 | not started | Not implemented | — |
| X01.16 | Print scale and preview | E3 | not started | Not implemented | — |
| X01.17 | Fit-to-page warning | E3 | not started | Not implemented | — |
| X01.18 | PDF/image underlays | E3 | not started | Not implemented | — |
| X01.19 | Underlay page/calibration/rotation/crop/reload | E3 | not started | Not implemented | — |
| X01.20 | Vector underlay snapping | E3 | not started | Not implemented | — |
| X01.21 | Raster limitation diagnostics | E3 | not started | Not implemented | — |
| X01.22 | Declared DWG versions/entities | E3 | not started | Not implemented | — |
| X01.23 | Declared DXF versions/entities | E3 | not started | Not implemented | — |
| X01.24 | CAD layers/colors/line types/hatches | E3 | not started | Not implemented | — |
| X01.25 | CAD blocks/text/font mapping | E3 | not started | Not implemented | — |
| X01.26 | CAD units/coordinates/paper/model space | E3 | not started | Not implemented | — |
| X01.27 | CAD external-reference packaging | E3 | not started | Not implemented | — |
| X01.28 | Independent CAD validation | E3 | not started | Not implemented | — |
| X01.29 | Native/IFC/CAD live links | E3 | not started | Not implemented | — |
| X01.30 | Relative link paths/shared coordinates | E3 | not started | Not implemented | — |
| X01.31 | Pin/reload/unload/replace links | E3 | not started | Not implemented | — |
| X01.32 | Nested-link/cycle policy | E3 | not started | Not implemented | — |
| X01.33 | Link visibility/missing diagnostics | E3 | not started | Not implemented | — |
| X01.34 | Portable linked packages | E3 | not started | Not implemented | — |
| X01.35 | Stable references across reload | E3 | not started | Not implemented | — |
| X01.36 | Snapshot versus live-link versions | E3 | not started | Not implemented | — |
| X01.37 | Translator licensing/redistribution approval | E3 | not started | Not implemented | — |
| X01.38 | Two independent PDF viewers | E3 | not started | Not implemented | — |
| X01.39 | Physical plot scale measurement | E3 | not started | Not implemented | — |
| X01.40 | Permissioned third-party roundtrip fixtures | E3 | not started | Not implemented | — |
| T01.01 | Single-writer ownership locks | E3 | not started | Not implemented | — |
| T01.02 | Lock expiry policy | E3 | not started | Not implemented | — |
| T01.03 | Read-only open | E3 | not started | Not implemented | — |
| T01.04 | External-change detection | E3 | not started | Not implemented | — |
| T01.05 | Deliberate writer handoff | E3 | not started | Not implemented | — |
| T01.06 | Recoverable backups | E3 | not started | Not implemented | — |
| T01.07 | Self-hosted worksharing ADR | E3 | not started | Not implemented | — |
| T01.08 | Editor identity | E3 | not started | Not implemented | — |
| T01.09 | Ownership/borrowing | E3 | not started | Not implemented | — |
| T01.10 | Atomic synchronization | E3 | not started | Not implemented | — |
| T01.11 | Conflict resolution | E3 | not started | Not implemented | — |
| T01.12 | Team permission checks | E3 | not started | Not implemented | — |
| T01.13 | Audit history | E3 | not started | Not implemented | — |
| T01.14 | Recoverable local work | E3 | not started | Not implemented | — |
| T01.15 | Concurrent model/annotation/view/sheet/content edits | E3 | not started | Not implemented | — |
| T01.16 | Host/dependent conflicts | E3 | not started | Not implemented | — |
| T01.17 | Undo after another user sync | E3 | not started | Not implemented | — |
| T01.18 | Interrupted-connection recovery | E3 | not started | Not implemented | — |
| T01.19 | Stale ownership recovery | E3 | not started | Not implemented | — |
| T01.20 | Crash/restart/central restore | E3 | not started | Not implemented | — |
| T01.21 | Version mismatch handling | E3 | not started | Not implemented | — |
| T01.22 | Federation distinct from shared editing | E3 | not started | Not implemented | — |
| T01.23 | Read-only review | E3 | not started | Not implemented | — |
| T01.24 | Portable project/issue archives | E3 | not started | Not implemented | — |
| T01.25 | Offline installer | E3 | not started | Not implemented | — |
| T01.26 | Qualified OS/hardware matrix | E3 | not started | Not implemented | — |
| T01.27 | Controlled upgrades | E3 | not started | Not implemented | — |
| T01.28 | Project-preserving uninstall | E3 | not started | Not implemented | — |
| T01.29 | Admin content/plugin versions | E3 | not started | Not implemented | — |
| T01.30 | Keyboard shortcuts | E3 | partial | Existing desktop keyboard/DPI checks; not full platform qualification | os-ui desktop_tests; docs/ui-design-guide.md |
| T01.31 | High-DPI/multi-monitor qualification | E3 | partial | Existing desktop keyboard/DPI checks; not full platform qualification | os-ui desktop_tests; docs/ui-design-guide.md |
| T01.32 | Accessible focus/contrast | E3 | partial | Existing desktop keyboard/DPI checks; not full platform qualification | os-ui desktop_tests; docs/ui-design-guide.md |

## Cross-cutting architecture and acceptance obligations

| ID | Requirement | Status | Evidence / dependency |
| --- | --- | --- | --- |
| ARCH.01 | Persistent object distinctions and stable schemas/dependencies | partial | Model schema 4 adds building-scoped straight grid entities with validated references, commands/history and frozen migration tests; typed plan settings remain version 1. See architectural-grids.md; native grid tools and drafting/content/library distinctions remain |
| ARCH.02 | Model/view/paper coordinate transforms and tolerance policy | partial | Horizontal world/view/screen transforms and pan/zoom tested; paper transforms and production precision qualification remain |
| ARCH.03 | Shared deterministic semantic 2D representation | partial | Bounded UUID-bearing polygon/line IR, room faces/labels and matching room picking; screen/output adapters, curves and style system remain |
| ARCH.04 | Cut/projected/symbolic graphics and override precedence | partial | Native prism cut/projected/depth roles and deterministic ordering; symbolic/category/template behavior remains |
| ARCH.05 | Revision/style/font/content/plugin/link-aware cache keys | partial | Native plan consumption checks session/model/view/settings and derives room faces from revision-bound same-level walls; style/font/content/plugin/link participation remains |
| ARCH.06 | Targeted regeneration and bounded background work | partial | Opt-in Editor uses bounded v2 workers and invalidates plugin meshes conservatively across revisions; transitive dependencies are tested, but targeted plugin caching, visible desktop controls and plan regeneration remain |
| ARCH.07 | Discard stale document/view replies | partial | V1/v2 Wasm worker session/model/view and generation checks tested in wasm_transport.rs and generic_commands.rs; desktop/plan pipeline not yet integrated |
| ARCH.08 | Frozen validated issue snapshots | not started | Existing model/transaction foundations do not establish this full requirement |
| ARCH.09 | Transactional annotation/sheet/content/schedule edits | partial | Saved schedule definitions, opening instance-name edits and schedule-table placement/removal use validated undoable document commands; annotation/content transactions and full runtime acceptance remain | `os-document/tests/schedules.rs`; `os-storage/tests/sheet_tables.rs`; `os-ui/src/plan_workspace/sheet_tests.rs` |
| ARCH.10 | Unknown plugin payload preservation and migration | partial | Model/storage extension tests, frozen envelope fixture, read-only UI, atomic core migration proposals; plugin-owned executable migration dispatch and full B/C installation acceptance remain |
| ARCH.11 | Callable content/tag/snap/graphics/schedule/export/preflight providers | not started | Existing model/transaction foundations do not establish this full requirement |
| ARCH.12 | Independent Rust SDK and Wall/column installation workflow | partial | Separately locked Rust Wasm Wall and column copied to installation directories; probes verify descriptors, checked worker geometry, native Wall/extension create/edit/delete/history and absent-plugin reopen. See native-wall-plugin.md. General SDK packaging and complete installed desktop authoring acceptance remain |
| ARCH.13 | Plugin deadline/cancellation/disable lifecycle | partial | Worker cancellation/result deadlines and unload/reload tested; compiler/desktop manager integration and full runtime acceptance missing |
| ARCH.14 | 2D plugin pointer/work-plane/preview/cancel/commit contracts | partial | plugin-plan-contract-design.md specifies context/gesture/snap/graphics validation and D acceptance; no callable 2D provider or wire DTO yet |
| ARCH.15 | Two-wall snapped exact-input split-view acceptance | partial | Bundled-provider native 4 m/3 m perpendicular snapped walls, cancellation, save/reopen and shared selection verified; independent plugin route and broader D workflow remain (native-pointer-walls.md) |
| ARCH.16 | Cross-view shared identity/selection and single-action undo | partial | Native room UUID is shared by plan/browser selection and each room edit is one history action; 3D room graphics and broader cross-view categories remain |
| ARCH.17 | Persist plan cut ranges and verify late inactive-view replies | not started | Existing model/transaction foundations do not establish this full requirement |

## Production release gates

| Gate | Status | Missing required evidence |
| --- | --- | --- |
| G1 | not started | New multi-storey and renovation reference projects; create/check/issue/revise/archive native workflows; no architect sign-off required |
| G2 | partial | Existing prism/IFC/depth tests only; architectural geometry, annotations, independent outputs, physical plots and approved tolerances missing |
| G3 | partial | Atomic save/corruption tests exist; checkpoint/restore UI, crash/disk/network/sync/publish failure matrix and clean-machine archive tests missing |
| G4 | not started | Approved hardware/project budgets, p95 measurements, memory bounds, five eight-hour scripted sessions per profile |
| G5 | not started | Agreed topology/concurrency, conflict/restore and declared CAD/PDF/plot/second-workstation checks |
| G6 | partial | Unsafe lint, bounded Wasm spike, dependency audit; expanded fuzzing, license decision, installer integrity, SDK/support/maintenance owner and full documentation remain |

## Work sequence and next evidence

1. Complete B/C without enabling data-losing external elements: preservation, versioned generic SDK including 2D contexts, worker lifecycle, extension envelopes and independently built examples.
2. Complete D's actual plan/split-view workflow and independent 2D plugin acceptance before E1–E4.
3. Implement and verify every E1–E4 capability and G1–G6 gate; resolve owner profile/license/translator/support choices before qualification or publication.

[Opaque file decision](decisions/0007-opaque-container-files.md) addresses container bytes; [semantic envelope decision](decisions/0009-semantic-extension-envelopes.md) now adds bounded plugin payload preservation. Keep source-backed evidence current as each workflow lands. Initial classifications above are conservative and do not replace requirement-by-requirement completion audits.
