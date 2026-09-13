# Exact destination takes precedence over snap acquisition

2026-09-12 regression: the pointer workspace queried snapping before considering
complete exact input. At more than 256 nearby segments, intersection acquisition
rejected the query and prevented even a fully specified offset from previewing
or committing. The limit correctly protects pointer feedback, but must not gate
a destination that no longer depends on acquisition.

`WallGesture::has_exact_destination` now identifies an anchored draft with a
nonempty length and angle, or an anchored offset copy with nonempty signed
distance. The workspace bypasses snap acquisition for those drafts. Exact
parsing, finite/geometry checks and session/view/provider/revision validation
still apply. Invalid exact strings take this path so numeric validation is not
masked by unrelated snap errors. A missing anchor or partial dimensions retain
normal acquisition, including its bounds. No snap candidates are fabricated.

The real egui regression creates 257 coincident grid datums and a source wall.
It first observes the dense-query diagnostic, then enters exact 2 m offset,
previews and commits the parallel wall and undoes once with all grids/source
unchanged. It failed before the correction and passes after it. Controller tests
cover complete/partial/missing-anchor/invalid-input routing. Full offline
all-feature workspace tests, strict Clippy, formatting and default build are
checked. Native inspection of this dense case remains open.

This preserves the existing exact-input contract; it does not remove resource
limits, qualify dense-model performance or complete the independent plugin route.
