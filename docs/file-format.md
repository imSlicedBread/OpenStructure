# Native .osb format, container 2 / model schema 28

Schema 25 adds optional frame width/depth fields to reusable opening families
and advances their family payload to version 2. The 24→25 migration defaults
frame width to zero and depth to 50 mm, preserving the pre-frame opening geometry.
Frame members are derived; they add no independent model entities or UUIDs.
Native full-model plugins use API 9/schema 28. The 27→28 migration adds empty
plan-graphics template and view-binding maps and advances native headers without
rewriting opaque plugin payloads. The 26→27 migration added the first-class
native `columns` map. Generic API 2 is unchanged.

Schema 25→26 advances opening families to version 3 and adds an independent
`cut_profile`, defaulting to the full rectangle so existing door/window host
geometry remains unchanged. A family may select the profile as its host cut;
straight-wall meshes, plan cut/projection footprints, sections, and layer
quantities are derived from the same bounded polygon partition. The component
profile must fit inside the host cut. Generated rectangular frame rails require
a rectangular host cut. Migration updates native headers and preserves all
other fields, identities, and opaque extensions.

Schema 23 adds `pane_position` to project-owned opening types. Existing window
types migrate to `Center`, preserving their current geometry; doors are valid
only with `Center`. Legacy opening definitions remain independent and resolve
to `Center` without acquiring a type field. Migration 22→23 rejects ambiguous
pre-existing values, advances native entity headers, and preserves all other
fields and identities. See [native hosted openings](native-hosted-openings.md)
for the plan/3D behavior. Native full-model plugins now use API 5/schema 23;
generic API 2 is unchanged.

Schema 22 adds required `wall_types` and `wall_type_assignments` maps for
project-owned compound wall assemblies and per-wall type/flip assignments.
Migration 21→22 rejects pre-existing fields, adds empty maps, advances native
entity headers, and leaves pre-existing walls independent with their saved
dimensions and material; no legacy walls are automatically grouped into shared
types. See [native wall types and layers](native-wall-types.md) for the profile
resolver, geometry, quantities, and export boundaries. A frozen schema-21
fixture verifies identity-preserving migration and save/reopen.

Schema 21 adds the required `wall_joins` UUID map of tagged `core.wall_join`
entities for explicit butt, perpendicular corner and perpendicular tee
relationships between native straight walls. Geometry is derived from stable
wall UUIDs, endpoint anchors, and (for tees) a host station; contact alone never
creates a join. Migration 20→21 preserves explicit butt-join UUIDs and anchors
as tagged butt joins, rejects ambiguous payloads, and advances the model/native
header versions. Migration 19→20 still creates an empty join map without
inferring connectivity from coincident endpoints. See
[native-wall-joins.md](native-wall-joins.md) for constraints and limitations.
Frozen schema-19 and schema-20 fixtures are under
`crates/os-storage/tests/fixtures/`.

Schema 19 adds the required `room_separation_lines` UUID map of
`core.room_separation_line` entities. Each line stores an existing level and
two world XY endpoints in metres. It has no thickness or 3D solid. Level-owned
lines join same-level wall centerlines in the room face solver, independent of
view crop; crop only clips their visible plan graphics, picking, and snapping.
The existing room `boundary_signature` remains an ordered directed cycle of
globally unique UUIDs, so wall-only signatures retain their exact representation
and mixed wall/separator boundaries need no source tag. A topology change may
leave an existing room unresolved without changing its saved identity or seed.
Migration 18→19 rejects any pre-existing `room_separation_lines` field, inserts
an empty map, and advances the model and native entity header versions. The
focused migration and save/reopen test is
`crates/os-storage/tests/room_separation_lines.rs`; native-window and print
qualification remain open.

Schema 18 adds the required `detail_lines` UUID map of `core.detail_line`
entities. Each line stores `view`, `start` and `end`; points are world XY metres.
The view must be a configured Plan with a valid level. Endpoints must be finite,
within ±1,000,000 m, and more than 1e-9 m apart. Identity is globally unique,
and the collection is limited to 10,000 lines. Line data is independent of
walls and 3D solids. The explicit 17→18 migration rejects any pre-existing
`detail_lines` field, adds an empty map, and advances every native entity header,
including sheets and schedules. Opaque extension payloads are unchanged. The
frozen schema-17 fixture and atomic rejection tests live in
`crates/os-storage/tests/detail_lines.rs`.

Schema 17 adds required `schedule_placements` to every sheet. Each placement has
an independent UUID, a saved schedule UUID and `paper_rect_mm` (`min_mm`, `max_mm`).
Paper coordinates use millimetres from the top-left, +X right, +Y down. Validation
checks global IDs, schedule references, a 16-placement bound, page/frame bounds,
and overlap with viewports, other tables and the fixed title block. Rows are never
stored in placements. Removing a referenced schedule fails document validation.
The explicit 16→17 migration adds an empty array to every old sheet, rejects any
pre-existing placement field, and advances native headers including schedules.
The bounded UI currently composes one viewport and one table per page.

An `.osb` file is a ZIP container, not a directory or a proprietary BIM format:

```text
project.osb
  manifest.toml
  model.json
  assets/
  previews/
```

The manifest contains `format = "OpenStructure"`, `container_version = 2`,
`schema_version = 23`, a UUID `project_id`, `model_entry = "model.json"`, and
`units = "metres"`. The same project identity and model schema must match the
model. Unknown future container or model versions fail with an explicit error.

Every entity has a header with stable UUID, namespaced type identifier, entity
schema, extensible JSON properties and named relationships. Concrete parameters
hold semantic parent references. Maps are keyed by UUID; map keys and entity IDs
must match and IDs must be globally unique and non-nil. All relationships must
resolve. Geometry is derived and is regenerated on open. Doors and windows are
stored as hosted openings in the `openings` map; each keeps its name, straight
host wall reference, along-wall offset and explicit `definition`. A definition
is either `{"Legacy":{"kind":"Door","width":0.9,"height":2.1,"sill":0.0}}`
or `{"Typed":{"type_id":"<UUID>"}}`. Typed instances never store duplicate
dimensions. The required `opening_types` map holds project-owned
`core.opening_type` entities, whose parameters are `name`, `kind`, `width`,
`height`, and `sill` in metres. Type names may change without changing identity;
type kind is immutable through update commands. Unused types are also validated.
History, selection and navigation camera state are not persisted in model schema 23. Named plan
settings are persisted separately from navigation; see below.

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
container 2 and model schema 23 without changing model IDs; older builds reject newer versions
instead of silently dropping opaque contents. Opening never rewrites the source.
See the earlier container-only change in
[ADR 0007](decisions/0007-opaque-container-files.md).

Migration 1→2 adds empty extension/requirement maps and updates native header
schemas from 1 to 2. Contradictory old headers or pre-existing new fields fail.
Migration 0 chains through 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13 and 14. Schema 4→5 adds the required empty
`openings` map and advances native core headers; extension payload schemas and
data remain unchanged. Schema 5→6 adds the required empty `rooms` map and advances
native core headers; extension payload schemas and data remain unchanged.
Schema 6→7 adds the required empty `dimensions` map and updates every native
entity header (including rooms) to 7. Existing dimension fields in schema 6,
missing entity maps and contradictory header versions fail atomically. Extension
envelopes and their payload schema versions remain unchanged. The
public migration function operates on a
copy and validates the entire graph before replacing its input. The frozen
`intersecting-walls.osb` tests schema-1 compatibility. No plugin code runs during
native migration. Future versions are rejected. Each additional
version must add an explicit migration and a frozen fixture test. Do not infer
versions from arbitrary property shapes.

Schema 8→9 adds the required empty `floors` map and advances all native entity
headers to 9. Existing `floors` in schema 8, missing maps or contradictory native
header versions fail atomically. Extension envelopes and opaque payloads remain
unchanged. The frozen schema-8 fixture embedded in
`crates/os-storage/src/tests/floors.rs` verifies migration and real archive
save/reopen independently of current model constructors.

Schema 9→10 adds required `hinge: "Start"` and `swing: "Left"` to every
`core.opening` instance and advances every native header, including floors and
opening types, to 10. Start/Left retains the previous leaf endpoints and positive
host-normal swing. It is also the default for new doors. Hinge accepts Start/End;
swing accepts Left/Right relative to the stored wall start→end direction. Reversing
the host reverses that frame, without rewriting orientation. Windows require the
default values and retain existing graphics. These fields do not belong to opening
types and do not affect aperture dimensions. Schema-9 input containing either new
field, missing native maps, or inconsistent headers fails atomically. Schema 10
requires both fields rather than supplying deserialization defaults. IDs, shared
type definitions, dimensions, opaque extensions and payload schemas are preserved.
Frozen schema-9 typed/legacy fixtures in `crates/os-storage/src/tests/openings.rs`
verify exact migration, old archive load/save/reopen and source-file preservation.

Schema 11→12 enables the `Angular` dimension layout and advances the model and
native entity headers. Existing dimension parameters, IDs and other header
metadata are preserved. The frozen schema-11 aligned-dimension fixture and
Angular save/reopen test cover this transition; container version remains 2.

Schema 10→11 adds `layout = "Aligned"`, an empty `additional` endpoint list,
and `baseline_spacing_m = 0.25` to existing dimensions, then advances every
native entity header. Contradictory pre-existing dimension fields or header
versions fail atomically. The schema-10 dimension migration test checks these
defaults, stable IDs, and rejection of ambiguous inputs.

Each `core.floor` stores `name`, `level`, optional `material`, an ordered XY
`boundary`, positive `thickness`, and `top_offset`, in metres. Its stable UUID
identifies the whole floor. Absolute top elevation is level elevation plus
`top_offset`; bottom elevation is top minus thickness. The authored ring has
3–256 vertices with no duplicated closing vertex. Either winding and simple
concave polygons are supported; nonfinite/out-of-range coordinates (beyond
±1,000,000 m), duplicate vertices/edges of at most one micrometre,
self-intersections, backtracking and area at most 1e-8 m² are rejected.
Thickness must exceed one micrometre and be at most 1,000,000 m; top offset
must be finite and within ±1,000,000 m. Level and material references must resolve.
Area, CCW triangulation and closed outward-wound extrusion are derived, never
persisted. Floor commands participate in atomic history and level/material/plan
invalidation. Holes, slopes, layers and wall/floor joins are unsupported. The
restricted IFC wall exporter explicitly refuses models containing floors.

Schema 7→8 adds an empty `opening_types` map and wraps each opening's exact
kind/width/height/sill in `definition.Legacy`. It preserves UUID, name, host,
offset, metadata and geometry, without implicitly sharing sizes between old
instances. It advances every native header (including rooms and dimensions)
to 8; extension envelopes, payload schemas and payload data remain untouched.
An existing `opening_types` map, new or unknown opening parameter fields,
missing legacy fields/maps, or contradictory native headers fail atomically.
The frozen schema-7 test in `crates/os-storage/src/tests/openings.rs` combines
the fixed schema-3 building fixture with explicit old opening data, independently
of current constructors and serializers. It verifies exact preservation and
real `.osb` migration/save/reopen. The map counts toward both serialized model
limits and document-history memory accounting.

`AddOpeningType`, `UpdateOpeningType`, and `RemoveOpeningType` use whole-model
transaction validation. An edit must preserve every affected opening's host fit,
end/head clearance and non-overlap. Removing a type with remaining references
fails; reassignment/removal of its instances may be included in the same atomic
transaction. `UpdateOpening` assigns a type or converts a legacy definition;
creating a type and converting an instance can be one undoable transaction.
Type edits invalidate referenced openings, their host walls and plan views.

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

Schema 12→13 adds the required empty `room_tags` map and advances every native
entity header. Each `core.room_tag` stores a plan-view UUID, room UUID, and world
XY position; label text is resolved live from the room and is not duplicated in
the tag. Missing or differently leveled room references remain diagnostic
orphans, while a tag's plan-view reference must remain valid. The frozen
`schema-12-room.json` fixture verifies that migration preserves existing IDs,
headers, parameters, and maps apart from the explicit version advancement and
new empty map. Migration tests also reject a pre-existing `room_tags` map,
missing native maps and inconsistent header versions atomically. Current-schema
archive round trips preserve both live and orphan tags; see
`crates/os-storage/tests/room_tags.rs`.

Tag parameters are exactly `view`, `room` and `position` in metres. Positions
must be finite and within ±1,000,000 m; each room/view pair is unique. Creation
requires a live room on the owning plan's level. Tags must be removed before
their view, which may be done in one document transaction. Transient previews
and history are not persisted.

Plan settings have their own version, independent of native model schema 23,
container 2, extension envelope 1 and plugin APIs 2/5. Range offsets must be finite,
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

## Architectural grids (introduced in model schema 4)

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

## Sheet and view viewport persistence (introduced in schema 14)

`sheets` is a required UUID-keyed map of `core.sheet` entities, distinct from
`views`. A sheet has the ordinary native header and `SheetParams`: `number`,
`name`, `paper_size`, and an ordered `viewports` collection. The number is unique
across the document after trimming whitespace and ASCII case folding. Numbers
are 1–64 UTF-8 bytes; names are 1–256 bytes. Both must contain non-whitespace text
and no control characters. At most 1,000 sheets and 64 viewports per sheet are
allowed. Empty sheets are valid.

The only supported `paper_size` is `"A3Landscape"`, also the constructor default:
420 × 297 mm. Page dimensions are derived from that fixed enum, not stored as
unbounded numbers. Paper coordinates use a bottom-left origin, +X to the right
and +Y up. The document manifest still declares architectural model units as
metres; explicitly suffixed paper fields use millimetres.

Each `SheetViewport` stores:

- `id`: a stable, non-nil UUID, globally unique against all native entities,
  extension entities, and other embedded viewport IDs.
- `view`: an existing configured Plan or Section view UUID with an associated
  marker level and its corresponding settings.
- `model_center_m`: the persisted center in source view-plane metres: the
  plan-plane coordinates after the plan basis transform, or section station and
  absolute elevation for a Section. This is independent of navigation cameras.
  Both coordinates must be finite and within ±1,000,000 m.
- `paper_center_mm`, `width_mm`, and `height_mm`: finite paper geometry, at least
  0.01 mm wide/high and entirely within the fixed page, including its edges.
- `scale_denominator`: a finite value in 0.001–1,000,000. Model metres convert to
  paper millimetres using `1000 / scale_denominator`. Scale belongs to each
  viewport independently of its source view context.
- `title_override`: null (inherit the source view name) or nonempty text of
  1–256 UTF-8 bytes without control characters.

Overlapping viewports and repeated placements of a 2D view are representable;
each has its own ID, model center and scale. Unknown sheet/viewport parameter
fields and unknown point fields are rejected. No title block, title layout,
sheet preview, UI placement, rendering, or PDF output is provided by this
persistence foundation. Storing multiple viewport scales is not evidence of
integrated multi-scale rendering or publishing.

`AddSheet`, `UpdateSheet`, and `RemoveSheet` validate the complete candidate
document and commit atomically with one-step undo/redo. Updating parameters
preserves the sheet header; retained viewport IDs remain stable. A placed view
cannot be removed. To remove it in one transaction, first remove all placements
with `UpdateSheet`/`RemoveSheet`, then issue `RemoveView` (and remove any other
view-owned annotations as required by their existing rules). The reverse order
is rejected without changing the document, revision, events, or history.

Change events identify edited sheets and added/edited/removed viewport IDs.
Placement edits invalidate both old and new source views, sheets and viewports.
Invalidated source views also invalidate their placed viewports and sheets;
this includes model geometry edits, view-setting or section-definition changes,
removal, undo and redo. The before/after dependency union also propagates to
extension dependents.
Sheet maps, headers, strings, viewport capacity and title strings count toward
the existing document-history memory budget.

Migration 13→14 inserts an empty `sheets` map and advances the root and all native
headers (including room tags) from 13 to 14. All earlier IDs, parameters,
properties, relationships, plugin requirements, extension envelopes and payload
schemas remain unchanged. A pre-existing `sheets` field, missing/invalid native
maps, or contradictory native header version rejects atomically. The frozen
`crates/os-storage/src/tests/schema-13-sheets.json` fixture includes a room tag
and opaque extension data. Tests in `crates/os-storage/src/tests/sheets.rs`
verify exact preservation, real archive migration without rewriting the source,
and schema-14 save/reopen of viewport IDs, centers, sizes, titles and independent
scales with a rotated source plan. Container version remains 2. The desktop paper
preview and PDF path currently support a single Plan or Section viewport per
sheet; multiple persisted viewports are not yet composed.

## Schedule definitions (schema 16)

The required `schedules` map contains model-owned `core.schedule` entities with
stable native headers. They are independent of geometric views and sheet
viewports. `ScheduleParams` stores `name`, `category` (`Door`, `Window`, `All`),
ordered `columns` (`Name`, `Type`, `Level`, `Host`, `Width`, `Height`, `Sill`, `Id`),
and ascending `sort` (`LevelKindName`, `Name`, `Type`, `Width`, `Height`, `Sill`).
Stable instance ID breaks row sort ties. Row values are never persisted.

At most 256 schedules are allowed. Names must be trimmed, nonempty, at most 128
UTF-8 bytes, and free of control characters; Unicode lowercase names must be
unique in the model. Columns must contain 1–8 distinct supported enum values.
Unknown fields/enum values, wrong header types/versions, nil/duplicate/mismatched
IDs and dangling header relationships fail validation. Header allocations, name
capacity and column capacity count toward snapshot retention memory.

`AddSchedule`, `UpdateSchedule`, and `RemoveSchedule` use atomic document
transactions and change events, with one-step undo/redo. Updates preserve headers
and UUIDs. Definition drafts are UI-only and cancel on stale session/revision.

Migration 15→16 requires `schedules` to be absent (even an empty or null value is
ambiguous), inserts `{}`, and advances the model and every existing native entity
header from 15 to 16. All existing native maps and source header versions must be
valid. Opaque properties, extension envelopes/payloads, IDs, and plugin requirements
are preserved. Migration runs on a copy, validates the complete resulting model
and serialized size, then adopts it. The 14→15 step continues into 15→16; no old
document receives an invented schedule. Container version remains 2.
`crates/os-storage/tests/schedules.rs` covers representative schema-15 input built
independently from a frozen fixture, atomic malformed/ambiguous rejection, opaque
data preservation, and multiple configured definitions saved/reopened together.
Schedule sheet placements, table pagination, CSV, room schedules and family
authoring are outside this slice.

## Section view definition (schema 15)

`ViewParams.section` is an optional, versioned `SectionViewSettings` object.
When present, it is valid only for a `Section` view and carries finite start/end
points in world XY plus absolute bottom/top elevations, all in metres. The
associated `ViewParams.level` is the intended plan-marker level. The line must
be longer than one micrometre and no longer than 10,000 km; the vertical range
must be positive and no larger than the same bounded extent. Plan and perspective
views may not carry section settings. Creating a Section view through a document
edit requires both its marker level and section definition.

Migration 14→15 advances the model and native entity headers while preserving
existing view parameters; legacy views acquire no inferred section definition.
If a schema-14 view already contains a `section` field, migration rejects the
ambiguous input. The data type and compatibility behavior are documented in
[native-sections.md](native-sections.md). The desktop currently provides a
two-click plan marker, linked contour drawing for supported native walls/openings
and floors, and source selection. Section sheet placement and cut fills/poché
remain unimplemented. The separate `os-geometry::section::vertical_section`
kernel helper operates on already-built closed meshes.

## SQLite migration path

`StorageBackend` isolates the format implementation. A future container version
will use `model.sqlite`, normalized entity/relationship tables and a schema
migration table. Its importer must still read the current ZIP/JSON format and
produce the identical semantic model with unchanged UUIDs. Choose the new format
through manifest version dispatch; do not silently reinterpret `model.json` as
SQLite or discard extensible properties.
