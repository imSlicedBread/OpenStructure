# Opaque native files: implementation and evidence

This closes the known unknown-ZIP-entry loss gap, a prerequisite of BUILD_PROMPT
milestone C. The overall BUILD_PROMPT goal is active and far from production
completion. See [the full coverage ledger](2d-coverage.md), not a reduced scope.

## Observable workflow and compatibility

Open a supported container 1 or 2 `.osb` containing auxiliary files, edit its model,
undo/redo, and save to a new path. File names/bytes and empty directories survive;
the old source is unchanged until explicitly overwritten. Save writes container 2;
older container-1-only builds reject it. Model schema and plugin JSON stay at 1.
IFC exports report auxiliary-file loss and still require explicit acknowledgement.
There is no attachment editor, automatic plugin execution or general unknown
semantic entity support. Do not mistake inert file preservation for plugin editing.

Public additions: `Document::from_model_and_files`, immutable `auxiliary_files`,
`os_storage::CONTAINER_VERSION`/auxiliary size limits, and
`WallIfc::export_report_with_files` for document-aware loss reporting. Model-only
`export_report` cannot know about document attachments; desktop/CLI use the new path.
No Rust dependencies or packages were added. See [file-format rules](file-format.md)
and [ADR 0007](decisions/0007-opaque-container-files.md).

## Verification performed

Windows x64 / Rust 1.98.1 / MSVC; untracked scaffold, no commit or configured remote.
Baseline was 64 passing tests; after this work 75 pass, including 11 added tests.
All following commands passed:

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs\opaque-files-verified.osb
.\tools\cargo.ps1 metadata --all-features --locked --offline --format-version 1 | C:\Python314\python.exe tools/check-license-metadata.py
```

Use a fresh smoke output path when repeating. License metadata covers 347 packages;
legal approval remains pending. Clippy still emits the prior Windows incremental
cache access-denied note for `os-geometry`; exit zero, no source warnings.
The previous refreshed RustSec audit remains the dependency baseline (no lockfile
change): no known vulnerabilities and the unmaintained `ttf-parser` warning.

Evidence: `os-storage/tests/auxiliary_files.rs` tests legacy opaque contents,
directories, unsafe/ambiguous paths, refused-save recovery, individual/total size
caps, CRC failure, symlink rejection, a frozen old `.osb`, and 128 deterministic
truncation/bit-mutation cases. This is a fuzz smoke test, not coverage-guided
production fuzz qualification. `os-ui/tests/vertical_slice.rs` verifies editor
edit/history/save/reopen plus failed-open preservation of model and file bytes.
`os-app/tests/ifc_cli.rs` proves auxiliary loss alone requires export consent and
leaves the original native file unchanged. Existing UI and IFC tests still pass.
The CRC fixture initially used compressed data and its mutation locator failed;
the test now explicitly writes Stored ZIP entries and verifies corruption refusal.

No new controls/layouts were introduced; the IFC confirmation receives an
additional existing-style warning. Native visual inspection of that warning has
not been performed in this increment. Independent IFC geometry was not rerun:
the representation/parser did not change. Hosted Windows/Linux CI was not run;
macOS remains unverified.

## Remaining work and next three dependencies

1. Generic SDK with 2D context design and worker request identities, cancellation,
   deadlines and stale-result rejection; retain compatibility policy for v1.
2. Versioned extension entities/dependencies, missing-plugin preservation and
   transactional migrations, then descriptor-driven independent Rust examples.
3. Complete B/C acceptance and implement D's linked floor-plan workflow before
   E1–E4. Every production capability and G1–G6 gate stays required.

Owner qualification profile remains undecided (jurisdiction, projects, users,
platform/output/hardware targets). These choices do not block foundational work.
License, translators, team design and support remain later release decisions;
no purchases, publication or approval has been inferred.
