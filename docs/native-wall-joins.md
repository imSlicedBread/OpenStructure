# Native straight-wall joins

Native straight rectangular walls support explicit butt, perpendicular corner,
and perpendicular T joins. Persisted `core.wall_join` entities store stable wall
UUIDs and a tagged `Butt`, `Corner`, or `Tee` relationship; contact alone never
creates connectivity. The selected-wall **Wall end joins** inspector exposes
compatible join/unjoin actions. Corner joins carry an owner wall. Tee joins
store a host UUID and a station measured from the host's authored start, plus
the branch endpoint.

All joined members must have exactly matching level, height and thickness.
Butts require exact collinear opposing endpoints. Corners and tees require
exactly perpendicular axes and exact centerline contact; corner ownership
extends the owner half a thickness past the node and trims the other wall back
by half a thickness. A tee keeps its host continuous and trims the branch to
the host face. This permits a clear four-corner loop while rejecting crosses,
oblique or ambiguous third-wall contacts, competing anchors, tiny residual
pieces, and incompatible profiles. Openings must clear the derived trims.

One model-aware geometry derivation supplies adjusted material spans, meshes,
plan/section interfaces, opening clearances, per-wall picking and net quantities.
Contact faces/edge intervals are removed from both members where appropriate;
individual wall IDs, authored axes, opening ownership and material cells remain
independent. Plan crop clips the visible contact intervals. Updates that would
break a join fail atomically; moving a connected node requires updating all
affected wall endpoints in one document transaction. IFC export still refuses
joined models because this writer cannot faithfully preserve the join topology.

## Connected junction drag

A selected visible native wall shows a diamond grip at a Butt, Corner,
or Tee junction (6 logical-pixel visual radius, 10 logical-pixel hit radius).
Both immediate members must be visible in the checked drawing, and all walls
in the reachable join component must be native. The active join is identified
by endpoint anchor or Tee join UUID. Additional joins and clear cycles are
supported. Unconnected endpoints on joined walls show unavailable grips and
cannot fall through to single-wall resize. Hidden or
cropped-out grips do not acquire.

- Butt: dragging projects onto the shared axis and moves both joined endpoints
  to the same exact node. Both far endpoints stay fixed.
- Corner: either member's joined endpoint can be selected. Dragging projects onto
  that wall's original axis, keeps its far endpoint fixed, and translates the
  perpendicular peer. Both joined endpoints receive the same exact node; the
  corner owner remains unchanged.
- Tee: drag the branch contact endpoint or the separate interior station diamond
  on the selected host. The host stays fixed; the branch translates along the
  host axis. Its contact comes from `wall_point(host, station, 0)` and the existing
  join keeps its UUID with the updated station. Endpoint hit regions take
  precedence over the interior station grip when they overlap.

These rules also apply to reversed authored wall directions.

Acquisition freezes the reachable walls, joins and model. Endpoint displacements
are solved simultaneously: each wall keeps equal perpendicular displacement at
both ends, Butt/Corner endpoints share a displacement, and Tee branch contacts
stay on their host lines. The active gesture supplies fixed endpoint and rigid
peer constraints. Downstream walls may translate or resize, including a closing
side of a rectangular loop. Other Tee stations are derived from the solved
contact and candidate host start; each branch contact is assigned the exact
`wall_point` result. Walls outside the component remain unchanged.

For remaining freedom, the solver minimizes the sum of squared Cartesian
endpoint displacements with equal endpoint weights. Reorthogonalized constraint
rows use UUID-sorted walls/joins and fixed coordinate order; dependent rows
check cycle consistency. Original equations are checked again after solving.
This is a dense solve and interactive editing is limited to 64 walls per
connected component to bound frame-time and memory cost. Performance at that
limit has not been visually qualified. Larger components show an unavailable
grip and are rejected without mutation. Ill-conditioned numerical results are
rejected atomically.

The accent preview is rebuilt from a frozen model snapshot. It changes no live
model, history, checked plan drawing, or 3D scene. Semantic snapping excludes
the entire affected graph. On-axis filtering happens before candidate ranking,
so a closer off-axis point cannot mask a legal snap. The checked snap result
includes the exclusion set and acquisition axis in its identity.
A valid release submits the affected `UpdateWall` commands, any required
`UpdateOpening` offsets, and changed Tee `UpdateWallJoin` commands in one document transaction.
Openings on a resized wall retain their world positions when its authored start
moves along its axis; any perpendicular line translation carries them with it.
Openings on rigidly translated walls retain their local
offsets and travel with that wall; Tee host openings remain fixed. IDs, wall types,
profiles, materials and other parameters remain intact. One undo/redo covers
the entire edit; 3D regenerates after commit.

Invalid Tee stations, collapsed or insufficiently trimmed walls, opening clearance failures, competing third-wall contacts and
other invalid candidates cancel without a partial edit. Escape, pointer loss
or leaving the canvas, selection/view/settings changes, document session or
revision changes, provider activation changes, and checked drawing replacement
cancel the draft. Pointer ownership lasts until release so cancellation cannot
turn the drag into a pan. Ordinary body dragging still pans/selects.

Installed Wall providers show an unavailable explanation: their bounded worker
currently supports one wall edit, not this atomic batch. No partial worker job
is submitted. There are no schema, storage or plugin protocol changes.

Focused evidence: `crates/os-ui/src/plan_workspace/junction_tests.rs` runs real
headless egui frames at 1280×800/100% and 1000×650/150%, covering the Butt regression,
both Corner owners and selected members, reversed axes, both Tee grip roles,
exact contact/station, peer translation, preview isolation, identity and opening
world positions, one-step undo/redo, invalid clearance/short walls/stations/third
contacts, hidden/cropped handles, hit radius, endpoint/station precedence, pan
precedence, and cancellation/stale contexts for isolated and connected grips.
Graph tests cover a resized-start Butt chain with an opening and hosted Tee,
Corner/Tee propagation into a Butt chain, a four-Corner loop with both owners
and reversed directions, and a Tee path returning to a second station on the
fixed host. They assert preview isolation, exact contacts, metadata preservation,
unaffected walls and one-step undo/redo. A reversed/collapsed loop target is
rejected without model/history changes; a separate equation-level test rejects
inconsistent cyclic constraints and checks minimum motion. Snap tests cover
downstream exclusion, legal candidate ranking and checked axis identity.
Missing-provider acquisition also rejects without work.
The separate
installed-provider acquisition/no-pending-job test is opt-in with
`OPENSTRUCTURE_WALL_TEST_PLUGIN`; it was not run here because that fixture is
unset. These are headless UI checks, not native-window visual qualification.

Graph-drag validation: focused junction tests **17 passed, 1 ignored**; the
checked snap-axis regression passed. Full `os-ui` and `os-render` all-feature
tests passed with `--locked --offline`, as did scoped all-target/all-feature
Clippy with warnings denied, workspace formatting checks and diff checks.

Schema 21 adds tagged `wall_joins`. Migration 20→21 preserves every explicit
butt-join UUID/anchor as a tagged butt and rejects ambiguous payloads; earlier
schemas do not infer joins from coincident geometry. Native API-1 Wall guests
must be rebuilt and reinstalled for API 3/schema 21; generic API-2 guests use
their separate DTO contract. See [file-format.md](file-format.md).

Evidence is in `crates/os-ui/tests/wall_joins.rs`,
`crates/os-ui/tests/perpendicular_joins.rs`,
`crates/os-ui/src/desktop_tests/wall_join_tests.rs`,
`crates/os-storage/tests/wall_joins.rs`,
`crates/os-plugin-host/tests/wasm_transport.rs`, and the Wall provider tests in
`plugins/walls/src/lib.rs`. Curves, layered/variable-profile walls, arbitrary
angled junctions, installed-provider
junction drag, joined IFC exchange, and native
window/print qualification remain open; this is not full Revit-like join
behavior.
