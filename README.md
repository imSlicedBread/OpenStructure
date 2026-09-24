# OpenStructure

OpenStructure is an independent, open-source, plugin-friendly BIM authoring platform built with Rust.

The project aims to provide a practical alternative for parametric building design, documentation, and openBIM interoperability. It is not affiliated with Autodesk or Revit and does not use proprietary Autodesk code, assets, formats, or branding.

## Current status

OpenStructure has a 13-package Rust workspace with a native egui/eframe desktop,
typed semantic model, atomic edits and bounded snapshot undo/redo, versioned
`.osb` storage, restricted IFC4 wall and hosted door/window exchange, and a depth-buffered 3D viewport
with matching entity selection.

The [B/C development baseline](docs/bc-baseline-audit.md) is verified on Windows.
The optional `external-plugins` build supports explicit directory installation,
a minimal plugin manager, a bounded Wasm runtime, descriptor-driven tools,
background commands/geometry, cancellation/stale-result protection, and staged
project opening. Independently built [Rust Wall](examples/rust-wall/README.md)
and [Rust column](examples/rust-column/README.md) examples exercise authoring,
geometry, history, native storage and missing-provider preservation without
rebuilding the host after installation. Native workflows are recorded for
[Wall](docs/native-wall-workflow.md) and [column](docs/native-column-workflow.md).

[Plugin-owned migrations](docs/plugin-migration-service.md) use isolated candidate
batches and one final validated transaction; independent single/mixed-type examples
cover cancellation, late failures, undo/redo and persistence. Container 2 preserves
bounded opaque auxiliary files; model schema 28 preserves plugin envelopes, exact
provider requirements and versioned named plan settings. Opening a file never
installs or automatically executes a plugin from that file.

Model schema 5 added [native hosted doors and windows](docs/native-hosted-openings.md)
for straight walls, with repeatable click-to-place previews and real 3D/plan
apertures. Schema 8 adds reusable project door/window types with type-shared
dimensions, typed placement, legacy conversion and atomic regeneration. Model
schema 6 added [native wall-bounded
rooms](docs/native-rooms.md) with derived centerline areas, plan labels,
enclosure diagnostics, and stable topology identity.
Level-owned straight room-separation lines now participate in room enclosure
and plan authoring. Schema 24 introduced a visual profile-extrusion editor for
reusable door/window types; one bounded component profile drives the 3D panel/pane
and plan-cut symbol. Schema 25 adds an optional editable frame around the inset
panel/pane. Hosted voids are still rectangular, and advanced family
operations, nesting, formulas and content libraries remain outside the implemented
scope. Curved or plugin-defined opening hosts are also unsupported.

Schema 11 extends native wall-endpoint dimensions with **Chain** and
**Baseline** layouts. One reporting annotation retains ordered wall endpoints;
chains measure consecutive segments, while baselines measure each later point
from the first on successive parallel lines. This remains a focused native
subset, not Revit-equivalent dimensioning.

Schema 10 adds per-door hinge jamb and swing side, a pickable quarter-circle plan
arc and a matching open 3D leaf. Existing and new doors default to Start / Left
(positive host normal), preserving the previous leaf geometry through schema-9
migration. Exact opening properties preview and apply these instance settings in
one undoable edit. Windows, shared dimensions and wall apertures are unchanged;
orientation follows the wall's stored start→end direction.

Schema 9 adds [native floors/slabs](docs/native-floor-slabs.md): straight-edged
concave boundaries, level-relative elevation, thickness, plan fill/picking,
split-view 3D extrusion, and one-transaction undo/redo. Holes, slopes, layered
assemblies, joins, floor property editing, and IFC slab exchange remain future work.

Schema 7 introduced [native aligned dimensions](docs/native-dimensions.md): a
three-click plan tool, live wall-endpoint measurements, editable offsets and
visible orphan diagnostics. Schema 11 adds chain and baseline annotations.
Driving constraints, reference repair and print-faithful annotation remain
future work.

[History retention](docs/history-retention.md) now has measured synthetic cost,
entry/estimated-byte limits and atomic over-budget rejection. The estimate is not
a total application-memory guarantee. [IFC4 exchange](docs/ifc-roadmap.md)
has independent validation and native desktop controls, with explicit subset/loss
limits.

D linked floor-plan authoring is in progress, starting with the tested
[cut/projection and semantic drawing foundation](docs/plan-geometry-foundation.md)
and native [floor/slab sketch workflow](docs/native-floor-slabs.md).
[Named plan commands and persisted settings](docs/persisted-plan-settings.md)
now support migration, undo/redo and save/reopen at the model/controller layer.
[The native plan/split workspace](docs/native-plan-workspace.md) now supports
plan creation/selection, background native-wall drawings, fit/pan/zoom and shared
selection with 3D. [Plan-settings forms](docs/plan-settings-form.md) now edit names,
ranges, basis, crop, scale and visibility as one validated transaction.
[Semantic snap queries](docs/semantic-plan-snapping.md) are available to the
controller. [Two-point wall drawing](docs/plan-wall-gestures.md) now uses snap
feedback and exact length/angle drafts with the bundled Wall provider.
[Finite-line intersection snapping](docs/plan-intersection-snapping.md) also
supports starting walls at checked semantic crossings.
[Perpendicular snapping](docs/plan-perpendicular-snapping.md) projects the current
gesture anchor onto finite semantic lines with explicit acquisition checks.
Optional [axis extensions](docs/plan-axis-extension-snapping.md) acquire beyond
finite endpoints without changing model geometry or picking.
[Signed wall offsets](docs/plan-wall-offset.md) preview and create parallel wall
copies with exact metre input and one-step undo.
[Native verification](docs/native-pointer-walls.md) covers two connected exact-input
walls in split view, cancellation, save/reopen and shared selection.
[Architectural grid entities](docs/architectural-grids.md) now have persistent
identity, validated commands, history and migration. [Grid plan integration](docs/grid-plan-integration.md)
adds clipped datums, selection and grid-axis snapping for the bundled wall gesture.
[Grid create/edit forms](docs/grid-authoring-forms.md) now provide exact coordinates,
previews and transactional Apply/Cancel. [Native grid verification](docs/native-grid-authoring.md)
covers editing, grid-snapped walls and save/reopen. [Wall move/resize click tools](docs/plan-wall-edit-gestures.md)
now preserve identity with transient previews and single-step undo.
[Installed column plan integration](docs/native-column-plan.md) now displays
independent provider outlines with Fit plan, shared selection, form edits,
undo/redo and save/reopen. Independent pointer authoring remains unfinished.
[Native edit inspection](docs/native-wall-edits.md) verifies split updates and
save/reopen. The discovered [untitled quick-Save path defect](docs/untitled-save-destination.md)
is corrected with destination prompting and regression tests. Drag handles,
independent plugin pointer authoring, architectural modeling and
the E1–E4 production drawing/team workflows remain unfinished. The
[coverage ledger](docs/2d-coverage.md) tracks them; this is not a production BIM
system or a general architecture-firm release. Production deployment targets,
license and publication decisions remain open. Historical slice reports retain
their original evidence; use the B/C audit and coverage ledger for current status.

## Build and run

Install stable Rust and the native C++ linker prerequisites for your platform.
On Windows, use Visual Studio C++ Build Tools and the Windows SDK.

```sh
cargo build --workspace --locked
cargo run -p os-app
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
```

On the initial Windows development machine, Rust is installed locally under the
ignored `work/toolchain/` directory. Use the convenience launcher without changing
the system PATH:

```powershell
.\tools\cargo.ps1 run -p os-app
.\tools\cargo.ps1 test --workspace --all-features --locked
.\tools\cargo.ps1 run -p os-app '--' --smoke outputs\example.osb
```

The smoke command requires an existing parent directory and a new output path
and leaves a real project file. Create `outputs` first, or use `example.osb` in
the current directory on a fresh clone.
Quote the `'--'` separator when using the PowerShell script so PowerShell forwards
it to Cargo. The normal `cargo` executable uses the unquoted separator as shown below.
Open it with `cargo run -p os-app -- outputs/example.osb`. It exercises plugin load,
wall creation, length/thickness/height/level edits, regeneration, undo/redo and
save/reopen without requiring a display.

## First modeling session

1. Start the application; a project, site, building, ground level and 3D view exist.
2. Choose **Architecture → Wall**. In the upper-left Properties palette, enter
   two XY endpoints, thickness and height, then choose **Create wall**. Scroll
   the fields to reach Endpoints; the action stays visible. Values are in metres.
3. Select a wall in Project Browser or the viewport. Edit its properties and
   choose **Apply changes** in Properties or **Modify | Walls**. Changing Length
   preserves its start point and direction.
4. Use **Architecture → Add level**, then assign a wall using its Properties
   Level field and Apply changes. Select an active level in Project Browser and
   edit its elevation under **Manage** to regenerate attached walls.
5. Drag in the viewport to orbit, scroll to zoom, or choose **Fit model** from
   the view footer or View ribbon. Drag the left-column edge or palette divider
   to adjust the workspace.
6. Open **File**, enter a `.osb` path, and choose **Save** or **Open**. Save,
   Undo and Redo are also in quick access. Ctrl+Z/Ctrl+Y control history;
   Ctrl+S saves when a text field is not focused. **Manage** also renames projects.

The light ribbon workspace is specified in the [UI design guide](docs/ui-design-guide.md).
The status bar shows operation feedback; **Details** expands complete diagnostics.

New/Open/Close prompt before discarding committed unsaved edits. Property fields
are a draft until Apply changes. Save to a different existing file prompts before
replacement. A failed open preserves the current document.

## Current limits

- Native storage is ZIP + JSON, not SQLite yet; history and derived meshes are not
  saved. See [file format](docs/file-format.md).
- Default builds use trusted bundled plugins. The optional external-plugin build
  adds bounded Wasm execution, desktop activation and descriptor forms. Deadlines
  revoke late results; they do not forcibly terminate compiler/file IO work.
  See the B/C audit for runtime, installation and qualification boundaries.
- Geometry supports rectangular wall prisms, segmented hosted doors/windows, and
  contour-only linked building sections through native walls and floors. General
  booleans, wall joins, curved walls, constraints solving and CAD kernel
  integration remain unimplemented. See [native sections](docs/native-sections.md).
- The viewport uses cached CPU depth-buffered rendering and visible-pixel picking,
  fixing triangle-order artifacts at wall intersections. Resolution is capped at
  two million pixels; no production GPU renderer or antialiasing is implemented.
  Plan workflows include native snapping and a basic A3 single-view sheet preview
  with one-page vector PDF export, not full sheet sets or production printing. See
  [viewport verification](docs/viewport-depth-buffer.md) and
  [native sheet preview](docs/native-sheet-preview.md).
- Site, building, material and view types exist; their general authoring tools,
  reports, analysis services and associative drawings remain future work.
- IFC exchange supports straight rectangular walls and bounded rectangular hosted
  door/window voids with linked IFC fillings and types; fillings have no IFC Body,
  and custom opening families or arbitrary third-party models remain unsupported.
  Native views/materials/extension data are not preserved; desktop exchange uses
  explicit loss and replacement confirmations. See [the IFC guide](docs/ifc-roadmap.md).
- Licensing and dependency legal review remain pending. Packages cannot be
  published; `LICENSE.pending` makes no open-source license grant.

## Foundation goals

- Semantic, parametric BIM model
- Versioned native `.osb` project format
- IFC-based interoperability
- Plugin-first architecture
- Transactional edits with undo/redo
- Offline-first project ownership
- Reproducible builds and testable architecture

## Repository guide

- [Build prompt](BUILD_PROMPT.md)
- [Build prompt completion audit](docs/build-prompt-audit.md)
- [Architecture plan](docs/architecture.md)
- [UI design guide](docs/ui-design-guide.md)
- [Contributing](CONTRIBUTING.md)
- [File format](docs/file-format.md)
- [Plugin protocol and example](docs/plugin-api.md)
- [Foundation decision](docs/decisions/0001-foundation.md)
- [Test guide](tests/README.md)
- [Verification results and next tasks](docs/verification.md)

## Name and legal note

OpenStructure is a working project name pending trademark clearance. The project must maintain an independent visual identity and must not imply affiliation with Autodesk, Revit, or any other vendor.

The project license is intentionally pending a dependency and legal review before the first public software release.
