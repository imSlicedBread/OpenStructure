# Independent Rust Wasm guest verification

Historical semantic-artifact slice. The subsequent [geometry slice](generic-plugin-geometry.md)
adds callable prism generation and supersedes the geometry-unavailable statements below.

Progress within B/C, not acceptance of the full installation workflow or production
release. `examples/rust-column` now builds an actual Rust Wasm artifact against
the contract-only API. No host/model/document/kernel types are linked. The host
probe was built first; the later artifact and manifest were copied to a new
installation directory and invoked without rebuilding that executable.

## Reproduce

See [example setup and installation](../examples/rust-column/README.md).
Local verification used the workspace's Rust 1.98.1 Windows x64/MSVC toolchain
and its newly installed official `wasm32-unknown-unknown` standard library.

```powershell
.\tools\cargo.ps1 test --manifest-path examples/rust-column/Cargo.toml --locked --offline
.\tools\cargo.ps1 build -p os-plugin-host --features wasm --example generic_probe --locked --offline
.\tools\cargo.ps1 build --manifest-path examples/rust-column/Cargo.toml --target wasm32-unknown-unknown --release --locked --offline
# Copy into a new directory as shown in the example guide, then:
.\target\debug\examples\generic_probe.exe outputs/rust-column-install
.\tools\cargo.ps1 metadata --manifest-path examples/rust-column/Cargo.toml --locked --offline --format-version 1 | C:\Python314\python.exe tools/check-contract-dependencies.py openstructure-rust-column
.\tools\cargo.ps1 metadata --manifest-path examples/rust-column/Cargo.toml --locked --offline --format-version 1 | C:\Python314\python.exe tools/check-license-metadata.py
```

Observed: four native guest tests pass, including buffer ownership/length bounds,
single-use invocation, descriptor validation, semantic lifecycle, preservation of
unknown payload fields and invalid selection. Installed Wasm probe passes Describe,
create/edit/delete, stable identity, undo/redo, denied grants, cancellation, stale
revision rejection, unload and actual native save/reopen. Each call uses dynamic
request/context/reserved identities, unlike the fixed WAT transport fixture.
No Wasmi memory/fuel/stack/structural limits were increased to run this example.

Full workspace: 117 all-feature tests pass, including existing malicious-message,
trap, fuel, deadline and document/view-switch tests. Workspace and native guest
Clippy/formatting pass. Independent dependency boundary and license-metadata
checks pass for 23 package versions (22 names); publishing remains disabled.
Wasm-target release Clippy also passes. The optimized artifact is 291,631 bytes.
The desktop build and `--smoke outputs/rust-guest-verified.osb` pass; the independent
API tests remain four passing tests and workspace license metadata covers 347 packages.
The guest's new lockfile uses existing cached package versions, not new host
dependencies. No native desktop interactions, geometry or IFC mappings changed;
their visual/independent-IFC checks were not rerun.

## Compatibility and remaining work

No API-2 wire, buffer ABI-1, model-schema-2 or container-2 changes. Workspace
unsafe prohibition stays untouched; only the independent guest adapter permits
three wasm-only export attributes. See [ADR 0012](decisions/0012-rust-wasm-guest-adapter.md).
The artifact has no host imports or WASI access. Runtime interpreter defects still
affect the host process; cancellation revokes acceptance rather than terminating
OS threads. Loading, snapshot preparation and commit still need desktop integration.

This example does not generate geometry, edit native Walls, provide plan graphics,
render property panels, migrate payloads or automatically install itself. It is
not the required installed Wall-first acceptance followed by a column desktop
workflow. Opaque envelopes remain inspectable/preserved but geometrically unavailable
in the desktop; unsupported plugin geometry still blocks IFC output.

Next, in dependency order:

1. Generic geometry and native Wall mapping, shared by an independently built Wall
   guest and the column; validate geometry without leaking kernel structs.
2. Descriptor-driven desktop tools, plugin manager lifecycle and transactional
   migrations; pass full installed Wall then column B/C acceptance.
3. D's linked floor plans/independent 2D provider, then all E1–E4 and G1–G6.
   L1/L2 remain later milestones, not a substitute for any production requirement.

The whole source remains an untracked initial scaffold with no HEAD or remote;
no commit, publication or hosted CI success is claimed. CI now has explicit Rust
guest build/installation/probe steps on its Windows/Linux matrix. Only this local
Windows run is observed. Licensing and deployment/project/concurrency/output
targets remain owner decisions. No production gate is closed by this slice.
