# Independent Rust column guest

Developer-only API-2 semantic command example. Build without any host/model/kernel
dependency. The buffer adapter is isolated as described in
[ADR 0012](../../docs/decisions/0012-rust-wasm-guest-adapter.md).
The first rectangular-prism geometry provider, optional migration editions and
opt-in desktop workflows are implemented. The optional `plan-graphics` edition
now supplies independently installed plan outlines through the checked service;
native display and pointer authoring remain pending.
See [geometry evidence](../../docs/generic-plugin-geometry.md) and
[migration evidence](../../docs/plugin-migration-service.md).
The [B/C baseline audit](../../docs/bc-baseline-audit.md) now includes both Wall and
column workflows. This remains a bounded development example, not production qualification.

## Mixed-type migration acceptance edition

For the separately installed plan edition, see
[plan service build/probe evidence](../../docs/independent-column-plan.md).
Use `plugin-plan.toml` with `--features plan-graphics` in a new installation
directory; do not replace the existing default/migration installations. This
edition is payload 1/plugin version 1.0.0 and is not the migration-v2 manifest.

The optional `mixed-migration` feature implies `migration-v2` and adds an explicitly
test-only `org.example.columns.test-block` type and `migrate-block` command. This
is a migration fixture, not a new supported architectural category. Columns rename
`height` to `height_m`; blocks also rename `width` to `width_m`. Both preserve
opaque payload data and reject existing destination keys rather than overwrite them.
Blocks have a rectangular-prism geometry recipe but no create/edit/delete tools.
Default and `migration-v2` catalogs are unchanged; use the matching manifest.

Build the host probe first, then compile and copy the guest into a **new** separate
installation directory. Do not overwrite an existing installation:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_migration_probe --locked --offline
.\tools\cargo.ps1 test --manifest-path examples/rust-column/Cargo.toml --features mixed-migration --locked --offline
.\tools\cargo.ps1 build --manifest-path examples/rust-column/Cargo.toml --features mixed-migration --target wasm32-unknown-unknown --release --locked --offline --target-dir work/mixed-migration-build
New-Item -ItemType Directory outputs/rust-column-mixed-install -ErrorAction Stop
Copy-Item examples/rust-column/plugin-mixed.toml outputs/rust-column-mixed-install/plugin.toml
Copy-Item work/mixed-migration-build/wasm32-unknown-unknown/release/openstructure_rust_column.wasm outputs/rust-column-mixed-install/openstructure_rust_column.wasm
.\target\debug\examples\plugin_migration_probe.exe outputs/rust-column-geometry-install outputs/rust-column-v2-install outputs/rust-column-mixed-install
```

The first two installed editions must already exist as described below. On a
machine without the local Cargo wrapper, use ordinary Cargo and omit `--offline`
if dependencies are not cached. The mixed probe seeds eight v1 columns and five
v1 test blocks, explicitly reviews both commands, and verifies final second-type
failure after 12 candidate conversions without live mutation. Success produces
13 meshes, distinct payload conversions and one undoable transaction with native
storage round-trip. This independently built guest still links only the contract
layer; metadata resolves 23 package versions/22 names with no host/model/kernel
dependencies. No new buffer ABI, dependency, native format or unsafe policy.

Local mixed-feature tests pass 5; default and migration-v2 tests pass 4 each. The
three-installation probe and host/guest strict Clippy pass. CI now builds and invokes
this variant, but no hosted result is claimed. Native mixed-type layout/interaction
acceptance and broader B/C qualification remain incomplete.

With Rust installed, from the checkout root:

```powershell
rustup target add wasm32-unknown-unknown
cargo test --manifest-path examples/rust-column/Cargo.toml --locked
cargo build -p os-plugin-host --features wasm --example generic_probe --locked
cargo build --manifest-path examples/rust-column/Cargo.toml --target wasm32-unknown-unknown --release --locked
New-Item -ItemType Directory outputs/rust-column-install
Copy-Item examples/rust-column/plugin.toml outputs/rust-column-install/plugin.toml
Copy-Item examples/rust-column/target/wasm32-unknown-unknown/release/openstructure_rust_column.wasm outputs/rust-column-install/openstructure_rust_column.wasm
./target/debug/examples/generic_probe.exe outputs/rust-column-install
```

Use a new installation directory; do not overwrite an existing plugin installation.
On Linux the executable has no `.exe` suffix. On the current Windows checkout,
`tools/cargo.ps1` selects the workspace-local toolchain when needed. Target installation
must use that same toolchain's rustup environment. Once dependencies/target are cached,
build with `--offline`. The independent lockfile is retained; generated targets are not.

The probe executable is built before the guest. Copying the compiled artifact and
manifest requires no host rebuild. It verifies actual registered descriptors,
create/edit/delete, checked prism geometry, stable IDs, undo/redo, denied grants, cancellation, stale
revision rejection, and unload/save/reopen. Temporary projects are test-owned.
The normal desktop never discovers or executes this artifact automatically.

Create/edit inputs are width, depth and height in metres. Creation requires one
scoped level and one host-reserved ID; editing/deletion require one selected
owned column. Editing preserves unrecognized payload fields. The host remains
responsible for authorization, canonical IDs, full graph validation and commit.
Protocol errors return structured errors; malformed JSON cannot be correlated
and returns an invalid zero-length result, rejected by the host.

Local verification status and remaining acceptance gaps are recorded in
[the coverage ledger](../../docs/2d-coverage.md).
# Migration edition

The optional `migration-v2` build emits payload schema 2 and must be paired with
`plugin-v2.toml` (plugin version 2.0.0). The Cargo source package remains the shared
1.0.0 example package; its two build variants demonstrate separate installed plugin
versions. Do not mix the v1 manifest with a v2 artifact or overwrite the v1 install.
Schema 2 renames the metre-valued `height` field to `height_m`; width/depth and
placement are unchanged. Explicit migration preserves IDs, relationships and other
payload keys, rejecting an existing `height_m` key instead of overwriting it.

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_migration_probe --locked --offline
.\tools\cargo.ps1 build --manifest-path examples/rust-column/Cargo.toml --features migration-v2 --target wasm32-unknown-unknown --release --locked --offline --target-dir work/migration-guest-build
# Use a new installation directory; never overwrite an existing installation.
New-Item -ItemType Directory outputs/rust-column-v2-install -ErrorAction Stop
Copy-Item examples/rust-column/plugin-v2.toml outputs/rust-column-v2-install/plugin.toml
Copy-Item work/migration-guest-build/wasm32-unknown-unknown/release/openstructure_rust_column.wasm outputs/rust-column-v2-install/openstructure_rust_column.wasm
.\target\debug\examples\plugin_migration_probe.exe outputs/rust-column-geometry-install outputs/rust-column-v2-install
```

That local installation now exists: reuse it for the probe, or choose a fresh
directory when installing another build. The schema-1 installation remains intact.
See [migration evidence](../../docs/plugin-migration-service.md) for limits and
current acceptance. This edition requires a host supporting the Migrate catalog
mode. The ordinary build remains schema 1 and registers no migration command.
