# Installed Wall move, resize and offset

2026-09-12, Windows untracked working tree. Headless controller/egui evidence;
native visual acceptance remains open.

`WallGesture::begin_edit_installed` now supports Move, Resize start, Resize end
and Offset copy for the loaded independent native-Wall provider. Move/resize
choose its unique enabled Edit descriptor; offset chooses Create. Neither mode
falls back to the other if missing, disabled or ambiguous. The same six metre
inputs and numeric bounds are required. Preview computation, semantic snap
exclusion and signed offsets reuse the tested host gesture math.

Move/resize write only the selected source UUID with no new identities; copy
reserves one new UUID and has no write selection. The captured document/view/
settings/activation must remain current at submission and worker consumption.
Source level is preserved. The existing host native-Wall adapter preserves
unexposed metadata on replacement. Copy leaves the source unchanged, uses provider
naming and rejects material assignment rather than silently losing it.

Installed edits currently require the source wall's level plan: the existing
single-level ToolContext is also the active-level intent check. Cross-level visible
wall editing needs a richer intent/scope distinction and remains unsupported.
Invisible source walls are rejected. This is not a relaxation of the eventual
multi-level architectural requirements. Arbitrary plugin types/callbacks are not
enabled by matching field names; this adapter is for the native Wall contract.

The four plan buttons now route to this adapter for a loaded Wasm Wall owner.
Successful admission uses the existing pending/cancel form, not an immediate
success claim; failed admission retains the gesture. No separate command queue,
new permissions, wire version or native schema is introduced.

## Verification

The expanded `wall_gesture_probe` passed against the unchanged independent guest
at `outputs/rust-wall-install` (installation/build instructions are in
[the guest README](../examples/rust-wall/README.md)). It tests all four modes in a
translated/rotated plan, scope/write/create counts, exact preview-to-committed
coordinates, preserved source on copy, expected counts and one-step undo. Existing
creation, snapping, cancellation, inactive-view, unload and save/reopen checks pass.

The explicitly run ignored installed egui test also passes all four buttons,
their pointer commits, preview agreement and undo back to the identical two-wall
model. It retains creation, snapping, exact length and pending Escape tests. It
uses a real installed Wasm guest, not a simulated success reply. Normal CI skips
this test without an explicit installation; do not count the skip as a pass.

The first UI run failed to activate Resize start because Plugin tools' default
floating position overlapped the toolbar. Moving its default Y from 170 to 260
made all edit buttons usable in the 1280×800 headless test. Existing user-positioned
windows are not forcibly moved. Native/high-DPI/narrow-layout inspection remains.
A unit test verifies Edit selection never falls back to Create and ambiguity is
mode-specific. No native input was attempted during this follow-up; the previous
native inspection remains paused after user activity/occlusion.

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example wall_gesture_probe --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\examples\wall_gesture_probe.exe outputs/rust-wall-install
$env:OPENSTRUCTURE_WALL_TEST_PLUGIN = 'C:\Users\Youca\Documents\ChatGPT\OpenStructure\outputs\rust-wall-install'
.\tools\cargo.ps1 test -p os-ui --all-features --locked --offline --target-dir work/completion-build installed_pointer_ui_creates_snapped_walls_through_worker '--' --ignored --nocapture
.\tools\cargo.ps1 test --workspace --all-features --lib --bins --tests --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 test --workspace --all-features --doc --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
```

Adjust the installation path for another checkout. Tests are split to avoid
relinking the still-open older native acceptance executable; rebuild that harness
after closing it before inspection. The public Rust controller gains
`begin_edit_installed`; old bundled methods and file/wire formats remain compatible.
Wasm is still bounded in-process execution, not process isolation. IFC remains
restricted; unknown extension preservation remains unchanged.

Both split workspace test commands pass, as do strict Clippy, formatting and the
default workspace build. The fresh bundled baseline smoke passes too; this is
not independent native-interaction evidence:

```powershell
.\work\completion-build\debug\os-app.exe --smoke outputs/installed-wall-edits-verified.osb
```

Next: native installed create/edit/history/save/reopen acceptance; D cut/level and
failure/late-result completion; remaining D input/graphics gaps before E1. D and
E1–E4/G1–G6 remain incomplete; L1/L2 later. Production profile, licensing and
publication choices remain open. No HEAD/remote/GitHub URL or hosted CI result.
