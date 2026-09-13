# Native Windows checkpoint: snapped exact-input walls

Status: the bundled-provider two-wall pointer workflow is observed in the native
Windows application. This is a bounded D checkpoint, not D or production completion.

## Observed workflow

1. Start the current default `os-app.exe`; create Floor plan 1 on Ground and enable
   Split 2D / 3D. Begin Draw wall in plan and click a start point in the plan.
2. Enter Length 4 m and Angle 0° in the gesture's exact-input fields, then click
   the endpoint. Observe a wall footprint and matching 3D wall.
3. Begin another wall and click about six logical pixels beyond the first wall's
   endpoint. Observe the Endpoint marker at the actual endpoint. Enter Length
   3 m and Angle 90°, then commit. Both panes show the connected perpendicular walls.
4. Start a third wall and press Escape after its first click. The tool exits,
   leaving two walls in the browser and both panes.
5. Save to the new `outputs/native-pointer-walls.osb`, reopen through File, and
   select Floor plan 1 from Browser. Both walls regenerate in the split workspace.
6. Click each reopened footprint. Properties displays the corresponding 3 m or
   4 m length, with matching browser selection and 3D highlighting.
7. Close the clean inspection window. Existing evidence files and other windows
   were not overwritten or discarded.

Creation Undo/Redo remains covered by automated real-input/controller tests;
this particular native run did not exercise native Undo of wall creation.

## Archive check

The saved ZIP's model.json was independently inspected and assertions checked two
walls, lengths 3/4 m within 1e-12 m, and exact equality of the shared endpoint's
stored X and Y values. This verifies semantic data, not merely screen appearance.

| Entity | ID | Geometry |
| --- | --- | --- |
| First wall | df965a8f-c122-41f2-8c46-6d77aa3e7749 | (−1.984615384615385, −0.8) → (2.0153846153846153, −0.8), 4 m |
| Second wall | 2a2e8e6b-4a9c-4858-9a9e-009a7580b49a | (2.0153846153846153, −0.8) → (2.0153846153846153, 2.2), 3 m |
| Plan | 16eadb2e-a4e1-40ca-99b3-dfbd917dae03 | Floor plan 1, default settings version 1 |
| Shared level | 6154822b-86f2-4f64-bdf7-3e9f43f37933 | Ground |

Both walls retain 0.2 m thickness and 3 m height; native headers/model use schema 3.
The arbitrary initial origin reflects the freely chosen first pointer position,
not a grid or exact-coordinate input claim.

## Inspection-driven correction and remaining work

The first-click idle state initially displayed a premature zero-length error.
It now gives a neutral instruction to choose the other endpoint or enter exact
dimensions. The real-input regression test asserts that instruction. Actual
zero-length/invalid commits still fail; validation was not weakened. This wording
correction was tested after native inspection, not separately observed natively.

Grid/axis authoring and snapping, exact offsets, move/resize tools, richer previews,
independent plugin pointer/plan providers, native broader-size/platform coverage,
and drawing/output qualification remain unfinished. No new dependencies, format
versions, licenses, publishing or production-readiness claims are introduced.
