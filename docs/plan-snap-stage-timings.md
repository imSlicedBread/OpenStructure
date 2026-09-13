# Synthetic snap-stage timings

2026-09-12. This measures the host snap engine separately from source generation
and checked scene construction. It is not end-to-end pointer latency, committed
geometry regeneration, UI paint timing, peak memory or production qualification.

Run from the checkout with:

```powershell
.\tools\cargo.ps1 run -p os-render --example plan_snap_probe --release --locked --offline --target-dir work/completion-build
```

The checked-in example uses fixed scene geometries and pointer jitter, fresh
semantic UUIDs, eight warmups and 256 timed query/consumption samples per case.
It reports microseconds, nearest-rank percentiles and observed hit/error counts.
It asserts expected outcomes, not timing thresholds. Sparse scenes are a 20 m
lattice of 10 m segments; dense scenes have 256/257 radial segments through a
common point. All supported kinds, including optional axis extensions, are
enabled except in the explicit no-intersections comparison.

## Local result

Optimized build, debug assertions false; Windows 10.0.26200 x64, eight logical
processors reported, Intel64 Family 6 Model 140 Stepping 1. Detailed CPU/RAM CIM
queries were access-denied; no permission changes were attempted. Hardware is
not a declared production reference system. Other system load was uncontrolled.

Second run, without concurrent agent compilation (microseconds):

| Case | Source creation | Scene validation | Query p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|---:|---:|
| Sparse 32 | 4.2 | 20.9 | 7.6 | 7.9 | 8.3 | 20.4 |
| Sparse 1,000 | 121.7 | 194.3 | 151.8 | 235.6 | 264.0 | 289.4 |
| Sparse 10,000 | 1,025.9 | 1,866.9 | 1,591.3 | 2,319.4 | 2,413.8 | 2,710.7 |
| Dense 256, intersections on | 43.8 | 37.5 | 1,836.9 | 2,659.4 | 2,759.2 | 2,867.8 |
| Dense 256, intersections off | 48.7 | 41.7 | 42.7 | 43.6 | 57.2 | 57.8 |
| Dense 257, rejected | 35.2 | 32.9 | 50.7 | 70.9 | 91.8 | 93.0 |

All valid cases produced 256 hits/no errors; the overload case produced exactly
256 explicit local-limit errors. The first run overlapped compilation and showed
substantially higher dense-256 tails: p50 2,118.2 µs, p95 5,756.6 µs,
p99 16,403.6 µs, max 33,986.0 µs. The probe must not hide that variability by
presenting only the quieter run as a guarantee.

## Implications and limits

Intersection pairs dominate the dense case (~43× median compared with disabling
intersections in the second run). The explicit local cap bounds work but does
not guarantee a frame-time budget. Sparse 10k work remains a linear scan and
needs representative sustained profiling before deciding on a spatial index.
The menu override and fully specified exact-input bypass provide explicit
alternatives, not automatic quality reduction or silent candidate truncation.

Renderer tests and strict all-workspace/all-target/all-feature Clippy pass with
the example. Production budgets, representative real projects, full event-to-paint
latency, installed provider latency and all E4/G6 acceptance remain open.

## Prepared-segment follow-up

The same day's follow-up caches each validated segment's length and unit direction
inside the immutable revision-bound `SnapScene`. Queries and intersection pairs
reuse these three f64 values instead of repeatedly normalizing the same lines.
No priority, tolerance, crop, exclusion, finite-span or 256-nearby rejection rule
changes. The scene remains a linear scan plus bounded local pair search, not a
spatial index. This trades additional scene preparation and three f64 values per
segment (240,000 bytes of numeric fields at the 10k cap, excluding layout/allocator
overhead) for less repeated query work; peak process memory is still unmeasured.

Two optimized probe runs completed after this change. Below is the second run,
again before starting agent compilation/tests (same host and uncontrolled system
load; microseconds):

| Case | Source creation | Scene validation/preparation | Query p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|---:|---:|
| Sparse 32 | 3.5 | 18.1 | 5.1 | 5.9 | 6.3 | 8.4 |
| Sparse 1,000 | 129.4 | 266.3 | 189.6 | 282.6 | 328.8 | 332.7 |
| Sparse 10,000 | 1,366.8 | 3,171.3 | 1,085.8 | 1,680.9 | 1,825.7 | 1,909.1 |
| Dense 256, intersections on | 36.0 | 180.3 | 1,006.2 | 1,422.0 | 1,491.6 | 1,608.3 |
| Dense 256, intersections off | 55.0 | 56.2 | 40.8 | 45.9 | 57.8 | 76.9 |
| Dense 257, rejected | 36.8 | 41.6 | 29.9 | 50.3 | 61.8 | 77.2 |

The first post-change dense-intersection run measured p50 986.0, p95 1,423.2,
p99 1,541.3 and max 1,676.5 µs. Both runs retained all expected hit/error counts.
Compared with the earlier quieter baseline, dense median query cost decreased
about 45%, and sparse-10k median decreased about 32%. Sparse-1k was slower in
the second run (189.6 versus 151.8 µs baseline; first post-change was 109.4 µs).
These separate runs are evidence of a useful dense-query optimization, not a
controlled causal speed guarantee for every scene or end-to-end application.

Regression `prepared_dense_scene_preserves_crossings_order_and_revision_rebuilds`
checks 256 differently sized/reversed segments at large coordinates, unchanged
results under reversed source order, and rebuilt/stale scene behavior over four
revisions. Existing renderer tests retain geometric boundary, crop, exclusion,
priority, overflow and overload coverage. No public Rust API, plugin wire contract,
native file format, dependency or desktop interaction changed.

Follow-up verification on the untracked Windows working tree (no commit or remote):
all commands below passed with `--locked --offline --target-dir work/completion-build`
where accepted by Cargo. The baseline renderer suite passed before editing;
the updated suite includes 14 snapping tests. No new native UI inspection or
independent guest installation was performed for this private renderer change.

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline --target-dir work/completion-build --quiet
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline --target-dir work/completion-build '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\tools\cargo.ps1 run -p os-app --locked --offline --target-dir work/completion-build '--' --smoke outputs/prepared-snap-verified.osb
```

The smoke created the new named artifact and passed bundled plugin load, wall
edits, level reassignment, regeneration, undo/redo and save/reopen. Choose a new
output filename to repeat it. Independent SDK/examples and trust boundaries remain
as documented in the B/C audit; independent plan providers still block D.
