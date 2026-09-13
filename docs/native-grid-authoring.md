# Native grid authoring checkpoint

Observed on Windows, 2026-09-12, using the default built desktop executable in
`work/completion-build/debug/os-app.exe`, not a fixture launcher. Window capture
was 1282×832 including nonclient chrome. This is one native workflow checkpoint,
not a production platform/DPI qualification. The computer-use skill required
one observed action followed by refreshed state; no existing project was replaced.

## Observed workflow

1. Start a new project, create Floor plan 1 on Ground, and open New grid.
   All fields, preview and Apply/Cancel were visible. Apply the default grid
   (-3,0)→(3,0); the datum appears in plan and Properties reports its endpoints.
2. Edit grid, change End X from 3 to 4. The preview changes from 6 m to 7 m.
   Apply visibly extends the datum; Undo restores End X=3 and Redo restores 4.
3. Reopen Edit grid, enter NaN in End X and press Apply. Validation rejects it
   and retains the draft; Cancel returns to the unchanged End X=4 datum.
4. Enable split 2D/3D. Start Draw wall in plan and click at window (624,534),
   approximately five logical pixels below the visible datum. GridAxis marker
   appears on the actual line. Enter length 2 m and angle 90°, then commit at
   (624,399). One perpendicular wall appears in both plan and 3D; the grid remains.
5. Save to the previously nonexistent `outputs/native-grid-authoring.osb`.
   Open that file, choose the saved plan from Browser, and observe both elements
   again. Picking the wall shows Length 2.0 m and matching plan/3D highlighting.
   Picking the grid off-line at (460,533) switches Properties to Grid 1 and
   reports (-3,0)→(4,0). The clean saved window was closed and its absence verified.

Undo of wall creation and grid deletion were not performed through native UI;
automated tests cover gesture history/reference-safe model deletion. Grid editing
does not drive already-created wall coordinates: snapping here is acquisition,
not a persistent alignment constraint or general solver.

## Independent archive assertions

Read `model.json` directly from the resulting ZIP with the .NET ZIP reader;
asserted schema 4, one grid and one wall. Grid start/end are exactly (-3,0) and
(4,0). Wall start is exactly (1,0); end is (1.0000000000000002,2), with horizontal
deviation below 1e-12 m and a 2 m vertical extent. Thickness is 0.2 m, height 3 m.
No screenshot-derived measurement substitutes for those stored assertions.

| Object | Saved identity |
| --- | --- |
| Grid 1 | 7df69f33-4237-4907-b76b-e6041a29f3b8 |
| Wall | dbf0bc93-1cfd-48fc-a4d7-1bdf3573018d |
| Floor plan 1 | c0b71d36-626d-47ee-b730-a188552c3a6e |
| Ground | bad436c9-5aeb-4bdd-b6ac-40c764264020 |
| Building | 04e2fb86-77c1-467c-b9b8-8e1a17d96603 |

Plan settings remain version 1/revision 0: range 2.5/1.2/0/−1 m, origin zero,
zero yaw, no crop, scale 1:100, native walls and extensions visible. Native schema
and header versions are 4; no new format or plugin protocol change was needed.

## Inspection-driven correction and remaining scope

Invalid Apply displayed the same validation message twice (preview and commit).
The form now displays a matching message once, in the footer, with a regression
assertion in the existing six-layout input test. Distinct transaction/staleness
errors still remain visible. That wording/layout correction was made after the
native run; its verification is automated, not a second native inspection.
The targeted six-layout grid-form test, strict all-target/all-feature Clippy,
default workspace build and format check passed after the correction. The full
suite was not rerun for this message-deduplication-only change.

The prior full workspace checkpoint was 211 passing tests. Grid numeric authoring,
edit/undo/redo, invalid/cancel, bundled grid-snapped wall placement, shared selection
and save/reopen now have this bounded native evidence. D remains incomplete:
wall move/resize/offset, fuller snaps/constraints and independently installed
plan/pointer providers remain. E1–E4/G1–G6, production targets and license decisions
are unchanged; L1/L2 remain later milestones.
