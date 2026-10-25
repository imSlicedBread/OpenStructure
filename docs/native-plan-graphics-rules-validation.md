# V02.03 implementation and validation handoff

Workspace: `E:\fkey labs second drive\OpenStructure`. All build/temp/log output
for this increment uses the new `target-rule-graphics` directory on E:.
The full Revit-parity goal remains open; the implemented contract and exclusions
are in [native rule-based plan graphics](native-plan-graphics-rules.md).

## Behavior changed

- Required schema-83 ordered rule definitions with strict typed predicates, bounded
  validation, duplicate rejection and no persisted match IDs.
- Current resolved wall/opening values and slab/phase properties determine matches.
  Highest-priority matching stroke per role wins above phase and below explicit overrides.
- Atomic replacement, history accounting, deleted/incompatible-owner cleanup and
  independent duplicate/callout definitions with fresh IDs. Project limits use
  the final owner graph, including transactions that replace a deleted owner.
- Revision-bound settings editor with creation/removal, enable, priority,
  category, predicate and stroke controls.
- Shared snapshot/drawing resolution for canvas, sheets and vector PDF; generic
  provider graphics remain unstyled, and rule edits invalidate in-flight results.
- Strict 82→83 migration, API-63 compatibility diagnostics/manifests and independent
  Wasm probe. Generic API 2, container 2 and family/package 10 remain unchanged.

## Executed checks

Run from the workspace above with:

```powershell
$env:CARGO_TARGET_DIR = 'E:\fkey labs second drive\OpenStructure\target-rule-graphics'
$env:TEMP = 'E:\fkey labs second drive\OpenStructure\target-rule-graphics\tmp'
$env:TMP = $env:TEMP
```

| Exact command | Result |
| --- | --- |
| `cargo check --offline -p os-ui --all-features` | Passed. |
| `cargo test --offline -p os-model -p os-document -p os-storage --test plan_graphics_rules` | 9 passed: 3 model, 3 document, 3 storage. |
| `cargo test --offline -p os-ui --all-features --lib rule_` | 3 passed, 606 filtered. Includes real egui at 1280×800/100% and 1000×650/150%, priority editing, Escape, undo/redo and shared paper/PDF output. |
| `cargo test --offline -p os-ui --all-features --test provider_plans` | 5 passed, including new rule/provider-isolation and stale-response acceptance. |
| `cargo test --offline -p os-render --test plan_graphics` | 1 passed. |
| `cargo test --offline -p os-storage --all-features --no-fail-fast` | 209 passed across library, integration and doc-test targets. Historical fixture helpers were corrected to represent their declared schema. |
| `cargo test --offline -p os-plugin-api -p os-plugin-host -p os-walls --all-features --no-fail-fast` | 76 passed, 1 failed. Native API63, Wasm transport/probe (26 tests), plan graphics and Walls passed. The failure was the generic worker stale-reply case below. |
| `cargo test --offline -p os-plugin-host --all-features --test generic_commands edits_undo_reopen_and_document_switch_revoke_generic_reply -- --test-threads=1` | 1 passed, 14 filtered. The matrix failure is consistent with timing sensitivity; this isolated pass does not prove its precise cause. Generic worker code was not changed. |
| `cargo clippy --offline -p os-model -p os-document -p os-plugin-api --all-features --lib -- -D warnings` | Passed. |
| `cargo clippy --offline -p os-ui --all-features --lib -- -D warnings` | Blocked by existing dependency lints: suspicious assignment formatting in os-storage/src/opening_type_package.rs:88; collapsible if in os-storage/src/lib.rs:460 and os-render/src/sheet/annotations.rs:36. These unrelated lines were not changed. |

New Rust files passed this exact formatting check (shared dirty files were not
formatted wholesale):

```powershell
rustfmt --edition 2024 --check crates/os-model/src/plan_graphics_rules.rs crates/os-model/tests/plan_graphics_rules.rs crates/os-model/tests/support/plan_graphics_rules.rs crates/os-storage/tests/plan_graphics_rules.rs crates/os-document/tests/plan_graphics_rules.rs crates/os-ui/src/plan_settings/rules.rs crates/os-ui/src/plan_settings/rule_tests.rs crates/os-ui/src/desktop_tests/plan_rule_graphics_tests.rs
```

Logs: [final storage](../target-rule-graphics/rule-final-storage.log),
[plugin matrix](../target-rule-graphics/plugin-tests.log),
[UI Clippy](../target-rule-graphics/ui-clippy.log). Temporary button diagnostics
were removed. Full-workspace tests/lints are not claimed green.

## Files edited in this increment

This inventory identifies the scoped work; it is not a clean-baseline Git diff.
The existing checkout still reports “this operation must be run in a work tree”
for status/diff, so no Git repair, reset, clean, or unrelated formatting was done.

### Feature, integration, protocol and documentation files

- [docs/native-plan-graphics-rules-validation.md](native-plan-graphics-rules-validation.md) (this handoff).
- [README.md](../README.md)
- [crates/os-document/src/callouts.rs](../crates/os-document/src/callouts.rs)
- [crates/os-document/src/lib.rs](../crates/os-document/src/lib.rs)
- [crates/os-document/src/plan_duplication.rs](../crates/os-document/src/plan_duplication.rs)
- [crates/os-document/tests/plan_graphics_rules.rs](../crates/os-document/tests/plan_graphics_rules.rs)
- [crates/os-model/src/lib.rs](../crates/os-model/src/lib.rs)
- [crates/os-model/src/memory.rs](../crates/os-model/src/memory.rs)
- [crates/os-model/src/plan_graphics_rules.rs](../crates/os-model/src/plan_graphics_rules.rs)
- [crates/os-model/tests/plan_graphics_rules.rs](../crates/os-model/tests/plan_graphics_rules.rs)
- [crates/os-model/tests/support/plan_graphics_rules.rs](../crates/os-model/tests/support/plan_graphics_rules.rs)
- [crates/os-plugin-api/src/lib.rs](../crates/os-plugin-api/src/lib.rs)
- [crates/os-plugin-host/src/lib.rs](../crates/os-plugin-host/src/lib.rs)
- [crates/os-plugin-host/tests/wasm_transport.rs](../crates/os-plugin-host/tests/wasm_transport.rs)
- [crates/os-ui/src/desktop_tests/plan_rule_graphics_tests.rs](../crates/os-ui/src/desktop_tests/plan_rule_graphics_tests.rs)
- [crates/os-ui/src/lib.rs](../crates/os-ui/src/lib.rs)
- [crates/os-ui/src/plan.rs](../crates/os-ui/src/plan.rs)
- [crates/os-ui/src/plan_settings.rs](../crates/os-ui/src/plan_settings.rs)
- [crates/os-ui/src/plan_settings/element_overrides.rs](../crates/os-ui/src/plan_settings/element_overrides.rs)
- [crates/os-ui/src/plan_settings/rule_tests.rs](../crates/os-ui/src/plan_settings/rule_tests.rs)
- [crates/os-ui/src/plan_settings/rules.rs](../crates/os-ui/src/plan_settings/rules.rs)
- [crates/os-ui/tests/provider_plans.rs](../crates/os-ui/tests/provider_plans.rs)
- [docs/2d-coverage.md](../docs/2d-coverage.md)
- [docs/file-format.md](../docs/file-format.md)
- [docs/native-plan-element-overrides.md](../docs/native-plan-element-overrides.md)
- [docs/native-plan-graphics-rules.md](../docs/native-plan-graphics-rules.md)
- [docs/native-plan-graphics-templates.md](../docs/native-plan-graphics-templates.md)
- [docs/plugin-api.md](../docs/plugin-api.md)
- [fixtures/wasm-probe/plugin.toml](../fixtures/wasm-probe/plugin.toml)
- [fixtures/wasm-probe/probe.wat](../fixtures/wasm-probe/probe.wat)
- [plugins/walls/plugin.toml](../plugins/walls/plugin.toml)
- [plugins/walls/src/lib.rs](../plugins/walls/src/lib.rs)

### Storage migration and historical fixture files

- [crates/os-storage/src/lib.rs](../crates/os-storage/src/lib.rs)
- [crates/os-storage/src/tests/casework.rs](../crates/os-storage/src/tests/casework.rs)
- [crates/os-storage/src/tests/ramps.rs](../crates/os-storage/src/tests/ramps.rs)
- [crates/os-storage/src/tests/sheets.rs](../crates/os-storage/src/tests/sheets.rs)
- [crates/os-storage/src/tests/stairs.rs](../crates/os-storage/src/tests/stairs.rs)
- [crates/os-storage/tests/arc_dimensions.rs](../crates/os-storage/tests/arc_dimensions.rs)
- [crates/os-storage/tests/common/mod.rs](../crates/os-storage/tests/common/mod.rs)
- [crates/os-storage/tests/detail_lines.rs](../crates/os-storage/tests/detail_lines.rs)
- [crates/os-storage/tests/dimension_faces.rs](../crates/os-storage/tests/dimension_faces.rs)
- [crates/os-storage/tests/dimensions.rs](../crates/os-storage/tests/dimensions.rs)
- [crates/os-storage/tests/drafting.rs](../crates/os-storage/tests/drafting.rs)
- [crates/os-storage/tests/drawing_issues.rs](../crates/os-storage/tests/drawing_issues.rs)
- [crates/os-storage/tests/facades.rs](../crates/os-storage/tests/facades.rs)
- [crates/os-storage/tests/grids.rs](../crates/os-storage/tests/grids.rs)
- [crates/os-storage/tests/group_migration.rs](../crates/os-storage/tests/group_migration.rs)
- [crates/os-storage/tests/lite_handedness.rs](../crates/os-storage/tests/lite_handedness.rs)
- [crates/os-storage/tests/material_colors.rs](../crates/os-storage/tests/material_colors.rs)
- [crates/os-storage/tests/material_patterns.rs](../crates/os-storage/tests/material_patterns.rs)
- [crates/os-storage/tests/moving_sash_accessories.rs](../crates/os-storage/tests/moving_sash_accessories.rs)
- [crates/os-storage/tests/opening_dimensions.rs](../crates/os-storage/tests/opening_dimensions.rs)
- [crates/os-storage/tests/opening_materials.rs](../crates/os-storage/tests/opening_materials.rs)
- [crates/os-storage/tests/opening_visibility.rs](../crates/os-storage/tests/opening_visibility.rs)
- [crates/os-storage/tests/openings.rs](../crates/os-storage/tests/openings.rs)
- [crates/os-storage/tests/paired_leaf_accessories.rs](../crates/os-storage/tests/paired_leaf_accessories.rs)
- [crates/os-storage/tests/plan_graphics.rs](../crates/os-storage/tests/plan_graphics.rs)
- [crates/os-storage/tests/plan_graphics_rules.rs](../crates/os-storage/tests/plan_graphics_rules.rs)
- [crates/os-storage/tests/room_finishes.rs](../crates/os-storage/tests/room_finishes.rs)
- [crates/os-storage/tests/room_materials.rs](../crates/os-storage/tests/room_materials.rs)
- [crates/os-storage/tests/room_separation_lines.rs](../crates/os-storage/tests/room_separation_lines.rs)
- [crates/os-storage/tests/room_tags.rs](../crates/os-storage/tests/room_tags.rs)
- [crates/os-storage/tests/schedule_filters.rs](../crates/os-storage/tests/schedule_filters.rs)
- [crates/os-storage/tests/schedule_grouping.rs](../crates/os-storage/tests/schedule_grouping.rs)
- [crates/os-storage/tests/schedule_phases.rs](../crates/os-storage/tests/schedule_phases.rs)
- [crates/os-storage/tests/schedules.rs](../crates/os-storage/tests/schedules.rs)
- [crates/os-storage/tests/sections.rs](../crates/os-storage/tests/sections.rs)
- [crates/os-storage/tests/wall_arcs.rs](../crates/os-storage/tests/wall_arcs.rs)
- [crates/os-storage/tests/wall_joins.rs](../crates/os-storage/tests/wall_joins.rs)
- [crates/os-storage/tests/wall_types.rs](../crates/os-storage/tests/wall_types.rs)
- [crates/os-storage/tests/window_panes.rs](../crates/os-storage/tests/window_panes.rs)
- [crates/os-storage/tests/window_sills.rs](../crates/os-storage/tests/window_sills.rs)

The historical test changes add the new empty map to current-schema expectations
or remove it when constructing/reversing pre-83 payloads. The schema-69 material
pattern fixture also needed its existing post-69 maps and cut-pattern property
removed so its own ambiguity assertions exercise valid old input.

## Remaining scope

Partial V02.03 covers native FloorPlan Walls, Doors, Windows and Slabs with AND
text/dimension/phase predicates and complete strokes. OR/nesting, arbitrary
parameter or material/layer predicates, fills/visibility/transparency effects,
rule libraries, external-provider styling and other view/category types remain
excluded. Native-window visual inspection and physical printing were not
qualified by this automated egui/vector-output increment.
