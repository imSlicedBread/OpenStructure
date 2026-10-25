# Native section-detail callouts

Schema 75 / native Model API 56 adds a distinct `section_callouts` map. Plan
callout wire semantics are unchanged. Container 2, generic plugin API 2,
opening family 9 and opening-type package 9 are unchanged.

Open a native Section, choose **Section callouts**, enter a name and scale,
then **Draw rectangular callout**. Drag a station/elevation rectangle; release
creates the stable marker and enlarged Section in one transaction. The target
copies start/end, side, CutOnly/CutAndProjection mode, far depth and marker level.
Its station crop, elevation crop and scale are independent. Geometry comes from
the existing section pipeline and authoritative model; annotations are not copied.
Existing section dimensions can be authored directly in the detail.

Choose a compatible existing Section in the same panel for a reference-only
marker. Creation and retargeting require the same authored plane start/end,
side, mode, depth and marker level. Rotated and reversed section axes are
supported; x always means station along the authored start-to-end axis and y
means absolute project elevation, never plan XY.

Compatibility is a creation/retarget guard, not a dependency constraint. After
creation, changing either view's plane, side, mode, depth, crop or scale does
not cascade into the other view and does not invalidate the project. Markers
remain in their parent's current station/elevation coordinates. This bounded
independent-view lifecycle can intentionally diverge from the target's geometry;
review and repair the marker or target when changing a section plane. Persisted
validation requires distinct native Sections on the same marker level, valid
bounds, a unique owner per target, acyclic ownership and valid explicit
viewport references.

Select a marker to edit bounds, move it, repair a reference-only target, choose
an explicit destination viewport, or open parent, target or referenced sheet.
**Edit target crop** opens the existing Section settings editor, including
station/elevation limits and scale. Removing a marker keeps its target.
Drafts do not modify model/history; Escape, focus/pointer loss, navigation,
camera/layout changes and stale document revisions discard the draft.

Labels resolve through the shared sheet resolver: `detail / sheet`, `Unplaced`,
or `Choose destination`. Explicit viewport UUIDs disambiguate multiple
placements. Renumbering or transferring that viewport updates the label.
Deletion of a resolved placement requires deliberate repair/removal. Issue
preflight rejects unresolved references and destinations omitted from the issue.
The native canvas, paper sheet and vector PDF consume the same marker graphics.

Migration 74→75 requires the new map to be absent in old input, adds an empty
map, advances native headers, and preserves other JSON values. Current input
requires the map. Nested bounds reject unknown point coordinates. Frozen
schema-74 evidence is in `os-storage/tests/fixtures/schema-74-section-callouts.json`.

## Validation

Focused tests cover model compatibility and later independent view edits;
strict bounds, ownership and destination validation; marker/target atomic
creation and undo/redo; save/reopen; destination renumber/transfer and issue
membership; real egui profiles at 1280×800/100% and 1000×650/150%, including
rotated/reversed axes and both section display modes; and shared section
geometry for typed doors/windows with frames, side lites, accessory extrusions
and instance pins. Render tests compare marker paths/labels with sheet and PDF
coordinates. No full-workspace test run was performed for this increment.

Commands below used `--locked --offline`; test invocations used
`-- --test-threads=1`. The final test runs used `./tools/cargo.ps1` with
workspace-local `TMP`/`TEMP` pointing to `work/issue-test-temp`. In PowerShell,
the wrapper received arguments as an array so the literal `--` was preserved.

| Focused command arguments | Result |
| --- | --- |
| `test -p os-model -p os-document -p os-storage --test section_callouts --test callouts` | 17 passed at that run, including existing plan-callout regressions and save/reopen |
| `test -p os-document --test section_callouts` | Final rerun: 4 passed, including the added native detail-dimension/annotation-ownership test |
| `test -p os-ui --all-features --lib callout` | 7 passed: 3 existing plan UI tests, 3 section UI tests, 1 typed-opening detail-output test |
| `test -p os-render --test section_callouts` | 1 passed: exact marker sheet/PDF path coordinates and live label |
| `test -p os-storage --test dimensions --test room_materials --test drawing_issues --test section_v2 --test section_dimensions` | 13 passed: selected legacy migration, archive and issue regressions |
| `clippy -p os-model -p os-document -p os-storage -p os-render -p os-ui -p os-plugin-api --all-features --lib -- -D warnings` | Passed; library scope only |
| `rustfmt --edition 2024 --config skip_children=true` with the explicit changed implementation/new-test file list | Completed successfully; two later test-only assertion/fixture edits were not separately format-checked |

An initial save/reopen attempt failed on Windows sandbox temporary-file
persistence. It passed with native access and again under the sandbox with
workspace-local temporary files. Sandbox builds also reported incremental-cache
hard-link fallback warnings. The dimension fixture initially placed its anchors
outside the detail crop; the corrected crop passed the final document test run.

Coordinator follow-up: all-feature/all-target tests for `os-model`,
`os-document`, `os-storage`, `os-render`, `os-ui`, and `os-plugin-api` passed.
Warnings-denied Clippy passed for those crates' libraries. The full-workspace run
stopped at seven `os-plugin-host` Wasm tests that failed to load/start plugins
with sandbox `Access is denied`; no section-callout test failed. All-target
Clippy and a final formatter check were blocked because the sandbox denied
canonicalizing the workspace/config path. The focused rustfmt run preceded the
last test-fixture edits, so those final edits were not independently format-
checked here. No further permission escalation was performed.

## Limits

This is a bounded native Section detail workflow, not Revit parity. No
elevation/facade callouts, plan-to-section creation, general dependent views,
annotation duplication, filled/masking regions, external-model references,
CAD/printing or new opening geometry are included. Marker bounds and target
crop are intentionally edited independently. Reference selection uses exact
authored plane compatibility rather than remapping between equivalent axes.
