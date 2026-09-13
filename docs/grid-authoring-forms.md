# Grid authoring forms

Follow-up: [native inspection](native-grid-authoring.md) now verifies create,
coordinate edit/undo/redo, invalid Apply/Cancel, grid-snapped wall, selection and
save/reopen in the actual desktop. The headless evidence below remains distinct.

The native desktop now provides **New grid** beside the plan wall tool and
**Edit grid** in a selected grid's Properties. This extends the persistent
[grid model](architectural-grids.md) and [plan integration](grid-plan-integration.md).
It is a numeric authoring workflow, not yet grid pointer placement/drag editing.

## Supported workflow

Open a floor plan and choose New grid. The grid belongs to that plan level's
building. A collision-free `Grid N` label is proposed; default endpoints span
six metres around the plan basis origin along its X direction. All four fields
explicitly show model XY metres, even in a rotated plan. Preview shows the true
extent length and a normalized direction sketch labeled not-to-scale. A crop
may hide the result; the form warns about this rather than changing the crop.

Apply creates one stable grid and selects it in Properties/Browser. Edit grid
changes its name and exact endpoints with the same identity/header/building.
Existing grids can also be edited from Browser with 3D active. Rehoming a grid
to another building and deletion controls are not part of this form.

Draft strings remain outside the document. Finite numbers, nonzero extents,
trimmed bounded names, unique building labels and references are checked before
commit. Errors retain the draft. Apply uses a single validated Document command;
exact no-ops preserve redo. Cancel or Escape discards the draft. Document session,
revision and active-view changes reject stale Apply. Global undo/redo/save keys
are blocked while the modal is open. Opening a grid form cancels wall preview.

## Regression evidence

Windows checkpoint: all 211 tests passed with `test --workspace --all-features
--locked --offline --target-dir work/completion-build --quiet`. Strict Clippy
passed with `--workspace --all-targets --all-features --locked --offline
--target-dir work/completion-build -- -D warnings` after moving the test module
below implementation items. Commands use `tools/cargo.ps1`; no dependency,
storage schema or plugin protocol change was introduced.

- `grid_tools::tests::create_edit_invalid_values_and_stale_grid_drafts_are_atomic`:
  incomplete/nonfinite/overflow/zero-length rejection; no model/history mutation;
  exact 3–4–5 extent; one create/edit; identity preservation; replay, view, undo
  and new-session rejection; no-op redo retention.
- `grid_tools::tests::naming_and_duplicate_rejection_are_building_scoped`:
  proposed labels, duplicate rejection and existing-datum editing from 3D.
- `desktop_tests::grid_forms_create_edit_cancel_and_keep_footer_visible`: actual
  egui events at 1280×800 and 1000×650, scales 1/1.25/1.5; visible footer actions,
  Ctrl-Z isolation, invalid-name rejection, Cancel, create, rename and coordinate
  change with 7 m preview, unchanged identity, Undo and Escape.
- `desktop_tests::plan_grid_selection_and_wall_axis_snap_use_semantic_datum` now
  creates its grid through New grid/Apply before picking it and drawing a wall
  from the grid axis. It no longer injects a grid through test-only commands.

These are headless tests through the real frame/input path, not an observed
native-window acceptance run. Grid save/reopen and rotated/building-scoped plan
derivation retain their separate storage/controller tests. A native workflow
inspection, grid pointer editing, remaining wall move/resize/offset operations
and independently installed plan services remain required in D. Production
E1–E4/G1–G6 scope and undecided qualification targets are unchanged.
