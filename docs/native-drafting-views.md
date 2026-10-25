# Standalone Drafting views

Schema 78 / native Model API 59 adds `ViewKind::Drafting`. Each view owns a
required rectangular crop and scale (1:1–1:10000), with drafting XY coordinates
in metres. It has no level, Plan/Section/Elevation/Facade settings, section
plane, model projection or 3D split. Opening family/package remain 10; generic
API 2, container 2 and issue bundle format remain unchanged.

Choose **New drafting view** above the drawing canvas. Select it in the named
view picker, then use **Drafting settings** to rename it or edit crop/scale.
Settings stay in an immutable-document draft until Apply; Cancel/Escape,
focus loss, changed active view/session or stale revision discard the draft.
Apply is one undoable command. The crop must have finite coordinates within
±1000000 m and positive spans greater than 0.000001 m.

Existing **Detail Line** and **Text note** tools work in the view. Lines retain
two-click creation, endpoint/body movement, deletion, selection, snapping,
crop clipping and undo/redo. Notes reuse bounded ASCII multiline editing,
paper-unit sizes, alignment and one optional leader. Snaps come only from the
view's detail lines. No model, provider or foreign-view annotation participates
in rendering, picking or snapping. Notes remain pickable through shared layout.
See [detail drafting](native-detail-drafting.md) for the bounded note contract.

The frozen snapshot holds only its context and copies of owned lines/notes;
derivation never requests model or provider geometry. The existing sheet source
cache and vector PDF composer consume it. Viewports may override scale while
preserving paper-size note layout. Drafting sheets participate in ordered sets
and frozen drawing issues, including archive verification and native reopen.

**Delete drafting view** never cascades annotations. Document commands reject
deletion while any owned line/note, live sheet viewport or frozen issue manifest
references the view. Remove annotations and live placements explicitly first;
an issued manifest still prevents deleting its source view. Rejections preserve
the model and undo history.

## Compatibility

Migration 77→78 advances headers while preserving existing view parameters,
UUIDs and opaque payloads. Any `drafting` field (even null) or Drafting kind in
schema-77 input is rejected atomically. Current Drafting views require valid
settings and reject level/model settings. Other view kinds reject non-null
Drafting settings. The frozen fixture is
`crates/os-storage/tests/fixtures/schema-77-drafting-views.json`.

Native providers require API 59/schema 78. Wall/probe manifests, the probe WAT,
transport fixtures and API mismatch diagnostics use the same versions. This
increment occupies 78/59 after the typed-formula 77/58 increment. Future annotation
crop or hatch work must allocate a later schema/API change and update the current headers in
`file-format.md`, `plugin-api.md`, README and the 2D coverage ledger separately.

## Focused evidence

All Cargo commands use `./tools/cargo.ps1`, `--locked --offline`, and PowerShell
argument arrays. Test arguments are `-- --test-threads=1`; TMP/TEMP point to
workspace-local `work/issue-test-temp`. No full-workspace suite was run.

| Scope | Result |
| --- | --- |
| Model/document/storage `--test drafting_views` | 4 passed: settings, strict migration, stable identity, preview/history, deletion guards and native roundtrip |
| UI `--all-features --lib drafting` plus focused issue rerun | 13 passed across final focused runs: 5 existing Section cases, 6 standalone line/output cases, settings lifecycle and ordered issue/archive/reopen/deletion guard |
| UI `--all-features --lib text_note` | 3 passed; Plan, Section and Drafting loops include scales 1:25/1:100, canvas glyph anchors, CRUD/history, cancellation/stale/text-focus and camera/canvas guards |
| UI pointer ownership rerun after adding inspector deletion/undo | 4 passed across final focused runs: standalone Drafting and existing Plan/Section/separation-line regressions |
| Plugin API `--lib` | 6 passed, including incompatible native versions |
| UI `check --all-features` | Passed |

Real-egui profiles are 1280×800 at 100% and 1000×650 at 150%. Output checks
assert no model picking/snapping, owned-line crop clipping, immutable snapshot
contents, shared paper/PDF glyph anchors at both scales and vector output.
The ordered issue check verifies page order, archive bytes, reopen and the
manifest-only deletion guard after live references have been removed.

The frozen fixture initially lacked family-10 extrusion bindings; correcting
the fixture made migration pass. The issue fixture needed its initial session
poll before setting active view/sheet, matching desktop lifecycle.
The deletion interaction test uses the existing inspector button; detail lines
do not currently have a Delete-key shortcut.

Rustfmt and scoped UI-library Clippy stopped with Windows `Access is denied`
(Clippy configuration discovery). Neither is verified; no escalation was used.
Builds also report incremental hard-link fallback/canonicalization warnings.

V01.13, A02.01 and A01.28 remain partial. No duplicate/dependent views, fills or
masks, reusable detail components, rich text, CAD interchange, native visual
acceptance or physical-print qualification is claimed.

## Drafting increment file manifest

Paths below are repository-relative. These identify this increment's edits;
many files also contain preserved, pre-existing work from other increments.

New files:

- `crates/os-model/src/drafting.rs`
- `crates/os-model/tests/drafting_views.rs`
- `crates/os-document/tests/drafting_views.rs`
- `crates/os-storage/tests/drafting_views.rs`
- `crates/os-storage/tests/fixtures/schema-77-drafting-views.json`
- `crates/os-ui/src/plan/drafting.rs`
- `crates/os-ui/src/plan_workspace/drafting_views.rs`
- `crates/os-ui/src/plan_workspace/standalone_drafting_tests.rs`
- `docs/native-drafting-views.md`

Runtime, integration, validation and documentation edits:

- `crates/os-model/src/lib.rs`
- `crates/os-model/src/memory.rs`
- `crates/os-model/src/detail_lines.rs`
- `crates/os-model/src/text_notes.rs`
- `crates/os-model/src/sheets.rs`
- `crates/os-document/src/lib.rs`
- `crates/os-storage/src/lib.rs`
- `crates/os-storage/tests/room_materials.rs`
- `crates/os-plugin-api/src/lib.rs`
- `crates/os-plugin-host/tests/wasm_transport.rs`
- `plugins/walls/plugin.toml`
- `fixtures/wasm-probe/plugin.toml`
- `fixtures/wasm-probe/probe.wat`
- `crates/os-ui/src/plan.rs`
- `crates/os-ui/src/plan_workspace.rs`
- `crates/os-ui/src/plan_workspace/detail_lines.rs`
- `crates/os-ui/src/plan_workspace/text_notes.rs`
- `crates/os-ui/src/plan_workspace/text_note_tests.rs`
- `crates/os-ui/src/plan_workspace/sheet_layout.rs`
- `crates/os-ui/src/plan_workspace/drawing_issues/tests.rs`
- `README.md`
- `docs/file-format.md`
- `docs/plugin-api.md`
- `docs/2d-coverage.md`
- `docs/native-detail-drafting.md`

Additional existing ViewParams constructors updated with `drafting: None`:

- `crates/os-constraints/tests/sections.rs`
- `crates/os-document/src/tests/openings.rs`
- `crates/os-document/tests/elevations.rs`
- `crates/os-document/tests/facades.rs`
- `crates/os-document/tests/sections.rs`
- `crates/os-model/src/sections.rs`
- `crates/os-model/tests/elevation_dimensions.rs`
- `crates/os-model/tests/elevations.rs`
- `crates/os-model/tests/facades.rs`
- `crates/os-model/tests/section_dimensions.rs`
- `crates/os-storage/tests/elevations.rs`
- `crates/os-storage/tests/facades.rs`
- `crates/os-storage/tests/sections.rs`
- `crates/os-storage/tests/sheet_tables.rs`
- `crates/os-ui/src/plan/elevations.rs`
- `crates/os-ui/src/plan/facades.rs`
- `crates/os-ui/src/plan/curtain_section_tests.rs`
- `crates/os-ui/src/plan/railing_drawing_tests.rs`
