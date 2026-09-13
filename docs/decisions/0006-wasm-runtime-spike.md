# ADR 0006: opt-in WebAssembly transport spike

Status: accepted for a developer-only compatibility spike, not milestone B/C release.

The current BUILD_PROMPT prioritizes external plugins before more modeling tools.
Evaluate Wasmi 1.0.9, an interpreter with Rust 1.86 MSRV and MIT/Apache-2.0 metadata,
behind the non-default `os-plugin-host/wasm` feature. Pin the evaluated version.
Use safe host APIs, fuel, bounded linear memory/table/stack/module resources and
an empty linker: no WASI, filesystem, network, clocks, randomness or host imports.
Wasmi 2 is available but is a separate upgrade evaluation; Wasmtime's JIT and
larger platform/dependency surface are not needed for this first transport test.

Use provisional buffer ABI 1: exported memory, version function, allocator and
JSON invocation returning a packed pointer/length. Validate export signatures
before activation. Each call gets a fresh store, one shared fuel budget for start,
version check, allocation and invocation, and checked input/output memory access.
Retain the existing v1 wall-specific JSON protocol and host authorization; do not
mislabel it as the future generic SDK. The built-in desktop path remains unchanged.

Explicit developer loading reads only a chosen manifest and its single adjacent
`.wasm` artifact, checks bounded input and path containment, and uses the same
atomic dependency/registration/grant checks. Projects cannot auto-load plugins.
An independently assembled WAT fixture proves the byte ABI without weakening
authored Rust's unsafe-code prohibition for guest export attributes.

Fuel bounds executed work, not elapsed time. Module input/structure caps are not
a hard process-memory or compiler-time ceiling. No desktop activation until a
worker/cancellation/deadline and stale-result contract exists. Runtime defects
are still host-process risks; this is not an OS process sandbox. Generic commands,
Rust guest SDK, descriptors, manager lifecycle and durable unknown plugin data
remain required before the full external Wall/column acceptance and release gate.

Primary references: [Wasmi package](https://crates.io/crates/wasmi/1.0.9),
[fuel configuration](https://docs.rs/wasmi/1.0.9/wasmi/struct.Config.html).
