# BUILD_PROMPT completion audit

Historical foundation audit. The subsequent experimental IFC CLI implementation
and its independent checks supersede the IFC-unavailable statements below; see
[the current IFC slice report](ifc-roadmap.md). Desktop exchange is now enabled
after the independent viewer check described there.

This checklist follows `BUILD_PROMPT.md`, including its explicit alternatives.
It is not a claim that the future production BIM platform has been built.
The live worktree and tests are authoritative; a past green build cannot prove
completion while another task is modifying the same files.

## Vertical slice and required UI

| Requirement | Implementation and acceptance evidence |
| --- | --- |
| Create a project and level | `Document::new`, `Command::AddLevel`, desktop New/Add level controls; controller integration and desktop input test |
| Load Wall plugin | `PluginHost::load`, bundled `plugins/walls/plugin.toml`; integration checks loaded manifest |
| Wall between two points | `WallParams`, CreateWall JSON request, Wall plugin; integration and desktop input test |
| Edit length, thickness, height, level | `WallParams::with_length`, EditWall and property editor; tests assert dimensions, direction, volume and elevation |
| Regenerate derived geometry | Document invalidation events → Editor queue → plugin Solid → GeometryKernel mesh; level-change, wall-edit and failure-recovery tests |
| Undo and redo | Validated document snapshots, fresh events, redo-branch invalidation; atomic history, integration and desktop input tests |
| Save and reopen `.osb` | `StorageBackend` and `ZipJsonStorage`; actual container round trips and migration/resave tests |
| Basic 3D viewport | `os-render` camera, projection, culling and picking, native egui canvas; native inspection and renderer regression tests |
| IFC or explicit deferred alternative | Deferred alternative selected: `os-ifc::AVAILABLE = false`, unsupported adapter, disabled UI and `docs/ifc-roadmap.md`; no IFC output or validity claim |
| Level list, Wall tool, property editor, history and file controls | Desktop input test uses rendered controls to perform creation, all wall edits, level assignment, Undo/Redo, Save, New and Open |

## Architecture, entities and invariants

| Requirement | Evidence |
| --- | --- |
| Stable Rust Cargo workspace | Root Cargo.toml, rust-toolchain.toml, Cargo.lock and build command; 12 required `os-*` crates plus `plugins/walls` |
| Core state and logic in Rust | Model, constraints, transactions, storage, contracts, editor state, UI and renderer are Rust packages |
| Replaceable geometry, UI, renderer | GeometryKernel and plain boundary values, standalone os-render, Editor controller and eframe shell; no kernel types in os-model |
| Avoid unsafe code | Every workspace crate inherits workspace `unsafe_code = "forbid"`; third-party implementation is outside that authored-code guarantee |
| Seven typed entities | Project, Site, Building, Level, Wall, Material and View aliases over Entity<T>; all-entity storage test includes a nonempty material and wall |
| UUID, type, parameters, relationships, properties and schema | Entity<T>/Header and Model::validate; exact round trip with material/level references, custom relationship and nested extension data |
| SI units and explicit conversion | Point coordinates/dimensions in metres, LengthUnit metre/millimetre/foot conversion; unit tests and UI labels |
| All edits through transactions | Document model exposed read-only; commands applied to candidate then validated before commit; plugin receives JSON, not Document/database access |
| Atomicity, changes and dependencies | Failed-command batch preserves model/revision/history; change events contain changed IDs and old/new dependent walls |
| Regeneration scheduling | Editor coalesces invalidations, removes stale/deleted meshes and retains failures for retry; failed regeneration blocks application save and remains undoable |
| Save/open validation | Container/schema/identity/units/backend/relationship corruption tests; failed Editor open preserves current model |
| Future associative drawing design | View entities and stable model IDs, architecture rule for future plans/sections/annotations; no disconnected drawing system introduced |

## Native storage

| Requirement | Evidence |
| --- | --- |
| Versioned manifest and model container | ZIP contains manifest.toml, model.json, assets/ and previews/; manifest/container/model versions checked on open |
| Backend interface and temporary-backend migration path | StorageBackend trait; JSON is the explicit first-slice alternative permitted by the prompt; SQLite migration described in file-format.md and ADR 0001 |
| Schema migration tests from the beginning | Frozen schema-0-model.json, in-memory migration test and real old-version container open→migrate→save→reopen test |
| Robust reopen | Bounded reads, central-directory duplicate detection, complete graph validation, atomic file replacement; corruption and duplicate-entry tests |

Reserved assets/previews are empty in this version. Unknown ZIP entries are not
preserved. History and geometry are intentionally regenerated/not persisted;
these limits are documented rather than represented as complete integrations.

## Plugin and geometry contracts

| Requirement | Evidence |
| --- | --- |
| Manifest ID/name/version/API/dependencies/capabilities/permissions/entrypoint | Manifest struct and validation; bundled TOML and manifest tests |
| Register types, tools, panels, views, exchange, schedules/reports, analysis | All nine RegistrationKind values load and are discoverable by kind and owning plugin in the registry acceptance test |
| Unique registration ownership | Host global index; overlapping namespaces rejected before any registration is installed |
| Dependency and permission enforcement | Missing/version-mismatched dependencies rejected; excess host grants never confer unrequested model.read/model.write; rejected request never invokes plugin or mutates document |
| Serializable versioned boundary | RequestEnvelope/ResponseEnvelope with API_VERSION, tagged JSON messages; mismatch and hostile-response tests |
| Built-in first runtime and future transport design | Plugin trait is a built-in JSON transport, not a dynamic Rust ABI; Wasm/process isolation design in plugin-api.md |
| Plugins use transaction service | Host validates emitted commands against registered element types, then Document validates whole batch; Wall plugin does no storage writes |
| Geometry concepts | Profile, Transform, Solid, Mesh, BooleanOperation, GeometryKernel::tessellate and SectionResult; booleans/sections explicitly return unsupported |
| Valid initial wall solid | Rectangular prism kernel, outward winding, paired edge/topology test, volume and translated/rotated regeneration tests |

Registration support is complete as metadata discovery. General third-party
panels and execution of future view/exchange/report/analysis services are not
claimed. Built-in plugins are trusted code; protocol permissions are not an OS
sandbox. Neither is required to be a production isolated runtime in this slice.

## IFC conditional gate

The prompt explicitly permits either a valid IFC adapter **or** a clearly isolated
IFC implementation task (vertical-slice item 9 and first-deliverables item 9).
This implementation takes that alternative. `docs/ifc-roadmap.md` specifies the
native→IFC→native fixture, independent IFC validity checks and external-viewer
fixture that must pass before the flag and UI are enabled. The unavailable-adapter
test verifies errors, not IFC validity. IFC validity/round-trip tests are therefore
deferred with the permitted adapter task; they must never be reported as passing.

## Repository, quality and delivery

| Requirement | Evidence |
| --- | --- |
| All named paths/artifacts | Cargo.toml, README.md, LICENSE.pending, NOTICE; docs/architecture.md, file-format.md, plugin-api.md, decisions/; all named crates, plugins/walls, fixtures, tests, tools |
| Tests for identity/units/walls/geometry/transactions/save/migration/manifests/permissions | Workspace crate tests plus `os-ui/tests/vertical_slice.rs` and desktop input test; final command results in verification.md |
| CI format, Clippy, unit/doc tests, dependency audit, license metadata | `.github/workflows/ci.yml`; local equivalent checks, no claim that hosted CI was executed |
| Independent implementation and identity | Original authored code and layouts, OpenStructure app title, NOTICE; no native `.rvt` implementation or proprietary schemas/assets imported |
| No unrequested infrastructure | Local application and file storage; no telemetry, cloud accounts, AI services or collaboration servers added |
| Architecture decisions before major implementation | ADR 0001 foundation choices and ADR 0002 audit hardening |
| Developer docs and example plugin | README commands/workflow/limits, architecture/format/protocol docs, working `plugins/walls` and smoke example |
| Final feature/limit/command/decision/next-task report | verification.md and user-facing completion report |

License selection remains pending as in the initial repository. LICENSE.pending
and non-publishable packages make that status explicit. Dependency metadata is
checked; a legal review is not claimed.

## Current audit status

The complete first vertical slice passes the requirements above, with the
prompt's explicitly permitted JSON storage and deferred IFC alternatives.
Final combined verification passed 34 tests (including the palette resize
regression and one doctest), strict Clippy, formatting, and a full workspace
executable build. The finished executable passed the complete smoke workflow,
writing `outputs/final-audit.osb`. Dependency license metadata passed for all
333 packages. The separate UI task has completed; its changes were preserved.

The verified executable is `work/completion-build/debug/os-app.exe`. A separate
target directory avoided replacing the executable open for native inspection.
See verification.md for commands, platform coverage, the dependency maintenance
warning, intentional limits and next tasks. Completion means this foundation,
not the future production BIM platform or an implemented IFC adapter.
