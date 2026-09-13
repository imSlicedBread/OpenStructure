# Generic plugin commands — API 2

Status: callable developer service, not desktop authoring or full B/C acceptance.
The independent `os-plugin-api::generic` DTOs work with `default-features = false`.
The host converts validated proposals to internal document commands; guests never
import Model, Command or kernel types. V1 Wall messages and buffer ABI 1 remain.

## Implemented workflow

API-2 manifests activate through a document-free Describe handshake. Responses
echo the host-issued request ID and API version; catalog context must be null.
The catalog lists registered element types with payload schemas/field constraints
and registered create/edit/delete commands with inputs and enabled state. Loading
validates metadata, grants, dependencies and registration collisions before
invoking Describe, and validates the catalog before publishing registrations.

`PluginHost::catalog` returns immutable validated descriptors.
`execute_generic(plugin, document, label, Invocation)` dispatches a registered tool
by ID, not a new wall-specific enum case. `Invocation` supplies inputs, timeout and
host-owned `Scope { read, write, create_count }`. Only explicitly scoped extension
objects and levels are projected into the current snapshot. `selection` identifies
writable objects; other snapshot objects are read-only. Creation uses host-reserved
UUIDs. No whole Model, project name, native property maps or storage handles are sent.

Before guest invocation, the host checks requested/granted read/write/tool
permissions, registration, enabled state, typed inputs, scope membership, owned
write targets, payload versions and exact document plugin requirements. Each reply
must echo project/session/revision/request ID, use an allowed command mode and
owned type, reference only scope objects, and use unique authorized targets.
Payload fields must validate against the type descriptor; retained vendor fields
are allowed. The complete native candidate still undergoes envelope/graph validation.
Creation and the owner's exact requirement record commit in one undoable batch.

Replies carry Edits or a structured error code/message/optional field. Current
host errors present that information as diagnostic text; a structured desktop
validation-message renderer remains to be integrated. Invalid replies never
partially commit. Core migration commands exist separately, but v2 commands reject
schema/version changes that need explicit plugin-owned migration.

## Supported descriptors and limits

Numeric fields have finite min/max/default and metre/radian/scalar units; choices
have bounded unique options/defaults; text has a byte limit; booleans have defaults.
Missing/unknown tool inputs are rejected. Type fields validate known payload keys
while preserving additional payload fields. Disabled commands require explanations.
These are usable data/validation contracts, not yet an egui panel renderer.

Request/response limit 1 MiB; at most 16 edits or reserved creation IDs, 256 scoped
objects, 64 catalog types, 128 commands and 64 fields per descriptor. References
must already be in read scope: creating interdependent new objects in one batch
is not supported yet. Current snapshots support extensions, levels and native
straight walls. [Wall authoring](native-wall-plugin.md) now has an explicit host
projection and transaction adapter. [Generic geometry](generic-plugin-geometry.md) has its own read-only
operation. Plan graphics/snaps, arbitrary panels, schedules, checks,
exports and other registered provider categories are not callable on this route.

The synchronous service is developer-only; do not invoke it from desktop frames.
For Wasm, `start_generic_job(plugin, document, Invocation, optional_view_context)`
now uses the same bounded pool and `poll_job` lifecycle as v1. Preparation freezes
scope, inputs, reserved IDs and correlation on the host. Poll rechecks document,
view and plugin generation, then validates proposals through the same synchronous
validator before one atomic transaction. Cancellation, expiry and stale replies
cannot commit; settled tickets release their retained authority. Callers must
cancel when selection/authorization intent is revoked. See [worker details](plugin-workers.md).
Timeout is result-acceptance expiry, not thread/process termination. Trusted
built-ins cannot start workers; Wasm retains fuel/memory/import limits. Loading,
preparation and commit remain synchronous; desktop integration is still pending.
See [ADR 0011](decisions/0011-generic-command-protocol.md) and the required
[2D interaction/graphics design](plugin-plan-contract-design.md).

## Reproduce and inspect evidence

```powershell
.\tools\cargo.ps1 test -p os-plugin-host --all-features --test generic_commands --locked --offline
.\tools\cargo.ps1 run --manifest-path examples/contract-consumer/Cargo.toml --locked --offline '--' fixtures/generic-column/plugin.toml fixtures/generic-column/catalog.json
.\tools\cargo.ps1 test -p os-plugin-api --no-default-features --locked --offline
.\tools\cargo.ps1 metadata --manifest-path examples/contract-consumer/Cargo.toml --locked --offline --format-version 1 | C:\Python314\python.exe tools/check-contract-dependencies.py
```

The Rust test fixture creates/edits/deletes a typed column through wire messages,
preserves unknown vendor fields, exercises undo/redo, unloads the plugin, then
saves/reopens the native entity unchanged. It tests denied permissions and bad
inputs before invocation; out-of-selection writes despite read access; malformed,
oversized, wrong-schema/owner/type/reference/identity/command-mode/context replies; duplicate
targets; structured errors; expired replies; incompatible requirement versions;
and atomic catalog activation failure. The catalogue tests cover numeric/choice/
text/boolean validation and unregistered or invalid descriptors.

The Wasm test assembles and explicitly loads an external WAT module without host
source changes. It echoes the Describe nonce and uses the reserved creation ID,
then proves creation/history/unload/save/reopen through the same checked host path.
This is a transport fixture with fixed creation values, not an independently
compiled Rust guest, arbitrary geometry implementation or installed desktop tool.
The later [Rust guest verification](rust-guest-verification.md) separately proves
an actual compiled Rust artifact with dynamic create/edit/delete and installed
worker execution. Desktop/geometry and complete Wall/column acceptance remain open.

Six additional generic worker tests cover one-time commit/history/persistence,
single-flight exclusion, edits/undo/reopen/document switches, views/settings,
unload/reload generations, cancellation/drop, malformed/trapping/fuel-exhausted
guests, expired unread replies and invalid dispatch. The legacy worker tests remain
unchanged and pass. The WAT fixture still supplies fixed creation payload/context;
it is not a general-purpose guest implementation.

Observed Windows x64/Rust 1.98.1: full all-feature suite 117 tests passed; contract-only
suite 4 tests passed; separate consumer validates three generic descriptors with
23 package versions/22 names and no OpenStructure host dependencies. Full build,
Clippy, formatting, legacy wall smoke and license metadata checks are recorded with
this slice. Existing production/reference-project gates are not inferred from tests.
No desktop interaction or IFC geometry mapping changed, so their native/external
visual checks were not repeated. No new runtime dependencies were added.

CI repeats standalone v1 and v2 catalog consumption as well as the full test suite.
The working tree remains an untracked initial scaffold: no HEAD, remote, hosted CI
result, publication or license grant. Supported production targets remain undecided;
only the local Windows checks are observed.

Next: (1) generic geometry DTOs with native Wall mapping;
(2) descriptor-driven desktop tools/lifecycle and independently compiled Rust
Wall/column installation, including migrations; (3) D's linked floor plans using
the documented 2D contract, then full E1–E4/G1–G6. L1/L2 remain after E4.
