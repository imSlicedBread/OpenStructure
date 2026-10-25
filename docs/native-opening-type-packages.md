# Native door/window type packages

OpenStructure can transfer one authored native door or window type between
projects using a bounded `.osot` file. The Manage ribbon imports a package; the
selected opening type's Properties panel exports one. The package is portable
JSON, independent of the project model schema, and contains the type parameters
plus exact snapshots of only the referenced panel/pane, frame and optional
fixed-lite and authored-solid materials and transitive Length/Scalar parameters.
It does not include placed openings, hosts, levels, or other project entities.

Version 10 carries family-10 extrusion bindings and typed Length/Scalar formulas.
Packages include exactly the transitive dependencies of width/height/sill and
extrusion X/Y/depth/normal-offset bindings, up to 64 definitions. Import allocates
new parameter UUIDs before remapping all formula references and bindings; names
and colliding source IDs never overwrite project parameters. Family/package
9→10 migration adds literal-only formula/binding defaults and rejects ambiguous
new fields in old input. The frozen v9 fixture and near-budget formula import
tests are documented in [typed parameter formulas](native-parameter-formulas.md).

Version 9 introduced family-9 `ActiveLeaf` and `InactiveLeaf` paired-door attachments.
Package 9 explicitly advances family 8→9 while retaining old attachment values;
new leaf attachments or paired accessory definitions in legacy wires are rejected.
Import retains exact material dependency remapping and fresh solid UUIDs.

Version 8 introduced family-8 `MovingSash` attachments for rectangular Casement and
Sliding Start-sash accessories on straight hosts. Package 7 explicitly migrates
family 7→8, preserving `OpeningFixed` and `PrimaryPanel` exactly; new attachments
or operable-window accessories hidden in old-version payloads are rejected.
The same exact material dependency remapping and fresh imported solid UUIDs apply.
Instance poses are not included in a type package. End-sash and
curved-host accessory attachments remain unsupported.

Version 7 introduced an exact `material_patterns` dependency map for hatch patterns
referenced by the included material snapshots. Import remaps each pattern to an
independent project UUID and preserves sharing among the package's materials;
name conflicts receive a visible imported name. Version 6 migrates to an empty
pattern map only when the new field is absent. A legacy material already using
a pattern or a pre-existing `material_patterns` field is rejected as ambiguous.

Manage also provides a local package browser. Enter one directory, scan it, and
search the valid packages by type name, door/window kind, path, or referenced
material name. Scanning runs off the UI thread and does not recurse. Symlinks and
non-package files are ignored; corrupt/oversized package files appear as issues
while other valid entries remain usable. Selecting a result re-reads that file
and uses the same conflict-aware import preview described below. The folder path
is retained only for the current application session; it is not saved in the
project or across restarts.

## Import and conflict behavior

Version 6 carries family-7 additive extrusions with their source solid identities,
attachment frames, profiles, exact positions/depths and material references.
Package 5 migrates unambiguously from family 6 by adding an empty extrusion list;
legacy payloads already containing that field (even null) are rejected. Prior
package upgrades continue through this step without altering old geometry.
Import assigns each solid a fresh UUID, including on type update, and remaps its
material through the existing conflict-aware dependency plan. Up to 19 distinct
material snapshots are allowed (three base composition materials plus 16 solids).
The supplied dependency set must match references exactly; extrusion-only
materials are not omitted. The 1 MiB package limit remains unchanged.

Version 5 carries the paired-door `family.door_leaves` setting in addition to
the version-4 shared Length snapshots. `Single` preserves the existing
single-leaf behavior. `Paired { active_fraction }` stores the active leaf's
share of the clear primary bay; each leaf must resolve to at least 1 mm. Paired
layouts are valid only for rectangular door components and full rectangular
host cuts; windows remain single-leaf. The existing material and Length
dependency copying/remapping rules are unchanged.

Version 4 carries exactly the shared Length snapshots referenced by width,
height and window-sill bindings. Every import copies each distinct parameter
once to a fresh UUID, preserving sharing within that type. The preview lists
source and destination names/UUIDs and values; conflicting names get an
`(Imported)` suffix. Existing parameters are never overwritten or implicitly
reused, even on type update. Users can explicitly rebind the imported type in
the type editor afterwards. Per-dependency reuse during import is deferred.
Parameter additions, material additions and type changes commit in one undo step.
The preview and Apply use the same document candidate evaluator, with session
and revision guards and component/host geometry preflight.

Import reads a package and builds a read-only preview. It creates a new type by
default. When a same-kind type is selected, the user may instead explicitly
update that type. The preview reports affected instances and pinned dimensions,
rematerializes referenced materials, and validates the candidate model plus
every affected opening component and unique host before enabling commit.

An exactly matching destination material is reused. A material with a colliding
name but different parameters is never overwritten; the imported copy receives
a visible `(Imported)` suffix (numbered when necessary). A conflicting new type
name is likewise given a visible unique suggestion. Updating preserves the
selected type UUID and all placed opening parameters, including width, height,
and sill overrides. The imported definition replaces the destination type's
authored fields; local type-field merge is not offered.

All newly imported materials and the type add/update are committed as one
document transaction. One undo reverses the complete import; redo restores it.
Cancel, invalid host fit, stale document revision, or package parse/validation
failure leaves the project model and history unchanged. If scene regeneration
fails after commit, the transaction is rolled back. Export warns before replacing
an existing package and writes through a temporary sibling for atomic replacement.

## Format and limits

Format identifier: `OpenStructure.OpeningTypePackage`, version 10. Version-1
packages remain importable and are upgraded as single-panel families (family
version 4 to 5 with `side_lite: null`); version-1 and version-2 imports default
window operation to Fixed. Version 2 can carry the optional two-bay
side/lite-width/mullion settings and an independent lite material reference.
Version 3 adds required type-level Fixed/Sliding/Casement window-operation
metadata. It affects plan symbols only; generated 3D panes remain static.
Version 4 requires `length_parameters` (source UUID → named metre value) and
`length_bindings` (three explicit nullable UUIDs). Versions 1–3 migrate to empty
snapshots and literal-only bindings; pre-existing new fields in legacy input
are rejected as ambiguous. Versions 4–9 limited snapshots to three direct Length
bindings. Version 10 permits up to 64 typed transitive dependencies and rejects
missing or extra snapshots. Labels and validation
use resolved package dimensions rather than inactive fallback literals. Version
5 adds the required family-version-6 `door_leaves` field. Version-4 packages
migrate to `Single` only when their family version is 5 and the field is absent;
ambiguous legacy content is rejected. Versions 1–3 chain through earlier
migrations and then receive the same `Single` default.
Export includes exact material snapshots when set; import remaps them with the
panel/frame dependencies and applies the same collision rules. Reads reject
unknown fields, duplicate JSON keys, malformed or excessively deep JSON, invalid
opening profiles/material references, unsupported versions and files over 1
MiB. Material snapshots are deduplicated and UUID-sorted; encoding is
deterministic. The source UUID is only a reference for remapping and is not
installed as the destination type identity.

Version 7 adds required `material_patterns` snapshots. Their source UUID set
must exactly match the `cut_pattern` references in the package materials.
Version-6 packages migrate to an empty map only when they contain neither the
new map nor material pattern references; older versions chain through this
migration. Pattern definitions are validated and remapped independently during
import, including when the source UUID already exists in the destination.

The local browser examines at most 4,096 directory entries, 128 `.osot` files,
and 32 MiB of package bytes per scan. A global cap failure aborts the scan;
individual invalid files are reported separately. Results are sorted by path.

Acceptance evidence:

- `crates/os-storage/tests/opening_type_package.rs`: door/window round trips,
  exact dependencies, strict parsing, deterministic encoding, atomic writes and
  size limits; `two_bay_packages_v1_v2_upgrade_to_v3_strict_and_three_materials`
  checks legacy defaults, strict current fields and all three material dependencies.
- `crates/os-storage/tests/paired_doors.rs`:
  `paired_doors_package_v5_v4_upgrade_and_save_reopen` checks v4→v5 migration,
  preservation of material/Length dependencies, and native save/reopen.
- `crates/os-ui/src/desktop_tests/opening_type_package_tests.rs`: import/export
  at 1280×800/100% and 1000×650/150%, material conflict remapping, no mutation
  during preview, update identity and pinned dimensions, one-step undo/redo,
  stale/cancel behavior and rejected host-fit preflight; paired-door replacement
  with explicit pose reset is covered at both DPI profiles.
- `crates/os-storage/tests/opening_type_library.rs` and
  `crates/os-ui/src/desktop_tests/opening_type_library_tests.rs`: scan limits,
  deterministic listing, invalid-entry reporting, search, fresh preview handoff,
  cancellation and door/window import at both DPI profiles.

This increment is a local single-folder browser, not an office/cloud content
service or Revit-compatible family system. The optional side lite is a bounded
two-bay composition, not arbitrary or nested family content. Bulk packages,
office/cloud catalogs, catalog revisions, selective field merge, general nested
families, multiple authored solids, constrained sketches, formulas and graphical
side-by-side update previews remain future work. See the [2D coverage ledger](2d-coverage.md).
