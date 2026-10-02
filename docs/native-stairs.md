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

Select a stair in Properties and add a left or right railing (side is relative
to travel uphill). Each railing is an independent stable-ID, stair-hosted
instance referencing a shared railing type. The first placement creates a
standard type; subsequent instances reuse a compatible type. The shared type
controls top-rail height/section, post dimensions, maximum post spacing and
material. Posts are regenerated at the stair's tread stations; a type is rejected
when its spacing cannot cover a tread or its members do not fit the host. A
selected railing exposes staged instance and shared-type edits, with preview
validation and one undoable apply. Deleting the stair removes all hosted
railings in the same transaction; direct model deletion that would orphan a
railing is rejected.

The continuous sloped top rail and individual posts are separate checked closed
members. The combined mesh feeds the 3D scene; plan centerline/post marks and
section contours retain the railing's identity and flow through sheet/vector-PDF
output. Changes to the host stair, its levels, shared type or material invalidate
the railing and its derived views.

## Persistence and limits

Model schema 37 adds a required `stairs` map; the atomic 36→37 migration inserts
an empty map and advances native headers. Schema 59→60 adds `railings` and
`railing_types`; native plugins now require API 43/schema 62. Generic API 2 and
the `.osb` container version do not change. The bounded IFC writer refuses
projects containing native stairs or railings rather than silently dropping
them.

Schema 60→61 adds the required `ramps` map for native straight-run ramps; see
[native ramp authoring](native-ramps.md). Native plugin compatibility remains
API 43/schema 62.

This increment does not create stair landings or turns, separate stringers,
slab openings/joins, stair quantities, railing extensions, infill panels,
custom profiles, free-standing rails or code-compliance results. The first
railing type is a simple rectangular top rail with vertical posts; only straight
stair hosts and left/right sides are supported. Generated post corners and rail
elevations are validated against the architectural coordinate envelope before
commit. An arrival tread may conflict with an upper slab unless the modeler
provides an opening. A stair or railing-member section cut exactly coplanar
with a riser/triangulation edge remains ambiguous in the section kernel. No
native-window or physical-print qualification is claimed.

Automated model, geometry, transaction, migration/save-reopen, plan, section,
scene-regeneration and egui evidence is recorded in the [2D coverage ledger](2d-coverage.md).
