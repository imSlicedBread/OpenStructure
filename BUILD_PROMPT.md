# OpenStructure implementation prompt

Use this prompt to continue building the existing OpenStructure repository.
It replaces the original bootstrap prompt. Editing or reviewing this document
does not itself authorize executing the implementation or publishing to GitHub.

---

You are the lead implementation engineer for **OpenStructure**, a Rust BIM
authoring application intended to be open source, extensible through plugins,
and usable offline. Continue the existing application and complete the next
milestones toward production architectural 2D authoring and documentation with
the supporting BIM modeling and content foundations. Deliver working code, tests,
and developer documentation. A basic floor-plan editor is an intermediate
milestone, not the final production scope.

## 1. Start from the actual repository

The current local project is:

```text
C:\Users\Youca\Documents\ChatGPT\OpenStructure
```

This path identifies the current checkout, not a path to embed in application
code. Other contributors must be able to build from their own checkout location.

Before changing code:

1. Read applicable repository instructions and inspect Git status and remotes.
2. Read `Cargo.toml`, `rust-toolchain.toml`, `README.md`, and the relevant source.
3. Read `docs/architecture.md`, `docs/file-format.md`, `docs/plugin-api.md`,
   `docs/ifc-roadmap.md`, `docs/ui-design-guide.md`, and applicable decisions in
   `docs/decisions/`.
4. Read `docs/2d-production-spec.md` in full. Its required capability groups,
   BIM foundations and production release gates are mandatory for the first
   architecture-firm release; L1/L2 are explicitly later milestones. Maintain a
   source-backed `docs/2d-coverage.md` during implementation.
5. Use `docs/build-prompt-audit.md` and `docs/verification.md` as historical
   evidence. Verify current behavior before relying on their completion claims.
6. Run an appropriate baseline and record existing failures separately from any
   caused by your changes. Preserve unrelated work, including uncommitted files.

The repository already contains a 13-package Rust workspace, an egui/eframe
desktop application, typed BIM entities, transactional editing and undo/redo,
ZIP/JSON `.osb` files, a bundled Wall plugin, and restricted IFC4 wall exchange.
Source and ADR 0005 also contain a CPU depth-buffered viewport with entity picking;
some older documentation still describes painter sorting. Verify this baseline,
then correct stale descriptions as the affected areas are updated.

Do not scaffold a second application or repeat completed bootstrap work. Keep
working through the requested milestone until its acceptance checks pass, or
report the exact external dependency or owner decision that prevents completion.

## 2. Product goals and scope

Build a dependable desktop BIM editor in which users own their project files
and contributors can add tools without rebuilding the host application.

The foundation covers projects, sites, buildings, levels, grids, straight walls,
materials, editable floor plans linked to 3D, extensibility, native persistence,
and limited IFC exchange. Treat 2D plan authoring as a foundational workflow,
not only a later drawing export. One building model must support multiple editable
views without duplicating building elements into separate 2D and 3D models.
Slabs, openings, doors/windows, associative sections/elevations, annotations,
independent drafting views, sheets, and schedules follow as bounded milestones,
but are required parts of the production architectural scope rather than optional
future ambitions. The target is a complete architecture-firm drawing workflow:
model-linked plans and documentation, reusable details/content, coordinated
annotations and schedules, office standards, revisions, consultant exchange,
reliable plotting, safe team use, recovery, and qualified release support.

Four companion BIM capabilities are required for the first production release:
a no-code parametric 3D component editor, complete architectural creation/editing
workflows, centralized parameter/material management, and model checking with
linked-model coordination. Develop them in E1-E3 alongside documentation and
qualify them in E4; they are not optional substitutes for the existing 2D scope.

Advanced site modeling and 3D presentation are planned follow-on milestones L1
and L2, after E4. Existing site plans, survey/terrain references, shared coordinates
and basic linked 2D/3D editing remain mandatory. Advanced rendering and specialist
analysis stay plugin-oriented follow-on work.

Use `docs/2d-production-spec.md` as the detailed requirement and acceptance
contract. Complete every required capability before claiming general production
readiness; any narrower preview release must state its limitations. Confirm
firm/project, jurisdiction, concurrent-user, output-format and performance targets before
qualification. Structural/MEP coordination views fit this scope; specialist
analysis and discipline-specific authoring need not be implemented now.

Native `.rvt` support, cloud collaboration, telemetry, online accounts, AI
services, a plugin marketplace, and certified engineering analysis are outside
this foundation unless separately requested. Use independently authored code,
UI assets, documentation, and examples. Do not use proprietary Autodesk code,
assets, or branding. Keep the chosen OpenStructure name; record the existing
name-clearance status without restarting the naming discussion.

## 3. Preserve the Rust architecture

Keep the existing workspace boundaries:

```text
crates/
  os-core          identity, units, shared boundary values and errors
  os-model         semantic entities, relationships and validation
  os-constraints   dependency invalidation and future constraints
  os-document      transactions, history, revisions and change events
  os-storage       native container, compatibility and migrations
  os-geometry      kernel interface, geometry recipes and mesh validation
  os-plugin-api    versioned protocol and public extension contracts
  os-plugin-host   loading, grants, dispatch and extension lifecycle
  os-ifc           explicit IFC import/export subset and loss reports
  os-render        viewport rendering and picking
  os-ui            editor controller and desktop presentation
  os-app           executable and command-line entrypoints
plugins/
  walls            working reference plugin
```

Preserve Rust 2024, the declared minimum Rust version, `Cargo.lock`, and the
existing UI framework unless a demonstrated problem justifies an ADR and change.
Verify new dependencies against their primary documentation and actual builds.
The model and document layers must remain usable without a GUI or plugin runtime.
Preserve the workspace's authored-code `unsafe_code = "forbid"` policy; isolate
any future foreign-function adapter rather than weakening unrelated crates.

Avoid circular dependencies. As plugin APIs become independently usable, move
wire DTOs into a small contract layer rather than exposing document internals to
plugins. Internal Rust traits are fine within the compiled workspace; ordinary
Rust trait objects are not the external binary compatibility boundary.

## 4. Model, identity, units and transactions

- Treat the semantic model as authoritative. Geometry and drawing output are
  derived data and must be reproducible from model parameters.
- A floor-plan edit and a 3D edit must operate on the same entity IDs through the
  same document commands. Share model selection and history across views; keep
  navigation and display settings view-specific. View-owned drafting/annotation
  objects must remain distinct from building elements and generated geometry.
- Preserve stable UUIDs and namespaced type IDs across edit, undo, save, import,
  and migrations. Validate references and define deletion/dependency behavior.
- Use `f64` for model coordinates, metres internally, and explicit unit
  conversion at input/output boundaries. Document axes, handedness, local
  origins, and tolerance policy. Do not use machine epsilon as CAD tolerance.
- Reject nonfinite values, zero-length walls, invalid dimensions, inconsistent
  IDs, dangling references, and unsupported dependency cycles.
- Distinguish type-level defaults from instance parameters before expanding
  reusable components. Keep host relationships and level constraints explicit.
- Extend the planned semantic model for reusable 3D component definitions/types,
  instances and host bindings; centralized parameter definitions, project-wide
  constraints and material-library entries; and revision-bound diagnostic records.
  Reuse stable IDs and namespaced definitions across geometry, drawings, tags and
  schedules. Validate units, formula dependencies and library updates before commit.
- Apply every committed edit through the document command/transaction service.
  A rejected batch must leave model, revision, history, and saved state unchanged.
- Preserve undo/redo semantics, including redo invalidation after a new edit.
  One continuous drag should normally create one undoable transaction.
- Regeneration must invalidate affected dependents and run outside slow UI paths.
  Tag asynchronous requests/results with document identity and revision; reject
  stale results after edits, undo, closing, or switching documents.
- Keep property drafts separate from committed state. Preserve existing dirty
  state, failed-open, failed-regeneration, and overwrite protections.

Measure snapshot-history cost before replacing it with deltas. Bound history
memory without changing user-visible transaction behavior. Cloud collaboration
is not a reason to introduce distributed state machinery at this stage.

## 5. Native persistence and compatibility

The implemented `.osb` version is a ZIP container containing:

```text
manifest.toml
model.json
assets/
previews/
```

Keep this format readable. SQLite is a future option behind `StorageBackend`,
not a reason to replace working persistence during plugin development.

Required guarantees:

- Validate the complete candidate document before replacing the open document.
- Preserve bounded parsing, duplicate-entry checks, path validation, and atomic
  save replacement. Failed saves must leave a previously good file usable.
- Make container version, model schema version, and plugin data schema versions
  distinct. Add explicit migrations with frozen old-file fixtures.
- Read older supported versions and reject unsupported future versions clearly;
  never silently reinterpret a file or migrate the user's only copy in place.
- Before storing plugin-defined entities, add a versioned extension envelope:
  owner plugin ID, type ID, payload schema version, stable entity ID, relationships,
  and an opaque payload with bounded size. Preserve unknown extension data.
- Record the plugin dependencies needed to edit a document. A project file must
  never install or execute a plugin automatically.
- When a plugin is missing or disabled, retain its entities and relationships,
  show an unavailable state, and allow safe inspection. Optional cached preview
  geometry is derived data, not a substitute for the original payload.
- Saving without an optional plugin must preserve its opaque data. If lossless
  preservation is not implemented, use read-only mode and block destructive save.
- Plugin-owned migrations must be transactional, operate on a copy, and preserve
  the original on failure. Do not execute embedded code from a project file.
- Persist named views and their level/reference, projection basis, cut/view range,
  crop, scale, and visibility settings as versioned data. Later persist view-owned
  annotations, drafting objects, templates, sheets and issue records with stable
  IDs as their milestones land. Migrate existing views using
  documented defaults and preserve model identity when saving/reopening any view.
- As E1-E3 land, version component definitions and packages, parameter/material
  libraries, monitored link relationships and retained diagnostic state. Preserve
  definition/instance identity through save, migration and supported library updates;
  document collision/remapping behavior when transferring content between projects.

The present container discards unknown ZIP entries. Implement attachment/extension
preservation before claiming documents with such content can safely round-trip.

## 6. Plugin architecture: the next main milestone

The current Wall plugin runs as trusted built-in Rust code through versioned JSON
messages. Its manifest uses integer `api_version = 1` and entrypoint
`builtin:os-walls`. Registered tools/panels are partly implemented directly in the
desktop shell. This is a useful baseline, but it does not yet allow independent
third-party plugins or provide an OS security sandbox.

Complete one runtime for separately built WebAssembly plugins first. Evaluate
the runtime with a small compatibility/build spike, record its ABI, supported
targets, dependency cost, and resource limits, then implement it end to end.
Keep external-process JSON-RPC as a later transport for Python/C++ tools. Do not
build three unfinished runtimes in parallel. Use bundled trusted code only where
the trust distinction is explicit.

### Contract and extension points

- Specify protocol negotiation, request IDs, structured errors, cancellation,
  document revision, and size limits. Version incompatible protocol changes and
  provide a compatibility policy for the existing v1 Wall plugin.
- Define generic registered command dispatch and typed/validated element payloads.
  Adding a new extension must not require a new wall-specific enum case in the UI.
- Support callable modeling commands and host-rendered property/tool descriptors
  first: numeric inputs with units and limits, enum fields, validation messages,
  actions, selection context, and enabled/disabled states.
- Design the 2D contracts in this milestone and implement their callable path in
  the floor-plan milestone: active view/level and work-plane context, model-space
  pointer input, snap candidates, transient previews, cancellation, and one
  validated commit. Support bounded plan graphics/symbols and hit-test references
  tied to semantic IDs. Define how providers honor cut/view ranges and visibility;
  use host-derived geometry as a documented fallback where supported. A top-view
  mesh or metadata registration alone does not establish plan-view support.
- Route every proposed edit back through host authorization and document validation.
  Plugins receive scoped snapshots/queries, never mutable host pointers or storage
  handles. A plugin owning a type does not automatically gain access to all objects.
- Reserve broader custom view and specialist analysis categories where appropriate.
  Plan graphics/tools are required in D; annotation/content, schedule/report,
  export and preflight services needed by production 2D must become callable in
  E1-E3. They cannot remain metadata-only reservations at production release.
  Label unimplemented providers accurately until integration tests pass.
- Extend the E1-E3 contracts for versioned component definitions and geometry/2D
  representations, typed parameter/material libraries, and model-check providers.
  Providers query scoped revision snapshots and return bounded diagnostics with
  rule IDs, affected entity/link references and actionable explanations. Checks
  are read-only; any repair is a separate authorized document transaction.
  Component authors must be able to use the editor without writing Rust or
  installing a separate executable plugin for each content item. Apply existing
  sandbox and validation rules to executable providers and safe formula evaluation.
- Keep plugin schemas independent of egui, geometry-kernel structs, and database
  layout. Use the same service contract for bundled and external extensions.

### Loading, permissions and lifecycle

- Load manifests and artifacts from a documented application plugin directory.
  Validate IDs, versions, API compatibility, dependency cycles, registration
  collisions, bounded metadata, and artifact paths before activation.
- Keep requested permissions separate from granted permissions. Denied operations
  must fail before invocation or document mutation; enforce grants per request.
- Default to no network, filesystem, process-launch, or secret access. Expose
  only narrow host services with explicit grants; do not inherit broad WASI access.
- Set executable limits for memory, instruction/fuel use or interruption, wall
  time, response size, and command count. Keep long-running invocation off the
  UI thread. A trap, infinite loop, or allocation failure must leave the host and
  document usable, with an actionable diagnostic.
- Add a minimal plugin manager: list, load/enable, disable, version, capabilities,
  grants, missing dependencies, and load/error status. No marketplace is required.
- Drain or cancel work before disabling/unloading; reject late callbacks. Preserve
  document data and do not unload code while an invocation is active.
- State accurately what the runtime isolates. A child process alone is crash
  isolation, not a filesystem/network permission sandbox. Signing is provenance,
  not a substitute for runtime enforcement.

### Plugin acceptance test

A contributor must be able to build an example Rust plugin against the documented
SDK, copy its artifact and manifest into the plugin directory, and use its tool
without changing or recompiling the host. First prove this with the Wall workflow;
then prove extensibility with one small new element/tool, such as a rectangular
column. Its parameter panel must come from the registered descriptor.

The example must create/edit an element, regenerate geometry, participate in
undo/redo, survive save/reopen, and retain data when the plugin is absent. Test a
denied write, incompatible API, oversized/malformed response, timeout, trap, and
stale reply after a document switch. All must preserve document consistency.

The floor-plan milestone must extend this same separately built example with a
2D drawing/editing workflow and a working plan-graphics or snapping provider.
Prove that these contributions work without host source changes or recompilation;
test cancellation, invalid provider output, and replies for an inactive view.

## 7. Geometry, linked 2D/3D authoring and documentation

Keep `GeometryKernel` and serializable geometry recipes independent of native
kernel types. Validate topology, winding, indices, bounds, and nonfinite values
before geometry reaches rendering or export. Test translated and rotated models
and large-coordinate behavior using documented tolerances.

Preserve the current depth-buffered rendering and matching entity-ID picking.
Hardware acceleration is a separate adapter task justified by measured frame
time, memory, and regeneration results. Do not regress intersecting-face visibility
or selection while optimizing rendering.

### First-class floor-plan editor

The current `Plan` and `Section` view kinds are only a starting point. Inspect
their actual behavior; an enum variant or an orbiting camera is not a completed
2D workspace. Complete basic floor-plan authoring immediately after the plugin
and document-durability gate, before hosted openings or sheet production. Shape
the plugin contracts for this workflow from the start.

- Let users create/select named floor plans associated with a level, switch
  between plan and 3D, and display both in a split workspace. Provide 2D pan/zoom,
  fit, selection, and a visible active level, view name, units, and scale.
- Define a level-relative horizontal cut plane and top/bottom/depth visibility
  range, with documented category behavior. Distinguish cut geometry, projected
  geometry, and symbolic graphics. Use an orthographic view basis and correct
  screen-to-work-plane conversion; a top-down camera alone is insufficient.
- Begin with the supported straight-wall geometry. Implement and test the slice
  and projection needed for that subset without waiting for a general-purpose
  section/Boolean kernel. Diagnose unsupported geometry explicitly.
- Author/edit levels and grids, then draw, select, move, and resize walls in plan
  using point/endpoint/grid/axis snapping and exact length, angle, and offset
  input. Show provisional geometry and measurements before commit. Escape must
  cancel cleanly; one completed drag or drawing action makes one undo step.
- Update every open model view from the same committed revision. Share entity
  selection and property edits; keep view-specific visibility and navigation.
  Tag derived work with view identity/settings revision as well as document
  identity/revision so changing a cut height cannot display a stale result.
- Keep line weights, cut fills/hatching, visibility rules, and draw order separate
  from model geometry. Use semantic references for picking and snapping, never
  transient mesh indices. Do not derive model dimensions from screen pixels.
- Expose the plugin input, preview, snapping, and plan-graphics contracts through
  this workspace. Apply the same permissions, limits, validation, and transaction
  rules used in 3D. Disable unavailable plugin tools with an explanation.
- Save/reopen named plan settings through the versioned storage path. Keep plan
  editing fully functional offline; do not introduce a separate 2D document file.

Acceptance: open a level plan, draw two walls with snapped endpoints and exact
dimensions, and see those same entities in a split 3D view. Select either view,
edit a wall, then undo/redo and verify matching model state in both. Change the
level elevation and plan cut range, verify the expected cut/projection and
picking, save/reopen, and confirm identities and view settings survive. Repeat
the supported workflow with the independently installed example plugin. Include
a cancelled edit, a failed edit, and a late result after switching views.

### Required production architectural 2D and BIM scope

Keep model-linked views distinct from independent 2D drafting. A drafting view
contains view-owned detail lines/arcs, filled regions/hatches, text, symbols, and
dimensions; these do not create walls or other 3D objects. Model-view annotations
can reference building elements, but must not become a second source of geometry.

After the floor-plan editor, implement E1-E4 from `docs/2d-production-spec.md`:

1. Architectural creation/editing workflows, a parametric 3D component editor,
   centralized parameter/material management, and reliable floor/ceiling/roof/site/
   area plans, sections, elevations and enlarged views; cut ranges, plan regions,
   underlays, dependent views, callouts, templates and graphics standards.
2. Complete accurate drawing/editing, associative dimensions and tags, text and
   keynotes, independent detailing, reusable parametric content, room/area and
   phase/option workflows, schedules and legends. Reuse E1's component and library
   foundations for consistent 3D geometry, 2D symbols, tags and quantities.
3. Full sheet sets, title blocks and automatic references, revision/issue control,
   preflight, dependable PDF/printing, declared DWG/DXF and linked-data workflows,
   office libraries/deployment, tested multi-user coordination, and model checks
   for clashes, duplicates, missing hosts, broken constraints and linked-datum changes.
4. End-to-end reference-project tests, numeric/visual/physical output checks,
   crash recovery, sustained performance, security, licensing and support gates.

Each group is a sequence of tested vertical slices, not a one-pass promise.
Architectural modeling dependencies must produce truthful drawing representations;
the rectangular-wall prototype cannot qualify plans of unimplemented element
classes. Required translators and multi-user services need separate ADRs and
verification; unresolved dependencies remain explicit production blockers.

Build a deterministic semantic 2D drawing representation shared by screen and
output adapters, with explicit model/view/paper coordinates and stable references.
Changing a model must update geometry, views, dimensions, quantities and drawing
references from the same committed revision, or visibly report a stale result.
Publishing requires a frozen, complete validated snapshot; an acknowledged stale
preview is not acceptable issued output. Do not mark features implemented simply
because an interface exists or silently reduce scope to a minimal sheet/PDF demo.

### Planned follow-on capabilities after E4

- L1, advanced site modeling: editable terrain, survey-point/contour import,
  contour generation, grading and cut/fill quantities with documented tolerances.
  This expands rather than defers the required site-plan/reference workflow.
- L2, 3D design and presentation: saved cameras, section boxes, walkthroughs,
  conceptual massing, material previews and sun/shadow studies. Advanced rendering
  and analysis integrations use plugin/adaptor boundaries; do not build them into
  the first-production release gate or require hosted services for local work.

Track L1/L2 separately in the coverage ledger. Their later delivery must not weaken
the existing view, model, geometry, material or consultant-exchange requirements.

## 8. IFC exchange

Preserve the implemented restricted **IFC4** rectangular-wall import/export and
its explicit loss reporting. IFC4.3 remains a separate roadmap target; do not
rename the schema header or describe the current exporter as IFC4.3-compatible.

For every added IFC entity/representation, document supported schema versions,
units, placements, storey containment, IDs, relationships, and export losses.
Maintain native-to-IFC-to-native semantic tests for the supported subset.
Use the existing independent validation tool and external viewer acceptance;
successful round-tripping through our own parser does not establish conformance.

Never silently omit unsupported elements or extension data. Provide a loss report
and require an explicit export choice where the existing workflow requires one.
An export must not alter the native document. Plugin elements need an explicit
export mapping or a reported unsupported status.

This restricted subset is the development baseline, not the production firm's
entire coordination solution. Expand mappings and link/reload behavior needed by
the declared architectural profile using the same independent validation rules.
PDF/CAD delivery and consultant-link requirements are specified separately in
`docs/2d-production-spec.md`; IFC alone does not replace architectural 2D exchange.

## 9. Milestones and completion gates

Implement in this order unless the user names a narrower task:

| Milestone | Required result |
| --- | --- |
| A. Establish current baseline | Reproducible build, relevant existing tests, source-backed status, and identified documentation drift |
| B. External plugin foundation | Versioned generic contract including 2D interaction/graphics design, bounded Wasm runtime, descriptor-driven tools/panels, plugin lifecycle, and a separately built example |
| C. Plugin document durability | Extension envelopes, dependency records, missing-plugin preservation, and transactional migration fixtures |
| D. Linked floor-plan authoring | Level/grid selection and editing, cut/view ranges, snapping and exact wall input, synchronized split 2D/3D, persisted views, and callable plugin plan tools/graphics |
| E1. Architectural model and views | Architectural creation/editing, parametric 3D components, parameter/material foundations, full linked views, graphics/templates, rooms/areas, phases and alternatives |
| E2. Detailing and information | Accurate drawing tools, dimensions/tags/text, shared 2D/3D content and library workflows, detailing, schedules and legends |
| E3. Firm drawing delivery | Complete sheets/revisions/issue packages, PDF/plot/CAD exchange, model checks and linked-datum monitoring, consultant links, and safe office/team workflows |
| E4. Production qualification | All required 2D and four companion BIM capabilities with G1-G6 gates, reference-project tests and measured reliability/performance; L1/L2 excluded |
| F. Contributor/GitHub handoff | Accurate setup/docs, buildable examples, repository/CI state verified, and publication/license decisions recorded |
| L1. Advanced site modeling (after E4) | Editable terrain, survey/contour import, grading and cut/fill; existing site-plan/reference support remains required in E1-E3 |
| L2. 3D design and presentation (after L1) | Cameras, section boxes, walkthroughs, conceptual massing, material previews and sun/shadow studies; advanced rendering/analysis via plugins |

Treat B and C as one release gate: do not advertise general third-party element
support while saving can lose their data. Complete the baseline plugin installation,
editing, durability, and failure-isolation acceptance checks before D; its added
2D plugin checks are part of D's completion gate. Implement concrete capabilities
instead of spreading placeholder interfaces across every planned feature. D is the first modeling
expansion: do not defer floor-plan editing behind openings, drafting, or sheets.
Complete its linked-view acceptance workflow before moving to E1-E4. None of
D/E1/E2/E3 alone establishes full production readiness. Keep F's local documentation
and CI work current throughout; remote publication still needs its own authority.
L1/L2 follow the first production qualification and are not E4 blockers. Qualify
each later capability before advertising it; do not claim full site/presentation
support from completion of the earlier architecture release.

Measure large synthetic models without committing generated bulk data. Record
hardware, element counts, open/save time, peak memory, regeneration time, and
viewport latency. Set numerical performance budgets from this evidence; do not
invent production-scale performance claims for the prototype.

## 10. Verification and reproducibility

Use the existing tests and add regression tests for observable behavior and data
integrity. In particular, cover atomic failures, undo/redo, dependency invalidation,
unknown plugin data, old file versions, import losses, and plugin failure isolation.
Add fuzzing for externally supplied containers/messages when those parsers change.
For 2D work, test screen/work-plane conversion, snapping tolerances, cut-plane
boundary cases, category visibility, plan picking, cross-view identity/selection,
single-action undo, cancellation, view-setting migrations, and stale view results.
Keep small deterministic plan-output fixtures for the supported geometry subset;
verify semantic/numeric expectations as well as images. Exercise the linked 2D/3D
acceptance workflow in the native UI, including the independent plugin path.

For E1-E4, use the production spec's G1-G6 gates and per-requirement evidence
ledger. Include geometry/annotation correctness, independently viewed and
physically plotted sheets, issue archive consistency, crash/disk/network failure
recovery, concurrent edits, permission/translator boundaries, and representative
long-session performance. Architect-reviewed pilots are not a release requirement.
Resolve required owner choices explicitly and never invent measured results to
close a gate.

Add companion-BIM acceptance scenarios: author and reuse a configurable 3D
component across projects without Rust; change a type/material and verify geometry,
2D symbols, tags and schedules; reject invalid formulas or conflicting constraints
without partial edits; detect clashes and linked-level/grid changes with navigable
diagnostics and no consultant-data mutation. Cover save/reopen, migrations,
undo/redo, library-version changes and stale diagnostic results across these flows.

Run the applicable workspace checks and report their actual output:

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p os-app --locked -- --smoke foundation-check.osb
```

The smoke output path must be new and its parent must exist. On the current
Windows machine, use `tools/cargo.ps1` if Cargo is not on PATH; preserve the quoted
`'--'` forwarding described in README. Use an isolated target directory if a
running Windows executable locks the normal build output.

Also run the existing dependency/license checks and independent IFC validation
when applicable. Do not suppress warnings merely to obtain a green report.
Preserve the current Windows/Linux CI checks; verify macOS before claiming it
is supported. Include native UI inspection for changed desktop interactions;
headless controller tests and screenshots are complementary evidence.

Record commands, platform, exact revision or working-tree state, results, and
remaining limits. Historical test counts and unexecuted CI configuration are
not evidence of a current successful build.

## 11. GitHub and contribution setup

When GitHub setup is included in the execution request, finish it against this
checkout. First inspect the actual remote, authenticated owner, visibility, and
Git identity. The earlier guide proposes `openstructure-bim`; use an existing
matching repository if one exists. Do not recreate it or overwrite its history.

- Prepare README, architecture/SDK guides, contribution instructions, issue/PR
  templates, security reporting, and the existing CI workflow for the actual state.
- Commit `Cargo.lock`, small owned fixtures, and source. Exclude `target/`, local
  toolchains, credentials, generated bulk outputs, and temporary work. Keep
  intentional `.osb`/`.ifc` fixtures trackable despite broad ignore patterns.
- Preserve the owner's Git identity; use repository-local configuration if a new
  identity is needed. Never invent an author email or expose credentials in logs.
- Use the authorized owner/visibility. Ask only for unresolved choices needed to
  create or publish the remote, and complete local preparation while blocked.
- Make focused commits, use `main` for a new repository, push without force, and
  verify the remote files and actual hosted CI result. Do not claim publication
  from a successful local commit alone.
- Configure requested repository protections only with real check names and
  available account features. Describe unavailable settings accurately.

Licensing is currently pending: `LICENSE.pending` and `publish = false` are
intentional. Do not silently replace them or call a pending license an open-source
grant. Present a dependency-aware license choice to the owner before applying it.
Keep SDK/application licensing, dependency obligations, and contributor terms
consistent with that choice; a public GitHub repository alone does not settle it.

If GitHub access or automatic approval is blocked, preserve completed local work
and give the precise blocker and next action. Do not bypass the restriction or
report that the remote has been created.

## 12. Required handoff

Update the relevant documentation and status as features land. Preserve historical
audits as historical, with links to new milestone evidence. Every final handoff
must contain:

1. What now works and a short reproducible user workflow.
2. What remains incomplete, including runtime trust and file-format limits.
3. Build/run/test commands with actual verification results.
4. Public API/file-format changes and compatibility implications.
5. Plugin SDK/example instructions and the independent installation result.
6. GitHub URL, commit and CI status when set up, or the exact remaining blocker.
7. The next three tasks, ordered by dependency and user value.
8. Architectural 2D and companion BIM coverage, supported deployment/project
   profile, open release blockers, reference-project test evidence and G1-G6
   results when applicable. Report L1/L2 follow-on status separately.

Start by inspecting the existing checkout and executing milestone A. Then complete
B and C, including the 2D contract design, followed by D's linked floor-plan editor
and E1-E4's architectural documentation, four required BIM foundations and
qualification scope. L1/L2 are later work. Deliver the requested milestone without
confusing intermediate progress with a production release. Keep progress updates
concise and support claims with observed behavior.

---
