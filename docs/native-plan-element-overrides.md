# Native plan per-element graphics overrides

Native schema 82/API 62 adds view-specific stroke overrides for elements in an
authored FloorPlan. Open the plan's settings, expand “Per-element graphics
overrides,” choose a wall, door/window, or slab on the plan level, then set the
cut and/or projected stroke. Each enabled role stores a complete color, paper
weight, and solid/dashed pattern. Turning a role off, or resetting the element,
returns it to inherited graphics.

Resolution order is category or linked-template stroke, then phase graphics,
then schema-83 [live rules](native-plan-graphics-rules.md), then the explicit
per-element stroke. A linked template never erases or
supersedes an element override. Door/window overrides affect their plan symbols;
the wall remains responsible for its host aperture. Slabs support projected
boundary strokes only. Selection highlighting remains a temporary canvas
overlay.

Assignments are stored even when an element is phase-excluded, category-hidden,
outside the current view range, or cropped. At draw time, only geometry and
roles that are actually present are styled, so an override cannot make hidden
geometry reappear. Deleted targets and targets that become incompatible with
the plan are pruned transactionally. The same resolved plan drawing feeds the
canvas, sheet preview, and vector PDF path.

This is deliberately narrower than Revit's graphics controls: only complete
native FloorPlan strokes for walls, door/window symbols, and slab boundaries
are supported. Property-by-property inheritance, fills,
halftone, transparency, visibility overrides, annotations, RCP/section/elevation
views, provider geometry, and other model categories remain future work.
Rule-based overrides are separately implemented as partial V02.03 in schema 83,
using predicates evaluated from current data rather than saved matching IDs.

The schema 81→82 migration initializes an empty assignment map. Because the
native plugin request contains the full model, this schema also advances the
native plugin contract to API 62; API 61 and older full-model guests need a
rebuild. Generic API 2, container 2, and family/package 10 are unchanged.

Focused acceptance covers view isolation, phase/category precedence, undo/redo,
invalid target rejection, target deletion cleanup, save/reopen, and shared plan
drawing resolution. Native-window and physical-print qualification remain open.
