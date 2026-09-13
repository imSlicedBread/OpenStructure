# Installed column native workflow evidence

Local Windows native interaction, 2026-09-12, using the existing `plugin_desktop`
acceptance harness and the unchanged installed Rust column v1 artifact. This harness
explicitly loads its configured local plugin with test-owned grants; this does not
qualify the production manager's permission-change workflow.

Using the computer-use skill, the native window was launched and inspected, then:

1. Opened `org.example.columns.create`, reviewed Width 0.4, Depth 0.6, Height 3.0
   and their metre bounds, and applied the command.
2. Selected Rectangular column in Project Browser. The properties and form target
   showed the selected extension rather than Wall instance inputs.
3. Opened the registered edit form, replaced Height with 6, applied, and collapsed
   the form to inspect the taller selected mesh.
4. Undid the height edit. Reopening the edit form showed the committed Height 3.0.
5. Used File/Save with a previously absent path, then File/Open at the same path.
   The reopened document was clean, its selection/history reset, and its status
   reported zero unavailable plugin elements. The verification window was closed
   cleanly and its absence confirmed.

The resulting artifact is `outputs/native-column-acceptance.osb`. A read-only ZIP/JSON
inspection confirmed one schema-1 `org.example.columns.rectangular` entity,
0.4×0.6×3.0 dimensions and its retained level dependency. No permission settings
were changed through native automation, and no existing project was overwritten.

## Findings and fixes

The status bar incorrectly said Incomplete view whenever any extension existed,
even though the viewport had a valid mesh. It now tests missing meshes like the
viewport warning. The shared-selection regression test now checks that a rendered
extension does not trigger that status.

After Undo, geometry visibly caught up on a subsequent interaction. Code inspection
showed that history/ribbon commands can queue geometry after the frame's early
worker poll, which can have reported idle. The end of the desktop frame now requests
a repaint whenever plugin work is pending, including work queued by those controls.
The updated code passes automated checks; the repaint fix has not yet been re-run
through native interaction. Do not treat the original delayed repaint as qualified
idle-frame behavior.

## Reproduction and checks

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_desktop --locked --offline
.\target\debug\examples\plugin_desktop.exe outputs/rust-column-geometry-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/native-column-followup-verified.osb
```

150 workspace tests pass, strict lint/format/default build and the named smoke pass.
The installed column Editor probe and installed v1/v2 migration probe pass after the
fixes. Use fresh output paths on reruns. Screenshots were inspected directly in the
native tool results; no separate screenshot files were saved.

Next: rerun native repaint behavior and complete the external Wall workflow; finish
scalable/mixed-type migration and joint B/C acceptance; proceed through D/E1–E4 and
G1–G6, with L1/L2 later. The [coverage ledger](2d-coverage.md) remains partial. This
does not prove full native cancellation, migration UI, permission or deletion flows,
nor architectural/companion BIM production, issued reference projects or plotting.
Deployment/jurisdiction/project/concurrency/output targets remain undecided.

No public protocol, file format, dependency or unsafe-policy change. Bounded Wasm
remains in-process, not process isolation. Existing IFC restrictions and unknown-data
preservation remain. Source remains an untracked scaffold without HEAD, remote,
GitHub URL or hosted CI result; license approval and publication remain pending.
