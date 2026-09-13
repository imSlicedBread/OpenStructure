# Snapshot-history cost and bounded retention

This foundation slice addresses BUILD_PROMPT section 4; it does not qualify G4
or complete the B/C gate. No new dependency, unsafe code, model schema or native
container version is introduced. [ADR 0013](decisions/0013-bounded-snapshot-history.md)
records the policy and alternatives.

## Behavior

`Document` retains whole before/after model snapshots under a combined undo/redo
limit: 128 entries and 256 MiB **estimated owned data**, whichever is reached first.
New transactions evict oldest undo entries. Undo/redo move existing entries without
changing their charged bytes. A new branch releases redo normally. Explicitly
shrinking limits releases oldest undo entries, then farthest future redo entries;
it never skips a retained intermediate state.

If one new transaction cannot fit, it fails before history pruning or commit.
The error gives required and allowed estimated bytes. Existing model, identity,
revision, events, history, saved state and auxiliary files remain intact. The
application never silently commits without an undo record. Empty/invalid batches
still fail, and a validated no-op neither allocates history nor invalidates redo.

`history_stats()` exposes both stack lengths, accounted bytes, cumulative retention
evictions and limits. `set_history_limits(HistoryLimits { ... })` is a session-local
Rust API, not a persistent user preference or a desktop settings editor. Zero
limits or a byte limit smaller than the next undo/redo entry are rejected atomically.
New/opened documents start with the default policy and empty history. Saved-state
comparison is independent of retention; releasing history cannot mark work saved.

Desktop Undo/Redo tooltips explain the policy and current statistics, including
when the controls are disabled. A **History limited** notice appears after the
first retention eviction. Limit rejection uses the existing operation-error path.
The notice/actions have automated egui coverage at 1280×800 and 1000×650, each at
1.0, 1.25 and 1.5 scale. Native compositor inspection of this new notice has not
been performed.

## What the estimate means

`Model::estimated_memory_bytes()` walks owned model data without serialization or
numeric conversion. Strings/vectors use capacities; maps/sets receive a deliberately
pessimistic full-node charge per entry (16 entry slots, 16 pointer words and 64
extra bytes). JSON arrays, object keys, arbitrary-precision number spellings,
headers, relationships, extension payloads and requirement versions are counted.
History also charges label, changed-ID nodes and inline/spare-stack metadata.
Saturating arithmetic cannot wrap an oversized reservation into an accepted one.

These are retention accounting bytes, **not measured allocator bytes or RSS**.
Rust container layouts and allocator bookkeeping are not stable public contracts.
Current/saved models, temporary transaction clones, validation/events, worker
snapshots, meshes and attachment storage are outside this budget. A transient
operation can exceed the retained-history budget. It does not establish bounded
total application memory or a safe maximum model size. Peak-memory profiling,
memory-growth tests and owner-approved project/performance profiles remain open.

## Measurements and reproduction

Observed locally on Windows x64, Rust 1.98.1/MSVC, release profile. Synthetic
rectangular walls form a regular grid; each scenario starts with no history, does
20 full-model clone/drop samples, then 20 distinct project renames, draining events
after each. Geometry, file I/O, UI and plugin execution are excluded. UUIDs are
new each run. Times are single-run observations, not p95 budgets or speedup claims.

Before retention was changed, the instrumented original two-snapshot algorithm gave:

| Walls | One model estimate, bytes | 20 entries' snapshot estimate, bytes | Mean clone/drop, µs | 20 edits, µs |
| --- | ---: | ---: | ---: | ---: |
| 100 | 391,957 | 15,678,280 | 22 | 2,565 |
| 1,000 | 3,819,157 | 152,766,280 | 280 | 21,408 |
| 10,000 | 38,091,157 | 1,523,646,280 | 3,903 | 226,972 |

That baseline estimates snapshots only, excluding history labels/changed IDs.
The first post-change run, including history metadata and estimation work, gave:

| Walls | Retained history estimate, bytes | Retained / evicted entries | Mean clone/drop, µs | 20 edits, µs |
| --- | ---: | --- | ---: | ---: |
| 100 | 15,715,278 | 20 / 0 | 29 | 2,335 |
| 1,000 | 152,803,278 | 20 / 0 | 336 | 21,264 |
| 10,000 | 228,552,492 | 3 / 17 | 4,116 | 282,564 |

No deltas were introduced. The 10,000-wall edit time increased in this run;
the result demonstrates retention, not a responsiveness improvement. Conservative
tree charging substantially overestimates expected owned allocation size. Three
retained steps on this fixture is an explicit current limitation, not a production
UX target. More precise profiling and snapshot sharing/deltas need a later decision.

Reproduce the current bounded policy and a 20-edit unbounded-retention reference
using the same checked model/transaction implementation:

```sh
cargo run -p os-document --example history_cost --release --locked --offline
cargo run -p os-document --example history_cost --release --locked --offline -- --unbounded-reference
cargo test -p os-document -p os-model --locked --offline
```

The reference option disables eviction for this synthetic workload only. It is
not an application setting. Both current modes include estimation and metadata,
so their timings need not reproduce the earlier algorithm's baseline. On the
first reference run, 10,000 walls retained 1,523,683,278 estimated bytes / 20 entries.

An additional Windows process measurement used `tools/measure-history.ps1`, which
launches each release-mode probe in a hidden process and samples every 5 ms. The
whole process runs all three model sizes above; these values include the current
model, cloning, validation, runtime and allocator effects, not only history:

| Mode | Observed peak working set, bytes | Sampled peak private bytes |
| --- | ---: | ---: |
| Default bounded retention | 48,193,536 | 45,256,704 |
| 20-edit unbounded reference | 178,683,904 | 170,573,824 |

The working-set value uses the OS peak counter observed while the process was
alive; private bytes are a sampled maximum. Short or final peaks can be missed.
These observations demonstrate reduced process footprint for this fixture, not
that estimated bytes equal allocations or that all supported workloads fit.
Processor environment identification was Intel64 Family 6 Model 140 Stepping 1;
CIM hardware/RAM queries were denied, and no access escalation was needed for
the process counters. The machine is not an approved production reference profile.

```powershell
.\tools\cargo.ps1 run -p os-document --example history_cost --release --locked --offline --target-dir work/history-measurement
.\tools\measure-history.ps1
```

## Verification

Local final verification: 160 workspace tests including the document doctest
passed; strict all-target/all-feature Clippy, formatting, default workspace build
and executable smoke passed. Smoke created the new
`outputs/history-retention-verified.osb`. The independently installed Wall and
column Editor probes passed again, as did all 33-column and mixed-type migration
probe scenarios. These probes exercise controllers/workers, not native controls.
No existing project or installed guest was overwritten.

```sh
cargo test --workspace --all-features --locked --offline --target-dir work/completion-build
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo build --workspace --locked --offline --target-dir work/completion-build
cargo build -p os-ui --features external-plugins --example plugin_editor_probe --example plugin_migration_probe --locked --offline --target-dir work/completion-build
```

Run the built probes with the separately installed artifacts:

```powershell
.\work\completion-build\debug\examples\plugin_editor_probe.exe outputs/rust-wall-install
.\work\completion-build\debug\examples\plugin_editor_probe.exe outputs/rust-column-geometry-install
.\work\completion-build\debug\examples\plugin_migration_probe.exe outputs/rust-column-geometry-install outputs/rust-column-v2-install outputs/rust-column-mixed-install
```

- `os-document/tests/history_retention.rs`: whole atomic batches, bounded bytes
  and entry counts, eviction order, revision/events, stable identity, attachments,
  branch release, oversized/invalid/no-op preservation, setting changes, nearest
  redo order, fresh document state, and a 2,000-action reference-timeline comparison.
- `os-model::memory` test: capacity and nested arbitrary-precision JSON accounting
  without changing payload values.
- `os-ui/tests/vertical_slice.rs`: retention cannot clear dirty state; failed open
  preserves model/history/revision and the saved file; undo/redo/save/open remain
  consistent after eviction.
- `os-ui` desktop test: visible notice and clickable Undo/Redo across the supported
  headless layout matrix. Existing plugin, storage and migration tests remain
  necessary; these new tests alone do not prove those workflows.

This is a development retention guard, not autosave, crash recovery, collaborative
history, persistent undo, or production sustained-use qualification.
