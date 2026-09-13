# ADR 0013: Bounded snapshot-history retention

Status: accepted for the development foundation; not a production memory budget.

BUILD_PROMPT section 4 requires measuring snapshot cost and bounding history
without changing transaction semantics. Previously each successful edit retained
two complete owned models in unbounded undo/redo vectors. A release-mode synthetic
measurement of 20 renames at 100, 1,000 and 10,000 walls confirmed retained cost
scales with both model size and transaction count. See [measurements](../history-retention.md).

Keep validated in-memory snapshots, including their existing exact numeric and
opaque-payload representation. Do not introduce deltas, a serialization-based
history, or a disk-backed database in this slice. The retained undo and redo
stacks share one policy: 128 transactions and 256 MiB of conservative owned-data
accounting by default. Whole oldest entries are released; the next available
undo/redo continues along a contiguous timeline. A new edit discards redo as before.

Reserve the complete incoming history entry before any commit or pruning. If it
cannot fit by itself, fail explicitly with required/allowed estimated bytes and
leave model, revision, events and both stacks unchanged. Never commit a valid
model change while silently omitting its undo record. This adds an explicit
resource-limit failure, not partial batches or extra undo steps. A no-op remains
a no-op and does not erase redo. One command batch is still one history entry.

Expose session-local limits and statistics through Document; impossible limit
changes fail without alteration. Show retention and eviction details on the
history controls and a visible notice once retention has released an entry.
Settings and history are not persisted. Immutable attachments stay outside the
model snapshots, and dirty state still compares the model with the saved model.

The byte counter is intentionally an estimate, not an exact heap/RSS measurement.
It includes model inline data, owned string/vector capacities, pessimistic tree
node charges, arbitrary-precision numeric spellings, relationships and plugin
payloads, plus history metadata. It excludes allocator overhead, current/saved
models, temporary clones/validation, queued events, worker snapshots and geometry.
The guarantee is bounded retained *accounted* bytes after each operation, not a
hard limit on total or peak process memory. Exhaustive model/header/envelope and
parameter patterns require accounting review when the semantic schema grows.

Consequences: large models retain fewer undo steps; a single oversized edit is
rejected rather than made non-undoable. The conservative estimate currently keeps
only three 10,000-wall rename transactions at the default budget. Better measured
allocation accounting, shared snapshots or deltas may improve that tradeoff later,
but must preserve the current transaction, identity, event and failure tests.
Actual peak-memory profiling and representative G4 qualification remain required.
