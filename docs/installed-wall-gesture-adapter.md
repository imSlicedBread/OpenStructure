# Installed Wall gesture controller adapter

Follow-up: [pointer submission UI](installed-wall-pointer-ui.md) now wires native
creation to this adapter and passes an installed headless egui acceptance test.
The controller-only/native-unwired statements below describe the preceding step.

2026-09-12, Windows untracked working tree. Controller-only D progress; native
Draw wall in plan still uses the bundled provider until its pending-intent UI is wired.

`WallGesture::begin_installed` explicitly opts into the installed Wasm Wall owner.
It requires a bounded worker and one enabled, unambiguous native-Wall create
descriptor with exactly the six supported metre inputs. Arbitrary command fields,
other types, disabled/ambiguous commands and material assignment are rejected.
Creation naming remains provider-owned, as in the existing generic form workflow.

The existing host gesture performs work-plane conversion, exact length/angle and
transient previews without invoking the guest or changing history. Its captured
document/view/settings and activation must still match when `installed_command`
freezes coordinates into a normal ToolDraft, creation scope (one level, no write
selection, one reserved entity) and persisted ViewContext ticket. Draft validation
enforces the provider's numeric bounds. The old synchronous commit method rejects
installed gestures; callers must use the bounded worker and keep cancellation
available until commit. Returning a draft does not itself submit or commit it.

This reuses API-2's native Wall field contract, not a new general plugin gesture
callback protocol. Generic plugin-defined input/preview callbacks, independent
edit/offset adapters and full D native failure acceptance remain unfinished.
The host's documented native-prism plan fallback supplies walls' cuts and snaps;
no opaque extension payload is interpreted as a wall.

## Evidence and reproduction

The new `wall_gesture_probe` passed against the unchanged independently built
`outputs/rust-wall-install` guest (SHA256
`E4017D3753C41BD4D6E0930344BE9DD469915BC69511349B2F137F4545A25A5C`).
Use [the independent guest instructions](../examples/rust-wall/README.md) to build
and install on a new checkout. This follow-up rebuilt the host, not the guest.

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example wall_gesture_probe --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\examples\wall_gesture_probe.exe outputs/rust-wall-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
```

The probe uses a translated/rotated plan: exact five-metre first wall, semantic
endpoint acquisition and a perpendicular three-metre second wall. It verifies
unchanged document during preview, identical first-wall preview/committed points,
one-step undo/redo with stable identities, invalid numeric input, cancelled worker
draining, inactive-view reply rejection, unload revocation and temporary native
save/reopen equality. The temporary output is not retained as a reference project.
The adapter test rejects ambiguous, missing, wrong-kind/unit/key/type and disabled
descriptors. Full workspace tests and strict lint passed during implementation;
see subsequent native evidence before making desktop usability claims.

The default workspace build and formatting pass. The bundled baseline smoke also
passes with the new `outputs/installed-wall-gesture-verified.osb` artifact (not an
independent gesture/native-UI artifact):

```powershell
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/installed-wall-gesture-verified.osb
```

No wire, payload, native-format, dependency or runtime-trust changes. The public
Rust controller gains two external-plugins-gated methods; old bundled behavior
is unchanged. Wasm remains bounded in-process execution, not process isolation.
IFC remains its restricted native-wall subset; unknown extension preservation is
unchanged. No new desktop UI is claimed or inspected in this step.

Next: native pending-gesture submission/cancellation; installed move/resize/offset;
complete D's native linked-view and failure scenarios before E1. D/E1–E4/G1–G6
remain incomplete, L1/L2 later. Production profile, license and publication remain
open. No HEAD/remote, GitHub URL or hosted CI result exists.
