# D: checked horizontal plan geometry and semantic drawing foundation

Historical geometry checkpoint. The subsequent [persisted settings slice](persisted-plan-settings.md)
replaces the transient `NativePlanSettings` described below with model schema 3
settings and named-view commands. The 172-test count below belongs to this earlier
checkpoint; neither checkpoint includes a desktop plan workspace.

Status: implemented and tested geometry/controller foundation, **not a desktop
floor-plan editor**. Work begins after the [bounded B/C baseline audit](bc-baseline-audit.md).
This slice adds no dependency, unsafe code, native format change or callable
plugin operation. It does not treat the existing `ViewKind::Plan` enum as a UI.

## Actual implementation

- `os-geometry::plan`: finite ordered ranges, level-relative-to-absolute height
  conversion, horizontal orthonormal view basis, checked rectangular-prism cuts/
  downward projections, rectangular crop clipping and geometric containment.
- `os-render::plan`: a bounded initial polygon drawing representation carrying
  semantic entity UUIDs, deterministic order, exact reverse-order picking, explicit
  unavailable IDs, and document/view/settings-bound consumption. A separate 2D
  camera supports screen/plane conversion, pan and cursor-anchored zoom.
- `Editor::native_wall_plan`: derives known native straight walls from the current
  validated model using the existing `os_walls::wall_solid` recipe and kernel checks.
  It requires an actual Plan view with a valid associated level. Native walls
  created by the external Wall guest have the same authoritative parameters; opaque
  extensions are reported unavailable rather than interpreted as wall-like JSON.

`NativePlanSettings` is currently caller-supplied transient controller data. The
existing view ID/name/level are model data, but range/basis/crop persistence and
versioned settings migration are **not implemented here**. Tests seed a Plan view;
users cannot yet create/select this plan through desktop controls. The controller
derives synchronously and is not called from desktop frames. It must be moved to
the appropriate bounded background/cache path before expensive interactive use.

## Initial straight-wall policy

Metres and `f64` remain authoritative. The horizontal basis has +X right, +Y up
and +Z normal; yaw is radians. Subtract XY origin before basis rotation; convert
to downward screen Y only in `PlanCamera`. Camera pixels are logical screen units,
not physical paper scale or device DPI. Paper transforms are still required.

Range order is `depth <= bottom <= cut <= top`, finite with a positive total span.
Default level-relative offsets are top 2.5 m, cut 1.2 m, bottom 0 m and depth −1 m.
These are development defaults, not an approved office or jurisdiction standard.

| Prism relative to absolute range | Initial role |
| --- | --- |
| Base at/below cut, top above cut | Cut polygon |
| Top at/below cut, above bottom | Projected polygon |
| Top at/below bottom, above depth | Depth polygon |
| Top at/below depth, or base at/above view top | Hidden |
| Entire prism above cut | Hidden by the initial native-wall policy |

Height classification uses 1e-9 m tolerance. A top face touching cut is projected;
a base at cut with material above it is cut. Top touching bottom is depth, while
top touching depth is hidden. No overhead/symbolic representation is inferred.
Roof/ceiling projection, wall joins/layers/openings, sloped/curved forms and general
section/Boolean geometry remain unsupported rather than drawn as misleading walls.

Kernel validation runs even for hidden inputs. Invalid geometry cannot look like
a successful empty view. Clipping occurs in view-plane coordinates; rotating the
basis rotates the crop. A rectangle clipped by a rectangle has at most eight
vertices; duplicate corner intersections are removed between clipping passes.
Zero-area edge/corner contact returns no polygon. Finite arithmetic, positive
winding/area and vertex counts are checked before publishing a drawing.

The 1e-9 m tolerance is geometry classification/containment, not a screen-space
snap radius. Area uses origin-relative triangles to avoid cancellation from a
large common offset. Tests exercise translated coordinates around 1,000,000 m
with 1e-8 m comparison tolerances; this is not arbitrary-coordinate qualification.

## Identity, completeness and staleness

The initial IR contains polygons only, not the complete production path/curve/text/
style system. Draw order is depth, projected, then cut; UUID order breaks ties.
Picking traverses that exact order backwards and uses the clipped polygons.
Invisible portions have no pick geometry. Screen/output adapters must consume
the same representation as they land; no screen or PDF adapter is claimed here.

A drawing carries session identity, model revision, view identity, settings
revision and the actual range/basis/crop. Consumption with any different context
fails, including changed settings that accidentally reuse a settings revision.
Undoing to geometrically identical state does not revive an earlier revision.
Construction rejects a bad candidate wholly, without modifying a prior drawing.
Input is capped at 10,000 semantic elements, including unavailable entities; this
is a development resource cap, not a production performance promise.

Unavailable extension IDs are explicit drawing diagnostics. The caller must show
them; an incomplete drawing cannot qualify an issue package. No cached 3D mesh or
opaque payload substitutes for a missing future plan provider. Provider activation,
styles, fonts and linked-source versions must join cache keys when those inputs
actually participate; current native-wall derivation does not query them.

## Tests and next integration

- `os-geometry/tests/plan.rs`: cut/bottom/depth/top boundaries, level elevation,
  rotated/translated basis, clipped area/picking, deterministic eight-vertex crop,
  independently calculated square-minus-corner-triangles area, corner-aligned crop
  combinations, invalid contexts and unsupported/invalid hidden geometry.
- `os-render/tests/plan.rs`: cut-before-projected picking agreement, stable UUIDs,
  unavailable IDs, stale contexts, rejected complete candidates, bounded inputs,
  screen/plane/world round trips and atomic pan/zoom errors.
- `os-ui/tests/plan.rs`: the same wall ID and dimensions feed both 3D mesh and plan;
  edit/undo/redo, level and range changes produce expected roles/areas/volumes;
  stale drawings fail and plan derivation does not mutate model/history; unknown
  plugin data is unavailable, and perspective/missing view requests fail.

Next: add typed/versioned persisted plan settings and named-view commands; connect
the drawing to a real plan/split desktop workspace with pan/zoom/fit and shared
selection; then grids/snapping/exact transactional gestures and independently
installed callable plan providers. Add frozen drawing/output fixtures and native
linked-view acceptance. D remains incomplete until the entire BUILD_PROMPT section
7 and independent plugin plan workflow pass. Do not advance to E1 early.

Reproduction uses the existing locked/offline workspace checks; targeted tests:

```sh
cargo test -p os-geometry -p os-render -p os-ui --all-features --locked --offline --target-dir work/completion-build
```

Final local checkpoint: 172 workspace tests including the document doctest pass
with `cargo test --workspace --all-features --locked --offline --target-dir work/completion-build`.
Strict all-target/all-feature Clippy, formatting and default workspace build pass.
The normal executable smoke passes and created the new
`outputs/plan-geometry-verified.osb`; it verifies the existing Wall/IFC baseline,
not native plan interaction. Source remains an untracked working tree without HEAD.

No desktop interaction changed in this slice; no new native plan observation is
claimed. Prior history-notice native inspection remains separately outstanding.
No remote, hosted CI, license approval or production deployment claim is made.
