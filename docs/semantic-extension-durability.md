# Semantic extension durability — milestone C foundation

Status: implemented preservation and core transaction boundary; B/C release gate
still incomplete. This is not an independently installed column authoring plugin.

## What works

Model schema 2 stores inert `ExtensionEntity` envelopes and exact
`plugin_requirements`. Each envelope has a stable UUID, owner/type namespace,
display name, envelope version 1, independent positive payload schema version,
named relationships, acyclic explicit regeneration prerequisites and opaque JSON.
No plugin is needed to validate references, inspect metadata, save or reopen.
Native saves preserve payload values and relationships through unrelated edits,
undo and redo; unknown payload versions are preserved rather than executed.

Every owner must have a requirement record. Requirements use exact numeric
major.minor.patch versions, not URLs or installation instructions. A native file
cannot install or run code. Model validation protects referenced native and
extension objects from deletion; explicit batch removal must leave a valid graph.
Reference changes conservatively invalidate transitive extension dependents.
Named-reference cycles are permitted; explicit `depends_on` cycles are rejected.

Core document commands are `AddExtension`, `ReplaceExtension`, `RemoveExtension`
and `SetPluginRequirement`. Replacement checks the expected previous payload
schema and cannot change owner/type/identity. A migration proposal can replace
payloads and update requirements in one transaction, with one undo step. A failed
batch leaves model, revision, history and events unchanged. This is not a schema
transform engine: an eventual authorized plugin service must supply the proposal.
V1 plugin command responses are explicitly denied access to these core commands.

## Limits and compatibility

- Model schema 0 migrates through 1 to 2; schema 1 gains empty extension maps and
  native headers at schema 2. Existing UUIDs are unchanged. Migration operates on
  a copy and validates before replacing it; opening never rewrites the original.
- Container remains version 2, with container 1 readable. Older model-schema-1
  builds reject model 2. Native header schema, payload schema, envelope version,
  container version and plugin API version are distinct.
- Envelopes: 1 MiB each, 16 MiB combined, 10,000 entities, 1,024 owner requirements,
  payload nesting at most 32. These are parser/validation caps, not qualified
  interactive project-size or performance guarantees. The existing 64 MiB native
  model and archive/attachment bounds still apply.
- Unknown root model/envelope fields and future envelope versions fail explicitly.
  Arbitrary extra fields inside existing typed native entity structs are not a
  general preservation format; use declared extension envelopes/payloads.
- Opaque numeric values retain arbitrary JSON precision using the existing
  serde_json package's `arbitrary_precision` feature; typed geometry still uses
  finite f64. `raw_value` supports a preflight scan rejecting duplicate JSON keys
  before Value parsing can collapse them. The private
  `$serde_json::private::Number` object key is explicitly unsupported/rejected
  rather than reinterpreted. No new dependency versions were added.
- Preservation is of JSON values, not original whitespace/object ordering. No
  arbitrary executable payload validators, cached plugin geometry or plugin-owned
  migration callbacks are activated. Snapshot-history and serialization costs
  still need representative measurement/budgets.
- IFC wall export refuses any extension entity, even with loss consent. There is
  no extension IFC mapping yet. Unused requirement metadata is reported as omitted.
- V1 wall replies must use current native header schema 2; older developer guests
  constructing schema-1 walls fail document validation and need rebuilding. Buffer
  ABI and geometry-only Wasm probes remain unchanged. A generic independent DTO
  contract, descriptors and SDK are still required before general plugin support.

## Reproduce the desktop inspection

Run from your checkout; output must be a new path in an existing directory:

```powershell
.\tools\cargo.ps1 run -p os-storage --example preserved_extension --locked --offline '--' example-preserved.osb
.\tools\cargo.ps1 run -p os-app --locked --offline '--' example-preserved.osb
```

The synthetic fixture contains `Preserved column` with unknown vendor data and
no executable plugin. The viewport says **Incomplete view** and explicitly says
this is not an empty project. Choose **Inspect preserved data**, then the entity.
Details shows its UUID, type, exact owner requirement, missing/disabled status,
payload schema and a read-only JSON preview (4,096 characters maximum). Rows are
virtualized; the complete data remains in the file. Native Save preserves it;
IFC export refuses it. Loaded owners also remain unavailable until generic
providers exist; matching a manifest alone is not authoring support.

## Evidence and handoff

Windows x64, Rust 1.98.1/MSVC, uncommitted/untracked initial scaffold, no HEAD or
remote. Baseline before this change: 86 tests passed; final all-feature suite:
101 tests passed, including the documentation test. Build, Clippy, formatting,
smoke and license metadata checks passed. Verification commands:

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/new-smoke.osb
.\tools\cargo.ps1 metadata --all-features --locked --offline --format-version 1 | C:\Python314\python.exe tools/check-license-metadata.py
```

Tests cover frozen old-container compatibility, opaque payload/precision round
trips, graph/namespace/schema/size/depth failures, cycles, transactional migration
rollback/history, transitive invalidation, v1 command denial, desktop preservation,
IFC refusal, compact/DPI read-only inspector input and deterministic malformed
envelope mutation smoke. These are regression checks, not coverage-guided fuzz
qualification or a production recovery test matrix.

Native inspection using the computer-use skill verified File→Open, the persistent
warning/non-empty state, entity selection, missing-owner/UUID/schema labels and
scrollable payload preview at 1280×800. The headless inspector test additionally
checks 1000×650 at scale 1.0 and 1.5 without changing revision/history. No clean
offline second-workstation or Linux/macOS native qualification was performed.

The example initially failed on Windows because the temporary file handle was
still open during replacement. It now closes that handle before saving and uses
no-clobber publication. Strict Clippy passes with the existing non-fatal Windows
`os_geometry` incremental-cache access-denied note on intermediate runs (exit 0),
not a suppressed source warning. The final Clippy run was clean. License metadata:
347 packages, publishing still disabled.

Independent IfcOpenShell 0.8.5 validation of the schema-2 wall smoke export passed:
IFC4, zero schema errors, one 12-triangle wall, 7.35 m³, expected bounds and native
identity/dimensions. The validator required sandbox escalation to read its
already-installed local packages; no packages were installed. Existing Wasm worker
probe still returned session/revision-checked geometry without document edits.

Final generated outputs were `outputs/semantic-extension-final.osb` (inert
inspection fixture) and `outputs/semantic-envelope-final-smoke.osb` (wall smoke).
CLI export of the former with `--allow-loss` was refused and created no IFC file.
These ignored outputs are reproducible, not committed fixture or release artifacts.

No independent Rust SDK/descriptor-driven Wall/column installation acceptance has
passed. No commit, remote publication or hosted CI result exists; publication and
the pending license still need owner authority/choices. The CI fixture-generation
step is configured for Windows/Linux but has not run remotely.

Next: (1) generic negotiated DTOs/descriptors, scoped authorization and standalone
Rust SDK, including 2D contract design; (2) desktop lifecycle/worker integration,
Wall/column authoring and plugin-owned migration acceptance; (3) linked floor-plan
workflow after B/C passes. D/E1–E4 and G1–G6 remain incomplete; jurisdiction, project
size, concurrency, output/platform/performance targets remain undecided. M02/M03/Q01
and the full architectural scope are not reduced. L1/L2 remain post-E4 follow-ons.
