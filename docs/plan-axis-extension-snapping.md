# Optional semantic axis extensions

2026-09-12: **Line axis extensions** in the Snaps menu enables acquisition of
the supporting line beyond either end of a finite semantic segment. It defaults
off and is also disabled by the master switch. Finite nearest/grid-axis snaps
are unchanged. This mode is construction guidance, not extended model geometry,
an invisible picking target or an associative constraint.

The pointer projects onto the source segment's unit direction. Only projections
strictly beyond its endpoints are offered under AxisExtension. The candidate
retains source UUID/feature identity, passes the same crop/screen-radius/exclusion
checks and has lower priority than all finite-feature kinds. Enabling/disabling
the mode changes query identity; previous results cannot be reused. Arithmetic
uses local differences and finite-value validation, with linear work in the
existing bounded scene. No persisted model or public plugin wire changes.

Renderer tests cover both ends, default-disabled behavior, preserved finite
nearest behavior, semantic identity, query staleness, crop/exclusion and a
diagonal extension at million-metre coordinates. The egui pointer test enables
the menu option, acquires `(3,0)` beyond a wall ending at `(2,0)`, creates a new
wall to `(3,2)` and undoes it without altering the source. Full offline
all-feature workspace tests, strict Clippy, formatting and default build pass.

Native-window inspection, two-axis intersection inference, temporary world-axis
guides, inferred anchors and independent plugin contributions remain open. This
is a partial E01.33 implementation, not completion of D or production snapping.
