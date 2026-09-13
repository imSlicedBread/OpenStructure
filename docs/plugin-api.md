# Plugin protocols: legacy v1 and generic v2

Current status: the [B/C baseline audit](bc-baseline-audit.md) verifies independently
installed Wall/column commands, descriptor forms, checked geometry, workers,
durability and migrations. The original v1/spike descriptions below are historical
where they say desktop/generic integration is still pending. For current API-2
usage, follow [generic commands](generic-plugin-commands.md),
[geometry](generic-plugin-geometry.md), [forms](plugin-forms.md),
[installation](plugin-manager.md) and [migrations](plugin-migration-service.md).
An [optional plan-graphics service](callable-plan-graphics.md) now has a scoped
synchronous/worker host call with bounded line output. Native integration and
independent plan services acceptance remain required implementation work in D.

Contract metadata can now be consumed with `default-features = false`, without
host/model/document/kernel dependencies. Wall-specific messages and the built-in
Rust trait require the default `legacy-v1` feature. See the separately resolved
[contract consumer and compatibility notes](independent-contract.md).
The independent [API-2 command service](generic-plugin-commands.md) now supports
registered extension create/edit/delete with descriptors and scoped transactions.
It is developer-only; desktop rendering and the complete Rust
SDK installation workflow remain pending. The remainder of this page describes
the legacy v1 Wall protocol. See also the required [plan contract design](plugin-plan-contract-design.md).
The [independent Rust Wasm example](rust-guest-verification.md) now proves semantic
command installation against the API-2 contract, without desktop or geometry support.

`os-plugin-api` defines serializable manifests and JSON envelopes. The built-in
`Plugin` trait transports JSON strings; it is not a stable Rust dynamic ABI. No
private model pointers, document mutation handles or database connections cross
the boundary. `plugins/walls` is the working reference plugin.

A manifest declares its ID, display name, numeric major.minor.patch version,
numeric `api_version = 1`, dependencies with exact versions, entrypoint,
capabilities, requested permissions and registrations. The prototype master
prompt used a string API version; the implemented TOML protocol uses an integer
consistently in manifests and messages. `builtin:os-walls` is the current
desktop entrypoint. The opt-in `os-plugin-host/wasm` feature adds an explicit
developer loader for `wasm:<adjacent-file>.wasm`; normal `load` stays built-in-only.
See [buffer ABI and runtime limits](wasm-runtime-spike.md). JSON stays at v1.

Registration IDs must live beneath the plugin namespace. Registrations cover
element types, tools, property panels, view providers, import/export handlers,
schedules, reports and analysis services. Each requires its corresponding
capability. Tool and panel registrations also require `ui.tool` and `ui.panel`.
Only the wall type, tool, panel and geometry operations have implemented behavior.
The desktop shell currently implements their controls directly; descriptors do
not yet construct arbitrary third-party panels.

Host loading validates metadata, duplicate IDs, dependencies and grants. A plugin
receives only its requested grants even if the host offers a larger permission
set. Modeling requests require `model.read` and `model.write`; geometry requests
require `model.read` and the Geometry capability. A geometry response cannot
contain commands. Command responses may modify only registered element types,
and the document validates the complete resulting graph before commit.

Requests are envelopes containing `api_version`, a tagged `request`, and an
authorized model snapshot. Operations are CreateWall, EditWall and GenerateWall.
Responses contain `api_version` and a tagged Commands or Solid result. JSON
messages are capped at 64 MiB and command batches at 1,000 operations. Invalid
JSON, unexpected response kinds and mismatched versions fail without document
mutation. Geometry is validated by the kernel before entering the scene.

The host also maintains a global registration index. Nested plugin namespaces
cannot claim an ID already owned by another loaded plugin. Loading is atomic:
dependency/grant/collision checks finish before any registrations are published.
`PluginHost::registrations(kind)` discovers every declared category with its owner.

## Example integration

```rust,ignore
let mut host = os_plugin_host::PluginHost::default();
host.load(Box::new(os_walls::WallsPlugin), grants)?;
host.execute(os_walls::PLUGIN_ID, &mut document, "Create wall",
    os_plugin_api::Request::CreateWall(parameters))?;
```

See `crates/os-ui/tests/vertical_slice.rs` for an executable complete example.

## Runtime status

Built-in plugins are trusted code in the host process. Permission checks constrain
protocol operations; they are not an OS security sandbox and do not prevent a
malicious built-in implementation from making independent filesystem calls,
allocating memory or panicking. Only audited bundled code should be loaded.

The developer Wasm spike preserves the versioned request/response validation and
adds bounded execution without host imports. It is not a process sandbox or a
general extension SDK; the desktop does not load external artifacts yet.
Its [worker API](plugin-workers.md) adds host-owned tickets, cancellation/result
deadlines, stale-reply rejection and safe unload while keeping the v1 wire intact.
Model schema 2 now accepts bounded inert [extension envelopes](semantic-extension-durability.md),
with generic callable commands and payload validators on API 2. V1
replies cannot add/replace/remove these envelopes or change plugin requirements,
even for a registered owner. Those commands are core transaction APIs, not a v1
SDK expansion. Native headers in current v1 wall replies must use model schema 4;
old guests that construct schema-1 or schema-2 headers fail validation without mutation and
must be rebuilt for this development model. There is no stable independent model
DTO compatibility promise in this wall-specific v1 protocol. The Wasm buffer ABI
and geometry-only probe remain unchanged. Do not advertise arbitrary third-party
element authoring yet; the generic negotiated SDK must resolve model DTO independence.
