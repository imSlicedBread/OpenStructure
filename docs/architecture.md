# OpenStructure architecture

## Mission

OpenStructure is an independent, Rust-first, plugin-friendly BIM authoring platform. The semantic model is the source of truth; geometry and drawings are derived views of that model.

## Layering

```text
Application shell
├── UI and commands
├── Plugin host
├── Renderer and viewport
└── Document service
    ├── BIM model graph
    ├── Constraint/dependency graph
    ├── Transaction history
    ├── Geometry abstraction
    └── Persistence
        ├── Native .osb format
        └── IFC adapter
```

## Core rules

1. Model identity is stable and UUID-based.
2. Model changes are transactional.
3. Plugins use public contracts and permissions.
4. Geometry-kernel types do not leak into the semantic model.
5. Native files are versioned and migratable.
6. IFC is an exchange format, not the complete native editing model.
7. Plans, sections, and annotations remain associated with model objects.

## Initial plugin boundaries

The first plugin contract should support element registration, modeling commands, property panels, view providers, and model transactions. Built-in plugins should exercise the same APIs available to future WebAssembly or external-process plugins.

## Initial non-goals

- Native `.rvt` support
- Revit UI or asset replication
- Cloud collaboration
- Full discipline coverage
- Complete family/template systems
- Certified code-compliance analysis

## Architecture decisions

Significant decisions belong in `docs/decisions/` using short records that state the context, decision, alternatives, and consequences.

## Implemented workspace

| Package | Responsibility |
| --- | --- |
| os-core | UUID identity, SI conversion, boundary values and errors |
| os-model | Versioned semantic entities, typed parameters and graph validation |
| os-constraints | Dependency invalidation; general solver is deferred |
| os-document | Atomic command batches, snapshot history and change events |
| os-storage | StorageBackend trait, bounded ZIP/JSON container, migrations |
| os-geometry | Serializable profiles/solids/meshes and replaceable prism kernel |
| os-plugin-api | Versioned JSON messages, manifests, registrations and permissions |
| os-plugin-host | Built-in and opt-in Wasm transports, manifest/dependency/grant/command enforcement |
| os-walls | Example wall modeling and solid-generation plugin |
| os-render | Camera projection, bounded depth/color/entity-ID buffers and visible-surface picking |
| os-ui | Editor controller, regeneration queue and original egui desktop shell |
| os-app | Desktop executable and complete headless smoke workflow |
| os-ifc | Restricted Rust IFC4 wall exchange; desktop availability separately gated |

Application changes use Document::execute or PluginHost::execute. A candidate
model is cloned, all commands are applied, and the complete candidate graph is
validated before commit. Failure leaves model, history and revision unchanged.
Undo and redo restore validated snapshots and produce fresh monotonic events.
Retained snapshots now share configurable entry/estimated-byte limits; whole
oldest transactions are released, and an oversized single record fails before
commit. See [history accounting and its limits](history-retention.md).

Events include changed entity IDs and invalidated walls from both sides of the
transaction. Editor coalesces these IDs, removes stale meshes and regenerates
through the plugin protocol and GeometryKernel. Deleted walls leave the scene;
failed regeneration remains queued and reports an error. Saved files contain
the semantic model and immutable opaque auxiliary files, and opening stages model validation and regeneration
before replacing the working document.

The UI owns selection, camera and uncommitted property drafts. These are not part
of undo history. The current viewport is a replaceable presentation of the model;
future plans and sections must refer to stable model IDs. Registered future
service categories have protocol metadata but no callable implementation yet.

See [ADR 0001](decisions/0001-foundation.md) for the concrete tradeoffs.

The original Wasm adapter is described in [ADR 0006](decisions/0006-wasm-runtime-spike.md).
It now serves optional descriptor-driven desktop commands and extension geometry
through workers. See the [current B/C gate audit](bc-baseline-audit.md); the older
spike report describes its original narrower state, not current integration.
