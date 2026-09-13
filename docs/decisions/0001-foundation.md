# ADR 0001: First executable vertical slice

Status: accepted for the foundation.

The semantic model is authoritative. A Rust Cargo workspace separates model,
constraints, document commands, storage, geometry, plugins, rendering and UI.
Snapshot transactions provide atomic validation and undo/redo; derived meshes are
invalidated by changed IDs and dependents. Snapshot memory use is a deliberate
first-slice tradeoff; deltas can replace history internals without changing commands.

The first native container is ZIP with `manifest.toml`, `model.json`, `assets/`
and `previews/`. JSON avoids a premature relational schema. A storage trait and
explicit version dispatch isolate a future SQLite backend. Files are validated
before opening, with bounded reads and atomic replacement on save. Schema 0 has
an explicit tested migration to schema 1. Unknown future versions are rejected.

Built-in plugins exchange versioned JSON request/response envelopes through a
host that validates manifests, grants and emitted commands. This is a protocol,
not a stable Rust ABI or a sandbox. Future Wasm/process transports must enforce
resource isolation separately. Registrations describe types, tools, panels,
views, exchange handlers, reports and analysis services; only wall tooling is
implemented now.

Geometry uses independent profiles, transforms, solids and indexed meshes. The
initial kernel extrudes rectangular wall profiles; booleans and sections return
explicit unsupported errors. An egui/eframe native shell uses a replaceable CPU
projection renderer for a basic orbitable 3D viewport. It is suitable for the
small initial model, not a production GPU BIM renderer.

IFC remains an explicit unavailable adapter. Export never writes placeholder IFC
files. IFC conformance and external-viewer round trips are deferred until an
adapter exists. No native proprietary format is supported.

The repository's pending license decision is preserved in LICENSE.pending and
package metadata rather than selecting a software license without owner review.
