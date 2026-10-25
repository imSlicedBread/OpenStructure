# Native detail drafting and plain notes

The later schema-78/API-59 increment adds [standalone Drafting views](native-drafting-views.md)
to the same line/note editing and output paths. The evidence below describes
the original Plan/Section increment.

Schema 76 / native Model API 57 extends independent drafting on configured Plans
and Sections, including existing enlarged callout targets. Opening-family and
package versions remain 9; container 2 and generic API 2 are unchanged.

Detail lines retain their existing `view/start/end` payload. The owning immutable
ViewKind discriminates coordinates: Plan endpoints remain world XY metres;
Section endpoints are station along its authored start-to-end axis and absolute
project elevation. Section snapshots use their native station/elevation basis,
including rotated/reversed axes. Native visible section segments and detail
lines feed the snap provider. Existing two-click creation, selection, endpoint
editing, dragging, deletion and undo/redo are reused.

Choose **Text note** in the Annotate ribbon. Edit multiline content, wrapping
width and text height in paper millimetres, and Left/Center/Right alignment.
**Apply note** commits the displayed numeric position; **Place / move on drawing**
previews an anchor that snaps to visible endpoints/midpoints and commits on a
canvas click. Clicking an existing note opens its editor. The inspector also
offers **Edit text note**. A checkbox adds at most one straight leader, with a
numeric tip and derived open arrowhead; moving translates the note and its tip.
**Delete note** removes the stable entity in one undo step.

All drafts are separate from the document. Cancel/Escape, pointer/focus loss,
view/document/provider changes and stale revisions discard the draft. A changed
camera/canvas cancels a claimed pointer gesture; pre-press layout transitions
remain valid. Text-entry focus retains typing/Delete
without invoking workspace drawing/deletion shortcuts. Annotations own no model
geometry and are not copied when creating a callout target.

The bounded text contract accepts printable ASCII plus LF newlines, 1–2048
bytes, at most 128 wrapped rows, 1–20 mm text height, and a width from one
character to 500 mm. Unsupported Unicode, tabs and control characters produce
an explicit validation error. Wrapping uses fixed one-em cells and preserves
spaces, blank rows and every character; it is hard wrapping, not word wrapping
or a structured-list feature. Rows have 1.25-em leading. At most 1000 notes and
100,000 text bytes are accepted per model.

One layout function produces glyph anchors and leader/arrow segments for the
native canvas and paper adapters. View scale controls native layout; each sheet
viewport's scale independently lays out the same paper-unit note. Text remains
horizontal in the owning view. The canvas uses its proportional UI font per
cell; PDF uses standard Helvetica per cell. Placement, size, wrapping and
alignment are shared, but this is not an identical-font-outline or typography
qualification. Crop/viewport rectangles clip partial glyphs and leaders; excess
row count or unsupported text is rejected rather than discarded.

Migration 75→76 rejects old input containing `text_notes`, even empty/null, or
detail lines owned by Sections. It adds an empty required map and advances
native headers, preserving Plan line payloads and opaque metadata. Current notes
have strict positions and nullable leader tips. Frozen schema-75 evidence is
`crates/os-storage/tests/fixtures/schema-75-drafting.json`.

## Focused evidence

Commands use `./tools/cargo.ps1`, `--locked --offline`, and test arguments
`-- --test-threads=1` supplied as a PowerShell array. `TMP`/`TEMP` point to
workspace-local `work/issue-test-temp`. No full-workspace suite was run.

| Command arguments | Result |
| --- | --- |
| `test -p os-model -p os-document -p os-storage --test drafting --test detail_lines` | 10 passed: Plan regression, Section/Plan notes, strict migration, preview/history and save/reopen |
| `test -p os-render --test drafting --test detail_lines` | 3 passed: note alignment/leader/glyph anchors and vector PDF at 1:25 and 1:100; existing line clipping/picking/snapping/strokes |
| `test -p os-ui --all-features --lib text_note` | 3 passed: rotated Plan/reversed Section, scales 1:25/1:100 and both 1280×800/100%, 1000×650/150% profiles; native canvas glyph anchors, edit/move/delete, cancel/stale/text focus, camera/canvas gesture guards and history |
| `test -p os-ui --all-features --lib drafting` | 5 passed: rotated/reversed Section lines, immutable preview, endpoint/body edits, cancellation/stale revisions, camera/canvas gesture guards and undo/redo at both profiles |
| `test -p os-ui --all-features --lib detail_line_tests` | 4 passed: existing Plan detail-line interactions after the camera guard |
| `test -p os-plugin-api --lib` | 6 passed: API compatibility and bounded native contracts |

The Section interaction tests initially exposed the absent native section snap
scene; the adapter now supplies its visible segments. Two inherited test
gestures used off-canvas positions at 150% and were corrected to visible points.
Note clicks now include a pointer-hover frame so egui can retire the closed
editor window's hit region. Camera/rect snapshots are captured only when a
pointer press is accepted, and tests explicitly cancel changed mid-drag contexts.
Sandbox Rust builds report incremental hard-link fallback/canonicalization
warnings. A scoped rustfmt attempt failed with Windows `Access is denied`
before formatting; scoped library Clippy also failed configuration discovery
with `Access is denied`. Neither formatting nor Clippy is verified; no
native/unsandboxed retry was made in this increment.

## Remaining limits

No rich text, arbitrary font shaping/language support, font selection, structured
lists, multiple leaders, fills/masks, reusable detail components, annotation
duplication, elevation/facade support, CAD/print
qualification or new opening geometry. This is a bounded drafting increment,
not Revit parity.
