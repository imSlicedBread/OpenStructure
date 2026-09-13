# Foundation verification — 2026-09-11

This records the first slice. The subsequent IFC CLI slice and independent
validation are documented in [the IFC report](ifc-roadmap.md); the test total
below is the historical foundation total, not the current expanded suite.
The subsequent [depth-buffer slice](viewport-depth-buffer.md) adds intersection
visibility/picking verification and records the historical 52-test suite.
The [Wasm runtime spike](wasm-runtime-spike.md) records the subsequent 64-test suite
and the current BUILD_PROMPT milestone order; older next-task lists below are historical.
The subsequent [opaque-file preservation report](opaque-files-verification.md)
records container 2 and 75 passing tests. The current production scope is tracked
in [2D coverage](2d-coverage.md).
The [plugin worker report](plugin-workers.md) records the next 86-test suite;
earlier totals remain historical evidence, not current production qualification.
The later [bounded-history report](history-retention.md) records 160 passing tests,
the installed-plugin regression probes, and observed synthetic process-memory cost.
The [B/C gate audit](bc-baseline-audit.md) now advances development to D; the
[first plan geometry report](plan-geometry-foundation.md) records 172 passing tests
and explicitly separates controller geometry from the unfinished desktop plan workflow.

Verified locally on Windows x64 with stable Rust 1.98.1 and the installed MSVC
compiler/Windows SDK payloads. Cargo.lock records the resolved dependencies.

| Check | Result |
| --- | --- |
| Workspace compilation | Passed |
| Desktop executable build | Passed; `work/completion-build/debug/os-app.exe` |
| Workspace tests, including integration and documentation | 34 passed, 0 failed |
| Clippy, all targets/features, warnings denied | Passed |
| Rustfmt check | Passed |
| Executable smoke workflow | Passed; `outputs/final-audit.osb` generated |
| Dependency license metadata | 333 packages passed; workspace publication disabled |
| RustSec audit | No known vulnerabilities; one unmaintained-dependency warning |
| Native desktop inspection | Create wall, undo, redo and solid-wall viewport verified |

The smoke workflow checks plugin loading, wall creation, length/thickness/height
and level editing, mesh volume, undo/redo, stable model identity, save/reopen and
identical regenerated geometry. Integration tests additionally cover changing
level elevation, deletion/undo, failed edits and failed-open preservation.
Storage tests cover actual schema migration/resave, duplicate ZIP entries and
all entity kinds. Host tests cover registration discovery/collisions, dependency
versions and least-privilege enforcement. Five desktop tests drive real egui
input through the finished light ribbon UI, including save/open, confirmations,
invalid drafts, compact/DPI layouts and palette resize handles. A regeneration
failure cannot overwrite a good saved project and can be recovered with undo.

The final build uses a separate target directory because Windows locks the
default executable while it is open for native inspection. The source includes
the completed concurrent UI changes; no older UI snapshot was substituted.
The requirement-by-requirement scope is recorded in build-prompt-audit.md.

Native inspection found a back-face visibility defect. The renderer was corrected
to cull hidden faces and the rebuilt application was visually checked. A camera
angle regression test verifies that a prism displays only its three visible sides.

GitHub Actions is configured for Windows and Linux build/test/lint/smoke checks
and dependency auditing. Hosted CI has not been run from this local session.
Linux and macOS desktop behavior has not been locally verified.

## Dependency maintenance warning

`cargo audit` reports [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192)
for unmaintained `ttf-parser 0.25.1`. The chain is egui → epaint → ab_glyph →
owned_ttf_parser → ttf-parser. It is an informational maintenance warning, with no
patched version listed, not a reported vulnerability. No advisory is suppressed.
The application uses bundled fonts and has no custom-font import feature. Review
an egui/font-stack update before public release; do not call this dependency stack
fully maintained or the audit warning-free.

The initial audit could not find Cargo for its registry check. It was rerun with
the workspace-local Cargo environment and completed successfully with the same
single warning. Project license selection and legal review remain pending.

## Reproduce on this machine

```powershell
.\tools\cargo.ps1 build --workspace --locked
.\tools\cargo.ps1 build --workspace --locked --target-dir work\completion-build
.\tools\cargo.ps1 test --workspace --all-features --locked
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 run -p os-app '--' --smoke outputs\new-example.osb
```

The last command requires an existing parent directory and a new output path.
Use normal `cargo` commands when Rust is installed on PATH. Native compositor
inspection is separate from the unattended egui input/layout tests run by Cargo
and CI. All final build/test/lint checks also passed with `--offline`.

## Next three engineering tasks

1. Implement the IFC adapter with independent validation and native/IFC/external
   viewer fixtures, following `ifc-roadmap.md`.
2. Add a production geometry/viewport adapter, wall joins/openings and reliable
   depth-buffered display for intersecting geometry.
3. Implement an isolated Wasm or process plugin runtime and generic property-panel
   registration, keeping the current transaction and message boundaries.
