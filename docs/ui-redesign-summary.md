# OpenStructure UI redesign — implementation summary

Implemented a light, Revit-inspired BIM workspace using OpenStructure's own
identity and original icons. The application remains a native Rust desktop app
built with egui/eframe.

The complete layout, visual tokens, interaction rules, and component states are
documented in the [UI design guide](ui-design-guide.md).

## Implemented changes

- Light theme with white palettes, an off-white canvas, blue accents, compact
  controls, and original labeled line icons.
- Quick-access Save, Undo, and Redo controls, project name, and unsaved indicator.
- Architecture, View, and Manage ribbon tabs, plus a contextual Modify | Walls
  tab for the selected wall.
- File menu with an editable `.osb` path, New, Open, Save, and disabled IFC
  exchange. The menu stays open while the path is edited.
- Properties above Project Browser in one resizable left column, with a draggable
  divider and independent scrolling.
- Grouped wall properties and a fixed Create wall/Apply changes action beneath
  the scrolling fields.
- Browser organization for the project, 3D view, levels, and walls; synchronized
  browser and viewport wall selection.
- A central 3D canvas with a view tab, Fit model control, navigation hints, and
  empty-project guidance.
- Status feedback with active level, units, wall count, and expandable diagnostics.
- Updated first-session instructions in the [README](../README.md).

## Behavior and compatibility

Wall fields remain drafts until Create wall or Apply changes. Existing validation,
geometry regeneration, undo/redo, file replacement confirmation, unsaved-document
prompts, and failed-open recovery are preserved. Commands continue to use the
existing Editor and document transactions.

Presentation is separated into theme, ribbon, palettes, and viewport modules in
`os-ui`. The theme is applied at application startup. Existing model types,
controller APIs, plugin protocols, and `.osb` files remain compatible; no storage
migration is required.

## Verification

The implementation session completed these checks:

| Check | Result |
| --- | --- |
| Workspace tests | 34 passed: 33 unit/integration tests and 1 documentation test |
| Workspace desktop build | Passed |
| Clippy with warnings denied | Passed |
| Formatting check | Passed |
| Executable smoke workflow | Passed; final fixture: `outputs/ui-final-smoke.osb` |
| Native visual inspection | 1280 × 800 and 1000 × 650 layouts inspected |
| Scale coverage | egui layout/input tests at 100%, 125%, and 150% |

Interaction tests cover wall creation and editing, selection, level management,
project renaming, deletion and undo, save/reopen, invalid drafts, file
confirmations, property scrolling, and both palette resize handles.

Scale tests use egui inputs. Windows display scaling itself was not changed;
native compositor behavior at 125% and 150% was not separately verified.

## First-release limits

Direct wall drawing, snapping, additional view types, floating palettes, new BIM
tools, and IFC exchange remain outside this redesign. Palette sizing is local to
the current session.

## Run locally

From the repository root in PowerShell:

```powershell
.\tools\cargo.ps1 run -p os-app
```

To reproduce the automated checks:

```powershell
.\tools\cargo.ps1 build --workspace --locked
.\tools\cargo.ps1 test --workspace --all-features --locked
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
```
