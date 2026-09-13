# ADR 0011: callable generic commands with host-owned object scope

API 2 lives in the independent `os-plugin-api::generic` module, with no core,
document, model or geometry types. Keep the v1 Wall wire and buffer ABI 1 intact;
manifests select API 1 or 2, never silently reinterpret one as the other.

After metadata/grant/dependency/collision checks, API-2 activation invokes a
document-free Describe handshake. Validate the correlated version/request ID,
catalog registration ownership, type schemas and field descriptors before any
registration is published. API-2 Wasm Describe runs under existing fuel/memory
limits. A malformed/trapping catalog fails activation atomically.

Invoke a registered command with host-issued request ID, project/session/revision,
explicit selected writable IDs, scoped extension/level snapshots, typed inputs and
reserved creation UUIDs. The scope is trusted application input, not guest output.
Do not equate type ownership or snapshot read access with blanket write access.
Validate permissions, command state, inputs, owned selection and exact required
plugin version before invocation. Validate response correlation, command mode,
ownership, reserved IDs, references, payload fields/schema and unique targets
before converting to document commands. The existing transaction validates the
final graph and atomically records the owner's exact plugin version on creation.

Descriptors support finite numeric limits/units, choices, text limits, booleans,
defaults and static enabled/disabled explanations. Only registered extension
create/edit/delete tools are callable in this first route. Other manifest
categories remain unavailable; this is not yet a property-panel renderer or a
geometry/2D provider. Generic payload migrations and native Wall mapping remain.

Cap request/response bytes at 1 MiB, edits/new IDs at 16, scope at 256 objects,
catalog types at 64, commands at 128 and fields per descriptor at 64. Numeric wire
units are metres/radians/scalars, not locale-formatted input. Unknown payload
fields can be retained; defined fields must validate. Native envelope validation
still applies, including references, prerequisites and size/depth bounds.

This synchronous developer service must not be called by desktop frames. Its
timeout revokes acceptance, not OS execution; trusted built-ins retain their trust
limitations and Wasm retains fuel/memory limits. The follow-up worker integration
shares host-only preparation and reply validation with `start_generic_job` and
the existing `poll_job` lifecycle. Legacy `start_job` still accepts only v1;
both routes share bounded permits, session/view/cancellation checks and plugin
generation rejection. Real Wasm worker tests now cover asynchronous stale replies.
Loading/preparation/commit and desktop activation remain separate work.

Verification includes a protocol-driven Rust test plugin and a separately loaded
WAT fixture performing Describe and creation with echoed nonce/reserved identity.
Neither substitutes for independently compiled Rust Wall/column SDK installation,
descriptor-driven desktop authoring, geometry or required 2D acceptance.
