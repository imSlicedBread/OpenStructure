# Native Wall mapping and independent Rust guest

The generic API-2 service now maps the reserved owner/type
`org.openstructure.walls` / `org.openstructure.walls.wall` to native Wall document
commands. It does not store a second opaque wall model. Other extension types
retain their envelope path. Adding future generic extensions still requires no new
wall-specific UI enum; this adapter bridges the existing built-in semantic type.

## Boundary and preservation

The independent `wall::Parameters` DTO contains start/end X/Y, thickness, height
and canonical level UUID. Its payload schema is 1, distinct from model schema 2.
The generic element carries identity/name and exactly the level prerequisite.
Native header properties, relationships and material assignment are not disclosed
by this projection. Updates use `UpdateWall`, preserving that undisclosed data;
new walls receive normal native headers and no material. Unknown payload fields,
proposed header relationships, inconsistent level prerequisites and invalid native
dimensions fail. This route does not edit material assignments or header metadata.

Native writes require the exact registered owner/type, explicit write selection,
correct payload version, requested/granted permissions and a validated proposal.
Native read access does not confer another plugin's write authority. Creation uses
host-reserved IDs; all proposals map to existing native transactions and whole-model
validation. Geometry projects scoped native walls and levels through the same
read-only worker/recipe validation path. Old native walls without requirement records
can be read; an existing exact requirement is enforced. First generic edits record
the loaded version atomically. No external plugin executes merely because a file
mentions it. Existing opaque envelopes are not silently migrated into native walls.

## Reproduction and observed checks

Follow [the standalone Wall example](../examples/rust-wall/README.md). Locally the
host `native_wall_probe` executable was built first; the separate Wasm artifact and
manifest were then copied into `outputs/rust-wall-install`. The prebuilt probe
passed creation/edit/deletion of native walls, rotated geometry (3 m³ and known
corner coordinates), stable identity, undo/redo, invalid-edit atomicity, unload,
save/reopen and core native editing with no external guest loaded.

Verification on Windows x64 / Rust 1.98.1: 127 all-feature workspace tests and two
independent Wall guest tests pass. Added tests verify hidden metadata/material
preservation, strict native proposal rejection, and denial of another owner's
attempt to edit a native wall before invocation. Native source schema/container
versions remain unchanged; the API-2 projection is new and strict older guests
cannot interpret it as their own payload schema. V1 Wall messages remain supported.

Workspace/native-Wall/Wasm-target Clippy, formatting and desktop build pass. The
native smoke `outputs/native-wall-plugin-verified.osb` passes. License metadata
checks cover 347 workspace packages and 23 standalone guest package versions
(22 distinct names); the independent dependency-boundary check passes. No package
versions were added to the host lockfile. The new guest lockfile is retained.

The Wall guest uses the existing isolated Wasm export adapter and imports no host,
model, kernel or GUI crates. Same bounded interpreter, no WASI/host imports, no new
runtime dependencies. All authored host/model code retains unsafe prohibition.
The adapter checks grant and semantic consistency, not correctness of arbitrary
third-party architectural modeling or interpreter crash isolation.

## Still required

1. Descriptor-driven desktop create/edit panels and regeneration using these
   native/extension worker paths, with safe plugin-manager load/grant/unload states.
2. Explicit plugin migration workflow and full installed Wall then column desktop
   acceptance, including offline persistence and failure recovery, to close B/C.
3. D's linked floor plans and installed 2D provider, then complete E1–E4 and G1–G6.
   L1/L2 remain later follow-ons.

There is no plan UI/provider, curve/join/opening support or desktop plugin manager
in this slice. Native IFC representation is unchanged; unsupported extension
geometry still blocks export. No production deployment/project/concurrency/output
profile is approved. License choice remains pending. The repository is still an
untracked initial scaffold: no HEAD, remote, publication or hosted CI result.
CI adds independent Wall build/installation/probe checks; only local Windows
execution is observed, not Linux/macOS or a clean-workstation deployment.
