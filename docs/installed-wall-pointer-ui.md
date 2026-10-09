# Installed Wall pointer submission UI

Follow-up: [installed Wall edits](installed-wall-edits.md) adds all four edit
buttons to the worker adapter and verifies them headlessly against the real guest.
The creation-only scope below records the preceding step.

2026-09-12, Windows untracked working tree. Native workflow acceptance is pending.

Draw wall in plan now selects the bounded installed-Wall adapter when that owner
is loaded with a Wasm worker; otherwise bundled behavior remains. No installation
or activation is inferred from project data. The external-Wall Properties panel
has next-pointer-wall height/thickness in metres. Those drafts are disabled during
a gesture/pending work; guest bounds are checked before submission. Creation uses
the provider's naming and does not support material assignment.

On the second click, admission freezes the exact six-field ToolDraft and persisted
view ticket. Admission errors leave the gesture intact. Successful admission moves
the values into the existing Plugin tools form and reports pending, not committed.
The form cannot collapse while it owns this submission. It supplies pending/cancel
feedback; Escape cancels before the next result poll. Errors retain inputs for
review/retry, subject to the original context and activation checks. View changes
revoke the retained draft, even if the user returns to the old view. Success alone
clears the draft and regenerates. New pointer gestures reject pending plugin work.

This reuses the existing command lifecycle instead of a second uncoordinated
command queue. Selection/level changes and model edits retain their cancellation
or stale-result protection. It is creation-only: installed move/resize/offset and
generic plugin-defined preview callbacks remain open. Native walls use their
documented prism cut/snap fallback, not plugin-specific opaque geometry inference.

## Evidence

The ordinary UI tests pass, including failed-admission preservation and a
gesture-origin draft's same-level-view switch/non-revival. The explicitly run
installed UI test uses real egui pointer events and the unchanged independently
built `outputs/rust-wall-install` guest. It verifies:

- Draw wall in plan routes to the installed adapter and commits through the worker.
- Two linked walls: the first is exact-input 4 m; the second is snapped to the
  first endpoint, exact-input 3 m at 90°, and asserted perpendicular.
- Pointer and exact-input previews leave model revision/history unchanged; matching
  model/scene counts and one-step undo.
- The host's documented wall-prism fallback changes the first wall from Cut to
  Projected under an elevated cut range while preserving picking; a plan shifted
  to a level 10 m above hides both walls and removes their pick targets. The
  original level/range are restored before persistence checks.
- Split 2D/3D retains shared selection while installed Move, both endpoint resizes,
  and Offset copy commit through the worker and undo to the same two-wall model.
- Stopping at mouse release leaves the command pending; Escape on the next frame
  cancels before consumption even if guest execution already completed. Draining
  leaves the original two-wall model unchanged.
- Save/reopen preserves the two wall UUIDs and parameters; the persisted plan can
  be reopened after the document session reset.

The installed test is ignored in a normal suite because it needs a user-selected
installation. It was explicitly executed and passed, not counted as an ordinary
CI pass. It is headless egui evidence, not a completed native desktop inspection.

```powershell
$env:OPENSTRUCTURE_WALL_TEST_PLUGIN = (Resolve-Path 'outputs/rust-wall-install').Path
.\tools\cargo.ps1 test -p os-ui --all-features --locked --offline --target-dir work/completion-build installed_pointer_ui_creates_snapped_walls_through_worker '--' --ignored --nocapture
.\tools\cargo.ps1 test --workspace --all-features --lib --bins --tests --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 test --workspace --all-features --doc --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
```

Adjust the installation path for another checkout; it is not embedded in code.
Both split test commands, strict Clippy, formatting and the default workspace
build pass. The combined test command attempted to relink the running
`wall_desktop.exe` and failed with Windows LNK1104; this was not a test assertion
failure. It is not recorded as a passing combined run. The fresh bundled smoke
also passes; its artifact is baseline evidence, not native installed UI evidence:

```powershell
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/installed-wall-pointer-ui-verified.osb
```

Use the [independent Wall instructions](../examples/rust-wall/README.md) to install.
No guest rebuild or public wire/native-format migration was required. Public Rust
controller APIs from the preceding adapter remain; this adds private UI handling.
Wasm remains bounded in-process, not process isolation; IFC/storage limits remain.

The computer-use skill launched the native harness and inspected the initial
workspace. Further input was paused after user activity and occluded captures;
no native creation/pending/cancel acceptance is claimed. The acceptance app was
left open without further automated input. Rebuild it after closing before the
next native check, since the Properties draft controls were added afterward:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example wall_desktop --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\examples\wall_desktop.exe outputs/rust-wall-install
```

Next: native creation/pending/cancel inspection; installed editing adapters;
full D linked-view/failure acceptance before E1. D/E1–E4/G1–G6 remain incomplete,
L1/L2 later. Production profile, license and publication choices remain open.
There is no HEAD/remote, GitHub URL or hosted CI result.

2026-10-06 acceptance extension: the ignored installed UI test was rerun against
the existing local guest after adding preview immutability, split selection/edit,
and save/reopen identity checks. It passed. This remains headless egui evidence;
native-window inspection, independent-provider graphics qualification, and the
broader D stale/invalid/wrong-scope and late-reply failure matrix remain open.

2026-10-06 acceptance extension: the installed guest scenario now asserts the
exact 4 m + 3 m perpendicular geometry, endpoint snap, Cut-to-Projected range
change with picking, and elevated-level visibility/picking behavior. The ignored
test passed against `outputs/rust-wall-install`; the range/picking assertions
exercise the host-owned, documented straight-wall prism fallback, not guest-
supplied plan graphics. It then restores the original plan settings and passes
the existing save/reopen checks.
