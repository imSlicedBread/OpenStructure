# Native detail lines

Open a configured Plan view and choose **Detail Line**. Click a start point, move
the pointer to preview the straight segment, then click its end point. The preview
does not change the model. A completed line is a view-owned drafting element with
a stable UUID, stored as world XY metre endpoints. It does not change wall geometry
or the 3D model. Press Escape to cancel an unfinished line.

Select a visible line to show its two endpoint handles. Drag a handle to reshape
the line, or drag the selected line body to move it. A blank canvas drag pans the
plan. The line uses the plan's snapping options; editing excludes the line's own
snap segment. An invalid collapsed line is rejected. Each completed create or
edit is one document transaction and participates in Undo/Redo.

Only configured Plan views can own these lines. Endpoints must be finite, within
±1,000,000 metres, and more than 1e-9 metres apart. Removing a view with lines
requires removing those lines in the same transaction. The persistent collection
is bounded at 10,000 lines. Cropping clips visible segments, but the original
endpoints stay in the model. The rendered drafting stroke is solid black at
0.25 mm paper weight in plan, sheet preview, and vector PDF.

## Verification and limits

Focused tests cover model references, coordinates, identity; document history
and view deletion; frozen schema-17 migration and archive reopen; drawing crop,
pick and snap; and sheet/PDF vector weight. Headless egui tests exercise two-click
preview, Escape, handle/body/blank-canvas precedence and history at 1280×800/100%
and 1000×650/150%. These are simulated frames, not native window or physical
print acceptance. Searchable PDF text is verified for the sheet number; the line
itself is a vector path and has no text label.

This slice supports straight, view-owned line segments. It does not provide
curves, line-style editing, filled or masking regions, detail components,
repeating details, detail groups, or a standalone detail view. See the
[A02.01 coverage row](2d-coverage.md) for the acceptance boundary.
