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
angled junctions, connected-node drag handles, joined IFC exchange, and native
window/print qualification remain open; this is not full Revit-like join
behavior.
