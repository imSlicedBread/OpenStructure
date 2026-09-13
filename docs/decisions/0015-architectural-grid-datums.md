# ADR 0015: persistent architectural straight grids

Status: accepted for the D development milestone.

## Decision

Native model schema 4 adds `grids: BTreeMap<Id, Grid>`. A `core.grid` entity
uses the common stable UUID/header and typed `GridParams`: name, building UUID,
and directed start/end points in model XY metres. The axis defines a vertical
datum plane; the endpoints delimit its finite model extent. It is not a wall,
level, viewport lattice or view-owned drafting line. Curves and per-view datum
extents are not encoded by overloading these endpoints.

Names must be trimmed, nonempty, at most 256 UTF-8 bytes and free of control
characters. Exact, case-sensitive names are unique within a building; names
are labels, not identities. Building references must resolve. Coordinates and
extent length must be finite, with length greater than one micrometre. Unsupported
parameter/point fields are rejected, including z coordinates.

AddGrid/UpdateGrid/RemoveGrid run in the existing whole-candidate transaction and
bounded history service. Update preserves identity and metadata. A referenced
grid cannot be removed unless its references are also removed in that batch;
there is no silent cascade or geometric reattachment. These are reference rules,
not a geometric constraint solver. Grid changes conservatively invalidate plans.
Snapshot accounting explicitly includes grid entities and owned names/metadata.

Migration 3→4 adds an empty grid map and upgrades native headers, preserving all
other data. Older schemas chain through it. An old file already containing grids
or conflicting native headers is rejected atomically. No grid positions or names
are inferred. Opening never rewrites the old archive. The frozen schema-3 fixture
was captured from the actual native pointer-wall output before the schema change.

## Consequences and remaining work

Container 2, plan-settings 1, extension-envelope 1, API 2 and Wasm ABI 1 remain
unchanged. Legacy native Wall messages use current native header version 4;
independent API-2 Wall projection remains payload 1. Grid plugin authoring is not
implicitly granted by adding host commands. IFC wall export reports grid loss
and the loss-rejecting adapter refuses it without acknowledgement.

This establishes the persistent model and command boundary. Native grid authoring,
selection, datum graphics, grid/axis/intersection snapping, view-specific extents
and linked-datum monitoring still require implementation. No D or production
gate is closed by this foundation.
