# ADR 0012: isolate Rust guest export attributes

The independently resolved `examples/rust-column` builds a `cdylib` for
`wasm32-unknown-unknown` against only contract DTOs. It does not link the host,
model, document, kernel, UUID runtime, Wasmi or GUI. This advances the independent
artifact part of B, not the Wall/column desktop/geometry installation gate.

Rust 2024 requires unsafe qualification on `no_mangle`. Keep the workspace's
`unsafe_code = forbid` untouched. The separate guest has `deny(unsafe_code)`;
only `src/abi.rs` permits its three Wasm-only export attributes. Semantic code
uses `forbid`. No raw-pointer dereference or unsafe block is needed: the adapter
owns input/output byte vectors, compares supplied addresses/lengths with its
allocation, and dispatches through its owned slice. Exports are not present in
native builds; native tests exercise the buffer state without exported names.

The host must write only the returned input allocation, between guest calls.
One caller invokes each fresh module instance. Successful allocation invalidates
the old buffer; invocation consumes readiness once. Output stays owned until
another allocation or instance disposal. Invalid allocation, pointer, length or
unparseable JSON returns zero, which the host rejects. Memory and fuel limits
remain enforced by the host. This does not make arbitrary host writes into guest
linear memory safe, nor isolate interpreter defects from the host process.

Protocol remains API 2, buffer ABI 1. No persistence version changes. The example
creates/edits/deletes preserved column envelopes and returns its own descriptors.
It is not a general-purpose SDK export macro, geometry provider, migration service,
descriptor-rendered desktop tool or the prerequisite installed Wall acceptance.

Primary references: [Rust 2024 unsafe attributes](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-attributes.html)
and [the Rust Wasm target](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html).

