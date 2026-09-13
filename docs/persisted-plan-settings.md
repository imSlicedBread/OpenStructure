# D: persisted named plan settings

Follow-up: [the native plan workspace](native-plan-workspace.md) now consumes
these settings through bounded background derivation. This page records the
earlier persistence checkpoint; [desktop settings editing](plan-settings-form.md) is now implemented.

Status: model/document/controller implementation with automated verification;
not a desktop floor-plan workflow or D completion.

`os-model/src/plans.rs` defines settings version 1. Native schema 3 saves each
plan's name, associated level, level-relative range, horizontal origin/yaw,
optional rectangular crop, scale denominator, wall/extension visibility and
host-managed settings revision. IDs remain stable. Defaults, bounds and migration
policy are documented in [file format](file-format.md) and
[ADR 0014](decisions/0014-persisted-plan-settings.md).

`Document` supports AddView, UpdateView and RemoveView atomically. Invalid settings,
dangling references, changed kind, exhausted revisions and removal of the last
view fail without changing history or model. Replacement in one batch is valid.
Editor `create_floor_plan` and `update_floor_plan` use these commands;
`native_plan_context(view)` and `native_wall_plan(view)` read persisted data.
Hiding walls affects plan output, not semantic elements or their 3D meshes.
Scale is stored/context-bound, not yet a paper rendering implementation.

## Evidence

- `os-document/tests/views.rs`: identity, edits, removals, replacement, undo/redo,
  no-op preservation of redo, invalid names/ranges/references and revision overflow.
- `os-storage/tests/plan_settings.rs`: frozen schema-2 plans in real ZIP files,
  opaque arbitrary-precision extension data and attachments preserved, source file
  unchanged, current save/reopen, ambiguous/future fields rejected, legacy
  unassigned plans preserved until explicit assignment.
- `os-storage` unit test: counting serialized size matches the native writer's
  bytes exactly, rejecting over-budget models without another encoded buffer.
- `os-ui/tests/plan.rs`: changed settings survive save/reopen; hidden walls remain
  in model/3D; invalid edits preserve dirty state and redo; earlier drawings fail
  after reopening. Existing shared wall/plan geometry tests remain in place.
- `os-render/tests/plan.rs`: context comparison includes scale and visibility.
  `os-geometry/tests/plan.rs` rejects extreme basis coordinates that collapse a
  valid footprint instead of silently publishing an empty drawing.

Run `cargo test --workspace --all-features --locked --offline --target-dir work/completion-build`,
strict all-target/all-feature Clippy, formatting and the default workspace build.
No dependencies or unsafe code were added. This is local Windows development
evidence; no hosted CI or production qualification is implied.

Local checkpoint: the full all-feature workspace suite, strict Clippy, formatting
and default workspace build pass. Existing installed API-2 Wall/column Editor
probes and the staged single/mixed-type migration probe pass against schema 3
without rebuilding the guests. Normal executable smoke passes and created
`outputs/persisted-plans-verified.osb`. These probes exercise controller/storage
compatibility, not new native desktop plan controls.

Next: visible named-plan controls and a linked plan/3D workspace with bounded
background derivation, pan/zoom/fit and shared selection, followed by snapping,
transactional gestures and independently installed callable plan providers.
The current controller remains synchronous and is not called from desktop frames.
