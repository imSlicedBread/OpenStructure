# ADR 0005: depth-buffered viewport and visible-surface selection

Replace average-triangle painter sorting with an independent software rasterizer
in `os-render`. For the existing orthographic camera, interpolate depth at pixel
centres and retain the closest fragment. Keep an entity-ID buffer alongside color
and depth, so clicking selects exactly the displayed sample. Coplanar ties use
stable entity identity, not draw order. This fixes crossing/overlapping triangle
visibility without changing wall solids, transactions, storage or IFC semantics.

Use a bounded DPI-aware framebuffer, uploaded as a nearest-filtered egui texture.
Cache the result by projected triangles, logical dimensions, resolution scale
and selection; unchanged frames must not rerasterize/upload. Pixel-count and
dimension caps bound allocations and offscreen triangles are clipped before loops.
Reject or skip nonfinite/degenerate geometry; no unsafe code or new runtime
dependencies. Grid and axes remain background guides, not depth-tested BIM solids.

This is a correctness-first CPU adapter, not a production GPU renderer. It remains
replaceable behind the render boundary. Hardware acceleration, antialiasing,
transparency, large-model performance, wall joins and openings are separate tasks.
Verify crossing-depth tests where average sorting necessarily fails, draw-order
invariance, occluded picking, edge coverage, clipping, DPI limits and real desktop
selection/edit/undo/resize behavior. Retain a reproducible visual fixture.
