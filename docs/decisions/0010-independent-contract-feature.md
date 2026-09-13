# ADR 0010: isolate the public contract from the legacy Wall API

The existing `os-plugin-api` imports `Document::Command`, `Model` and kernel
recipes, so an independent plugin cannot consume even metadata without linking
the host layers. Do not carry this coupling into the generic protocol.

Keep one public crate. Make the current wall-specific messages and built-in Rust
trait a default-enabled `legacy-v1` feature with optional host dependencies.
Contract-only consumers disable default features; manifests, permissions and
registrations are available without any other OpenStructure crate. Use a local
`ProtocolError` and convert it at the legacy host boundary. Existing default
consumers retain wire/API version 1 and their Rust names; manifest validation's
concrete error type changes, with an automatic conversion to `os_core::Error`.

Future generic DTOs, descriptors and the guest ABI helpers belong in the
contract-only surface and must not depend on legacy types. Their callable host
adapters must authorize edits and convert to internal commands. Do not enable
API version 2 or claim a generic tool exists just because its DTOs are planned.

Verify a separately resolved Cargo consumer with `default-features = false` and
assert its dependency graph contains no core/model/document/constraints/geometry/
render/UI/host packages. Workspace feature unification alone cannot prove this.
The first independent consumer validates an actual manifest; it is a dependency
boundary test, not the required independently installed Wall/column example.
