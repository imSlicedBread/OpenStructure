# Independent floor-plan duplication

Choose **Duplicate plan…** beside the plan controls while an authored native
FloorPlan is active. This includes enlarged-plan targets. The dialog supplies
`<source> Copy`, then numbered suffixes, truncating at UTF-8 boundaries within
256 bytes. Names must be nonblank, contain no controls, and be unique across
views after trimming and lowercasing. Existing names are not revalidated.

**Duplicate** copies the view settings and graphics binding. **Duplicate with
detailing** additionally copies all dimensions (Aligned, Chain, Baseline,
Angular, ArcLength), opening tags, room tags, detail lines, text notes and
parent-owned plan callout markers. Ownership UUIDs determine membership,
including hidden and off-crop annotations. The dialog shows counts and explains
that copied callouts become reference-only markers.

Both modes preserve level, phase/filter, range, basis, model/annotation crop,
scale, visibility, local graphics styles and the original template UUID.
The view and each copied annotation receive fresh UUIDs; view settings revision
starts at zero. Native header type/properties are preserved; explicit header
relationships within the copy set are remapped, external references retained,
and UUID-looking strings in opaque properties left untouched. Detailing fails
closed when an extension explicitly references the copy set: its payload has
no supported clone contract. Plain duplication remains available.

Copied callouts retain their existing target, bounds and destination, with
`owns_target=false`. Incoming markers continue to target the original view.
Duplicating an enlarged target produces an independent, unmarked plan. Target
views and section/elevation/facade markers are not copied. No sheet viewport,
detail number, schedule placement or drawing issue is created.

The document prepares a disposable candidate using normal AddView, optional
SetPlanGraphics and annotation commands, then validates again at commit.
Creation-time reference checks remain authoritative: retained orphan dimensions
or tags reject detailed duplication with the offending source UUID. Nothing is
silently omitted. Existing global limits, a 4096 copied annotation/marker cap,
and the model's aggregate note-byte limit apply. Any rejection leaves model,
history and events unchanged. Prepared proposals are bound to session/revision;
the dialog also discards its draft when the active view changes. Opening cancels
gestures; dialog input is handled before canvas/workspace shortcuts, including
Delete, while name editing remains available. Escape/Cancel do not mutate.

Apply creates one history entry and navigates to the new plan. Undo/redo removes
or restores the complete copy with identical UUIDs. Subsequent annotation and
view edits are independent; model references and template links remain shared.
**Delete plan** removes owned annotations and outgoing plan markers in the same
transaction before RemoveView. Incoming references, sheet viewports and frozen
issue manifests still guard deletion. Shared target views are never deleted.
Existing issue records remain unchanged; newly placed copies use current
callout issue-reference checks.

## Focused evidence

- `cargo test -p os-document -p os-storage --test plan_duplication --offline`:
  six document tests and one storage test cover both modes, every annotation
  kind/layout, rotated/cropped settings and graphics binding, headers, names,
  orphan/extension/limit rejection, stale proposals, enlarged targets, callout
  destinations, incoming/placement/issue guards, history, independent edits and
  native save/reopen.
- `cargo test -p os-ui --lib plan_duplication --offline`: two tests cover real
  egui interaction at 1280×800/100% and 1000×650/150%, gesture cancellation,
  text editing, canvas/shortcut isolation, Cancel/Escape, Apply/navigation,
  stale session/revision/view, undo/redo/deletion, and matching native annotation
  geometry and vector sheet/PDF output for a rotated, cropped copy.
- `cargo check -p os-document -p os-ui --all-features --locked --offline` passed.
- `git diff --check` passed. Rustfmt and scoped Clippy were attempted but stopped
  with Windows access denial during path/configuration discovery; unverified.

Duplication was introduced at native schema/API **79/60** without a wire change.
Schema/API **80/61** additionally copies per-view hidden-opening IDs in both
duplication modes; see [hidden openings](native-hidden-openings.md).
Family/package, generic API, container and issue-bundle versions are unchanged. V01.24/V01.25 remain
partial: this evidence covers authored FloorPlans, not other view kinds,
dependent views, extension payload duplication, native-window visual acceptance
or physical-print qualification. No full-workspace or Revit-parity completion
is claimed.
