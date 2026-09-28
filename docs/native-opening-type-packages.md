# Native door/window type packages

OpenStructure can transfer one authored native door or window type between
projects using a bounded `.osot` file. The Manage ribbon imports a package; the
selected opening type's Properties panel exports one. The package is portable
JSON, independent of the project model schema, and contains the type parameters
plus exact snapshots of only the referenced panel/pane, frame and optional
fixed-lite materials.
It does not include placed openings, hosts, levels, or other project entities.

Manage also provides a local package browser. Enter one directory, scan it, and
search the valid packages by type name, door/window kind, path, or referenced
material name. Scanning runs off the UI thread and does not recurse. Symlinks and
non-package files are ignored; corrupt/oversized package files appear as issues
while other valid entries remain usable. Selecting a result re-reads that file
and uses the same conflict-aware import preview described below. The folder path
is retained only for the current application session; it is not saved in the
project or across restarts.

## Import and conflict behavior

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

Format identifier: `OpenStructure.OpeningTypePackage`, version 3. Version-1
packages remain importable and are upgraded as single-panel families (family
version 4 to 5 with `side_lite: null`); version-1 and version-2 imports default
window operation to Fixed. Version 2 can carry the optional two-bay
side/lite-width/mullion settings and an independent lite material reference.
Version 3 adds required type-level Fixed/Sliding/Casement window-operation
metadata. It affects plan symbols only; generated 3D panes remain static.
Export includes exact material snapshots when set; import remaps them with the
panel/frame dependencies and applies the same collision rules. Reads reject
unknown fields, duplicate JSON keys, malformed or excessively deep JSON, invalid
opening profiles/material references, unsupported versions and files over 1
MiB. Material snapshots are deduplicated and UUID-sorted; encoding is
deterministic. The source UUID is only a reference for remapping and is not
installed as the destination type identity.

The local browser examines at most 4,096 directory entries, 128 `.osot` files,
and 32 MiB of package bytes per scan. A global cap failure aborts the scan;
individual invalid files are reported separately. Results are sorted by path.

Acceptance evidence:

- `crates/os-storage/tests/opening_type_package.rs`: door/window round trips,
  exact dependencies, strict parsing, deterministic encoding, atomic writes and
  size limits; `two_bay_packages_v1_v2_upgrade_to_v3_strict_and_three_materials`
  checks legacy defaults, strict current fields and all three material dependencies.
- `crates/os-ui/src/desktop_tests/opening_type_package_tests.rs`: import/export
  at 1280×800/100% and 1000×650/150%, material conflict remapping, no mutation
  during preview, update identity and pinned dimensions, one-step undo/redo,
  stale/cancel behavior and rejected host-fit preflight.
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
