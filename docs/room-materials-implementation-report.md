# Room material assignment implementation

Implemented directly in the existing dirty workspace. Earlier work was preserved.
No reset, revert, stash, commit, or full workspace test run was performed.

## Result

- Required nullable floor/wall/ceiling material UUIDs in schema 35, independent of the existing finish codes. Constructors and memory accounting updated.
- Whole-model validation requires references to exist. UpdateRoom and material removal reject invalid transactions atomically; undo/redo preserves identity.
- Room Properties stages project material selections alongside codes and commits all fields in one UpdateRoom. Escape and stale session/revision checks retain their existing behavior.
- The existing eight Room Finish columns/default remain unchanged. Cells compose live material names and independent codes, including material-only and code-only cases. Sheet preview/PDF shares these rows and existing overflow checks.
- Resolved active-plan room polygons get subdued floor RGB fills, clipped to crop beneath walls, room outlines, text, tags and selection. Color edits refresh live; unresolved rooms retain identity with no stale polygon/fill. Wall/ceiling references alone produce no floor fill.
- Atomic schema 34→35 migration inserts null references and advances native headers/root. It rejects pre-existing fields and preserves IDs, signatures, codes, material RGB, extensions and schedule definitions. Native full-model API is 16/schema 35; generic API 2/container 2 are unchanged.
- Coverage ledger P01.08 is partial, with bounded fill evidence and remaining legends/rule-based color limits.

No material quantities, finish-face/net areas, wall/ceiling takeoffs, legends,
IFC associations, room solids, floor construction geometry or physical-print
qualification were added. UI evidence is headless at 1280×800/100% and
1000×650/150%; no native-window inspection is claimed.

## Final focused checks

All commands below ran from `C:\\fKey Labs\\OpenStructure`. Final total:
**51 passed, 0 failed, 0 ignored**. The coordinator owns the full workspace run.

| Exact command | Result |
| --- | --- |
| `cargo test -p os-model --test room_finishes` | 3 passed |
| `cargo test -p os-document --test rooms` | 3 passed |
| `cargo test -p os-storage --test room_materials --test room_finishes --test material_colors --test room_separation_lines --test room_tags --test schedules` | 12 passed (2 in each target) |
| `cargo test -p os-storage --lib sheets_schema_thirteen` | 1 passed |
| `cargo test -p os-geometry --lib floors::tests` | 3 passed |
| `cargo test -p os-ui --lib room_ --no-default-features` | 21 passed |
| `cargo test -p os-plugin-api --lib` | 6 passed |
| `cargo test -p os-plugin-host --features wasm --test wasm_transport legacy_model_guest` | 1 passed |
| `cargo test -p os-plugin-host --features wasm --test wasm_transport external_fixture_loads` | 1 passed |
| `git diff --check` | Exit 0; no whitespace errors (Git emitted existing local ignore-access/line-ending warnings) |

Formatting was restricted to the 30 Rust files touched by this slice, with
child-module traversal disabled. Both commands exited 0:

```powershell
rustfmt --edition 2024 --config skip_children=true crates/os-document/tests/room_separation_lines.rs crates/os-document/tests/room_tags.rs crates/os-document/tests/rooms.rs crates/os-geometry/src/floors.rs crates/os-model/src/lib.rs crates/os-model/src/memory.rs crates/os-model/src/rooms.rs crates/os-model/tests/room_finishes.rs crates/os-model/tests/room_tags.rs crates/os-plugin-api/src/lib.rs crates/os-plugin-host/src/lib.rs crates/os-plugin-host/tests/wasm_transport.rs crates/os-storage/src/lib.rs crates/os-storage/src/tests/sheets.rs crates/os-storage/tests/material_colors.rs crates/os-storage/tests/room_finishes.rs crates/os-storage/tests/room_materials.rs crates/os-storage/tests/room_separation_lines.rs crates/os-storage/tests/room_tags.rs crates/os-storage/tests/rooms.rs crates/os-storage/tests/schedules.rs crates/os-ui/src/lib.rs crates/os-ui/src/opening_schedule/rooms.rs crates/os-ui/src/opening_schedule/rooms/tests.rs crates/os-ui/src/palettes.rs crates/os-ui/src/plan_workspace.rs crates/os-ui/src/plan_workspace/room_materials.rs crates/os-ui/src/plan_workspace/sheet_tests.rs crates/os-ui/src/room_tools.rs plugins/walls/src/lib.rs
rustfmt --check --edition 2024 --config skip_children=true crates/os-document/tests/room_separation_lines.rs crates/os-document/tests/room_tags.rs crates/os-document/tests/rooms.rs crates/os-geometry/src/floors.rs crates/os-model/src/lib.rs crates/os-model/src/memory.rs crates/os-model/src/rooms.rs crates/os-model/tests/room_finishes.rs crates/os-model/tests/room_tags.rs crates/os-plugin-api/src/lib.rs crates/os-plugin-host/src/lib.rs crates/os-plugin-host/tests/wasm_transport.rs crates/os-storage/src/lib.rs crates/os-storage/src/tests/sheets.rs crates/os-storage/tests/material_colors.rs crates/os-storage/tests/room_finishes.rs crates/os-storage/tests/room_materials.rs crates/os-storage/tests/room_separation_lines.rs crates/os-storage/tests/room_tags.rs crates/os-storage/tests/rooms.rs crates/os-storage/tests/schedules.rs crates/os-ui/src/lib.rs crates/os-ui/src/opening_schedule/rooms.rs crates/os-ui/src/opening_schedule/rooms/tests.rs crates/os-ui/src/palettes.rs crates/os-ui/src/plan_workspace.rs crates/os-ui/src/plan_workspace/room_materials.rs crates/os-ui/src/plan_workspace/sheet_tests.rs crates/os-ui/src/room_tools.rs plugins/walls/src/lib.rs
```

During implementation, a duplicate model test identifier and two render-test
setup errors (crop type name and initial plan activation) were corrected. The
final runs above include the corrected tests.

## Focused evidence

- Model test checks each material slot against valid, missing and absent references.
- Document test checks atomic UpdateRoom/material-removal rejection, revision/history/event stability, undo/redo, and clearing references plus removal in one transaction.
- Frozen schema-34 fixture and storage tests check field/header preservation, ambiguous/malformed/missing field rejection without adoption, dangling UUIDs and save/reopen.
- Properties tests exercise the material dropdown, local drafts, apply, cancel, stale revision/session, invalid reference rejection and history at both profiles.
- Schedule tests assert composite text, live rename, color-only stability, unassigned codes and unresolved-room identity.
- Sheet tests assert composite preview/PDF text, live rename/color, save/reopen, unresolved rows and existing overflow rejection.
- Plan tests assert mesh RGB, exact crop scissor, selected outline, wall/tag drawing order, live recolor without geometry change, immediate and settled no-fill after enclosure loss, history, and no floor geometry. Concave and 300-vertex rings conserve area in both windings.

## Exact touched paths

These 46 paths include pre-existing dirty files edited by this slice,
new files, and this report. Other dirty paths shown by Git predate this work.

- [README.md](<C:/fKey Labs/OpenStructure/README.md>)
- [crates/os-document/tests/room_separation_lines.rs](<C:/fKey Labs/OpenStructure/crates/os-document/tests/room_separation_lines.rs>)
- [crates/os-document/tests/room_tags.rs](<C:/fKey Labs/OpenStructure/crates/os-document/tests/room_tags.rs>)
- [crates/os-document/tests/rooms.rs](<C:/fKey Labs/OpenStructure/crates/os-document/tests/rooms.rs>)
- [crates/os-geometry/src/floors.rs](<C:/fKey Labs/OpenStructure/crates/os-geometry/src/floors.rs>)
- [crates/os-model/src/lib.rs](<C:/fKey Labs/OpenStructure/crates/os-model/src/lib.rs>)
- [crates/os-model/src/memory.rs](<C:/fKey Labs/OpenStructure/crates/os-model/src/memory.rs>)
- [crates/os-model/src/rooms.rs](<C:/fKey Labs/OpenStructure/crates/os-model/src/rooms.rs>)
- [crates/os-model/tests/room_finishes.rs](<C:/fKey Labs/OpenStructure/crates/os-model/tests/room_finishes.rs>)
- [crates/os-model/tests/room_tags.rs](<C:/fKey Labs/OpenStructure/crates/os-model/tests/room_tags.rs>)
- [crates/os-plugin-api/src/lib.rs](<C:/fKey Labs/OpenStructure/crates/os-plugin-api/src/lib.rs>)
- [crates/os-plugin-host/src/lib.rs](<C:/fKey Labs/OpenStructure/crates/os-plugin-host/src/lib.rs>)
- [crates/os-plugin-host/tests/wasm_transport.rs](<C:/fKey Labs/OpenStructure/crates/os-plugin-host/tests/wasm_transport.rs>)
- [crates/os-storage/src/lib.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/src/lib.rs>)
- [crates/os-storage/src/tests/sheets.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/src/tests/sheets.rs>)
- [crates/os-storage/tests/fixtures/schema-34-room-materials.json](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/fixtures/schema-34-room-materials.json>)
- [crates/os-storage/tests/material_colors.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/material_colors.rs>)
- [crates/os-storage/tests/room_finishes.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/room_finishes.rs>)
- [crates/os-storage/tests/room_materials.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/room_materials.rs>)
- [crates/os-storage/tests/room_separation_lines.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/room_separation_lines.rs>)
- [crates/os-storage/tests/room_tags.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/room_tags.rs>)
- [crates/os-storage/tests/rooms.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/rooms.rs>)
- [crates/os-storage/tests/schedules.rs](<C:/fKey Labs/OpenStructure/crates/os-storage/tests/schedules.rs>)
- [crates/os-ui/src/lib.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/lib.rs>)
- [crates/os-ui/src/opening_schedule/rooms.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/opening_schedule/rooms.rs>)
- [crates/os-ui/src/opening_schedule/rooms/tests.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/opening_schedule/rooms/tests.rs>)
- [crates/os-ui/src/palettes.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/palettes.rs>)
- [crates/os-ui/src/plan_workspace.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/plan_workspace.rs>)
- [crates/os-ui/src/plan_workspace/room_materials.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/plan_workspace/room_materials.rs>)
- [crates/os-ui/src/plan_workspace/sheet_tests.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/plan_workspace/sheet_tests.rs>)
- [crates/os-ui/src/room_tools.rs](<C:/fKey Labs/OpenStructure/crates/os-ui/src/room_tools.rs>)
- [docs/2d-coverage.md](<C:/fKey Labs/OpenStructure/docs/2d-coverage.md>)
- [docs/file-format.md](<C:/fKey Labs/OpenStructure/docs/file-format.md>)
- [docs/native-hosted-openings.md](<C:/fKey Labs/OpenStructure/docs/native-hosted-openings.md>)
- [docs/native-material-colors.md](<C:/fKey Labs/OpenStructure/docs/native-material-colors.md>)
- [docs/native-opening-materials.md](<C:/fKey Labs/OpenStructure/docs/native-opening-materials.md>)
- [docs/native-opening-tags.md](<C:/fKey Labs/OpenStructure/docs/native-opening-tags.md>)
- [docs/native-rooms.md](<C:/fKey Labs/OpenStructure/docs/native-rooms.md>)
- [docs/native-sheet-preview.md](<C:/fKey Labs/OpenStructure/docs/native-sheet-preview.md>)
- [docs/native-wall-types.md](<C:/fKey Labs/OpenStructure/docs/native-wall-types.md>)
- [docs/plugin-api.md](<C:/fKey Labs/OpenStructure/docs/plugin-api.md>)
- [docs/room-materials-implementation-report.md](<C:/fKey Labs/OpenStructure/docs/room-materials-implementation-report.md>)
- [fixtures/wasm-probe/plugin.toml](<C:/fKey Labs/OpenStructure/fixtures/wasm-probe/plugin.toml>)
- [fixtures/wasm-probe/probe.wat](<C:/fKey Labs/OpenStructure/fixtures/wasm-probe/probe.wat>)
- [plugins/walls/plugin.toml](<C:/fKey Labs/OpenStructure/plugins/walls/plugin.toml>)
- [plugins/walls/src/lib.rs](<C:/fKey Labs/OpenStructure/plugins/walls/src/lib.rs>)
