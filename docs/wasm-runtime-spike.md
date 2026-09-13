# Wasm runtime compatibility slice (B1)

Later API-2 behavior: explicit loading runs a document-free, fuel-limited Describe
handshake after manifest/grant/dependency/collision checks and before publishing
registrations. Generic commands use the same buffer ABI 1; see
[generic command verification](generic-plugin-commands.md). The v1 probe below is unchanged.

This is the first bounded step toward BUILD_PROMPT milestone B, not completion
of B or C. The desktop still uses trusted bundled Walls. No model, container,
IFC or v1 JSON protocol schema changes. New API: opt-in `os-plugin-host/wasm`,
`wasm::WasmPlugin` and `PluginHost::load_wasm_directory`.

## What works

An explicitly chosen directory containing `plugin.toml` and an adjacent `.wasm`
binary can be loaded without changing/recompiling host source. The manifest uses
`api_version = 1` and `entrypoint = "wasm:probe.wasm"`. Dependencies, grants and
registration ownership use the existing host checks. Failed loading publishes
no partial registration. No project file installs or executes a plugin.

Wasm JSON replies go through the same operation-kind, API-version, command-count,
type-ownership and document-validation checks as bundled replies. Tests prove a
supplied AddWall reply commits, undoes/redoes, saves and reopens; malformed replies,
denied writes and invalid batches preserve the model, revision, events and history.
This is transport evidence, **not** an independently authored parametric Wall SDK.

## Provisional buffer ABI 1

| Export | Signature and ownership |
| --- | --- |
| `memory` | One 32-bit linear memory; host never dereferences guest pointers |
| `os_abi_version` | `() -> i32`, must return 1 before request bytes are copied |
| `os_alloc` | `(length: i32) -> i32`, returns guest-owned writable input offset |
| `os_invoke` | `(input_offset: i32, input_length: i32) -> i64`, returns `(output_length << 32) \| output_offset` |

All pointer/length bit patterns are unsigned. Output must be nonempty UTF-8 JSON.
The host checks the whole range before allocating/copying output. Each invocation
uses a fresh store, including a fresh start function; all memory is reclaimed at
return/failure, so there is no deallocator export or persistent guest state.
Export signatures are checked at load without executing the module. The ABI
version **value** is checked on invocation under fuel, not during registration.
`WasmPlugin::invoke_json` is a transport primitive; application callers must use
`PluginHost::request/execute` for permissions and JSON/document authorization.

## Resource and trust boundaries

| Resource | Enforced bound |
| --- | --- |
| Manifest / binary input | 64 KiB / 4 MiB; binary Wasm only, not WAT |
| Request / reply | 1 MiB each (stricter than the existing 64 MiB JSON protocol) |
| Guest linear memory | One memory, 16 MiB maximum, including initial allocation |
| Tables / instances | One table, 4,096 entries; one instance per call |
| Execution | 1,000,000 fuel units shared by start, ABI check, allocation and invocation |
| Stack | 128 recursive calls; 64 KiB value stack; no cached stacks |
| Module structure | Wasmi `EnforcedLimits::strict()`; including 10,000 functions, 1,000 globals, one memory and bounded segments/parameters |
| Imports | None. No WASI, network, filesystem, processes, clock, randomness or secrets |

Growth above limits traps. Guest traps, fuel exhaustion, bad offsets, malformed
modules, unsupported ABI and invalid UTF-8 return errors. Authored Rust retains
`unsafe_code = "forbid"`; the third-party interpreter has its own implementation.
An interpreter bug remains a host-process risk. This is not OS process isolation.

Fuel is a work limit, **not a wall-clock deadline**. Compilation is not fuel-metered;
input/structure caps do not guarantee a hard compiler-time/process-memory ceiling.
The adapter is synchronous and must not run in desktop frames. Host serialization
still clones the authorized whole model before applying the smaller guest limit.
Install directories must not be concurrently writable by an attacker: canonical
containment checks reject path escapes but are not a race-proof filesystem sandbox.

## Reproduce the independent artifact load

Run from the repository root, with a new output directory and existing parent:

```powershell
.\tools\cargo.ps1 build -p os-plugin-host --features wasm --example wasm_probe --locked --offline
.\target\debug\examples\wasm_probe.exe --assemble outputs\my-wasm-probe
.\target\debug\examples\wasm_probe.exe outputs\my-wasm-probe
```

On other systems use `cargo` and `target/debug/examples/wasm_probe` without `.exe`.
Assembly is a separate process from loading. The loader reads only the installed
binary/manifest; it neither assembles WAT nor rebuilds the host. The included WAT
fixture returns a fixed unit prism, not geometry derived from a requested wall.
It intentionally registers no desktop tool. Real guest Rust SDK/export wrappers,
descriptor-driven panels and external Wall/column workflows are still pending.
The WAT fixture avoids weakening the Rust unsafe lint for exported symbol attributes.

## Observed verification

Windows x64, Rust 1.98.1/MSVC, untracked working-tree scaffold (no slice commit or
configured remote). Baseline: 52 passing tests. New suite: 64 including 12 Wasm
tests; no failures. Checks:

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs\wasm-slice-verified.osb
.\tools\cargo.ps1 metadata --all-features --locked --offline --format-version 1 | C:\Python314\python.exe tools/check-license-metadata.py
```

All checks above passed. Clippy emitted the pre-existing Windows incremental-cache
access-denied note for `os-geometry`, not a source warning; exit status was zero.
The refreshed RustSec audit scanned 347 dependencies and reported no known
vulnerabilities, with the existing unsuppressed unmaintained `ttf-parser 0.25.1`
warning (RUSTSEC-2026-0192). `cargo audit` is not installed on the wrapper PATH;
the existing `work/toolchain/audit/bin/cargo-audit.exe` was used with local Cargo
on PATH. License metadata is not a legal approval or a security guarantee.

Tests include loops in all four execution phases, memory/table growth and initial
allocation limits, recursion, version/signature/import refusals, UTF-8, exact
message limits, bad pointer ranges, state reset, registration atomicity and a
deterministic truncated/mutated binary corpus. This corpus is a regression smoke
check, not a sustained coverage-guided fuzz campaign.

Independent local install: `outputs/wasm-probe-installation`, loaded by the already
built executable in 15.98 ms including file read/compile/invoke/JSON validation.
This is one tiny-fixture observation, not a latency budget or large-model benchmark.
No changed desktop interaction, so native UI inspection was not repeated.
Windows/Linux CI retains existing gates and adds fixture assembly/loading; hosted
CI and local Linux/macOS execution have not been verified.

## Dependency evaluation and remaining gate

Wasmi is pinned at 1.0.9, using `std` and `extra-checks`, no runtime WAT parser.
Its metadata declares Rust 1.86 and MIT/Apache-2.0. Wasmi companion crates resolve
to 1.1.0 in Cargo.lock; WAT 1.240.0 is a development dependency. The lockfile adds
14 packages; all-feature license metadata passes for 347 packages (333 baseline).
The normal desktop runtime does not enable Wasmi. Project licensing/legal review
and publication remain pending. See [ADR 0006](decisions/0006-wasm-runtime-spike.md).

Next three tasks, in order:

1. Versioned generic request/response SDK, structured errors and revision IDs,
   worker/cancellation/deadline handling with stale-result rejection.
2. Durable extension envelopes, plugin dependencies and missing-plugin/unknown
   data preservation with migration fixtures, before enabling new element types.
3. Separately built Rust Wall then column plugins, descriptor-driven tools/panels
   and enable/disable/error lifecycle UI; complete B/C acceptance before openings.
