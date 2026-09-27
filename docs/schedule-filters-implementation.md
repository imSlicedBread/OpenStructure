# Persisted opening schedule filters — implementation handoff

Implemented for saved Door/Window/All schedules: ordered typed AND filters,
strict validation and serde, live unrounded comparisons, definition draft
controls, memory accounting, atomic schema 41→42 migration, and native API
22→23 compatibility. Generic API 2 remains unchanged. Existing dirty work was
retained; no reset, stash or checkout was used.

The schedule window and `paper_table` both consume `defined_rows`. The direct
sheet test also exercises the actual preview and vector-PDF composition: D01
is included at width > 0.8999999, excluded at width > 0.9, and included again
after its live width changes to 0.9000001. The displayed value remains 0.900.
Searchable PDF text and visible preview rows are checked at both display
profiles, through undo/redo and archive reopen.

RoomFinish filters, OR/nested rules, groups, totals and styles remain unsupported.
S01 stays partial. Headless egui evidence does not establish native-window or
physical-print qualification.

## Validation results

| Command | Result |
| --- | --- |
| `cargo test -p os-model --test schedules` | 2 passed |
| `cargo test -p os-model --test schedule_filters` | 3 passed |
| `cargo test -p os-storage --test schedules` | 2 passed |
| `cargo test -p os-storage --test schedule_filters` | 2 passed |
| `cargo test -p os-plugin-api --all-features` | 6 passed; no doctests |
| `cargo test -p os-walls --all-features` | 2 passed; no doctests |
| `cargo test -p os-ui --all-features --lib opening_schedule -- --test-threads=1` | Final run: 21 passed, 0 failed, 0 ignored; includes direct sheet/PDF propagation |
| Scoped `rustfmt --edition 2024 --check --config skip_children=true` | Passed for changed model/storage/filter/UI implementation and new tests |
| `cargo test --workspace --all-features -- --test-threads=1` | Passed |
| `cargo clippy --workspace --all-features --all-targets -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed after formatting the schema-migration test updates |
| `git diff --check` | Passed; existing CRLF warnings only |

The current-version documentation scan leaves schema 41/API 22 only where
describing historical milestones or rejected older guests. The full workspace
run included the schema migration expectation updates in the ceiling,
material-color, room-finish, room-material, and sheet-table tests. Those changes
only normalize older-schema fixtures by removing the newly introduced empty
`filters` field and update the expected current schema number.

## Exact files changed for this increment

Paths below are relative to the repository root. Other pre-existing dirty files
are outside this increment.

```text
crates/os-model/src/lib.rs
crates/os-model/src/memory.rs
crates/os-model/src/schedules.rs
crates/os-model/tests/schedule_filters.rs
crates/os-storage/src/lib.rs
crates/os-storage/src/tests/ceilings.rs
crates/os-storage/tests/material_colors.rs
crates/os-storage/tests/room_finishes.rs
crates/os-storage/tests/room_materials.rs
crates/os-storage/tests/opening_tags.rs
crates/os-storage/tests/schedule_filters.rs
crates/os-storage/tests/sheet_tables.rs
crates/os-storage/tests/fixtures/schema-41-schedule-filters.json
crates/os-plugin-api/src/lib.rs
crates/os-plugin-host/src/lib.rs
crates/os-plugin-host/tests/wasm_transport.rs
plugins/walls/src/lib.rs
plugins/walls/plugin.toml
fixtures/wasm-probe/plugin.toml
fixtures/wasm-probe/probe.wat
crates/os-ui/src/opening_schedule.rs
crates/os-ui/src/opening_schedule/definitions.rs
crates/os-ui/src/opening_schedule/definitions/filters.rs
crates/os-ui/src/opening_schedule/definitions/tests.rs
crates/os-ui/src/opening_schedule/definitions/tests/filters.rs
crates/os-ui/src/plan_workspace/sheet_tests.rs
crates/os-ui/src/plan_workspace/sheet_tests/filters.rs
docs/2d-coverage.md
docs/file-format.md
docs/plugin-api.md
docs/native-hosted-openings.md
docs/ai-and-residential-mep-plan.md
docs/native-material-colors.md
docs/native-opening-materials.md
docs/native-opening-tags.md
docs/native-rooms.md
docs/native-roofs.md
docs/native-stairs.md
docs/native-wall-types.md
docs/schedule-filters-implementation.md
```
