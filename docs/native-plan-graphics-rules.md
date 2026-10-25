# Native rule-based plan graphics (partial V02.03)

Model schema 83 / native full-model API 63 adds required `plan_graphics_rules`.
It maps authored FloorPlan view UUIDs to ordered, nonempty rule lists. Rules
contain predicates, never saved matching element IDs. Templates and explicit
per-element overrides remain separate. Container 2, generic API 2, PlanSettings,
and opening family/package 10 are unchanged.

## Rules and evaluation

Each rule has a stable UUID, name, enabled flag, one category (Walls, Doors,
Windows, Slabs), 1–8 AND predicates, and optional complete cut/projected strokes.
At least one stroke is required. Each stroke specifies RGB, 0.05–2 mm paper
weight and solid/dashed pattern. Slabs support projected strokes only.

| Predicate | Categories | Operators |
| --- | --- | --- |
| Instance name | All four | Equals, NotEquals, Contains |
| Effective type name | Walls, Doors, Windows | Equals, NotEquals, Contains |
| Phase status at target phase | All four | Equals, NotEquals |
| Wall length, height, effective thickness | Walls | =, !=, <, <=, >, >= |
| Effective width, height, sill | Doors, Windows | =, !=, <, <=, >, >= |
| Thickness, top offset | Slabs | =, !=, <, <=, >, >= |

Text is case-sensitive and literal; a missing effective type name never matches,
including NotEquals. Numeric operands are finite metres compared directly without
a tolerance. Wall values come from `resolve_wall`; opening values and type names
come from `resolve_opening`, including compound layers, formulas, defaults and
instance pins. Wall length follows its analytic path, including supported arcs.

The shared snapshot builder evaluates the current committed model. Precedence is
category/linked template → phase → rules → explicit per-element override.
Highest-priority rules appear first. For each element and role, the first enabled
matching rule supplying that role wins; omitted roles continue down the list and
then inherit phase/category appearance. Projected strokes also cover Depth.
Selection highlighting remains a canvas overlay.

Resolved styles are frozen into the existing revision-bound snapshot. Existing
session, document and settings context checks reject stale drawing results.
Only native geometry surviving visibility, phase, crop and range receives the
style. Door/window rules style symbols; aperture edges retain their wall owner.
Canvas, sheet preview and vector PDF consume the same resolved PlanDrawing.
The evaluator is bounded to 10,000 native candidates × 32 rules × 8 predicates;
effective values are resolved once per candidate rather than per predicate.

## Persistence and editing

Limits: 32 rules/view, 256/project; names and text operands are at most 256 UTF-8
bytes without controls, and names must be nonblank. Tagged serde rejects unknown
fields/variants. Nil/duplicate rule IDs, duplicate view keys, empty saved entries,
incompatible fields/categories, nonfinite numbers and invalid effects are rejected.
Disabled rules receive the same validation. Migration 82→83 inserts the empty map
and advances native headers atomically; any pre-83 occurrence of the new map,
including empty/null, is rejected. Current-schema wire requires the field.

`SetPlanGraphicsRules` replaces one view's list atomically and validates the final
transaction graph before cleanup. Empty replacements remove the entry. Rules
survive zero matches, hidden/cropped elements, releveling and element deletion.
Deleted/incompatible owner views lose their rule entry. Plain/detailing duplicates
and enlarged-plan callouts copy definitions by value with fresh row IDs.

Plan settings includes a separate “Rule-based graphics” section with add/remove,
enable, Up/Down priority, category, typed condition and stroke editors. Apply uses
the existing single undoable settings transaction. Cancel/Escape and stale drafts
leave saved state unchanged. Template changes/unlinking preserve rules.

## Acceptance and boundaries

Focused evidence is in `os-model/tests/plan_graphics_rules.rs`,
`os-storage/tests/plan_graphics_rules.rs`, `os-document/tests/plan_graphics_rules.rs`,
`os-ui/src/plan_settings/rule_tests.rs` and
`os-ui/src/desktop_tests/plan_rule_graphics_tests.rs`. These cover live dimensions,
wire rejection, migrations, ordering/inheritance, history, copying, settings and
shared canvas/paper/PDF output. See the
[validation handoff](native-plan-graphics-rules-validation.md) for exact executed
commands/results and the changed-file inventory.

This is partial V02.03: OR/nested expressions, regex/scripts, arbitrary parameter
paths, material/layer predicates, visibility/fills/transparency effects, reusable
template-owned rule libraries, external-provider graphics, other categories and
other view types remain excluded. Native-window/manual visual and physical-print
qualification are separate from the automated egui acceptance tests.
