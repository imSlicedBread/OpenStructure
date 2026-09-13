# Explicit extension migration service (bounded prototype)

Registered API-2 commands can now declare `Mode::Migrate`. It uses the existing
scoped Invoke/Edits transport and shared command validation used by Wasm workers;
it is never inferred from an ordinary edit or executed while opening a project.
This is partial C implementation, not complete migration qualification. The initial
single-invocation path below is historical. The staged host service and Editor
follow-ups at the end remove that path's whole-owner 16-element ceiling in forms.

The host requires ModelRead, ModelWrite and UiTool, current activation/document
correlation and an enabled registered descriptor. The migration must authorize
**every extension owned by that plugin**, one registered type, at most 16 elements
within the existing 256-object/1-MiB request bounds. It must replace each target
exactly once, preserving IDs, owner/type, names, relationships and dependencies.
Creates, deletes, partial batches, schema downgrades and out-of-scope references are
rejected. Result payloads must validate against the installed type's current schema.
The required plugin version is updated in the same atomic document transaction.
Undo restores payloads and the old version requirement together. Source files are
not changed until a later explicit native Save.

Ordinary edits retain exact plugin-version/payload-schema checks. For Migrate only,
older payload schemas and a different recorded plugin version can reach the scoped
guest. The guest owns the payload transformation; the host cannot infer semantic
equivalence. A migration provider must document supported source versions and loss
behavior. This route does not support native Wall migration, mixed owned types,
more than 16 owned entities or multi-stage large-project migration.

## Authoring and workflow

Register a command using mode `Migrate` and its current element type. Respond to
Invoke with Replace edits carrying each old `expected_schema_version`, the same
semantic identity/metadata, and a validated current-schema payload. The descriptor
form shows an explicit all-owner migration warning. Select an owned element, choose
the registered migration command, review inputs and Apply. ToolDraft scopes all
owned entities plus their explicit references. The warning recommends a backup;
it does not create one automatically.

Loaded-but-incompatible extensions now remain preserved with unavailable geometry
on staged Open instead of blocking access to the document needed for migration.
Ordinary regeneration skips those incompatible entities. After a successful
migration their new document revision permits current-schema geometry generation.
Unsupported future payloads are also preserved unavailable, but cannot be downgraded
by this route. Unknown-plugin preservation and ordinary native file validation remain.

## Compatibility and evidence

The pre-release Rust enum gains a variant: exhaustive source matches must add a
Migrate/rejection arm. The column example and host probe were updated accordingly.
Existing v1/v2 installed guest artifacts still load unchanged. This is an additive
API-2 catalog vocabulary extension: older hosts reject catalogs containing Migrate
as unknown input rather than treating them as Edit. Such catalogs require this host
revision; no broad backward-host compatibility is claimed. No buffer ABI or native
container/model/envelope version change, dependency addition or unsafe-policy change.

Three new service tests pass: full two-entity migration/version update with undo,
redo and save/reopen; invalid/partial/metadata-changing/ordinary-edit rejection;
and incomplete owner authorization rejection. The fixture adds required current
numeric fields while retaining its unknown payload keys. An initial fixture omitted
those fields and was correctly rejected, then corrected. These are trusted fixture
service tests, **not** an independently compiled migration guest or native migration
UI acceptance. Those remain required follow-ups.

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 test -p os-plugin-api --no-default-features --locked --offline
.\tools\cargo.ps1 test --manifest-path examples/rust-column/Cargo.toml --locked --offline
.\tools\cargo.ps1 test --manifest-path examples/rust-wall/Cargo.toml --locked --offline
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/plugin-migration-service-verified.osb
```

150 workspace tests pass, up from 147; contract-only 5, standalone column 4 and
Wall 2 pass. Strict lint, format and default build pass. Both installed Editor probes
still pass their non-migration workflows. Column Wasm rebuild and the named smoke
pass; use another unused smoke path to rerun. Only local Windows was observed.

Next: (1) independently installed migration guest and worker failure/stale/cancel
acceptance, (2) scalable multi-type migration and complete native B/C workflows,
(3) D/E1–E4/G1–G6, then later L1/L2. The [coverage ledger](2d-coverage.md) remains
partial: no production architecture/BIM, reference-project or plotting qualification.
Jurisdiction/project/concurrency/platform/output targets remain open owner decisions.
Wasm stays bounded in-process execution, not process isolation. IFC restrictions
are unchanged. No HEAD, remote/GitHub URL, hosted CI result or publication; the
scaffold remains untracked and license approval remains pending.

## Independently compiled guest follow-up

The column example now has a `migration-v2` build and separate `plugin-v2.toml`.
The host probe was built before that guest and then loaded the copied artifact
from `outputs/rust-column-v2-install`; the v1 installation was not overwritten.
Its schema-2 conversion renames `height` to `height_m` without changing metre units
or generated geometry. It rejects a pre-existing destination vendor key rather
than silently overwriting it. See the [example's installation steps](../examples/rust-column/README.md).

The new `plugin_migration_probe` passes with the two real installed versions:
create two v1 columns, save the original, switch explicitly to v2, stage/open old
data unavailable, cancel a migration and drain, reject a conflicting vendor payload,
reject a stale reply after a concurrent document edit, migrate all targets, verify
schema/requirement/geometry, undo/redo, save/reopen, and save/reopen without the
plugin. The original v1 file's bytes remain unchanged. This now supplies independent
guest and worker evidence missing from the earlier service tests, not native UI
acceptance or scalable multi-type migration qualification.

The full workspace still passes 150 tests. Both default and migration-v2 standalone
column builds pass their 4 native tests. Host/guest strict Clippy and formatting
pass; the upgraded Wasm build and installed migration probe pass. All-feature guest
metadata resolves 23 package versions/22 names and no host/model/kernel dependencies.
No dependency or ABI policy change in this follow-up. CI now builds the migration
probe before the upgraded guest, copies it separately and runs the same workflow
on its configured platforms; only local Windows results were observed.

Remaining priorities: scalable/mixed-type migration and native installed B/C
authoring acceptance, then D and E1–E4/G1–G6; L1/L2 remain later. Existing size limits,
runtime trust, production-profile gaps, pending license and unpublished/untracked
repository status above remain unchanged. No ordinary default app behavior changed.

## Multi-batch candidate follow-up

`PluginHost::start_migration` accepts an explicit owner and a map from every owned
type to a reviewed `MigrationCommand` (registered command ID and validated inputs).
Poll its `MigrationSession` with `poll_migration`; `false` means pending, `true`
means the entire live transaction committed. `progress` reports completed/total
candidate elements. Cancel or drop the session to discard the candidate. Nothing
in a project file initiates this service, and it never saves the source file.

The host groups by type and sends four elements per worker call. An initial real
guest run at the protocol ceiling of 16 exhausted the unchanged Wasm fuel budget;
four-element calls passed. This is conservative batching, not a guarantee that
every legal payload fits the fuel/message bounds. Each call still enforces the
existing grants, reply validation, 256-object read scope and 1-MiB message limit.
Only the private candidate path may use partial-owner scopes; ordinary invocation
still requires complete owner coverage. Batch commits do not update the candidate
plugin requirement. Final payloads and the exact installed version commit together
to the live document after source session/revision and activation checks.

The candidate's per-batch undo history is discarded after each successful reply.
The live model, auxiliary files, history and revision stay untouched until final
commit. Cancellation, failure, timeout or stale context discards the candidate;
existing worker permits remain occupied until actual execution drains. Overall
deadline is positive and at most five minutes, with each guest call at most five
seconds. This remains bounded in-process Wasm, not hard process isolation.

The existing installed `plugin_migration_probe` now also seeds 33 v1 columns and
passes four staged scenarios: cancellation after 16 candidate conversions, vendor
collision rejection after 32 conversions, concurrent-document-change rejection,
and full conversion with exactly one live revision and one undo/redo transaction.
Every pending poll verifies live model/revision equality. The earlier independent
two-column geometry/storage/absent-provider workflow also still passes.

```powershell
.\tools\cargo.ps1 run -p os-ui --features external-plugins --example plugin_migration_probe --locked --offline '--' outputs/rust-column-geometry-install outputs/rust-column-v2-install
```

Use the independently built installations documented in the column example. This
probe is already invoked by CI; no hosted CI execution is claimed. Local Windows
workspace tests, strict Clippy, formatting and default build pass. No native file,
wire ABI, dependency or unsafe-policy changes; the feature-gated host Rust API is
additive. The existing native migration form still uses the single-call service.
Mixed-type plan routing is implemented but has no independent multi-type guest
acceptance yet. Model cloning/revalidation cost and 10,000-element throughput are
unqualified; batches with excessive references/payloads safely fail, not auto-split.

Next: connect staged migration to Editor/forms; verify mixed-type, provider-change
and deadline scenarios; finish native B/C acceptance before D. Production targets
remain undecided per the owner's response. D, E1–E4/G1–G6 remain incomplete; L1/L2
are later. There is no new production support or publication claim.

Default smoke also passed with a new artifact `outputs/batched-migration-verified.osb`;
use another unused path when reproducing. Workspace checks use the commands above,
with the same 150-test suite plus the expanded installed probe (not a new unit-test
count). Repository remains untracked, with no HEAD/remote and license pending.

## Editor and native form follow-up

Editor now routes reviewed Migrate drafts through a staged session, with the same
single-pending-command lifecycle as ordinary tools. Selection, level or view changes
cancel authority; the host also checks document revision/session and provider
activation. Cancel is reported once through normal polling. Save remains blocked
while plugin work is pending, and successful migration queues current-schema geometry.
The form shows candidate progress and an all-owner warning; it does not create a
backup. It supports one owned type; owners with multiple types are safely rejected
until a multi-command review UI is implemented. Ordinary ToolDraft::start remains
the low-level single-call API, while Editor::start_plugin_tool dispatches migrations.

The installed probe additionally passes an Editor 33-column workflow: change
selection after 16 staged conversions and reject without live mutation, restart,
commit at revision 1, generate 33 meshes, undo to unavailable v1, redo, save and
reopen all migrated data. Earlier cancellation/rejection/stale checks now also run
through the staged Editor path. No new wire or native file versions/dependencies.

On local Windows, the `migration_desktop` harness was exercised at 1280×800. It
explicitly loads the independently installed v2 guest with test-owned grants and
clones a v1 source column into 33 disposable entities. Select Ground, select a
column, choose `org.example.columns.migrate`, review the visible warning and Apply.
Observed: commit status, disappearance of the 33-unavailable warning, geometry,
one Undo restoring 33 unavailable elements, and Redo restoring geometry without
additional interaction. Columns overlap intentionally, so this is not separate
object layout/rendering qualification. The transient progress text settled too
quickly to inspect; native cancellation remains unobserved.

Native Save wrote the new `outputs/native-migration-acceptance.osb`, then the clean
window closed. Read-only ZIP/JSON inspection confirms 33 schema-2 envelopes and
the exact column requirement 2.0.0. The source `outputs/native-column-acceptance.osb` was not a save
target. The harness accepts installation and source paths as positional arguments:

```powershell
.\tools\cargo.ps1 run -p os-ui --features external-plugins --example migration_desktop --locked --offline '--' outputs/rust-column-v2-install outputs/native-column-acceptance.osb
```

Supply your own saved v1 column project when the local evidence artifact is absent.
No permission UI automation or production automatic loading is involved. Workspace
tests pass (150), strict Clippy and formatting pass, the expanded installed probe
passes, default build passes and smoke passed at `outputs/editor-migration-verified.osb`.
Initial Clippy rejected a large enum variant; boxing the staged state resolved it.
CI already runs the expanded probe; no hosted result is claimed.

Next three tasks: independent mixed-type/failure acceptance, multi-type review and
remaining native B/C workflows, then linked floor-plan D. E1–E4/G1–G6 and production
targets remain open; L1/L2 remain later. Runtime and size limits above still apply.

## Lifecycle boundary follow-up

The installed probe now also rejects empty plans, ordinary Edit descriptors used
as migration commands, zero deadlines and deadlines beyond five minutes. After
16 successful candidate conversions it separately verifies:

- reopening byte-equivalent semantic data as a new Document session;
- unloading and reloading the same installed v2 provider/version;
- expiration of the overall migration deadline;
- changing the Editor view's settings revision while keeping selection unchanged.

Each rejection preserves live model/revision/history. Failed host sessions lose
their progress/commit authority and cannot be polled into a later commit. The
provider test uses the real independent Wasm installation; it checks activation
identity rather than only a version string. The deadline case deliberately waits
past a two-second timeout after partial progress; it is not a hard-interruption or
performance test. Worker draining remains bounded by existing runtime policy.
The same probe subsequently completes a valid migration, proving that these
rejections do not permanently exhaust worker capacity. No production source or
API/file-format change was needed for these additional checks.

Reproduce with the installed probe command above. Its additional cases run in the
existing CI step; this follow-up was observed locally on Windows only. Mixed-type
guest acceptance and multi-type desktop review remain the next migration work;
these lifecycle checks do not substitute for them or close the B/C gate.
The 150-test workspace suite, expanded installed probe, strict Clippy, formatting,
default workspace build and smoke at `outputs/migration-lifecycle-verified.osb`
all pass. Existing runtime/file bounds, open production profile and incomplete
D/E1–E4/G1–G6 remain unchanged; L1/L2 are later. No publication or hosted CI result.

## Explicit all-type review follow-up

Plugin tools now exposes **Review all <owner> migrations…**. It lists every type
owned by that provider in the current document. Choose an enabled registered
Migrate command separately for each type and review its typed fields. There is no
implicit first-command choice. Apply complete migration plan stays disabled until
all types have a valid choice; it submits one staged all-owner migration. Discard
drops the review. Selection/provider/document changes invalidate reviewed authority.
Unsupported types remain unreviewable and prevent partial-owner commit.

`MigrationDraft` and `Editor::start_plugin_migration` are additive feature-gated
Rust APIs. Ordinary command fields and migration plan fields share the same widget
renderer. The existing single-command shortcut remains available for single-type
owners. Wire ABI, native model/container/envelope versions and runtime grants are
unchanged. No automatic backup, project-directed loading or publication was added.

A new two-type controller fixture verifies missing choices, ordinary command and
wrong-type rejection, per-type input validation, read-only review and stale/context
checks. Initial fixture errors (missing registration and an invalid underscore in
a command ID) were rejected by the host and corrected, not bypassed. This fixture
does not supply mixed-type Wasm execution evidence. The independent 33-column probe
now commits through the new complete-plan Editor API, rejects an unreviewed plan,
and retains geometry/history/storage acceptance.

Native Windows inspection using `migration_desktop` confirmed the new entry point,
all-owner warning, disabled Apply with no choice, explicit command dropdown, enabled
Apply after choosing, and successful commit. No column selection was needed; Ground
was selected first. The 33-column disposable harness remains open and unsaved from
this inspection; no source file was changed. Two-type layout, missing-command UX
and mixed-type guest execution remain unqualified. Computer-use inspected only the
test harness; no grant/security controls were changed.

Current verification: 151 workspace tests, installed migration probe, strict Clippy,
format check, default build and `outputs/migration-review-verified.osb` smoke pass.
Use the existing probe/harness commands above. Next: independent mixed-type guest
success/late-failure evidence; native mixed-type review and remaining B/C workflows;
then D. E1–E4/G1–G6, production target decisions, L1/L2 sequencing and unpublished,
license-pending repository status remain as recorded above.

## Independent mixed-type guest evidence

The separate `mixed-migration` column build and `plugin-mixed.toml` now provide an
explicitly test-only block type alongside columns. Columns rename height; blocks
rename width and height, so routing the wrong transformer is observable. No new
architectural authoring capability is claimed. The host probe was built before
the guest, then loaded `outputs/rust-column-mixed-install` without rebuilding the
host or replacing either existing v1/v2 installation. See the
[exact build/install/probe instructions](../examples/rust-column/README.md).

The mixed probe passes with eight columns and five blocks. A plan with only the
column command is rejected. After both commands are explicitly reviewed, a vendor
key collision in the final block rejects the entire migration after 12 candidate
conversions (all eight columns plus four blocks). Model, revision and undo history
remain unchanged. A valid retry produces distinct schema-2 payload transformations,
13 checked meshes, one live revision, atomic undo/redo and native save/reopen.
The required provider version is updated with both types, not after the first type.
The earlier single-type lifecycle and Editor probes continue to pass.
Each resulting mesh volume is also checked against its source dimensions.

The guest has five mixed-feature native tests, including wrong-command rejection
and destination-key preservation; default and migration-v2 editions retain four
passing tests each. Host/guest strict Clippy, formatting, independent dependency
boundary and default smoke `outputs/mixed-migration-verified.osb` pass. No wire,
container/model/envelope version, dependency or unsafe-policy changes. CI contains
the mixed guest build and three-installation probe; only local Windows was observed.

Remaining: native mixed-type review and remaining B/C installed workflows, then D
linked floor-plan authoring, then E1–E4/G1–G6; L1/L2 remain later. Large-project
throughput, production targets and release qualification remain open. Publication,
license and runtime trust limitations are unchanged.

The full 151-test workspace suite passed using
`cargo test --workspace --all-features --locked --offline --target-dir work/completion-build`.
The normal target initially failed to link the still-running native acceptance
executable on Windows; the isolated target avoids disturbing its unsaved model.
This was an executable lock, not a test assertion failure. The prior native harness
was left untouched; no native mixed-type workflow was performed in this follow-up.

## Native mixed-type review follow-up

The `mixed_migration_desktop` harness now reuses the original migration harness
implementation, loading the mixed installation and seeding eight old-schema columns
plus five test blocks. It accepts installation/source paths in the same order as
the single-type harness. Defaults point only to developer-owned evidence artifacts,
not application plugin discovery. No production permissions or loading flow changed.

Observed on local Windows at 1280×800: select Ground, open Review all migrations,
select only the column command, scroll and verify Apply remains disabled with a
missing-type error. The block dropdown contains its separate migrate-block command.
Selecting it enables Apply. Apply commits, one Undo restores all 13 unavailable
elements, and Redo restores the migrated state. Save creates the new artifact
`outputs/native-mixed-migration.osb`; staged Open reports zero unavailable plugin
elements and the viewport displays regenerated geometry. The elements intentionally
overlap, so this is not independent-object picking or layout acceptance.

Read-only ZIP/JSON inspection confirms eight rectangular schema-2 envelopes, five
test-block schema-2 envelopes and requirement 2.0.0. Column payloads retain width
and rename height; block payloads rename both width and height. The source file was
never a save target. The new mixed harness closed cleanly after reopen. The earlier
single-type unsaved harness remains untouched and open.

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example mixed_migration_desktop --locked --offline
.\tools\cargo.ps1 run -p os-ui --features external-plugins --example mixed_migration_desktop --locked --offline '--' outputs/rust-column-mixed-install outputs/native-column-acceptance.osb
```

Choose a new save path when repeating. Build the mixed guest as documented in its
README first; source must contain old-schema columns. Native failure/cancellation
timing remains unobserved (the installed probe covers those rejection paths).
Large-type-count/parameter-rich form layouts remain unqualified.

Verification: all 151 workspace tests passed in `work/completion-build` to avoid
the earlier window's executable lock; strict Clippy, format, default build and
smoke `outputs/native-mixed-review-verified.osb` passed. Shared harness extraction
and the new example are the only executable-source changes this turn. No public
API, dependency, ABI or file-format changes. No remote/hosted CI/publication claim.
Next: native independent Wall acceptance; re-audit and close remaining B/C gaps;
then D linked floor-plan authoring. E1–E4/G1–G6 and production qualification remain
incomplete, with L1/L2 later and the production profile still undecided.
