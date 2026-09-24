# Native room placement and derived areas

The Architecture ribbon's **Room** tool places a numbered room by clicking
inside one enclosed face in an active floor plan. Rooms appear in the plan and
Project Browser; selecting one shows its number, name, derived area, and any
enclosure diagnostic. Number/name edits, placement, deletion, undo, and redo are
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
wall-finish area offsets, finish parameters, color fills/legends, room schedules,
phases, or IFC room exchange. See the [coverage ledger](2d-coverage.md) for the
remaining architectural workflow and release gates.

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
