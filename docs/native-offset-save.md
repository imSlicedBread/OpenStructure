# Native offset and destination-save checkpoint

2026-09-12, current default build, native window 18221458, 1280×800 client.
The computer-use skill drove pointer/keyboard input and screenshot inspection.

## Observed

Created the default 5 m wall and Floor plan 1, enabled split 2D/3D, then chose
Offset wall. Entered `-2` and positioned the pointer on the positive side. A
non-primary click allowed inspection without committing: the parallel preview
appeared on the negative side with `Offset -2.000 m`, while only one wall was
committed. A primary click created the second wall in both views; the original
wall remained selected and unchanged.

Opened Snaps and verified all controls and priority explanation were visible.
Disabled the master switch; the toolbar changed to Snaps off. This remained a
session preference after reopening the project. Acquisition behavior for the
individual controls is automated-test evidence, not a native claim here.

Quick Save opened the new destination form with Save project disabled until a
path was supplied. Cancel preserved the two unsaved walls. Ctrl+S reopened the
same form. Entering a new absolute workspace path and clicking Save project
succeeded without the earlier launcher-directory access-denied error. Reopened
the saved file and its named plan; both parallel walls displayed in split view.

## Saved data inspected independently

Read model.json directly from `outputs/native-offset-save.osb`:

- Schema 4, two native walls.
- Source `52403d65-ac16-4db2-ad15-fc3bacf9ca63`: `(0,0)` → `(5,0)`.
- Copy `80ed305c-9b89-4623-8d82-2e3862c91637`: `(0,-2)` → `(5,-2)`.
- Both height 3 m, thickness 0.2 m, material null, level
  `4faf4000-15cd-4249-9a81-87194d680462`.
- Plan `4b677a9e-0e5c-4d1e-acb1-7ce261e46475`, Floor plan 1, associated with
  that level, default basis/range and settings revision 0.

The file was new; no prior artifact was overwritten. Native undo of the offset
creation was not performed because the computer-use skill requires action-time
confirmation for deleting application data. Controller and egui tests cover its
undo/redo. Invalid-save retry, acquisition details, positive/rotated offsets and
independent plugins remain outside this native checkpoint. This is not a D or
production-readiness claim.
