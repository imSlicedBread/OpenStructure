# Linked wall and world-oriented facade elevations

Model schema 64 adds `ViewKind::Elevation` and strict, versioned
`ViewParams.elevation`. A view stores a native host wall UUID, wall-local
`Left`/`Right` side, and finite, ordered station/project-elevation crop bounds.
Schema 63→64 advances native headers without changing opaque metadata. Legacy
documents containing an elevation field (even null) or kind are rejected as
ambiguous. Native full-model providers now require API 56/schema 75; generic API 2
and container 2 are unchanged.

Select a native straight wall in plan, then choose **Wall elevation → View left
side / View right side**. The saved elevation is available in the named-view
picker. Use **Fit elevation**, drag to pan, and scroll to zoom. **Elevation crop**
edits the persisted horizontal/vertical bounds and viewed side with one Apply.

## Coordinate and host contract

Both sides use station increasing from the wall's current start to its current
end; vertical coordinates are absolute project elevations. Left observes from
the positive left normal of this directed wall; Right observes from its negative
normal. This station convention is explicit even when it differs from a
conventional mirrored architectural elevation.

The UUID link follows translations, rotations, length edits and level changes.
Reversing the endpoints moves station zero to the new start and **flips the
physical viewed face** for an unchanged Left/Right setting. It does not preserve
a world-facing direction. Existing opening offsets remain relative to the
current start. Numeric crop bounds are unchanged, so a height/level change can
move geometry outside an authored crop; update the crop explicitly.

Deleting the host preserves an orphan view and its sheet reference. Undo can
restore it. Derivation reports a missing host instead of showing old geometry.
Changing the host into an arc likewise preserves settings but fails with a
straight-host diagnostic. Creating/editing an elevation requires an existing
supported native straight wall; curved hosts are explicitly unsupported.

## Drawing and editing

The bounded frozen geometry snapshot uses the current native host meshes with
true apertures and the same independent component solids as 3D: door leaves,
paired leaves, window panes/sashes, frames and side-lites, including supported
profiles and poses. Orthographic visibility splits candidate edges at face and
depth crossings, retaining visible boundaries while removing hidden rear edges,
coplanar cell seams and triangulation diagonals. Glazing is opaque for hidden-line
calculation. Other walls do not occlude this single-host assembly.

Visible segments carry entity UUID and surface material. The existing checked
`PlanDrawing` pipeline clips them to the view crop, provides screen selection,
and supplies the same vectors to standalone sheet viewports and PDF output.
Normal derivation runs in the existing bounded native view-worker lane; obsolete
results drain without replacing the current drawing.

Choose an existing Door/Window type, then **Place door** or **Place window**.
The pointer positions the horizontal centre; its project elevation sets a
window's sill relative to the host level. Hover previews a disposable candidate.
A valid click opens the existing exact opening editor, where offset, width,
height and window sill can be reviewed and changed before **Apply opening**.
Select a visible opening and use **Edit opening** for the same exact editor.
Type dimensions can be inherited or overridden using the existing controls.

Preview validates the whole model/host and derives host and component geometry
without modifying document, history or 3D. Apply is one transaction, preserving
UUID, type assignment, materials and untouched instance properties. Invalid,
overlapping or out-of-host candidates cannot apply. Escape, changed document,
view, selection or provider authority cancel/reject drafts. Model revisions also
invalidate drawings after host, level, type and opening edits.

## Direct head and sill handles

Select one visible native door or window on the linked straight wall. A head
handle changes height while preserving sill; windows also have a sill handle
that preserves head elevation. Door bottoms stay level-bound. Handles have a
10-logical-pixel hit radius and 6-pixel visual radius, and use visible portions of
nominal rectangular aperture edges, not posed leaf/sash contours. Dragging keeps
the pointer grab offset and changes only project elevation, not wall station.
Ordinary canvas dragging still pans.

The shared snap preferences govern acquisition of visible nominal opening
heads/sills and host base/top elevations within 12 logical pixels. Enable snapping
and at least one of Endpoints, Midpoints, Nearest or Line axis extensions to use
these references. The edited opening is excluded; cropped or off-canvas
references cannot attract the handle. The candidate project elevation is shown
in metres. These are temporary alignments, not persistent constraints.

Preview replaces the full disposable host/component elevation, including aperture
and hidden-line geometry and dependent dimensions. Model, history and 3D remain
unchanged until a valid release revalidates and commits one `UpdateOpening`.
UUID/header, host, offset, width, type, materials, pose, frames, lites and
accessories are retained. Typed instances pin only changed height/sill dimensions;
legacy instances edit their existing dimensions. Returning to the original
geometry preserves the exact inherited/pinned representation and adds no history.
Invalid or collapsing height, negative sill, host overflow, overlap and component
geometry errors reject the edit without correcting the requested values.

Escape, pointer/focus loss, changed session/revision/selection/view settings,
provider authority, drawing identity, camera/canvas or competing elevation tools
cancel the draft. Pointer ownership persists through release so cancellation
cannot become a pan or selection. A replaced document session intentionally
clears cached view cameras. The existing commit regeneration updates linked views
and 3D; preview does not mutate those caches.

Focused real-egui coverage is in
`crates/os-ui/src/plan_workspace/elevation_resize_tests.rs`: both DPI profiles and
view sides, a rotated/reversed straight host at nonzero elevation, typed/legacy
head and sill edits, grab offsets, full preview/commit vector agreement,
model/history/3D isolation, one-step undo/redo, save/reopen, invalid/no-op release,
Escape/focus/pointer and stale-context cancellation, pan precedence, crop and
snap tolerance, midpoint-only snapping and self-exclusion.

Executed 2026-10-23: the broader `elevation_` filter passed 13/13, and
`plan_workspace::elevations::tests` passed 7/7 (four new resize tests plus three
existing UI tests). See the [coverage ledger](2d-coverage.md#profile-and-current-gate)
for exact validation scope and outstanding lint/environment checks. The new
resize fixtures do not independently qualify every frame/lite/accessory
combination or the installed-provider worker path.

Scope remains a single selected opening with a rectangular aperture in a linked
wall Elevation. Facade/Section drag editing, curved hosts, custom aperture
profiles, multiple-opening edits and persistent alignment
constraints remain outside this increment. These headless egui tests do not
qualify native-window mouse handling or physical printing. Schema 80, storage,
native API 61 and family/package versions are unchanged.

### Move opening

Select one visible native rectangular door/window on a straight host and click
**Move opening**, then drag inside its cropped nominal aperture. Resize handle
hit disks take precedence. The original pointer grab offset is retained. Doors
move along station only; windows move in station and project elevation. Ordinary
canvas drags continue to pan. Release ends the tool; Escape and stale/focus/pointer
contexts cancel, retaining pointer ownership through release.

Movement preserves width, height, identity, type, pose, materials and accessories.
Horizontal movement preserves dimension pins; vertical typed-window movement
pins only sill. Legacy movement updates offset/sill in its existing definition.
Exact no-op restores original parameters without history. Other visible nominal
jamb/head/sill and host edges support the existing snap controls at 12 logical
pixels; self and cropped references are excluded. Invalid overlap, host bounds,
negative sill, clearance locks and component geometry fail without correction.
The full disposable Elevation preview leaves model/history/3D unchanged. Release
revalidates and issues one `UpdateOpening`; undo/redo and reopen retain the result.

Evidence: `elevation_move_tests.rs` exercises both DPI profiles, both view sides,
typed/legacy doors/windows on a rotated/reversed host at a nonzero level,
off-centre grabs, XY/horizontal behavior, explicit and inherited pins, immutable
preview, committed drawing equivalence, undo/redo/save/reopen, no-op, snaps and
disabled/midpoint/tolerance/self/crop cases, overlap/clearance rejection, framed
host overflow, cancellation/stale guards, resize priority and pan. Framed overflow
is not exhaustive family/accessory/component-fit qualification; native-window
input remains unqualified. See the coverage ledger for executed validation.
Executed 2026-10-23 on E: using offline/locked and incremental=false: default-feature
`elevation_` tests **21/21 passed**, all-feature os-ui compile check passed, scoped
rustfmt check passed. No full os-ui suite rerun; the older **547/18/5** result
predates jamb handles as recorded below.
No schema, storage or plugin protocol changes.

### Start and End jamb handles

The same selected opening now exposes visible nominal vertical Start/End jamb
grips: 10 logical pixels for hitting and a 6-pixel visual radius. A jamb grip is
suppressed when its hit disk overlaps any other visible grip. Head/sill behavior
is retained. Horizontal dragging preserves the grab offset and opposite jamb;
Start changes offset and width, End changes width only. Height, sill, identity,
type, materials and pose are preserved. Typed instances pin only changed width;
legacy instances change their width. Returning exactly to the origin retains
the original representation and creates no history entry.

The existing snap controls acquire visible nominal opening jambs and wall ends
within 12 logical pixels, excluding the edited opening. Crop/canvas visibility
still applies. Snapping never relaxes host clearance or overlap rules: touching
another aperture or the wall end rejects the candidate rather than moving it
to a corrected location. Clearance locks and frame/component fit also reject
invalid candidates. Disposable full-elevation preview, one guarded UpdateOpening
on release, cancellation ownership and pan precedence use the head/sill path.

Tests in `elevation_resize_tests.rs` cover both DPI profiles, both view sides on
a rotated/reversed host, typed/legacy doors/windows, both jambs and fixed-edge
preservation, preview/model/history/scene isolation, undo/redo/reopen, no-ops,
snapping/self-exclusion, crop/hit radius/ambiguity, invalid geometry/clearance
locks/frame fit and stale/canceled drags.

Executed 2026-10-23: all-feature `elevation_` filter passed 17/17; exact
`plan_workspace::elevations::tests` passed 11/11; and `cargo check -p os-ui
--all-features` passed. Scoped rustfmt checks passed for the four elevation
interaction source/test files. An earlier serial all-feature `os-ui` suite,
before adding jamb handles, ran 570 tests: 547 passed, 18 failed and 5 were
ignored. Failures spanned plan-opening, annotation, grid and canvas tests; the
clean-checkout baseline is not established, and the full suite was not rerun
after the jamb extension. Clippy with warnings denied stopped in `os-model` at
`manual_flatten` in `crates/os-model/src/opening_family.rs:99`, before linting
`os-ui`.

## Evidence and remaining limits

### Associative dimensions (schema 65)

Choose **Horizontal dimension** or **Vertical dimension**, pick two visible
nominal wall/opening extents, then click the offset. Horizontal references are
wall Start/End or opening start/end jamb; vertical references are wall Bottom/Top
(base/top) or opening Bottom/Top (sill/head). The hit radius is 12 logical pixels.
Only visible collinear portions inside the crop can be acquired. A custom profile
does not turn its arbitrary contour into a reference. Both refs belong to the
view's linked host; wrong-axis, duplicate and coincident references cannot commit.

The preview is transient, with panning suppressed during acquisition. Escape,
view/document/settings/provider changes cancel it. Commit, property-panel offset
Apply, reference replacement and deletion each use one undoable transaction.
Properties support **Replace anchor 1/2**; missing references remain repairable
without discarding UUID or opaque metadata. Deleted openings render a lost-
reference marker. A missing/curved host still produces the whole-view diagnostic
described above; annotations remain stored and inspectable in properties.

Measurements derive from effective opening size/overrides and the host's current
station/project-elevation frame. They do not drive geometry and never use Plan
XY distance. The viewport and sheet/PDF share the same derived dimension item;
no measured value, tessellation or 3D mutation is persisted.

Evidence: `os-model/tests/elevation_dimensions.rs`, document/storage elevation
tests (including frozen schema-64 migration), and
`os-ui/src/plan_workspace/elevation_tests.rs` cover this bounded workflow.

Model, document and storage acceptance lives in each crate's `tests/elevations.rs`;
the frozen schema-63 fixture covers strict migration. Geometry visibility,
profiles, material attribution, both sides, rotated/reversed hosts and component
poses are covered by `os-geometry/src/elevation_tests.rs`.
`os-ui/src/opening_elevation_tests.rs` covers draft placement/editing, history,
save/reopen, stale inputs, crop/picking and exact sheet/PDF vector parity.
`os-ui/src/plan_workspace/elevation_tests.rs` exercises real egui frames at
1280×800/100% and 1000×650/150%. Execution results are recorded in the task report.

## World-oriented multi-wall facades (schema 66)

`ViewKind::Facade` is distinct from the wall-local `Elevation` above. Choose
**Place facade** in the named-view picker while a plan is active, pick two baseline
points, review viewer side, station
and absolute-elevation crop, and near/far depth, then **Apply facade**. One AddView
transaction saves version-1 settings and creates the linked plan marker, whose
identity is the view UUID. **Facade settings** applies edits with one UpdateView.
The persisted origin and horizontal unit axis are world-oriented: reversing or
moving a wall never changes them. Left observes from the axis's positive left
normal; Right observes from the other side. Positive depth runs away from the
baseline into the scene; finite ordered bounds require `0 <= near < far`.

The marker's associated level scopes the building, not just that level. All
levels in that building contribute supported straight walls, hosted doors and
windows, floor slabs and single-plane roofs. Their frozen native meshes enter
one shared hidden-line solve, so foreground objects occlude other walls and
opening components. UUID and material attribution survive projection and
selection. Crop and depth constrain both original candidate edges and occluders;
clipping does not invent cut edges. The existing single-host Elevation coordinate,
opening-edit and associative-dimension contracts are unchanged; those authoring
tools are not added to Facade views.

Baseline and settings drafts do not mutate model, history or 3D. Baseline preview
shows the direction and volume in plan; settings preview shows the crop frame
over the existing drawing, not a live hidden-line recomputation. Apply regenerates
through the existing bounded native worker. Escape, pointer loss, focus loss,
view/document/settings/provider changes discard drafts; stale worker results
drain without publication. Panning is suppressed during these drafts.

Facade views participate in linked multi-view sheets, source navigation and the
shared vector PDF path. The sheet layout workflow supports up to 64 source views
and one schedule; it is not limited to one or two viewports.

Evidence: `os-model/tests/facades.rs`, `os-document/tests/facades.rs` and
`os-storage/tests/facades.rs` cover validation, atomic history and migration/save
round trips. `os-geometry/src/facade/tests.rs` covers shared wall/opening/floor/roof
occlusion, depth/crop, rotated world coordinates, reversed walls, materials,
multiple storeys and budget rejection. `os-ui/src/plan_workspace/facades/tests.rs`
exercises real egui frames at 1280×800/100% and 1000×650/150%, baseline and settings
drafts, history, marker identity, wall selection, stale worker cancellation,
save/reopen and sheet/PDF parity. Numeric test values are assigned to drafts;
these tests do not qualify physical keyboard entry or native-window printing.

Limits remain explicit: curved walls and unsupported native 3D categories in the
selected building fail the view, even if they might lie outside its crop, because
their spatial support is not certified here. Generic extensions likewise fail
closed. Conservative mesh preflight and the shared 4096-triangle, 4096-output-
segment and bounded-work limits can reject complex views. General room elevation
automation, arbitrary orientations, elevation chains/angular dimensions/tags,
material fills, Facade opening drag handles and manual native-window/physical
print qualification remain open. This is bounded facade support, not complete
building-category coverage or Revit parity.
