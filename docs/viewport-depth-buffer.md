# Depth-buffered viewport slice

The viewport no longer sorts whole triangles by average depth. `os-render`
interpolates depth at each pixel centre and records color and the nearest entity.
Clicks read the same entity-ID buffer, so the selected object is the surface
actually displayed at the cursor. Equal-depth ties use the lower stable entity
ID; exact duplicate same-entity samples have a deterministic color tie-break.

This changes only presentation and picking. Wall solids remain independent
overlapping prisms: intersections are not boolean unions or wall joins. The
semantic model, transaction API, native format and IFC output are unchanged.

## Resource and rendering behavior

- Orthographic projection and outward-face culling remain in `os-render`.
- Bounded raster dimensions respect display DPI up to 2,000,000 pixels and
  4,096 pixels per side. Larger views scale down; picking uses the same mapping.
- RGBA, f64 depth and u32 owner buffers use 16 bytes per pixel (at most 32 MB),
  excluding the entity table, egui image, cached triangles and GPU texture/copies.
- Transparent background pixels reveal the existing grid and axes. Those are
  background guides, not depth-tested solids; particularly note this for walls
  below ground level.
- A nearest-filtered egui texture avoids blending colors across selection edges.
  No antialiasing or transparency is implemented, so silhouettes can be jagged.
- The UI compares projected triangles, logical size, DPI and selection before
  rerasterizing/uploading. Changes from edits, undo/redo, open/import, camera and
  layout are detected; unchanged UI frames reuse the texture.
- Offscreen bounding boxes are clipped before raster loops. Invalid dimensions
  return an error; nonfinite/degenerate triangles are skipped. Projection guards
  invalid mesh indices and nonfinite normals/coordinates instead of panicking.

This is a correctness-first CPU adapter for small models, not a GPU acceleration
claim or a guarantee of interactive performance for large BIM models.

## Verification

Final Windows workspace verification: **52 tests passed**, strict Clippy,
rustfmt and locked/offline build passed. The final executable completed the
save/reopen smoke workflow at `outputs/depth-buffer-verified.osb`. No new runtime
dependencies were added. Hosted CI and other desktop platforms were not run here.

Automated renderer checks include:

1. Two overlapping triangles whose nearer surface changes across the image.
   Neither draw order can represent this with average-depth sorting; the new
   color/depth/owner result is correct and order independent.
2. Exact coplanar identity ties and selection of the occluding surface.
3. Crack-free shared diagonal coverage, clipping, invalid/degenerate input,
   boundary clicks, DPI scales 1/1.25/1.5/2 and large viewport allocation caps.
4. Crossing closed prisms checked against independently calculated ray/box
   intersections at four camera angles, including background samples.
5. Desktop real-input test: visible pixel → selected wall → edit → undo, drag
   orbit, resize/DPI change; unchanged frames issue no viewport texture upload.

Native Windows inspection used `fixtures/intersecting-walls.osb`. All three
intersecting walls rendered with continuous faces and correct visible occlusion.
Clicking the tall north-south wall selected and highlighted that wall, with
occluding walls left unhighlighted. The computer-use skill required checking the
actual native window; screenshots were displayed in the task. The saved fixture
was not modified. Drag-to-orbit behavior is proven by the real-input egui test;
the native automation drag did not produce a visible camera change, so native
orbit behavior is not separately claimed from that attempt.

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked
.\tools\cargo.ps1 run -p os-app '--' fixtures\intersecting-walls.osb
```

Use Fit model, select the exposed faces around the intersection, and change a
wall height/undo to inspect regeneration. Source decision:
[ADR 0005](decisions/0005-depth-buffered-viewport.md).

Next rendering work: hardware acceleration/antialiasing and large-scene profiling.
Wall joins and openings require separate geometry/model work, not renderer tricks.
