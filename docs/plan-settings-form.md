# D: desktop plan-settings form

The active plan's **Plan settings** button opens a revision-bound modal draft.
It edits name, associated level, top/cut/bottom/depth offsets in metres, XY origin,
horizontal rotation in radians, scale denominator, native-wall/extension visibility,
and optional rectangular crop in view-plane metres. Existing unassigned legacy
plans require an explicit level selection; opening the form does not guess one.

**Apply plan settings** validates all enabled fields and submits one UpdateView
transaction. Incomplete/nonfinite input, invalid ranges/crops/names/references and
stale document/view drafts retain the form with an error and do not commit.
**Cancel plan settings** discards drafts. An unchanged Apply preserves redo.
Global history/save shortcuts are blocked while the modal is open. The scrollable
field region keeps Apply/Cancel outside its scroll area. Paper scale remains
separate from navigation zoom; this form does not implement plotting.

## Evidence

- `os-ui/src/plan_settings.rs` tests invalid text, finite checks, multiple-field
  atomic edits, one Undo/Redo, unchanged Apply preserving redo, document/session/
  view staleness, and exact settings values after native save/reopen.
- The real desktop frame/input test checks whitespace-name rejection, Cancel,
  rename/Apply/Undo, blocked Ctrl+Z and visible footer actions at 1280×800 and
  1000×650, each with 1.0/1.25/1.5 scale factors. The first test attempt used an
  empty text event, which does not delete egui's selected text; it was corrected
  to enter whitespace and assert the actual draft before applying.
- Native Windows inspection opened the existing `outputs/native-plan-workspace.osb`,
  selected its saved plan, opened settings and enabled the rectangular crop.
  All four crop fields and footer actions were visible. Wall visibility was then
  disabled in the same draft; Apply hid the plan footprint without deleting its
  browser/model element. One Undo restored the footprint and clean saved state.
  Reopening settings confirmed both wall visibility enabled and crop disabled.
  Cancel and clean Close completed inspection without writing the original file.

This adds no model/container/plugin version change, dependency or unsafe code.
The locked all-feature workspace suite passes (188 tests including the doctest).
Native visual evidence is limited to the observed Windows window; automated DPI
tests do not establish other OS/compositor behavior. Snapping, exact pointer tools,
external plan providers and production drawing/output workflows remain unfinished.
