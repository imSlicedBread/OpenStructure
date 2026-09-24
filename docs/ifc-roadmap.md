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
- Default rectangular hosted doors/windows through `IfcOpeningElement`,
  `IfcRelVoidsElement`, `IfcDoor`/`IfcWindow`, `IfcRelFillsElement`, and optional
  `IfcDoorType`/`IfcWindowType` with `IfcRelDefinesByType`. See the bounded
  opening contract below; the historical viewer acceptance covers walls only.

The importer deliberately rejects other entity types, units, schemas, placement
forms, unsupported solid transforms, extra representations, properties, materials,
custom opening families and unsupported optional fields. It does not import arbitrary files
produced by other applications. Malformed references, duplicate IDs, incomplete
hierarchies, invalid dimensions and oversized/deeply nested input fail explicitly.
Limits: 16 MiB input/output, 100,000 STEP entities, 1,000,000 parsed values, depth
32; export fewer than 3,000 spatial/wall/opening/type entities and names at most 255 characters.
STEP comments, alternate header layouts, typed property values and alternate
string escape forms are outside this reader. Export uses a fixed serialization
header timestamp for reproducibility, not the actual filesystem creation time.

## Bounded hosted opening contract

The host body remains the uncut source wall prism (stored start/end, full length,
thickness and height). Each opening is placed relative to that wall at
`(offset, 0, sill)`. Its solid has origin `(0, thickness/2, 0)`, axis
`(0,-1,0)` and reference direction `(1,0,0)`: the rectangular profile lies in
wall XZ and extrudes through the full wall thickness toward wall -Y. The
filling's placement is relative to the opening. Only the wall and filling are
contained in the storey; the opening is never spatially contained.

Instance UUID/name live on the filling root; type UUID/name live on the type
root. Void and relationship roots receive new UUIDs. Native offset, sill,
dimensions, kind, type assignment and door orientation are reconstructed from
the entities and placements. `Description` on each filling is strictly
`OpenStructure.Typed.v1` or `OpenStructure.Legacy.v1`; this dialect discriminator
prevents a missing type link from silently converting a typed instance to Legacy.
Dimensions and sill on all occurrences of one type must agree exactly. Unused
types are rejected because these values could not be reconstructed.

The mapping follows [IfcDoor placement and operation](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcsharedbldgelements/lexical/ifcdoor.htm)
and [IfcDoorTypeOperationEnum](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcsharedbldgelements/lexical/ifcdoortypeoperationenum.htm),
using host-relative jambs rather than national handedness names:

| Native hinge / swing | Filling origin relative to void | Filling X direction | IFC operation |
| --- | --- | --- | --- |
| Start / Left | `(0,0,0)` | `+X` | `SINGLE_SWING_LEFT` |
| End / Left | `(0,0,0)` | `+X` | `SINGLE_SWING_RIGHT` |
| Start / Right | `(width,0,0)` | `-X` | `SINGLE_SWING_RIGHT` |
| End / Right | `(width,0,0)` | `-X` | `SINGLE_SWING_LEFT` |

Filling +Y determines swing; -X above is a 180-degree yaw, with +Z still up.
Typed occurrences take operation from the type and leave occurrence
PredefinedType/OperationType unset. One shared native type is never split or
merged: occurrences requiring different IFC operation values cause export to
fail. Opposite hinge **and** swing can share one type through the placement
mapping. Windows use `SINGLE_PANEL`, centered pane alignment and identity
placement relative to the void.

Only the exact default `OpeningFamily` is supported. Any changed component or
cut profile, family parameter/frame setting, noncenter pane position, custom
property/map, tilt, mirror, transverse displacement or partial-depth cut is
rejected. The parser accepts exactly 9 arguments for OpeningElement, 13 for
Door/Window and their types, and 6 for Voids/Fills/DefinesByType relationships.
Missing, duplicate, wrong-kind and orphan relationships fail. Every filling
must share its host's storey. Import constructs a temporary model and runs
`Model::validate` (including clearance/overlap checks) before returning it.

Fillings deliberately have no IFC Body: native panels, panes, frames, component
geometry, family parameters and materials are not exported. This omission is
reported by `export_report`; ordinary `IfcAdapter::export` requires loss
acknowledgement. Import reports regeneration of the native default family.
This increment is a restricted bidirectional exchange, not general third-party
IFC import or new viewer qualification.

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

On this development machine the validator is isolated under `work/venv`;
invoke `work/venv/Scripts/python.exe` for the commands above. No Python runtime is needed by the
application. IfcOpenShell metadata declares LGPLv3+; it is development-only, not
linked or bundled into the Rust binary. Project license/legal review is still
pending. The Rust adapter adds no third-party runtime packages beyond existing
workspace dependencies. Validation transitive Python dependencies are not a
fully locked supply-chain environment.

Hosted-opening evidence (2026-09-23): `cargo test -p os-ifc --all-features`
passes 17 tests, with one independent gate ignored by default. The tests include
both kinds, typed/Legacy/mixed instances, shared and equal-but-distinct types,
all four door orientations, reversed/rotated/elevated hosts, regenerated cut
geometry, malformed links/transforms, nondefault families, atomic failure and
the STEP entity/value/size/depth limits. Package all-target Clippy with warnings
denied and the workspace formatting check pass.

The configured environment is now `work/venv/Scripts/python.exe` with
IfcOpenShell 0.8.5. `tools/validate-ifc.py` passes both existing wall fixtures;
it intentionally expects gross wall volumes and is not the hosted-hole gate.
`crates/os-ifc/tests/validate_hosted.py`, invoked by the ignored Rust test,
independently validates 20 generated fixtures / 80 openings using IFC4 EXPRESS
rules, source UUID/name/type comparisons, physical hinge/swing and placement
checks, Open CASCADE void bounds and net host volumes. Generated IFC and source
TSV evidence is retained in a unique temporary directory printed by the test.
No Python dependency is added to the application.

```powershell
$env:OS_IFC_PYTHON = (Resolve-Path work/venv/Scripts/python.exe).Path
cargo test -p os-ifc --test exchange independently_validate_hosted -- --ignored --nocapture
cargo clippy -p os-ifc --all-features --all-targets -- -D warnings
cargo fmt --all -- --check
```

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

General third-party IFC import, material/property mappings, custom opening families and
multiple unit systems are later work. Native proprietary BIM formats remain out
of scope.

## Public references

- [buildingSMART IFC4 ADD2 TC1 schema](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/)
- [IfcOpenShell validation API](https://docs.ifcopenshell.org/autoapi/ifcopenshell/validate/index.html)
- [ADR 0003](decisions/0003-ifc-wall-exchange.md)
