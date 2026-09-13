# Independent Rust column plan outline

2026-09-12. D progress: actual separately built guest -> Wasm worker -> checked
controller drawing. Native UI/pointer workflow acceptance remains incomplete.

The `examples/rust-column` optional `plan-graphics` feature routes service-1
requests while preserving generic command/geometry dispatch. It reuses its own
validated rectangular-prism dimensions and single-level binding, then computes
four finite semantic outline edges (feature keys 0-3) in the requested horizontal
view plane. Cut/projected/depth classification uses the documented 1e-9 metre
boundary convention; fully outside/hidden geometry returns an empty outline.
Invalid service/scope/level/dimensions/ranges or lost coordinate precision returns
an error rather than partial geometry. Original outline edges are returned before
host crop, preserving genuine semantic snap endpoints.

This guest's column remains axis-aligned at model XY origin and bound to one
level. Only that level is included in the current scoped plan request. View-origin
and yaw transforms are supported; arbitrary placement, cross-level scoped queries,
filled cuts, compound symbols and pointer authoring are not supplied by this
edition. Outline-only graphics do not support filled-interior picking: a crop
wholly inside the column may contain no outline edges. This must not qualify an
issued architectural drawing or be represented as complete column plan graphics.

## Independent installation result

Built the `column_plan_probe` host executable first. Then built the Wasm guest
against only the independent SDK and copied its manifest/artifact into the new
`outputs/rust-column-plan-install` directory. The existing host executable ran
against that directory without a rebuild after guest compilation/installation.
No existing installed edition was overwritten.

Installed Wasm SHA-256:
`813323450C1F47F16A880F00AC5C3C698203318FE9069C59A0ECF79210DA237C`.

The probe passed: registered create with width .4/depth .6/height 3 metres; actual
plan worker output; four cut edges and expected first-edge coordinates; controller
composition/unavailable resolution; unchanged model/revision during query; view
range change to projected graphics; stale drawing rejection; provider-unload
rejection; and native save/reopen retaining the same model without the provider.
The temporary round-trip file is test-owned and removed with its temporary folder.

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example column_plan_probe --locked --offline --target-dir work/completion-build
.\tools\cargo.ps1 test --manifest-path examples/rust-column/Cargo.toml --features plan-graphics --locked --offline --target-dir work/column-plan-build
.\tools\cargo.ps1 build --manifest-path examples/rust-column/Cargo.toml --features plan-graphics --target wasm32-unknown-unknown --release --locked --offline --target-dir work/column-plan-build
New-Item -ItemType Directory outputs/rust-column-plan-install -ErrorAction Stop
Copy-Item examples/rust-column/plugin-plan.toml outputs/rust-column-plan-install/plugin.toml
Copy-Item work/column-plan-build/wasm32-unknown-unknown/release/openstructure_rust_column.wasm outputs/rust-column-plan-install/openstructure_rust_column.wasm
.\work\completion-build\debug\examples\column_plan_probe.exe outputs/rust-column-plan-install
```

Choose a new directory for a fresh installation; rerunning the existing probe
does not need to recreate/copy the installation. The optional manifest adds Views
and a ViewProvider registration. API 2/service 1, Wasm ABI 1, payload 1, native
container 2/model 4 and plan settings 1 remain unchanged. No new dependency or
unsafe-code allowance. The default/migration artifacts are not silently upgraded.

Guest tests verify numeric rectangle corners, rotated view basis, cut/projected/
depth boundaries, invisible output, envelope dispatch and invalid version/scope/
range/level/precision handling. Plan-feature tests pass six. Host/guest strict
Clippy and formatting pass. The installed probe is headless, not native UI evidence.
The full host workspace all-feature suite also passes. Guest default tests pass
four and combined all-feature tests pass seven; the plan-only six-test result
above is the installed edition's configuration. Verification commands include:

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 clippy --manifest-path examples/rust-column/Cargo.toml --features plan-graphics --all-targets --locked --offline --target-dir work/column-plan-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 fmt --manifest-path examples/rust-column/Cargo.toml '--' --check
```

Follow-up: [native column plan integration](native-column-plan.md) now verifies
provider scheduling/rendering, form edits, shared selection and save/reopen using
the unchanged installed guest.

Next: independent pointer/preview workflow;
full linked-view acceptance including save/reopen, snapping and failure scenarios.
D and E1-E4/G1-G6 remain incomplete; L1/L2 later. Windows development evidence is
not a supported production profile. Licensing/publication and qualification targets
remain open; no commit, Git remote or hosted CI result is claimed.
