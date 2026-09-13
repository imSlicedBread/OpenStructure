# D: initial native linked plan workspace

Status: a working native-wall plan/3D workspace, not the complete D authoring
milestone. This follows [persisted settings](persisted-plan-settings.md).

## Supported workflow

Choose an active level, then **New floor plan** above the canvas. The host creates
a named plan with persisted defaults through AddView and opens it. Select saved
plans through the picker or Project Browser. Focusing a plan sets the active level
to its associated level without reassigning any selected wall. **3D** returns to
the model view; **Split 2D / 3D** displays the active plan alongside 3D.

The plan consumes the checked semantic polygon drawing: cut outlines are thicker
than projected/depth outlines, and unavailable extension counts stay explicit.
Click footprints to select the same model wall in Properties, Browser and 3D;
click empty space to clear selection. Existing numeric wall edits and Undo/Redo
update both representations. **Fit plan**, drag-to-pan and cursor-anchored scroll
zoom affect navigation only. Cameras are independent per plan within a document
session, and reset on opening a document. Persisted paper scale is not screen zoom.

## Background derivation and failure behavior

`os-ui/src/plan_workspace.rs` retains at most one native plan worker, including
obsolete work draining to completion, and one current drawing. The snapshot copies
only bounded geometry parameters and unavailable UUIDs (at most 10,000 combined
elements), not the whole model, wall names, opaque payloads or plugin state.
Prism generation, kernel checks, cuts and crop clipping run on the worker. Frame
polling checks completion before joining and requests repaint while work is pending.

Document session/revision, view ID, settings revision and actual settings identify
the requested drawing. Changing this context clears old drawing/picking immediately
and retires pending work. Returning to the same view cannot revive a retired job.
No replacement worker starts until the previous one drains. Failures are visible
and not retried every frame; switch away/back or change the model to retry. This
is bounded trusted native computation, not a new plugin sandbox, forced thread
interruption, wall-time guarantee or measured large-project performance claim.

## Verification

The full all-feature workspace suite passes (186 tests including the doctest),
as do strict Clippy, formatting and default build. New worker tests control
completion to verify retired-view rejection, document changes, replacement sessions
and failure retention. The real egui frame/input test creates a wall and plan,
enables split, fits/picks the same wall, verifies 3D selection, pans without model
revision changes and checks plan creation Undo/Redo at normal/compact window sizes.

Windows native inspection used the default executable, not a plugin harness:

1. Create a 5 m wall and **Floor plan 1** through the actual desktop controls.
2. Enable split and fit the plan; observe both panes and selected wall.
3. Change Length to 4 m, Apply; observe both representations shorten. Undo restores
   5 m in both panes. Click empty plan space then the footprint: selection clears
   and returns in Properties/Browser/3D.
4. Save to the new `outputs/native-plan-workspace.osb`, reopen through File, and
   select the named plan from Browser; the native plan regenerates in split mode.
5. Close the clean inspection window. The initial workspace test invocation met
   Windows' running-executable lock; after closing, the complete rerun passed.

The saved archive was independently read back: model schema 3, plan ID
`f132bcc3-803d-4ee4-abd1-f8d9ad3ec766`, wall ID
`71ffa6df-e615-4782-bb01-4e2d2dfac438`, shared level
`fc6a759c-2279-4eaa-93ee-e4de05aa1c69`. Wall endpoints are (0,0)→(5,0), thickness
0.2 m, height 3 m; plan settings version 1 retains documented defaults.

## Remaining D work

Follow-up: [plan-settings forms](plan-settings-form.md) now provide transactional
rename/range/basis/crop/scale/visibility draft controls. Split is an equal-width pair, not
resizable docking. Navigation is not persisted, and 3D retains one session camera.
Grids, snapping, exact pointer drawing/edit gestures, callable external plan/snap
providers, complete style/output adapters and broader native acceptance remain.
Unknown extension geometry is explicitly unavailable even if its 3D provider works.
There is no production plotting, office workflow or performance qualification here.
