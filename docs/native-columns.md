# Native rectangular architectural columns

OpenStructure now has a first-class `core.column` model element for vertical,
axis-aligned rectangular columns. It is distinct from the installed column
plugin example: the native column has stable project identity, appears in the
native model graph, participates in transactions and persistence, and derives
the same object in plan and 3D.

In a floor plan, use **Place column** and click a snapped center. A translucent
footprint is previewed without mutating the document. Escape cancels; one
successful click adds one column transaction. The initial instance defaults
are 0.4 × 0.4 m with 3 m height. Select the column to edit its name, level,
center, width, depth, height, base offset and optional material in Properties;
**Apply column** commits one edit. Delete removes the selected column. Undo,
redo, save and reopen retain its UUID and parameters.

The bottom elevation is the selected level elevation plus base offset; the top
is the bottom plus positive height. Plan representation uses the active view's
vertical range, horizontal basis and crop. A column crossing the cut plane is a
cut footprint; a column above the cut within the projection range is projected.
Interior plan picking uses the clipped footprint. The generated closed prism
uses the instance material identity on its faces. Level elevation/material
changes invalidate dependent geometry and views.

Schema 26 migrates to schema 27 by adding an empty native `columns` map and
advancing native headers. Unknown extension payloads are preserved. IFC export
explicitly rejects a model containing native columns until a mapping exists;
it never silently omits them.

Limitations: columns are vertical, rectangular and axis-aligned in world XY.
Rotation, circular profiles, slanted columns, reusable types, wall/slab joins,
section graphics, column schedules, IFC exchange, and visual/print production
qualification are not implemented. This is an initial architectural object,
not Revit-equivalent column authoring.

Focused automated evidence is in:

- `crates/os-model/src/columns.rs` and `crates/os-geometry/src/columns.rs` for
  bounded parameters, references, elevation arithmetic, face material and
  prism volume.
- `crates/os-document/src/tests/columns.rs` for atomic commands, stable
  identity, dependency invalidation and undo/redo.
- `crates/os-render/tests/columns.rs` for cut/projected classification,
  horizontal basis, crop and interior pick.
- `crates/os-storage/src/tests/columns.rs` for schema migration, extension
  preservation and native archive reopen.
- `crates/os-ui/src/plan_workspace/column_tests.rs` for egui placement,
  non-mutating preview, Properties edit, history and save/reopen at
  1280×800 / 100% and 1000×650 / 150%.

These are headless egui tests, not native-window or physical print acceptance.
