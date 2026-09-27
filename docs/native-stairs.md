# Native straight-flight stairs

## Supported element

`core.stair` currently represents one straight flight between two levels in the
same building. The base and upper elevations come from those level entities, so
level edits regenerate the stair instead of leaving a stored rise out of sync.
The plan start is the first riser centre; the end is the far edge of the upper
arrival tread. Width, riser count, vertical structural thickness and an optional
project material are authored in metres. Rise, run, equal riser height and going
are derived measurements, not persisted duplicates.

For `N` risers, the flight has `N-1` intermediate treads and one upper arrival
tread; each going is `run/N`. These are modeling bounds and geometry semantics,
not a building-code or accessibility check.

## Plan workflow and generated views

From an active floor plan, choose **Architecture → Stair**. The active plan level
is the lower level; choose a higher level in the same building, then click the
first riser and the far edge of the upper arrival tread. Existing plan snaps
position both points. Width, riser count and structural thickness are editable
in Properties before placement. The transient footprint/tread preview does not
change the document, undo history or 3D scene. Escape cancels the draft. A valid
second click commits one stair transaction; the repeat tool remains active until
Escape. Selected stairs expose numeric Properties, Apply/Cancel and Delete.

The scene uses one deterministic closed, outward-wound mesh: a stepped top over a
continuous sloped underside, extruded across the flight width. Plan fills, tread
strokes, direction arrow, section contours and vector sheet/PDF graphics derive
from the same checked stair entity/geometry. Commit, undo/redo, direct file open
and staged plugin-project open regenerate the 3D mesh.

## Persistence and limits

Model schema 37 adds a required `stairs` map; the atomic 36→37 migration inserts
an empty map and advances native headers. At introduction, native full-model
plugins used API 18/schema 37; current guests require API 26/schema 45. Generic
API 2 and the `.osb` container version do not change. The
bounded IFC writer refuses projects containing native stairs rather than
silently dropping them.

This increment does not create landings, turns, separate stringers, railings,
slab openings, stair/slab joins, stair quantities or code-compliance results.
An arrival tread may therefore conflict with an upper slab unless the modeler
provides an appropriate opening. A section cut exactly coplanar with an
intermediate riser remains a known degeneracy in the current section kernel;
other longitudinal and transverse cuts are supported. No native-window or
physical-print qualification is claimed.

Automated model, geometry, transaction, migration/save-reopen, plan, section,
scene-regeneration and egui evidence is recorded in the [2D coverage ledger](2d-coverage.md).
