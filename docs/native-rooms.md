# Native room placement and derived areas

The Architecture ribbon's **Room** tool places a numbered room by clicking
inside one enclosed face in an active floor plan. Rooms appear in the plan and
Project Browser; selecting one shows its number, name, derived area, and any
enclosure diagnostic. Number/name and finish-code edits, placement, deletion, undo, and redo are
native document transactions. Escape exits placement without changing the model.

Room identity is stable. The model stores the level, seed point, labels, and a
canonical directed cycle of boundary UUIDs; it does not store a frozen
polygon or area. Boundaries and areas re-derive from same-level native straight
wall centerlines and level-owned room separation lines. Every boundary ID is
globally unique, so the existing `(UUID, direction)` signature represents either
source without changing legacy wall-only room data. The persisted cycle is a
topology signature, so endpoint/length edits with the same enclosing boundaries
update the area, while a changed/deleted partition reports an unresolved room
instead of silently assigning the seed to a different face. Such boundary edits
remain allowed, and undo can restore the prior enclosure. Hosted doors and
windows remain part of their host wall boundary.

The **Room Separator** tool draws straight, zero-thickness XY lines on a level.
Two clicks place a line; its endpoints and body can be edited in the plan.
These lines participate in enclosure on every plan of that level. Their visible
strokes, picking, and snapping follow the active crop. The room calculation
ignores view crop, display visibility, wall thickness, and
hosted openings. Crop clips the room graphics and prevents placement outside the
active crop; it does not alter area. This initial area convention uses boundary
centerlines, not finish-face offsets or net room area rules. Room annotations are
2D only; no room solid/volume is generated in 3D.

Room tags are separate native entities scoped to one plan view and a stable room
UUID. Each room can have one tag per plan view. The badge position is editable by
dragging the tag or applying X/Y in Properties; its displayed number and name
resolve from the current room, while the derived area stays at the room seed.
Deleting a room or moving it to another level leaves an orphan-safe tag with a
diagnostic in the plan, Browser, and Properties. The badge remains selectable
and movable; deleting its owning view requires removing its tags in the same
transaction. A badge anchored inside the view crop is clipped to the crop, with
rendering and picking sharing the visible bounds; placement and Properties
positions whose anchor is outside that crop are rejected.

The Room Tag tool accepts a selected same-level room or a room-face click,
followed by a position click. An explicit tag replaces the automatic number/name
label in its view. Placement and dragging preview without changing the model or
history, then commit one undo/redo step. A tag press wins over canvas pan and an
overlapping selected wall endpoint; drags begun elsewhere still pan. Escape,
document revision/session changes, provider changes, view/crop changes and drawing
replacement cancel a held drag through release. Outside-canvas or outside-crop
release preserves the saved position. Browser selection focuses the owning view,
and cropped orphan tags remain reachable there.

The face solver is deterministic on a 1 µm coordinate lattice. It accepts at most
1,024 combined wall and separator segments and 16,384 split entries, with coordinates within ±1,000,000
m. It nodes segment intersections and T-junctions. Collinear overlaps, nested
loops/holes, ambiguous topology, and resource-limit cases produce diagnostics;
open dangling chains do not prevent closed faces from being used. Plugin-defined
elements do not provide room boundaries.

Schema 18 projects migrate to schema 19 with an empty required
`room_separation_lines` map and updated native headers. Existing room IDs, seeds,
and directed wall signatures are preserved. Changed separator or wall topology
can leave a room unresolved; it is not silently reassigned to a nearby face.
Schema 5 projects migrate to schema 6 with an empty room map and updated native
entity headers. Schema 12 projects migrate to schema 13 with an empty room-tag
map and updated native headers. Automated evidence covers geometry, face
identity, room command history/invalidation, frozen migrations, room-tag
orphaning and save/reopen, and egui placement/move/selection/cancel/undo at
1280×800/100% and 1000×650/150%. Native manual visual inspection has not been
performed.

Not implemented: curved/layered/plugin hosts, holes,
wall-finish area offsets, finish-material quantities, legends,
phases, or IFC room exchange. See the [coverage ledger](2d-coverage.md) for the
remaining architectural workflow and release gates.

## Room Finish schedules

Room Properties includes optional Floor Finish, Wall Finish and Ceiling Finish
codes. An empty field means no code. Present codes must be trimmed, contain no
control characters, and use at most 128 UTF-8 bytes. These are text codes, not
physical materials. Each finish also has an independent project material selector
stored as a stable UUID. **Apply room properties** commits labels, codes and references together
in one `UpdateRoom` transaction. Escape discards the draft; a changed document
session or revision reloads current properties and cancels stale edits.

Floor/Wall/Ceiling Finish cells use the live catalog name and code as
`Oak · F-01` when both are present. Assigned materials without codes show the
name; unassigned finishes show their code. Renaming a material updates the
schedule and sheet/PDF table without rewriting room intent. Color edits preserve
cell text. The existing eight columns and default order are unchanged.

The active plan paints a subdued floor-material RGB fill inside resolved room
polygons, including concave boundaries. The fill is clipped to the view crop and
painted beneath walls, room outlines, text, tags and selection graphics. Losing
enclosure removes the polygon and fill while preserving codes and references.
This is a 2D display effect; it generates no room solids or floor construction.
Wall and ceiling references provide finish identity only. There are no material
quantities, finish-face/net areas, wall/ceiling takeoffs, legends or IFC associations.

Schema 34→35 atomically inserts the three required nullable material fields,
rejects pre-existing fields in schema 34, and preserves existing codes, IDs,
signatures, materials/colors, extensions and schedule definitions. All non-null
references must resolve; removing a referenced material rejects the entire
transaction. At the schema-36 milestone, native API 17 required schema 36.
Focused evidence is in
`os-model/tests/room_finishes.rs`, `os-document/tests/rooms.rs`,
`os-storage/tests/room_materials.rs` and its frozen schema-34 fixture,
`os-ui/src/opening_schedule/rooms/tests.rs`,
`os-ui/src/plan_workspace/room_materials.rs`, and
`os-ui/src/plan_workspace/sheet_tests.rs`. Headless UI and fill checks cover
1280×800/100% and 1000×650/150%; native-window and physical-print qualification
remain open.

In **Schedules**, choose **New Room Finish schedule**. Save a name, choose and
order up to eight columns: Level, Number, Name, Floor Finish, Wall Finish,
Ceiling Finish, Area and Enclosure Status. Sort ascending by level/number,
number, name or area; ties use the room UUID and unresolved areas sort last.
Number sorting is lexical. Door/Window/All definitions keep their opening-only
meaning. Switching between opening and room categories clears filters and resets columns and sort
to the new category's defaults.

Rows retain room UUIDs and clicking a cell selects the room and opens a plan on
its level. If none exists, selection remains available with guidance to create
a plan. Area resolves from current same-level boundary topology on every row
build; no area or row cache is persisted. Partition edits update the plan and
table. A broken or changed enclosure preserves the room and finish codes,
shows a diagnostic, and leaves Area blank. It never publishes zero as an
unresolved area. Add the saved schedule to a sheet using the existing single
table placement; preview and vector PDF use those same live rows and explicit
fit checks. See [sheet preview](native-sheet-preview.md).

Schema 32→33 atomically initializes all three required nullable fields to null,
preserving IDs, boundary signatures and legacy schedule definitions. Native
full-model plugins now require API 26/schema 45; generic API 2 and container 2 remain
unchanged. Frozen migration and save/reopen evidence is in
`crates/os-storage/tests/room_finishes.rs` and its schema-32 JSON fixture.

Focused evidence:

- `crates/os-model/tests/room_finishes.rs`: code validation and category-specific
  column/sort validation.
- `crates/os-document/tests/rooms.rs`: atomic finish updates, identity and history.
- `crates/os-ui/src/opening_schedule/rooms/tests.rs`: current topology, ordering,
  unresolved rows, room navigation, keyboard finish editing, one-step history,
  Escape and stale revision/session cancellation at both display profiles.
- `crates/os-ui/src/opening_schedule/definitions/tests.rs`: saved Room Finish
  creation and category switching at both display profiles.
- `crates/os-ui/src/plan_workspace/sheet_tests.rs`:
  `room_finish_sheet_live_area_preview_pdf_and_explicit_overflow_at_both_dpis`
  covers live area reconciliation, preview/PDF text, blank unresolved area,
  diagnostic output, save/reopen and atomic fit rejection at 1280×800/100% and
  1000×650/150%.

The schedule uses the existing centerline area convention. Finish-face or
regulatory area rules, finish quantities, phases/options, legends, CSV,
pagination and multiple-table UI remain outside this increment.

### Room Finish validation checkpoint (2026-09-26)

- `cargo test -p os-ui --all-features --locked --offline room_finish --lib --quiet`:
  4 passed, covering definition controls, live rows, property input and sheet/PDF.
- `cargo test -p os-model --all-features --locked --offline --test room_finishes --quiet`:
  2 passed.
- `cargo test -p os-document --all-features --locked --offline --test rooms --quiet`:
  2 passed.
- `cargo test -p os-storage --all-features --locked --offline --test room_finishes --quiet`:
  2 passed.
- `cargo test -p os-plugin-api --all-features --locked --offline --quiet`:
  6 passed, including rejection of native API 13.
- `cargo test --workspace --all-features --locked --offline --quiet --no-fail-fast`:
  597 passed, 0 failed, 3 ignored; UI library: 175 passed, 2 ignored.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`:
  passed.

### Paths changed for this increment

Earlier dirty changes in these files were retained. Room test constructors and
older storage migration expectations acquire the new fields or null defaults.

```text
crates/os-model/src/lib.rs
crates/os-model/src/memory.rs
crates/os-model/src/rooms.rs
crates/os-model/src/schedules.rs
crates/os-model/tests/room_finishes.rs
crates/os-model/tests/room_tags.rs
crates/os-document/tests/rooms.rs
crates/os-document/tests/room_tags.rs
crates/os-document/tests/room_separation_lines.rs
crates/os-storage/src/lib.rs
crates/os-storage/src/tests/sheets.rs
crates/os-storage/tests/rooms.rs
crates/os-storage/tests/room_tags.rs
crates/os-storage/tests/room_separation_lines.rs
crates/os-storage/tests/schedules.rs
crates/os-storage/tests/room_finishes.rs
crates/os-storage/tests/fixtures/schema-32-room-finishes.json
crates/os-ui/src/lib.rs
crates/os-ui/src/palettes.rs
crates/os-ui/src/room_tools.rs
crates/os-ui/src/opening_schedule.rs
crates/os-ui/src/opening_schedule/definitions.rs
crates/os-ui/src/opening_schedule/definitions/tests.rs
crates/os-ui/src/opening_schedule/rooms.rs
crates/os-ui/src/opening_schedule/rooms/tests.rs
crates/os-ui/src/plan_workspace/sheet_tests.rs
crates/os-plugin-api/src/lib.rs
crates/os-plugin-host/src/lib.rs
crates/os-plugin-host/tests/wasm_transport.rs
plugins/walls/plugin.toml
plugins/walls/src/lib.rs
fixtures/wasm-probe/plugin.toml
fixtures/wasm-probe/probe.wat
README.md
docs/native-rooms.md
docs/native-sheet-preview.md
docs/file-format.md
docs/plugin-api.md
docs/2d-coverage.md
```

## Room-separation acceptance evidence

`crates/os-geometry/tests/rooms.rs` covers mixed wall/separator cycles, separator
closure and partition-driven `FaceChanged` behavior. `crates/os-model/tests/room_separation_lines.rs`
and `crates/os-document/tests/room_separation_lines.rs` cover model validation,
same-level boundary collection, atomic commands, invalidation and history.
`crates/os-render/tests/room_separation_lines.rs` covers crop-clipped geometry,
picking and semantic snapping. `crates/os-storage/tests/room_separation_lines.rs`
covers schema-18→19 migration, ambiguous-input rejection and archive save/reopen
without rewriting legacy room identity. Five headless egui tests at
1280×800/100% and 1000×650/150% cover two-click preview/commit, undo/redo,
endpoint/body edits, pan precedence, invalid collapse, Escape, stale revision and
view-change cancellation. Native-window inspection and print qualification remain
open. Full workspace validation remains a separate coordinator gate.

## Room-tag acceptance evidence

- `crates/os-model/tests/room_tags.rs`: per-view/room uniqueness, invalid syntax
  and coordinates, creation level checks, live room labels and retained orphans.
- `crates/os-document/tests/room_tags.rs`: add/update/remove atomicity, duplicate
  rejection, stable identity, invalidation, undo/redo, orphan retention and batched
  view removal after tags are removed.
- `crates/os-storage/tests/room_tags.rs`: frozen `fixtures/schema-12-room.json`
  migration preserves every existing UUID, parameter and non-version header field;
  schema-13 live and orphan tags survive archive save/reopen.
- `crates/os-render/tests/plan.rs`, `room_tag_bounds_crop_and_pick_share_stable_uuid`:
  bounded screen badges, crop/pick agreement and tag UUID hits.
- `crates/os-ui/src/desktop_tests/room_tag_tests.rs`: six real-frame egui tests at
  1280×800/100% and 1000×650/150%, covering placement, move, preview isolation,
  live room rename, history, invalid release, Escape/stale cancellation,
  tag selection/pan/endpoint precedence, and Browser/Properties position/delete
  and cropped orphan access.

Focused commands are `cargo test -p os-model -p os-document -p os-storage -p
os-render --all-features room_tag -- --nocapture` and `cargo test -p os-ui
--all-features room_tag -- --nocapture`. Native-window inspection, print/plot sizing,
leaders/styles, and general element/material/area tags are not accepted by these
headless tests. Full workspace validation remains a separate coordinator gate.
