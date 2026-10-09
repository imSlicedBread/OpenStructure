# Native sheet preview and vector PDF

## Coordinated A3 sheet

Open a named Plan, Section or straight-wall Elevation and choose **New sheet from
view**. The persisted A3 landscape page is 420 × 297 mm; the next unused A-series
number starts at A101. Its first viewport links to the active source. Plans start
at the navigation camera center; Sections and Elevations use their source bounds.

**Add linked view** places an existing named Plan, Section or Elevation in the
layout draft. Up to 64 viewports are supported. Adding never changes existing
frames, source links, placement UUIDs, scales or titles. A new viewport starts at
(210,120) mm with a 100 × 65 mm frame; overlaps are reported until the user moves
or resizes frames. Duplicate placements of a source share its checked drawing.

Select a placement with the picker or on paper. Drag its frame to move it; drag
the bottom-right square to resize it. Exact fields edit paper center, width and
height in mm, source center in metres, scale denominator and title override.
An empty title inherits the source name. Source centers use plan-plane coordinates
for Plans and station/absolute elevation for Sections and Elevations. At 1:100,
one source metre maps to exactly 10 paper mm; at 1:50 it maps to 20 mm.

**Apply layout** validates the entire draft and commits one UpdateSheet transaction.
Undo/redo restores the complete arrangement. Until Apply, the model, history and
3D scene are unchanged. **Reset layout draft** or Escape discards it. Document
session/revision, sheet selection or provider activation changes invalidate a
draft; source definition changes are document edits and also invalidate it.
Navigation occurs only through the selected placement's **Edit source plan**,
**Edit source section**, or **Edit source elevation** action, which discards any
unapplied draft. Return using the sheet picker.

New edits reject overlapping frames, invalid/missing sources, invalid scales,
and crossings of the 8 mm border or bottom 48 mm title-block reservation.
Historical single-viewport sheets retain their existing renderer/storage
contract; opening an old sheet does not rewrite its layout or schema.

## One live saved schedule

Choose a saved door/window or Room Finish schedule and **Add saved schedule to
sheet**. The new table starts at (18,178)–(402,238) mm without rearranging any
viewport. Move/resize its frame or edit its left/top/right/bottom bounds. **Remove
placement** removes the selected viewport or table from the draft; Apply commits
all placement changes together. At least one drawing viewport is required.

Rows, columns, order and heading resolve from the current saved definition at
each page build. Opening edits refresh the plan, linked elevations/section and
table together after their checked drawings regenerate. Room Finish rows retain
the existing live area, catalog-name/finish-code and unresolved-enclosure behavior;
material renames reach both preview and PDF.

All rows and cells must fit. Height, column/heading width, unsupported WinAnsi
text and output budgets fail explicitly; nothing is truncated or paginated.
Text uses fixed 2.5 mm typography and conservative 1.05-em character advances.
Reduce columns or shorten names in the schedule editor when necessary.
Only one table is supported by this UI; legends and split tables remain open.

## Checked graphics and export

The paper preview and PDF consume the same composed SheetPage vectors, with an
independent mapping, crop and clip for every viewport, source titles/scales and
one page frame/title block. Geometry comes from current checked PlanDrawing
snapshots. Sheets with three or more viewports reserve the bottom 7 mm of each
frame for its clipped title/scale caption; model graphics clip above that footer,
without changing the scale or model center. Captions that cannot fit the frame
width fail explicitly. One/two-view title placement remains unchanged. Multi-view
sheet names/numbers must also fit their reserved title-block cells.
Geometry includes provider-owned plan graphics. Sources deduplicate by view
and context, not placement count. The auxiliary cache uses one native derivation
lane and one sequential bounded provider lane, not one worker per viewport;
active model-view lanes remain separate. Obsolete workers drain before replacement
and cannot publish. Missing/incomplete provider output blocks export, including
for a Plan that is not the active model view; no native-only substitute is used.

**Export vector PDF** is enabled only for a checked, applied layout. Choose a
path and confirm export; existing destinations require explicit replacement.
Pending confirmation is invalidated if the sheet/document/provider context or
checked vectors change, or if a draft is edited. A synced same-directory temporary
file is atomically persisted. Export does not save the project or alter its dirty
state. Mark, point, text and PDF-byte budgets remain enforced.

## Automated evidence and limits

`sheet_layout::tests` exercises a plan, two differently scaled elevations,
section and opening schedule at 1280×800/100% and 1000×650/150% through real egui
frames: visible Add/Apply/source controls, frame move/resize, draft immutability,
exact layout values, no automatic rearrangement, one-step history, save/reopen,
live door-width refresh in every source and table, and identical prepared PDF
bytes. It also covers Escape, stale drafts, bad scales/sources, overlap/border/
table-fit rejection and draft export rejection. Exact numeric values are supplied
to the draft in the harness; independent keyboard-entry usability qualification
is still outstanding.

`sheet_sources::tests` covers duplicate-source caching, obsolete-worker drainage,
session/revision rejection and inactive-plan provider failure. Existing
`sheet_tests` retain single/combined sheet, live schedule/filter, room finish,
atomic writing and navigation regression coverage. Render tests verify exact
mixed-scale mappings, clipping, deterministic multi-view output and bounds.

This is not a full production sheet/print system:

- One fixed A3 landscape page; no rotation, independent crop editor, alignment
  snapping/guide grids, custom title blocks, sheet sets, revisions or batch export.
- Elevations still derive from one straight wall, not whole-building occlusion.
- One schedule; no pagination, multiple-table UI, legends or automatic arrangement.
- Standard Helvetica/WinAnsi searchable vector text; fonts are not embedded.
  No independent native-window, external PDF-viewer or physical plot-scale
  qualification is claimed.

No model schema, storage format or plugin API change is required. Existing
SheetParams and placement UUIDs are reused. Historical schema 13→14 sheets and
16→17 schedule placements remain documented in [file format](file-format.md).

```powershell
cargo test -p os-render --lib sheet::tests
cargo test -p os-ui --lib --all-features sheet_
```
