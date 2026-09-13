# IFC wall-exchange slice

Status: experimental Rust IFC4 export/import is implemented through `WallIfc`
and CLI commands. Desktop exchange is enabled (`AVAILABLE = true`) after
[independent viewer acceptance](ifc-viewer-acceptance.md). `UnavailableIfc` remains an explicit disabled adapter,
not the implementation used by the CLI. This is not a general IFC importer.

## Supported subset

- IFC4 STEP, a single project, project → sites → buildings → storeys, wall
  containment and standard compressed UUID GlobalIds.
- SI metres, zero site/building origins, storey elevations, planar wall positions
  and yaw, rectangular profiles, positive vertical extrusions and IfcWall bodies.
- UTF-16 STEP string escaping, including quotes, backslashes and Unicode names.
- Native wall name, identity, endpoints, length, thickness, height and level are
  reconstructed from actual IFC entities; there is no hidden native JSON payload.

The importer deliberately rejects other entity types, units, schemas, placement
forms, nonvertical/offset solids, extra representations, properties, materials,
openings and unsupported optional fields. It does not import arbitrary files
produced by other applications. Malformed references, duplicate IDs, incomplete
hierarchies, invalid dimensions and oversized/deeply nested input fail explicitly.
Limits: 16 MiB input/output, 100,000 STEP entities, 1,000,000 parsed values, depth
32; export fewer than 3,000 spatial/wall entities and names at most 255 characters.
STEP comments, alternate header layouts, typed property values and alternate
string escape forms are outside this reader. Export uses a fixed serialization
header timestamp for reproducibility, not the actual filesystem creation time.

## Data loss and commands

Export reports omitted native architectural grids (including their metadata), views, materials/densities/material assignments,
extension properties and additional named relationships. CLI export requires
`--allow-loss` when there are warnings. Keep the original `.osb` file. Import
creates a fresh default 3D view and reports that original native-only data cannot
be recovered. Native save is still `.osb`; IFC is an exchange copy, not a backup.

Model-schema-2 extension entities have no IFC mapping and block export, including
with `--allow-loss`. This avoids making an incomplete wall-only representation
of a plugin model. Unused plugin requirement records are separately reported as
omitted metadata. Supported wall exchange remains IFC4, not IFC4.3.

```powershell
.\tools\cargo.ps1 run -p os-app '--' --export-ifc project.osb exchange.ifc --allow-loss
.\tools\cargo.ps1 run -p os-app '--' --import-ifc exchange.ifc imported.osb
```

Both CLI outputs must be new paths in existing directories. Publication uses a
same-directory temporary file and no-clobber persistence. Rejected input creates
no output, cannot overwrite an existing file and cannot mutate an open document.
Import validates the complete native graph and regenerates wall geometry before
publishing a native project.

`IfcAdapter::export` rejects losses; use `WallIfc::export_report` to obtain the
bytes plus warnings for an explicit acknowledgement workflow. Prefer
`import_report` for its default-view diagnostic. Neither API writes files.

## Verification performed

Final desktop-slice local workspace check: 46 tests passed (including one doctest), strict
Clippy, rustfmt, locked/offline executable build and 333-package Rust license
metadata check passed. The IFC implementation adds no Rust package versions.

- Rust round trips: stable semantic IDs, spatial relationships, all dimensions,
  rotated endpoints, elevations and identical regenerated meshes; Unicode names,
  empty models, loss reporting, malformed/unsupported input and bounded parsing.
- CLI integration tests: full native→IFC→native flow, acknowledgement refusal,
  no-clobber behavior, invalid input and extension/argument rejection.
- `fixtures/wall-exchange.ifc`: independent IfcOpenShell 0.8.5 schema/EXPRESS
  validation with zero errors. Open CASCADE tessellation gives 12 triangles,
  volume 7.35 m³ and bounds (0, −0.15, 3) to (7, 0.15, 6.5) metres.
- `fixtures/rotated-walls.ifc`: independent schema/EXPRESS validation with zero
  errors; two 12-triangle walls, 5.25 m³ each, verified rotated bounds and storey
  elevations 0 and 4.2 metres. These are independently authored native examples.
- Reproducible independent checks are configured in CI for frozen fixtures and
  freshly exported CLI output. Hosted CI has not been run locally.
  Supplying the original `.osb` as a second validator argument additionally checks
  standard IFC GUID expansion against source UUIDs and compares wall placements,
  directions and dimensions directly with the native source.

```sh
python -m pip install -r tools/ifc-validation-requirements.txt
python tools/validate-ifc.py fixtures/wall-exchange.ifc
python tools/validate-ifc.py fixtures/rotated-walls.ifc
cargo test --workspace --all-features --locked
```

On this development machine the validator is isolated under `work/ifc-validation`;
set `PYTHONPATH` to that absolute directory. No Python runtime is needed by the
application. IfcOpenShell metadata declares LGPLv3+; it is development-only, not
linked or bundled into the Rust binary. Project license/legal review is still
pending. The Rust adapter adds no third-party runtime packages beyond existing
workspace dependencies. Validation transitive Python dependencies are not a
fully locked supply-chain environment.

## Desktop workflow

Open **File → IFC4 wall exchange** and enter the separate `.ifc` path.
**Export IFC…** previews the losses and requires acknowledgement. Existing IFC
files require explicit replacement consent; a race-created destination triggers
another confirmation. Export includes committed changes only and never changes
the native save path or clears its dirty state.

**Import IFC…** parses and regenerates a staged document before confirmation.
If there are unsaved edits, cancel and save them first or choose **Discard and
import**. Cancellation/errors preserve the current model, history, scene, paths,
selection and drafts. Acceptance replaces the document with the prepared snapshot,
clears the native path, marks it unsaved and asks you to choose a new `.osb` path.
Import cannot be undone. Unapplied property drafts are explicitly warned about.

The external-viewer gate passed in the localhost-only upstream viewer. See the
[inspection report](ifc-viewer-acceptance.md) for fixtures, screenshots, reproduction
and dependency limitations. The desktop acceptance tests additionally exercise
File-menu input, acknowledgement/cancellation, overwrite races, blocked shortcuts,
compact/DPI layouts and the import→native-save dirty-state lifecycle.

General third-party IFC import, material/property mappings, wall openings and
multiple unit systems are later work. Native proprietary BIM formats remain out
of scope.

## Public references

- [buildingSMART IFC4 ADD2 TC1 schema](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/)
- [IfcOpenShell validation API](https://docs.ifcopenshell.org/autoapi/ifcopenshell/validate/index.html)
- [ADR 0003](decisions/0003-ifc-wall-exchange.md)
