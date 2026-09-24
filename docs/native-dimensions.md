# Native wall dimensions

Model schema 7 introduced `core.dimension`, a view-owned reporting annotation
between native straight-wall endpoints. Schema 11 adds **Chain** and
**Baseline** layouts. Aligned takes two endpoint clicks and one offset click;
Chain/Baseline collect an ordered sequence of endpoints, require **Finish
anchors**, then take one offset click. Preview never changes the model, and the
completed annotation is one undoable entity. Chain measures adjacent endpoints
on a shared dimension line. Baseline measures each later endpoint from the
first on successively offset parallel lines. Dimensions render and pick above
wall bodies, expose metric values and signed offset in Properties, and appear
in the Project Browser.

## Public integration contract

Types are re-exported from `os_model`:

```rust,ignore
DimensionParams {
    layout: DimensionLayout,
    additional: Vec<DimensionReference>,
    baseline_spacing_m: f64,
    view: Id,
    first: DimensionReference,
    second: DimensionReference,
    offset_m: f64,
    orphan_hint: Point2,
}
DimensionReference { wall: Id, endpoint: DimensionEndpoint }
DimensionEndpoint::{Start, End}
DimensionLayout::{Aligned, Chain, Baseline, Angular}
// Dimension = Entity<DimensionParams>; Model.dimensions is keyed by stable UUID.
```

`additional` stores later references in click order. Aligned requires no
additional references; Chain/Baseline require at least one. All anchors must be
distinct, strictly forward, and collinear with the first-to-second axis within
1 μm. Chain measures consecutive points; Baseline measures each later point
from the first. `baseline_spacing_m` is a positive model-space spacing between
baseline lines. `offset_m` is signed metres along the left normal of the directed
first-to-second anchor segment in model XY coordinates. `orphan_hint` is the
clicked placement point in model XY metres, retained to locate an unresolved
annotation. Measured lengths and resolved coordinates are derived, not stored.
Swapping anchor order flips the normal; side preservation is not implemented.

`DimensionParams::validate_creation(&Model) -> os_core::Result<()>` is the public
preflight API used by `Command::AddDimension`. It checks syntax then requires two
resolvable native wall endpoints on the owning plan's level, with finite distance
greater than one micrometre. Use the candidate document model at commit time;
preflight results are not a substitute for document revision checks in the UI.

`DimensionParams::resolve(&Model) -> Result<ResolvedDimension, DimensionDiagnostic>`
remains the two-anchor aligned compatibility API and returns
`ResolvedDimension { first, second, length_metres }`. `resolve_points` resolves
all ordered anchors for Chain/Baseline; missing later references identify their
one-based anchor index. Resolution does not apply visibility, crop, projection,
offset or formatting. Both references may address opposite endpoints of one
wall; identical references are rejected.

## Validation, edits and persistence

Schema 12 adds Angular: select two distinct visible native straight-wall bodies
on the plan level, then click inside the desired sector to set its arc radius.
The four non-reflex sectors are supported. `first` and `second` identify walls;
their endpoint values select positive (End) or negative (Start) axis directions.
`additional` must be empty, `baseline_spacing_m` must be 0.25, and `offset_m`
is a positive radius in metres. `resolve_angular` derives the infinite-axis
intersection and angle in degrees; Angular does not use the length resolver.
Parallel/near-parallel axes (absolute unit-direction cross product <= 1e-6),
wrong-level walls, degenerate geometry, and placement on a ray are rejected.
Parallel walls are currently rejected at placement, not at the second wall click.
Backspace leaves placement mode and removes the latest wall choice; Escape
cancels. Preview does not mutate the model; placement adds one history entry.
Missing, releveled or newly parallel references retain their intent and can
resolve again after geometry is restored. Angular arcs use 64 chords and two
radial witnesses; painting and picking share clipped segments and label bounds.

Persistent syntax requires non-nil wall/view IDs, distinct reference identities,
finite offset and hint coordinates bounded to +/- 1,000,000 metres, and an extant
floor-plan View. Header identity/type/schema and global identity uniqueness use
the same checks as other native entities. Parameter and reference objects reject
unknown fields, including a persisted measurement field.

Missing walls, changed wall/view levels and subsequently coincident anchors are
valid stored intent. Ordinary wall edits/deletion do not rewrite or discard a
dimension or fail solely because its anchors become unresolved. Undo can restore
resolution. Unresolved dimensions show a red plan marker, a Properties diagnostic,
and missing wall references in the Project Browser; reference repair is not
implemented.

`AddDimension(Dimension)`, `UpdateDimension { id, parameters }`, and
`RemoveDimension(Id)` use the existing atomic transaction/history path.
Updates retain the header UUID and validate persistent syntax; they may edit an
already orphaned dimension. Creation alone requires currently resolved anchors.
`RemoveView` rejects an owned view until its dimensions are removed; a transaction
can remove dimensions first and then their view. Wall, level and view edits
invalidate dependent dimensions; annotation changes invalidate the owning view.

Schema 6→7 inserts an empty dimensions map and advances all native entity header
versions. Schema 10→11 defaults prior dimensions to `Aligned`, no additional
anchors and 0.25 m baseline spacing; it advances every native entity header and
preserves IDs and opaque plugin payloads. Schema 11→12 advances native headers
and the model version without changing prior dimension parameters or IDs.
Older readers reject schema 12. Previous migrations remain chained.
The ZIP container stays version 2. No plugin protocol or arbitrary plugin
reference support is added.

## Evidence and limitations

Focused backend tests are in `os-model/src/dimensions.rs`,
`os-document/tests/dimensions.rs` and `os-storage/tests/dimensions.rs`; renderer
coverage is in `os-render/tests/plan.rs`; real-egui pointer, preview, undo/redo,
orphan, save/reopen, Chain and Baseline tests are in
`os-ui/src/plan_workspace/endpoint_tests.rs`. The UI matrix runs at
1280×800/100% and 1000×650/150%. These remain headless checks; native-window
inspection and print-output validation are not claimed.

Angular acceptance tests cover four sectors, exact/near-parallel and same-level
constraints, invalid geometry and orphan recovery; bounded arc segments, crop
and UUID picking; and real-egui authoring at both listed DPI profiles, including
duplicate-wall rejection, preview immutability, Backspace/Escape, one commit and
undo/redo. The frozen `fixtures/schema-11-aligned-dimension.json` proves migration
preserves the previous annotation and all IDs/header metadata; storage tests also
save/reopen an Angular annotation. The schema-10 fixture remains unchanged.

Only straight-wall references, metric lengths and degree reporting are represented.
Face references, linked/plugin anchors, radial dimensions, driving
constraints, text overrides, style controls, deliberate reference repair, and
print-faithful annotation remain unimplemented. Baseline spacing is model-space,
not paper-scale. Dimension lines and tags respect plan crop in the on-screen
view; paper-accurate text/tick sizing and export remain future work.
