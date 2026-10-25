# Native plan overhead lines

Native floor plans retain supported model geometry above the cut plane and below
the top clip as `PlanRole::Overhead`. The classifier uses the absolute plan range
and the existing 1e-9 m geometry tolerance. Geometry that crosses the cut remains
cut geometry; overhead applies to the above-cut portion. View range, crop,
visibility, ownership, phase, and model revision checks remain in force.

The current coverage includes straight and profiled walls, columns, casework,
curtain components, railings, sloped roofs, stairs, ramps, and horizontal slabs.
Upper roof, stair, and ramp fragments carry the overhead role through the shared
plan drawing. Overhead polygon and floor graphics have no fill. Their boundaries
remain visible and selectable; screen hits and area selection use the clipped
outlines, while interior picking is reserved for cut/projected geometry. The
overhead role does not create new snap features, and overhead stair/ramp lines are
excluded from their derived snap segments.

Without a resolved category or element style, overhead uses a 0.18 mm gray dashed
stroke with a 3 mm dash and 1.5 mm gap. Existing projected category/template
styles provide color and weight when configured. Phase styles, graphics rules,
and per-element overrides retain their established precedence; projected rules
also style overhead, so an explicit solid override is honored. Canvas and sheet
output use the same resolved appearance.

This is projected overhead graphics, not full occlusion-based hidden-line
removal. Reflected ceiling plans remain outside this work. Doors and windows
above the cut plane are omitted because their current native symbols are cut
specific; above-cut symbol generation remains future work. Independently
editable overhead style libraries are also not included.

Focused evidence covers vertical range boundaries at nonzero levels, default and
explicit overhead styles, outline-only wall/floor acquisition, and no floor fill
in sheet output. Remaining validation should include rotated crops and mixed
roof/stair/ramp transitions as those geometry families evolve.
