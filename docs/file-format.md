# Native .osb format, container 2 / model schema 4

An `.osb` file is a ZIP container, not a directory or a proprietary BIM format:

```text
project.osb
  manifest.toml
  model.json
  assets/
  previews/
```

The manifest contains `format = "OpenStructure"`, `container_version = 2`,
`schema_version = 4`, a UUID `project_id`, `model_entry = "model.json"`, and
`units = "metres"`. The same project identity and model schema must match the
model. Unknown future container or model versions fail with an explicit error.

Every entity has a header with stable UUID, namespaced type identifier, entity
schema, extensible JSON properties and named relationships. Concrete parameters
hold semantic parent references. Maps are keyed by UUID; map keys and entity IDs
must match and IDs must be globally unique and non-nil. All relationships must
resolve. Geometry is derived and is regenerated on open. History, selection and
navigation camera state are not persisted in model schema 4. Named plan settings
are persisted separately from navigation; see below.

The reader never extracts paths from the archive. It reads the manifest, model
and bounded opaque auxiliary files, rejects missing/duplicate required entries, caps entry count at 1,024,
manifest size at 64 KiB and decompressed model size at 64 MiB. Malformed UTF-8,
ZIP, JSON, TOML and invalid model graphs fail before replacing the open document.
Required-name checks are preceded by a raw central-directory scan: the ZIP
library's lookup index deduplicates names, so counting that index alone would
silently accept duplicate model files. Duplicate archive names are rejected.

The writer validates and serializes first, creates a unique temporary file in
the destination directory, finishes and syncs it, then atomically replaces the
destination. The original file survives errors before replacement. Crash
durability ultimately depends on the operating system and filesystem. The UI
asks before overwriting a different existing file.

Auxiliary files (including unknown extension files) and empty directories are
preserved by name and bytes across edit/undo/save/reopen. Each file is capped at
16 MiB and combined auxiliary contents at 64 MiB, both declared and actual
decompressed sizes. All entries, including empty directories, count toward 1,024.
Paths must be canonical relative UTF-8 paths with slash separators; traversal,
absolute/device paths, control/reserved characters, case collisions, file/parent
collisions and special entries such as symlinks are rejected. No bytes are executed.
Compression, timestamps, comments and platform attributes are not preserved.

`Document::from_model_and_files` and `auxiliary_files()` retain inert contents
outside semantic history snapshots; no mutable file accessor or attachment editor
exists. Storage validates the file map on save, before touching the destination.
Model commands do not edit attachments. Model schema 2 separately preserves
plugin entities in `extensions`, keyed by UUID, and exact owner versions in
`plugin_requirements`. See [extension envelopes](semantic-extension-durability.md).
Unknown root model fields and unknown envelope fields/versions are rejected;
opaque payload values are preserved. This is not arbitrary unknown-field support
for existing native entity structs. IFC document export separately reports omitted
auxiliary files and requires loss consent; unsupported extension entities block
IFC export entirely until a mapping exists.

## Migrations

The synthetic schema-0 fixture records levels with `z` rather than `elevation`
and headers without metadata fields. Migration 0→1 renames `z`, fills header
schema/properties/relationships, and then runs full model validation. The fixture
is an engineering compatibility baseline, not a claim of an earlier release.
Tests also package the old model in an actual `.osb`, open and migrate it, then
resave and reopen it with current manifest/model versions and unchanged UUIDs.

Container 1 remains readable, including its auxiliary files. Saving writes
container 2 and model schema 4 without changing model IDs; older builds reject newer versions
instead of silently dropping opaque contents. Opening never rewrites the source.
See the earlier container-only change in
[ADR 0007](decisions/0007-opaque-container-files.md).

Migration 1→2 adds empty extension/requirement maps and updates native header
schemas from 1 to 2. Contradictory old headers or pre-existing new fields fail.
Migration 0 chains through 1, 2 and 3 to 4. The public migration function operates on a
copy and validates the entire graph before replacing its input. The frozen
`intersecting-walls.osb` tests schema-1 compatibility. No plugin code runs during
native migration. Future versions are rejected. Each additional
version must add an explicit migration and a frozen fixture test. Do not infer
versions from arbitrary property shapes.

Migration 2→3 updates native header schemas to 3 and adds `settings_revision = 0`
and `plan` to each view's parameters. Plan views receive settings version 1:
level-relative top/cut/bottom/depth of 2.5/1.2/0/−1 metres, XY origin (0,0),
zero yaw, no crop, scale denominator 100, and walls/extensions visible. Other
view kinds receive `plan = null`. Existing IDs, names, level references, metadata,
opaque extension payloads and provider requirements are preserved. Legacy plans
without an assigned level stay unassigned; deriving or editing them requires an
explicit valid level, never a guessed one. Contradictory schema-2 headers or
pre-existing new fields are rejected. `fixtures/schema-2-plans.json` is the frozen
compatibility fixture, also packaged into real ZIP round-trip tests.

Plan settings have their own version, independent of native model schema 4,
container 2, extension envelope 1 and plugin API 2. Range offsets must be finite,
ordered depth ≤ bottom ≤ cut ≤ top with positive total span, and remain usable at
the associated level elevation. Basis is a finite origin and horizontal yaw in
radians; optional crop is a finite positive rectangle in view-plane metres.
Scale denominator must be finite and within 0.001–1,000,000. These are development
bounds/defaults, not office standards or qualified plotting limits. Visibility
currently distinguishes only native walls and extensions. Unknown plan settings
fields or versions are rejected. View edits advance a host-managed revision;
undo restores settings while the document revision still invalidates old drawings.

Before adopting an opened/migrated model, storage also checks its pretty-serialized
size against the writer's 64 MiB limit with a counting sink. This rejects defaults
or formatting expansion that would otherwise make a successfully opened model
unsavable. Opening never modifies the original archive; explicit saving writes
the current schema. See [persisted plan settings](persisted-plan-settings.md).

## Architectural grids (model schema 4)

`grids` is a required UUID-keyed map of `core.grid` entities. Grid parameters
contain a building reference, unique case-sensitive trimmed name within that
building (1-256 UTF-8 bytes, no controls), and finite XY start/end extents in
metres. Extent length must be finite and greater than one micrometre. Identity,
header metadata and references follow normal model validation. Unknown parameter
and point fields are rejected. See [grid semantics and tests](architectural-grids.md).

Migration 3→4 adds the empty map and changes native header versions from 3 to 4;
it leaves all existing parameters, plan-settings versions, UUIDs, metadata and
opaque extensions unchanged. Contradictory headers and a pre-existing grid map
in schema 3 reject before adoption. `fixtures/schema-3-pointer-walls.json` is the
frozen compatibility fixture, packaged into actual ZIP migration/save tests.
Old archives are never rewritten on open. Container 2 and plan settings 1 remain
unchanged, as do independent plugin contracts.

## SQLite migration path

`StorageBackend` isolates the format implementation. A future container version
will use `model.sqlite`, normalized entity/relationship tables and a schema
migration table. Its importer must still read the current ZIP/JSON format and
produce the identical semantic model with unchanged UUIDs. Choose the new format
through manifest version dispatch; do not silently reinterpret `model.json` as
SQLite or discard extensible properties.
