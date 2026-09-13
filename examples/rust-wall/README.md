# Independent native Wall guest

The [installed gesture controller probe](../../docs/installed-wall-gesture-adapter.md)
now exercises exact snapped plan creation through this guest's worker commands.
This is controller evidence; native pointer integration remains in progress.

This separately resolved Rust Wasm example uses API-2 generic commands and geometry
to edit the host's native straight walls. It does not link model/document/kernel
types. The wire `wall::Parameters` projection has payload version 1 independently
of native model schema 4. Material assignment and arbitrary header metadata are
not exposed; edits preserve them on the host. Creation has no material assignment.

The [native Wall workflow](../../docs/native-wall-workflow.md) now records create,
edit, undo/redo, save/open and shared-selection evidence through this independently
installed guest. Run `cargo run -p os-ui --features external-plugins --example wall_desktop -- outputs/rust-wall-install`
after installation to reproduce. This developer harness supplies explicit test
grants; it is not a production plugin-installation acceptance claim.

From the checkout root, with the official `wasm32-unknown-unknown` target installed:

```powershell
cargo test --manifest-path examples/rust-wall/Cargo.toml --locked
cargo build -p os-plugin-host --features wasm --example native_wall_probe --locked
cargo build --manifest-path examples/rust-wall/Cargo.toml --target wasm32-unknown-unknown --release --locked
New-Item -ItemType Directory outputs/rust-wall-install
Copy-Item examples/rust-wall/plugin.toml outputs/rust-wall-install/plugin.toml
Copy-Item examples/rust-wall/target/wasm32-unknown-unknown/release/openstructure_rust_wall.wasm outputs/rust-wall-install/openstructure_rust_wall.wasm
./target/debug/examples/native_wall_probe.exe outputs/rust-wall-install
```

Choose a new directory rather than overwriting an existing installation. On Linux
omit `.exe`. The current Windows checkout can use `tools/cargo.ps1` in place of
Cargo, and cached builds support `--offline`. The probe runs without a host rebuild
after copying the guest. The example reuses the adjacent column example's small
Wasm buffer adapter; retain both example source directories when building.

Inputs are start/end X/Y, thickness and height in metres. Creation reserves one
identity and scopes one level. Editing selects one native wall and scopes one
target level (including for reassignment); deletion selects one wall. Names remain
unchanged on edit. Geometry requires the wall and its level in read scope and
returns a rectangular prism positioned on the wall's centerline at level elevation.
The example's coordinate/dimension limits are developer bounds, not approved
production project limits.

Its owner/version match the bundled Walls plugin (`org.openstructure.walls`,
`0.1.0`), so the two cannot be loaded simultaneously. Explicitly unload one before
loading the other. Project files never load plugins automatically. Native walls
remain native and can be edited with core/bundled workflows without this guest.

See [native Wall verification](../../docs/native-wall-plugin.md). This is a developer
probe, not a completed desktop installation workflow, property-panel renderer,
plugin manager, floor-plan tool or general BIM production release.
