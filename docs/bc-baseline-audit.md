# B/C baseline gate audit

Verdict: **the bounded development B/C baseline is verified on Windows; proceed
to D linked floor-plan authoring**. This is not general production plugin support,
an approved deployment profile, or completion of the BUILD_PROMPT. D's callable
2D contributions and E1–E4/G1–G6 remain required. Earlier reports' next-task lists
are historical, not independent evidence that already implemented features are missing.

Audit state: local untracked working tree, no HEAD/remote or hosted CI result.
The immediately preceding retention checkpoint passed 160 workspace tests,
strict Clippy/format/build/smoke and all installed Editor/migration probes.
This audit reread the current contract, runtime/loading, generic worker tests,
installation guides and native evidence; reran independent guest/contract tests
and dependency boundaries; and read the existing native output archives again.

## Required B evidence

| Requirement | Current implementation and evidence | Scope/limits |
| --- | --- | --- |
| Independently consumable versioned SDK | `os-plugin-api` with `default-features = false`; API-2 request/response/catalog/command/geometry DTOs; separately locked Rust Wall and column | Both dependency checks pass: 23 versions / 22 names, no host/model/kernel, eframe, uuid or wasmi dependencies. Contract-only tests: 5; Wall: 2; mixed column: 5 |
| Negotiation, correlation, errors, scopes and permissions | `generic.rs`, `generic_commands.rs`: version/request/session/revision checks, exact scopes/reserved IDs, typed payload/catalog validation, per-request authorization | API-1 remains explicit legacy compatibility, not an independent semantic model contract |
| Descriptor-driven create/edit and parameter panels | `plugin_tools.rs`, `plugin_forms.rs`; independent Wall/column catalogs; typed number/unit/range, choice/text/bool and disabled-state tests | Hosted generic forms, not arbitrary executable UI. Native Wall six-number form requires scrolling |
| 2D interaction/graphics design | `plugin-plan-contract-design.md` covers work planes, view ranges, gesture lifecycle, snap ownership, semantic graphics, limits and stale-view policy | Design only as required by B. No callable plan service is claimed; required in D |
| Bounded Wasm transport | `wasm.rs`, `wasm_transport.rs`: ABI 1, fresh instances, no imports/WASI, 4 MiB artifact, 1 MiB messages, 16 MiB linear memory, 1M fuel, bounded stack/table/recursion | In-process interpreter, not process isolation; fuel includes startup/ABI/allocation/invocation |
| Explicit installation and minimal manager | Adjacent `plugin.toml` / named `.wasm` in a user-selected installation directory; `loading.rs`, `plugin_manager.rs`; documented manager load/disable/restore, metadata/status/dependencies/grants | Opt-in `external-plugins`; no automatic discovery, project execution, persistent grant store or marketplace. Those are not implied by this baseline |
| Validation before activation | Manifest parser and loader tests reject invalid IDs/versions/API/dependencies/collisions/paths/size/imports; background adoption rechecks collisions | A rejected installation publishes no registrations |
| Off-UI work, deadline/cancellation/unload | `worker.rs`, `loading.rs`, generic worker tests and Editor probes; bounded jobs, runtime references retained until drain, generation/session/view-stamped adoption | Deadlines revoke results, not forcibly kill OS file IO/compiler work; guest execution remains fuel-bounded |
| Rust Wall installation and authoring | `examples/rust-wall`, `native_wall_probe`, `plugin_editor_probe`, `native-wall-workflow.md` | Host built before guest installation; registered create/edit, checked geometry, history and storage. Native create/edit/undo/redo/save/open/shared selection observed using explicit test grants |
| Independent new type without host changes | `examples/rust-column`, `generic_probe`, `plugin_editor_probe`, `native-column-workflow.md` | Registered column create/edit/delete, geometry, history/save/open and missing-provider retention. Native create/edit/undo/save/open observed; no production grant-UX qualification |

### Required failure matrix

These tests inspect real refusal paths, not just an unavailable feature flag.

| Required failure | Source-backed evidence |
| --- | --- |
| Denied write before invocation | `generic_commands::permissions_inputs_and_unregistered_commands_fail_before_invocation`; installed probes; `wasm_transport::denied_writes_fail_before_guest_trap_and_preserve_history` |
| Incompatible API | Contract manifest version tests, `wasm_transport::loading_errors_leave_no_registrations_or_plugins`, generic response `version` faults |
| Oversized/malformed reply | `generic_commands::every_rejected_reply_preserves_model_revision_history_and_events` includes malformed/oversize/version/correlation faults; Wasm byte/range/batch tests |
| Timeout | Generic `expired_generic_reply_cannot_commit_and_releases_its_slot`; load expiry; migration overall deadline after partial progress |
| Trap/infinite loop/allocation limits | Generic `cancelled_dropped_trapping_and_fuel_exhausted_workers_drain_without_edits`; Wasm startup/invoke fuel, memory/table/recursion tests and recoverable trap test |
| Stale reply after document switch | Generic `edits_undo_reopen_and_document_switch_revoke_generic_reply`; identical reopened models get a new session, and undo cannot revive old work |
| Disable/unload and late callbacks | Generic view/provider-generation revocation, Wasm unload/reload/drain tests, Editor cancellation/scene invalidation and manager disable/restore tests |

## Required C evidence

| Requirement | Current implementation and evidence |
| --- | --- |
| Opaque bounded extension envelopes and exact requirements | Model schema 2, envelope version 1, stable UUID/type/owner/payload schema/relationships; `os-model/tests/extensions.rs`; arbitrary-precision values preserved |
| Missing/disabled provider remains inspectable and safely savable | Read-only unavailable-data UI; `os-ui/tests/vertical_slice.rs`; installed Wall/column probes unload before native save/reopen; no plugin installed from a project |
| Auxiliary preservation and safe native files | Container 2, bounded ZIP/JSON reader, duplicate/path/symlink/CRC checks, atomic writer; `os-storage/tests/auxiliary_files.rs`, `extensions.rs`, frozen old-container/model fixtures |
| Transactional native/schema migrations | Copy/validate/adopt native migration, explicit version dispatch and old-file resave tests; future versions fail without rewriting the source |
| Transactional plugin migrations | `migration.rs` isolated candidate, four-element fuel-bounded batches, one final live transaction. Independent v1/v2 and mixed guests exercise success, unknown-key preservation, late failure, cancellation, stale/reopened documents and replaced providers |
| All-type migration review and history | `MigrationDraft`, descriptor review and installed mixed probe; native mixed Apply/Undo/Redo/Save/Open recorded in `plugin-migration-service.md` |

Read-only artifact recheck: `native-independent-wall.osb` contains one native Wall
and Wall requirement 0.1.0; `native-column-acceptance.osb` contains one extension
and column requirement 1.0.0; `native-mixed-migration.osb` contains 13 extensions
and requirement 2.0.0. All are model schema 2. They are local generated evidence,
not production reference projects. No archive or original installation was changed.

## Decision boundary and remaining work

The gate accepts the documented, explicitly installed development runtime and its
baseline native/automated workflow combination. It does not claim every failure
was induced through native clicks or that permission controls were automated.
The minimal manager was natively inspected; security/grant changes were not made
by computer automation. Test harnesses provide explicit test-owned grants.
Production installation/update policy, native permission UX qualification, broad
third-party compatibility and performance remain separate release work.

Known non-blocking development rough edges include the collapsed tool-window
review behavior and large descriptor forms requiring scrolling. The history
notice has headless layout evidence but not native compositor inspection.
The default application remains trusted bundled code unless built with the
external-plugins feature. No unsupported geometry, plan service, annotation,
sheet, translator or team feature becomes available because this audit passed.

Next implementation is D: actual straight-wall cut/projection and semantic
2D graphics, named persisted level plans, split navigation/selection, snapping
and transactional gestures, and then independently installed callable plan tools/
providers. D's complete acceptance must pass before E1. Continue F documentation
and CI hygiene without publication. Owner jurisdiction/project/concurrency/OS/
CAD/PDF/plot/performance choices remain open; licensing is pending. L1/L2 remain
after E4, not grounds for delaying the required D/E1–E4 work.
