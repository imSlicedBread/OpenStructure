# Generic plugin geometry: first callable recipe

The later [native Wall adapter](native-wall-plugin.md) adds native wall snapshots
and an independent Rust Wall guest. Earlier column-only limits below describe
the initial geometry path; desktop integration remains incomplete.

API 2 now has an optional `GenerateGeometry { element_id, snapshot }` operation
and `Geometry { element_id, recipe }` reply. The independent `geometry::Recipe`
currently supports a rectangular prism: width/depth/height in metres, translation
in right-handed Z-up model coordinates, and rotation about local Z in radians.
Its lower local corner is (0,0,0). No kernel, document or UUID runtime types leak
into the contract-only dependency graph.

This is an additive operation/recipe, not a reinterpretation of existing messages.
Old v2 guests lacking Geometry capability are not invoked; guests that declared
that previously reserved capability but do not implement the operation will fail
explicitly. Older strict decoders reject the new variants. Rust exhaustive matches
need new arms. V1 Wall protocol, buffer ABI 1, model schema 2 and container 2 are
unchanged. This remains a development API, not a universal geometry compatibility
promise. Additional recipe kinds require explicit validation and compatibility work.

`PluginHost::generate_generic_geometry(plugin, document, element, read_scope, timeout)`
checks API, requested/granted model read, Geometry capability, registered owned
type, payload schema, exact plugin requirement and scope before invocation.
It serializes only explicitly scoped existing extension entities and levels.
References do not implicitly grant access to their targets. It refuses a busy
Wasm runtime. Replies must echo request/document correlation and the exact target;
Edits cannot masquerade as geometry. The host validates finite dimensions and
placement, maps the recipe into its kernel, and validates the tessellated mesh.
Dimensions must exceed one micrometre, the current prism-kernel minimum—not a
production tolerance policy. Overflow/collapsed world-space geometry is rejected.

The result stores host-owned session/revision and exposes geometry through
`GeometryResult::get(document)`. Edit, undo, redo or reopen invalidates consumption,
even if the same semantic model is restored. This is model geometry, not plan/view
output, and is not a claim of view-aware rendering. No document commands or events
are produced by generation. Validated derived results can be retained after plugin
unload; they cannot replace authoritative payloads or hide unavailable editing.

For Wasm, `start_generic_geometry_job(plugin, document, element, read_scope, view,
timeout)` now runs the guest on the shared bounded worker pool. Poll via `poll_job`;
success yields `JobOutcome::GenericGeometry(CheckedGenericGeometry)`. Its
`get(document, current_view)` checks document session/revision and optional host-owned
view/settings revision again at consumption. The view is a delivery guard, not
plan context sent to the guest. Cancel/drop disposes of acceptance; unload/reload
generation, stale document/view, expiry and invalid output prevent delivery.
Unread results and draining work retain the same per-runtime/four-per-host permits
as modeling commands. Preparation, response validation and the bounded 12-triangle
prism tessellation still run on the caller. Desktop activation remains separate.

## Verified implementation

The separate Rust column guest derives dimensions from its payload and base Z
from its explicitly scoped prerequisite level. Its current XY placement is the
model origin and yaw is zero; placement/editing expansion remains necessary.
The probe edits width to 0.8 m, generates a 0.8 × 0.6 × 3 m body, checks 1.44 m³
and semantic identity, then verifies that undo revokes the result. It runs the
compiled Wasm artifact against a host executable built before that artifact.
No runtime limits or native IFC mappings changed.

```powershell
.\tools\cargo.ps1 build -p os-plugin-host --features wasm --example generic_probe --locked --offline
.\tools\cargo.ps1 build --manifest-path examples/rust-column/Cargo.toml --target wasm32-unknown-unknown --release --locked --offline
# Copy manifest/artifact to a new directory per examples/rust-column/README.md
.\target\debug\examples\generic_probe.exe outputs/rust-column-geometry-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 test -p os-plugin-api --no-default-features --locked --offline
```

Observed on Windows x64/Rust 1.98.1: installed Wasm probe passed; 120 workspace
tests at the initial geometry slice; the worker follow-up has 124 passing workspace
tests. Five contract-only tests and four independent guest tests passed. New tests
cover independent DTO validation, rotated/translated numeric geometry, overflowing
placement, scoped snapshots, rejected edit/identity/dimension/context/version/
malformed/oversized replies, unchanged model/events and stale geometry after
reopen/undo/redo. Existing worker tests remain green. This is not a production
geometry tolerance or performance qualification.

The worker follow-up runs the existing compiled Rust column artifact through the
new worker path in `generic_probe`, including checked volume and stale consumption.
Four `generic_geometry_workers.rs` tests exercise real Wasm guest threads, read-only
single delivery, busy exclusion, document/view/reload rejection, cancellation,
invalid edit replies, malformed JSON, traps/fuel exhaustion, expiry and dropped
tickets. Full workspace Clippy, formatting, build and smoke are rerun for the slice.
No wire or persistence change is introduced by worker integration. The smoke
output is `outputs/generic-geometry-worker-verified.osb`; workspace license metadata
still passes for 347 packages. No new dependencies or unsafe exceptions were added.

## Remaining sequence and limits

1. Add native Wall mapping and the independently compiled Wall example. Do not
   call the synchronous geometry convenience service from desktop frames; integrate
   the worker service with the editor's regeneration lifecycle.
2. Integrate descriptor-driven authoring, manager lifecycle and plugin migrations;
   pass installed Wall then column B/C desktop/geometry/durability acceptance.
3. Complete D linked plans and independent 2D providers, then every E1–E4/G1–G6
   requirement. L1/L2 remain later work.

No general mesh/Boolean/section/plan provider is claimed. Desktop extension views
still warn about unavailable geometry; unsupported plugin entities still block
IFC export. Wasm retains fuel/memory/import bounds, not process isolation; timeout
revokes acceptance, not execution. Build/Clippy/format and installation evidence
do not qualify desktop interaction or production deployment. Owner platform,
project, output and concurrency targets remain undecided. Licensing is pending;
the source remains an untracked scaffold, with no HEAD, remote or hosted CI result.
