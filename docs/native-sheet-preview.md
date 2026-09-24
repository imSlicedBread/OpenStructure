# Native sheet preview and vector PDF

Open a named Plan or configured Section view and choose **New sheet from view**.
The app creates a persisted A3 landscape sheet (420 × 297 mm), assigns the next
unused A-series number starting at A101, and places one viewport linked to the
active view. Section viewports start centered on their authored section bounds;
plan viewports start centered on the navigation camera. Choose the sheet in the
picker to open paper preview. **Edit source plan/section** returns to model-space
authoring.

Plan viewports start centered on the current plan camera; Section viewports start
centered on the section's authored bounds. The viewport uses the source context's
scale (currently 1:100 for Sections). **Apply layout** commits a new viewport
scale and paper-space center (millimetres) as one undoable edit. At 1:100, one
source-view-plane metre maps to exactly 10 paper millimetres. Model center is
persisted separately from the navigation camera. Changes to source geometry,
Plan settings, or section definitions regenerate the checked 2D drawing before
preview/export.

Preview and export consume the same paper-space marks generated from the checked
`PlanDrawing`. Plan viewports can contain walls, grids, floors, rooms, room tags,
dimensions, angular dimensions and provider lines. Section viewports contain
native wall/opening/floor cut contours. Both use crop clipping, viewport clipping,
a simple frame, and a basic title block. A stale drawing or unavailable source
element blocks export rather than silently publishing partial geometry. Export
asks for an explicit path and confirmation; existing files require explicit
replacement consent. A same-directory temporary file is synced and atomically
persisted, and export does not save the native project or alter its dirty state.

## Saved schedule table (bounded slice)

In paper preview, choose a saved schedule and **Add saved schedule to sheet**.
The visible layout description explains the combined layout before applying:
viewport 384 × 124 mm centered at (210,80), table 384 × 84 mm with top-left at
(18,154). One `UpdateSheet` changes layout and placement; Undo restores both.
**Remove schedule table** removes the placement and retains the smaller viewport.
Rows, columns, order and heading resolve from the current saved definition on
each page build. Preview and vector PDF share the resulting searchable paper marks.

All rows/cells must fit. Height, column/heading width, unsupported WinAnsi text,
page and output budgets fail explicitly; composition errors disable export.
Text uses fixed 2.5 mm typography and a conservative 1.05-em advance per character;
some text which could fit with exact font metrics may be rejected. Reduce saved
columns or shorten names in the schedule editor. There is no automatic truncation.
Prepared PDF confirmation is cancelled if the document session/revision changes.

This slice has no numeric table layout editor, cancelable placement draft, free
table movement, multiple-table UI, pagination or split tables. Add/Remove are
immediate undoable actions. Headless egui tests cover the visible controls at
both display profiles and exercise the same transactional placement path;
independent native visual, viewer and physical print checks remain open.

This is an early workflow, not a complete sheet/print system. Current limits:

- One fixed A3 landscape size, one Plan or Section viewport per sheet, and one
  PDF page. Persisted multiple viewports are not yet composed or rendered.
- No viewport rotation, independent viewport crop, custom title blocks, sheet
  sets, revision/issue workflow, batch export, or native printing.
- PDF uses vector paths and searchable standard Helvetica/WinAnsi text. Fonts are
  not embedded; text outside WinAnsi is rejected. There is no independent PDF
  viewer or physical plot-scale qualification yet.
- Line weights and simple fills are emitted, but office styles, transparency,
  hyperlinks, bookmarks, mixed page boxes, and production font substitution are
  not supported.

The persisted sheet and viewport entities were introduced by schema 13→14; the
schedule placement field is added by schema 16→17, both documented in [the
native file format](file-format.md). Focused UI
evidence is in `crates/os-ui/src/plan_workspace/sheet_tests.rs`; it covers
1280×800 at 100%, 1000×650 at 150%, exact scale mapping, undo/redo, save/reopen,
PDF page dimensions, output-state preservation, and no-clobber writing.

```powershell
cargo test -p os-render sheet::tests
cargo test -p os-ui --all-features sheet_tests
```
