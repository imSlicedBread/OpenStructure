# Native wall dimensions

## Native Section dimensions (schema 69 / native API 50)

Section views expose Horizontal dimension and Vertical dimension. Pick two
visible native straight-wall or rectangular door/window aperture extents, then
click the offset. Properties edit the offset, explicitly replace either anchor,
or delete the dimension. Each successful operation is one undoable transaction.
Escape, invalid release, changed revision/session/view/settings/provider or
drawing identity cancel the transient draft without model/history/scene edits.
Canvas presses stay consumed through release; ordinary dragging still pans.

`SectionHorizontal` and `SectionVertical` layouts accept only `SectionWall` and
`SectionOpening` references. UUIDs and a semantic feature are stored, never mesh
edges or cached lengths. The four feature names reuse the existing
`ElevationDimensionFeature` vocabulary, but resolution is exclusively in the
owning Section view: Start/End are minimum/maximum **section station**, and
Bottom/Top are absolute project elevations. They are not wall-local elevation
references; cross-view resolution is rejected. The analytic nominal rectangular
footprint intersects the section plane in CutOnly, or its near/far volume in
CutAndProjection. Changing that volume can change the measured extents. Station
and height crop do not shorten measurements; fully excluded sources report
`OutsideSection`. Opening references follow the same opening UUID and its current
host, including live typed sizes and instance overrides.

Acquisition requires a collinear visible stroke from the current drawing,
clipped to the nominal edge and view crop, within 12 logical pixels. Aperture
references win exact ties against their host cut outline. Missing sources keep
the annotation with explicit diagnostics and can be repaired one anchor at a
time; there is no automatic nearest-edge reassignment. Curved hosts and custom
aperture cut profiles fail closed. Chains, constraints, arbitrary mesh references
and general annotation/print qualification remain outside this slice.

The frozen drawing stores one shared `PlanDimensionItem` used by canvas, sheet
and vector PDF. Tests: `os-model/tests/section_dimensions.rs`,
`os-document/tests/section_dimensions.rs`, `os-storage/tests/section_dimensions.rs`,
`os-render/tests/section_dimensions.rs`, and
`os-ui/src/plan_workspace/section_dimension_tests.rs` plus acquisition/context
tests in `section_dimensions.rs`. The real-egui tests cover 1280×800/100% and
1000×650/150%, preview immutability, history, cancellation, repair, pan precedence,
and sheet/PDF labels. These are automated checks, not physical-print approval.

## Linked elevation dimensions (schema 65)

Horizontal/vertical dimensions now belong to linked straight-wall elevations.
Pick two visible nominal wall/opening extents then the offset; properties edit
the offset, replace either reference, or delete with one-step history. References
are explicit elevation variants, independent of legacy Plan XY anchors. Derived
values use current host station and absolute project elevation, including typed
opening sizes and overrides. Preview does not mutate the model or 3D scene.
Missing references preserve intent and metadata. Viewport, sheet and PDF consume
the same dimension graphics. See [elevation workflow and limits](native-elevations.md).
No elevation chains, angular dimensions, driving constraints, arbitrary profile
references or external-provider gestures are introduced.

## Full circular-wall axis arc length (schema 63)

Choose **Arc length** in the dimension tools, click a displayed native circular
wall, then click to place a concentric offset arc. Acquisition uses the actual
clipped on-level wall body with a 12 logical-pixel tolerance. Distance then wall
UUID resolves overlapping circular candidates; foreground drafting/other
geometry blocks acquisition. Hidden walls, other levels and cropped-away
geometry cannot be acquired. Backspace clears the wall choice; Escape or pointer
loss cancels. Document session/revision, active view/settings, provider activation
and drawing identity changes cancel drafts. A claimed press stays consumed
through release so a cancelled gesture cannot pan or select underlying geometry.

`DimensionLayout::ArcLength` stores Start then End of the **same** wall, empty
`additional`, canonical `baseline_spacing_m = 0.25`, and a signed outward radial
`offset_m`. Its analytic axis length is `radius * abs(signed_sweep)`. Offset,
crop, view basis and display tessellation cannot change that measurement. Neither
the measurement nor samples are persisted. `resolve_arc_length` returns the
center, axis radius, display radius, start/signed sweep and metric length;
`resolve` also reports this full-axis length, never its chord. Creation and live
offset editing reject a display radius at or below 1 micrometre.

Preview changes no model, history or 3D geometry. Placement uses one
`AddDimension`; Properties offers **Outward offset (m)** and **Replace curve**.
Repair acquires another visible circular wall and replaces both references in
one `UpdateDimension`, retaining the dimension UUID, offset, hint and other
values. Each action is one undo/redo step. Host edits refresh the derived
measurement after commit. Missing/releveled hosts and conversion to a straight
path preserve orphan intent and render a diagnostic marker; restoring the host
restores resolution. Wall split does not remap one endpoint of this paired
reference to another child: intent follows the surviving wall UUID.

`PlanArcLengthDimension` supports clockwise/counterclockwise minor and major
arcs, with a bounded 256-chord display and two witnesses from axis endpoints to
display endpoints. Viewport painting and picking share crop-clipped strokes and
metric label bounds. Sheet/PDF consumes the same derived graphic and metric
label, with its paper viewport clip. Existing annotation and host-exclusion
visibility policy applies. Viewport text remains a logical-pixel label; general
paper-scale annotation styling is still outside this increment.

Schema 62→63 advances native headers and the model version without rewriting
legacy parameters, IDs or opaque metadata. That increment introduced native API
44/schema 63; current full-model plugins use API 56/schema 75. ZIP container 2
and generic plugin API 2 stay unchanged.
Frozen migration evidence: `os-storage/tests/fixtures/schema-62-arc-dimensions.json`.
Focused suites: `os-model/tests/dimension_arc_length.rs`,
`os-document/tests/dimensions.rs`, `os-storage/tests/arc_dimensions.rs`,
`os-render/tests/arc_length_dimensions.rs`, and
`os-ui/src/plan_workspace/arc_dimension_tests.rs`. The UI suite uses real egui
frames at 1280×800/100% and 1000×650/150%, plus a rotated large-coordinate
sheet/PDF case. Native-window visual inspection and physical print acceptance
are not implied by these automated tests.

Radial/diameter dimensions, partial-arc stations, curved faces/layers, plugin
curves and driving constraints are not supported by this layout.

## Associative outer wall faces (schema 53)

Wall-face station anchors remain straight-wall-only. Circular walls retain
analytic endpoint anchors, but tangent/curved outer-face dimensions are not
implemented; such anchors remain unresolved rather than being measured on a
chord.

Aligned, Chain and Baseline dimensions accept `WallFace { wall, side, station_m }`.
`side` is `DimensionWallSide::Left` or `Right` relative to the stored Start→End
wall axis. `station_m` is a metric distance from Start, not a normalized fraction.
The resolved point is the centerline station plus or minus half the current
effective thickness. Typed walls use the compound layer total from
`Model::resolve_wall`; changing a type, wall position, direction or thickness
updates the dimension. Reversing Start/End intentionally changes side/station
meaning. Angular dimensions still accept only endpoint references.

Placement and Properties reference replacement use the same 12 logical-pixel
picker. Endpoint and opening-jamb candidates take priority. Face candidates must
lie on a displayed clipped wall body, so hidden/cropped faces and opening voids
cannot be acquired. Foreground objects block picking through their geometry.
Distance, wall UUID and Left-before-Right break face ties deterministically.
Preview leaves model, history and scene unchanged; creation or replacement is
one transaction. Existing Escape and stale-context cancellation applies.

Missing walls, wrong levels, degenerate axes and stations beyond a shortened
wall remain orphan diagnostics. Non-finite or negative stations are invalid
persistent syntax. Out-of-range positive stations remain saveable for repair.
The snapshot resolver carries effective thickness into the common plan graphics
used by viewport, sheets and PDF. No resolved coordinates are stored.

Evidence: `os-model/tests/dimension_faces.rs`,
`os-storage/tests/dimension_faces.rs` with frozen schema-52 fixture, and
`os-ui/src/plan_workspace/face_tests.rs`. UI coverage uses real egui frames at
1280×800/100% and 1000×650/150%; native-window visual inspection and dedicated
sheet/PDF face-output comparisons are not yet qualified. Curved walls,
material-layer faces, linked/plugin geometry and driving constraints are out of
scope. Wall split does not remap face stations to a new child wall; shortened
hosts can leave an orphan to repair.

Model schema 7 introduced `core.dimension`, a view-owned reporting annotation
between native straight-wall endpoints. Schema 11 adds **Chain** and
**Baseline** layouts. Aligned takes two anchor clicks and one offset click;
Chain/Baseline collect an ordered sequence of anchors, require **Finish
anchors**, then take one offset click. Preview never changes the model, and the
completed annotation is one undoable entity. Chain measures adjacent endpoints
on a shared dimension line. Baseline measures each later endpoint from the
first on successively offset parallel lines. Dimensions render and pick above
wall bodies, expose metric values and signed offset in Properties, and appear
in the Project Browser.

## Public integration contract

Types are re-exported from `os_model`:

```rust,ignore
DimensionParams {
    layout: DimensionLayout,
    additional: Vec<DimensionReference>,
    baseline_spacing_m: f64,
    view: Id,
    first: DimensionReference,
    second: DimensionReference,
    offset_m: f64,
    orphan_hint: Point2,
}
enum DimensionReference {
    WallEndpoint { wall: Id, endpoint: DimensionEndpoint },
    OpeningJamb { opening: Id, jamb: DimensionJamb },
    WallFace { wall: Id, side: DimensionWallSide, station_m: f64 },
}
DimensionEndpoint::{Start, End}
DimensionJamb::{Start, End}
DimensionWallSide::{Left, Right}
DimensionLayout::{Aligned, Chain, Baseline, Angular, ArcLength,
                  ElevationHorizontal, ElevationVertical}
DimensionReference::{ElevationWall, ElevationOpening} // station/project elevation
// Dimension = Entity<DimensionParams>; Model.dimensions is keyed by stable UUID.
```

`additional` stores later references in click order. Aligned requires no
additional references; Chain/Baseline require at least one. Linear dimensions
may mix wall endpoints, outer wall faces and native opening jambs. An opening's Start jamb is
derived from its current host start plus instance offset; End uses the current
effective width (type value or instance override). Jamb identity follows the
host's stored start-to-end direction and is independent of door hinge/swing or
window pane position. Resolved points are never persisted. Angular dimensions
remain wall-endpoint-only. All anchors must be
distinct, strictly forward, and collinear with the first-to-second axis within
1 μm. Chain measures consecutive points; Baseline measures each later point
from the first. `baseline_spacing_m` is a positive model-space spacing between
baseline lines. `offset_m` is signed metres along the left normal of the directed
first-to-second anchor segment in model XY coordinates. `orphan_hint` is the
clicked placement point in model XY metres, retained to locate an unresolved
annotation. Measured lengths and resolved coordinates are derived, not stored.
Swapping anchor order flips the normal; side preservation is not implemented.

`DimensionParams::validate_creation(&Model) -> os_core::Result<()>` is the public
preflight API used by `Command::AddDimension`. It checks syntax then requires two
resolvable native wall endpoints, faces or opening jambs on the owning plan's level, with finite distance
greater than one micrometre. Use the candidate document model at commit time;
preflight results are not a substitute for document revision checks in the UI.

`DimensionParams::resolve(&Model) -> Result<ResolvedDimension, DimensionDiagnostic>`
remains the two-anchor aligned compatibility API and returns
`ResolvedDimension { first, second, length_metres }`. `resolve_points` resolves
all ordered anchors for Chain/Baseline; missing later references identify their
one-based anchor index. Resolution does not apply visibility, crop, projection,
offset or formatting. Both references may address opposite endpoints of one
wall; identical references are rejected.

## Validation, edits and persistence

Schema 12 adds Angular: select two distinct visible native straight-wall bodies
on the plan level, then click inside the desired sector to set its arc radius.
The four non-reflex sectors are supported. `first` and `second` identify walls;
their endpoint values select positive (End) or negative (Start) axis directions.
`additional` must be empty, `baseline_spacing_m` must be 0.25, and `offset_m`
is a positive radius in metres. `resolve_angular` derives the infinite-axis
intersection and angle in degrees; Angular does not use the length resolver.
Parallel/near-parallel axes (absolute unit-direction cross product <= 1e-6),
wrong-level walls, degenerate geometry, and placement on a ray are rejected.
Parallel walls are currently rejected at placement, not at the second wall click.
Backspace leaves placement mode and removes the latest wall choice; Escape
cancels. Preview does not mutate the model; placement adds one history entry.
Missing, releveled or newly parallel references retain their intent and can
resolve again after geometry is restored. Angular arcs use 64 chords and two
radial witnesses; painting and picking share clipped segments and label bounds.

Persistent syntax requires non-nil wall/opening/view IDs, distinct reference identities,
finite offset and hint coordinates bounded to +/- 1,000,000 metres, and an extant
floor-plan View. Header identity/type/schema and global identity uniqueness use
the same checks as other native entities. Parameter and reference objects reject
unknown fields, including a persisted measurement field.

Missing walls/openings/hosts, changed wall/view levels and subsequently coincident anchors are
valid stored intent. Ordinary wall edits/deletion do not rewrite or discard a
dimension or fail solely because its anchors become unresolved. Undo can restore
resolution. Unresolved dimensions show a red plan marker, a Properties diagnostic,
and missing wall/opening references in the Project Browser. Properties provides explicit
replacement of a chosen reference in the owning plan, as described below.

`AddDimension(Dimension)`, `UpdateDimension { id, parameters }`, and
`RemoveDimension(Id)` use the existing atomic transaction/history path.
Updates retain the header UUID and validate persistent syntax; they may edit an
already orphaned dimension. Creation alone requires currently resolved anchors.
`RemoveView` rejects an owned view until its dimensions are removed; a transaction
can remove dimensions first and then their view. Wall, opening, level and view edits
invalidate dependent dimensions; annotation changes invalidate the owning view.

Schema 6→7 inserts an empty dimensions map and advances all native entity header
versions. Schema 10→11 defaults prior dimensions to `Aligned`, no additional
anchors and 0.25 m baseline spacing; it advances every native entity header and
preserves IDs and opaque plugin payloads. Schema 11→12 advances native headers
and the model version without changing prior dimension parameters or IDs.
Older readers reject schema 12. Schema 42→43 strictly wraps legacy wall endpoint
references in the tagged `WallEndpoint` variant; unknown, mixed, or malformed
schema-42 shapes fail atomically. `OpeningJamb` references persist opening identity
and jamb side, never cached coordinates. The frozen schema-42 fixture covers all
layouts, orphan references, headers, opaque metadata and save/reopen. Older readers
reject schema 43. Previous migrations remain chained. The ZIP container stays
version 2. Native Model API advances 23→24 for the tagged reference wire shape;
generic plugin API 2 is unchanged. Arbitrary plugin reference support is not
added.

## Deliberate reference replacement (2026-09-27)

Select the dimension in its owning floor plan, then use **Replace anchor N** in
Properties. Hover a visible native wall endpoint or opening jamb for Aligned,
Chain or Baseline;
for Angular, hover a visible native wall body. The replacement must belong to
the plan's level. Angular retains the chosen reference's Start/End orientation.
When all references resolve, the existing annotation painters show the candidate's
live lengths or angle, offset and baseline spacing. If other anchors are still
missing or on the wrong level, preview marks the target and lists the remaining
unresolved anchor numbers, without drawing a measured dimension. Click a valid target to apply; Escape or
**Cancel dimension** discards the draft. Resolved references can also be replaced
deliberately, and further valid replacements can be started from Properties.

Preview keeps the model, revision, scene, drawing and undo/redo history unchanged.
Each successful click issues exactly one `UpdateDimension`, changing only the
chosen reference. Whole-model equality tests verify preservation of the entity
UUID/header, other references, layout, view, offset/radius, baseline spacing,
orphan hint and source walls. Dimensions remain reporting annotations.
Both preview and commit validate persistent syntax and unique reference identities.
When all anchors are present on the owning plan's level, they also call
`DimensionParams::validate_creation` against the current model, including
ordering, collinearity and parallel checks.
Invalid targets show an error and leave the tool active without a transaction.

The draft is bound to document session/revision, owning view, single selection,
Wall provider activation, provider signature, and native/displayed drawing
identities. Context changes, missing drawings, Escape and pointer loss cancel;
a claimed press stays consumed through release. Opening-jamb targeting uses a
12 logical-pixel radius and only openings with native symbol lines visible after
view-range and crop clipping can be acquired. No generic plugin gesture protocol
changed.

Multiple missing or wrong-level references can be repaired in separate transactions.
A visible same-level replacement may commit while other unchanged references
remain unavailable. The dimension retains its orphan hint and live diagnostic;
Properties offers a fresh repair action for the next anchor. Each replacement has
its own undo/redo step. Geometry validity is deferred while references are unavailable;
the final replacement must pass full creation validation, so a duplicate, backtracking,
off-axis or parallel result cannot be accepted as a resolved dimension. This is a
UI repair policy using the existing syntax-validating `UpdateDimension` command,
not a relaxation of creation or model validation.

Focused evidence in `crates/os-ui/src/plan_workspace/endpoint_tests.rs`:

- `dimension_repair_properties_preview_commit_and_history_at_both_dpis`: real
  Properties buttons, all four layouts, measurement painting, immutable preview,
  exact model preservation, one revision/undo/redo and retention of redo history.
- `dimension_repair_first_anchor_rotated_plan_and_repeated_replacement`: first
  anchor replacement in all layouts with a rotated plan basis; repeated linear
  replacements remain independent undoable transactions.
- `dimension_repair_cancel_and_stale_context_at_both_dpis`: Escape, pointer loss,
  session, revision, selection/single-selection set, view, provider and replaced
  or missing drawing cancellation, including release after cancellation.
- `dimension_repair_invalid_targets_leave_model_and_history_unchanged`: duplicate,
  empty, wrong-level, hidden/cropped, backtracking, off-axis and parallel targets,
  including duplicate rejection while another anchor remains missing.
- `dimension_repair_two_unavailable_anchors_commit_separately_at_both_dpis`: all
  four layouts with two missing references or one missing and one wrong-level
  reference; immutable target-only preview, continued diagnostic after the first
  commit, valid measured preview/resolution after the second, exact model/UUID
  preservation and independent undo/redo through both states.
- `opening_jamb_dimensions_create_follow_live_width_and_history_at_both_dpis`:
  typed door/window jamb selection, immutable preview, one-step dimension
  creation, live type-width resolution, regenerated plan labels and undo/redo.
  Model tests also cover legacy widths, overrides, reversed/rehosted walls and
  door hinge/swing independence. The frozen schema-42 fixture verifies strict
  atomic migration, metadata preservation and `.osb` save/reopen.
- `dimension_repair_can_target_visible_opening_jambs_at_both_dpis`: missing
  opening anchor repair through Properties, visible-jamb preview, one update,
  syntax/geometry validation and undo/redo.
- `opening_jamb_anchor_hit_radius_and_hidden_crop_precedence_are_logical_pixels`:
  exact 12-point hit radius at both DPI profiles; hidden and cropped opening
  symbols cannot supply anchors.

All five tests run at 1280×800/100% and 1000×650/150%. Commands run for this slice:

```text
cargo test -p os-ui --lib dimension_repair --no-default-features
# 5 passed, 0 failed
cargo test -p os-ui --lib dimension_repair --all-features
# 5 passed, 0 failed
cargo test -p os-ui --lib plan_workspace::endpoint_tests --all-features
# 59 passed, 0 failed, 3 ignored (explicitly installed Wall guest required)
cargo test -p os-model --test dimension_openings --offline
cargo test -p os-storage --test dimension_anchors --test dimensions --offline
cargo test -p os-plugin-api --all-features --offline
rustfmt --edition 2024 --config skip_children=true crates/os-model/src/dimensions.rs crates/os-storage/src/lib.rs crates/os-storage/tests/dimension_anchors.rs crates/os-ui/src/plan.rs crates/os-ui/src/plan_workspace.rs crates/os-ui/src/palettes.rs crates/os-ui/src/plan_workspace/endpoint_tests.rs
rustfmt --check --edition 2024 --config skip_children=true crates/os-model/src/dimensions.rs crates/os-storage/src/lib.rs crates/os-storage/tests/dimension_anchors.rs crates/os-ui/src/plan.rs crates/os-ui/src/plan_workspace.rs crates/os-ui/src/palettes.rs crates/os-ui/src/plan_workspace/endpoint_tests.rs
```

These are headless desktop checks, not native-window or physical-print acceptance.
No installed external-provider repair workflow or repair-specific save/reopen
test is claimed. Existing dimension persistence uses the unchanged command path.

## Evidence and limitations

Focused backend tests are in `os-model/src/dimensions.rs`,
`os-document/tests/dimensions.rs` and `os-storage/tests/dimensions.rs`; renderer
coverage is in `os-render/tests/plan.rs`; real-egui pointer, preview, undo/redo,
orphan, save/reopen, Chain and Baseline tests are in
`os-ui/src/plan_workspace/endpoint_tests.rs`. The UI matrix runs at
1280×800/100% and 1000×650/150%. These remain headless checks; native-window
inspection and print-output validation are not claimed.

Angular acceptance tests cover four sectors, exact/near-parallel and same-level
constraints, invalid geometry and orphan recovery; bounded arc segments, crop
and UUID picking; and real-egui authoring at both listed DPI profiles, including
duplicate-wall rejection, preview immutability, Backspace/Escape, one commit and
undo/redo. The frozen `fixtures/schema-11-aligned-dimension.json` proves migration
preserves the previous annotation and all IDs/header metadata; storage tests also
save/reopen an Angular annotation. The schema-10 fixture remains unchanged.

Straight-wall endpoints, associative outer wall faces and native opening jambs
support metric lengths; Angular dimensions report degrees using endpoints only.
Face evidence is in `os-model/tests/dimension_faces.rs`,
`os-storage/tests/dimension_faces.rs` and `os-ui/src/plan_workspace/face_tests.rs`.
The all-feature endpoint suite ran with 64 passed and 3 installed-guest tests
ignored. Curved walls, material-layer faces, linked/plugin anchors, radial dimensions,
driving constraints, text overrides, style controls, and
print-faithful annotation remain unimplemented. Baseline spacing is model-space,
not paper-scale. Dimension lines and tags respect plan crop in the on-screen
view; paper-accurate text/tick sizing and export remain future work.
