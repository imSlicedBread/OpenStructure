# Architectural grids: persistence and command foundation

Follow-up: [plan integration](grid-plan-integration.md) now draws, selects and
snaps to persisted grid axes; [grid forms](grid-authoring-forms.md) now provide
native create/edit controls with headless tests (native inspection remains).
The evidence below describes the earlier persistence checkpoint.

The D model foundation now stores building-scoped straight architectural grids,
independent of decorative viewport grid lines. See [ADR 0015](decisions/0015-architectural-grid-datums.md)
for the typed contract and boundaries. Native UI drawing/snapping integration is
still pending; this is not a complete grid authoring workflow.

Sources: `os-model/src/grids.rs`, model graph validation and memory accounting,
document grid commands, constraint invalidation, storage migration 3→4 and IFC
loss reporting. No dependency or independent plugin protocol change is required.

## Regression evidence

- `os-document/tests/grids.rs`: create, move/rename with unchanged identity/header,
  undo/redo, exact no-op retention, deletion, invalidation of named plans and
  memory accounting. Invalid names, geometry, references, type/schema and UUID
  collisions roll back the entire batch without revision/history/events. Duplicate
  names reject; referenced deletion rejects; deleting both entities is undoable.
- `os-model/tests/grids.rs`: parameter round-trip and rejection of unsupported
  parameter fields and start/end z coordinates.
- `os-storage/tests/grids.rs`: frozen schema-3 migration changes only native
  versions and the new empty map, then adds a grid and saves/reopens an actual
  archive with identical model/auxiliary bytes and untouched original archive.
  Ambiguous or malformed old input rejects without partially changing input.
- `os-ifc/tests/grids.rs`: a grid alone triggers explicit loss reporting and
  rejection by the non-acknowledging export adapter; native source remains unchanged.

`fixtures/schema-3-pointer-walls.json` is the model JSON captured from
`outputs/native-pointer-walls.osb`, the previously verified two-wall native
workflow. The old file and prior evidence remain unchanged; new saves use schema 4.

The baseline before editing passed all 197 workspace tests. Verification of this
change passed all 203 tests in the locked offline full workspace/all-features
suite, strict Clippy (`--all-targets --all-features -D warnings`), the default
workspace build and format checking on Windows. Independently installed Wall
and column Editor probes and the single/mixed-type staged migration probes
passed against the rebuilt host, without rebuilding the guests. These probes
did not activate native desktop controls. Test results are development evidence,
not performance or production qualification.

Reproduce with `tools/cargo.ps1`, `--locked --offline` and target directory
`work/completion-build`: `test --workspace --all-features --quiet`,
`clippy --workspace --all-targets --all-features -- -D warnings`,
`build --workspace`, and `fmt --all -- --check` (formatting needs no lock/target
flags). Build `os-ui` examples `plugin_editor_probe` and `plugin_migration_probe`
with `external-plugins`; use the existing `outputs/rust-wall-install`,
`rust-column-geometry-install`, `rust-column-v2-install` and
`rust-column-mixed-install` directories as in the B/C probe reports.

Next: consume these entities in bounded plan drawings and semantic queries, then
provide native grid creation/editing and grid-aware wall pointer workflows. D
also still requires move/resize and independently installed pointer/plan providers.
