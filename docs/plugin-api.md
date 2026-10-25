# Plugin protocols: native Model API 63 and generic API 2

Native API 63 requires schema 83; schema 82→83 adds required per-FloorPlan
`plan_graphics_rules` to the full Model payload. API 62 and older
native guests must rebuild/reinstall. The Wall manifest and Wasm probe use API 63.
Family/package 10, generic API 2 and container 2 remain unchanged.
See [rule-based plan graphics](native-plan-graphics-rules.md).

Native API 60 requires schema 79 and adds the optional independent floor-plan
annotation crop. API 59 and older native guests must rebuild/reinstall.
Opening family/package 10, generic API 2 and container 2 are unchanged.
See [annotation crops](native-annotation-crops.md).

Native API 59 requires model schema 78 and adds the Drafting view kind and
annotation crop/scale settings. Older native guests must rebuild. Opening
family/package remain 10, generic API 2 and container 2 are unchanged.
See [standalone drafting views](native-drafting-views.md).

Native API 58 requires model schema 77 and adds typed Length/Scalar formulas and
extrusion property bindings. Opening family and package contracts advance to 10.
Older native guests must rebuild. See [typed parameter formulas](native-parameter-formulas.md).

Native API 57 required model schema 76: `text_notes` is a required map, and
view-owned detail lines may belong to configured Sections. Plan line coordinates
remain world XY; Section line/note coordinates are station/project elevation.
Older native guests must rebuild. Generic API 2, container 2, opening family 9
and package 9 are unchanged. See [detail drafting](native-detail-drafting.md).

Native API 56 required model schema 75 and adds the distinct `section_callouts`
map to the native Model ABI. API 55 and older native guests must rebuild.
Generic API 2, container 2 and opening-family/package 9 are unchanged. See
[native section callouts](native-section-callouts.md).

Native API 55 required model schema 74 and adds plan-callout records plus
sheet-local viewport detail numbers to the full-model payload. No generic API 2
or pointer-gesture protocol changes are introduced. API 54 remains the prior
paired-door accessory increment; older native guests must rebuild/reinstall.

Native API 54 requires schema 73 / opening family 9 and adds explicit paired-door
`ActiveLeaf`/`InactiveLeaf` accessory attachments. API 53 and older native guests
must rebuild/reinstall. The current bundled Wall manifest and probe use API 63. Generic
API 2 and pointer-gesture protocols are unchanged.

Native API 53 required schema 72 / opening family 8. Full Model transport added
explicit `MovingSash` authored accessory attachment semantics. Older native
guests required rebuilding/reinstalling for that increment.
Generic API 2 is unchanged. See [moving-sash accessories](native-hosted-openings.md#moving-sash-accessories-schema-72--family-8).

Native API 52 requires schema 71. Full Model transport adds ordered sheet sets
and frozen drawing-issue metadata; archive/PDF bytes remain in standalone
`.osissue` bundles and never cross the Model transport. API 51 and older native
guests must rebuild/reinstall. That increment used API 52; generic API 2
and pointer-gesture protocols are unchanged. Provider activation IDs and actual
software/API versions are captured in issue manifests. See
[native drawing issues](native-sheet-preview.md#frozen-ordered-drawing-issues-schema-71--native-api-52).

Native API 51 requires model schema 70. Full Model transport includes the
project material-pattern collection and optional material cut-pattern
references. API 50 and older native guests must rebuild/reinstall. No generic
API or pointer-gesture protocol changes are introduced. See
[native material patterns](native-material-patterns.md).

Native API 49 requires model schema 68 and section settings version 2. Full
Model transport now carries persisted section side, cut/projection mode, finite
depth, station crop and scale. Schema 67 sections migrate with deterministic
CutOnly defaults; generic API 2, container 2 and plugin pointer protocols are
unchanged. Native API 48 and older guests must rebuild/reinstall. See
[native sections](native-sections.md) for supported projection categories and
explicit limits.

Native API 48 requires model schema 67 / opening family 7, adding bounded authored
profile-extrusion data to full-model transport. API 47 and older native guests
must rebuild/reinstall. Generic API 2, container 2 and generic pointer protocols
are unchanged. Native geometry consumers derive the same closed solids and
preserve opening-owner/material attribution; IFC rejects unsupported additions.
See [native hosted openings](native-hosted-openings.md) for attachment semantics,
supported combinations and explicit limits.

Native API 47 introduced model schema 66, adding distinct world-oriented Facade
view settings to full-model transport. API 46 and older native guests must
rebuild/reinstall. Bounded facade snapshots use the existing native worker lane;
this adds no generic plugin pointer gestures or new worker protocol. Existing
wall-local Elevation semantics are unchanged. Generic API 2 and container 2
remain unchanged. See [native elevations](native-elevations.md) for supported
geometry, depth/occlusion and explicit failure limits.

Native API 46 introduced model schema 65, adding explicit horizontal/vertical
elevation dimension references. Existing Plan coordinate semantics are unchanged.
The previous API 45/schema 64 increment added linked straight-wall Elevation
view settings. Native API 45 and older guests must rebuild/reinstall. The generic
API 2 and container 2 versions remain unchanged; no plugin elevation gestures
are introduced. See [native elevations](native-elevations.md).

Native API 44 required model schema 63 and added the `ArcLength` dimension layout
to full-model transport. Schema 62→63 preserves legacy parameters and IDs while
advancing native headers. API 43 and older native guests must rebuild/reinstall;
generic API 2 is unaffected. Arc-length dimensions resolve native circular wall
axes only and add no generic plugin pointer-gesture protocol. See
[native dimensions](native-dimensions.md) for the semantic pair and orphan rules.

Schema 61→62 added native casework types
and instances. Schema 60→61 adds the required native ramp collection; rise and slope are derived from two same-building levels, with
one rectangular straight-run slab. The bounded IFC writer refuses ramps rather
than dropping them. Schema 59→60 added stair railings and shared types; those
members remain derived and IFC also refuses them. Container 2 and generic API 2
are unchanged. API 43 and older native guests must rebuild and reinstall.

Schema 58→59 added required curtain system, panel-type and mullion-type
collections and persistent grid/panel/member UUID topology. Curtain systems are
native full-model data; generic API 2 and container 2 are unchanged.

API 39 introduced schema 58 and added the persisted paired-door layout to native
opening-family Model transport. Existing opening families migrate to `Single`;
per-instance opening poses are preserved. Native API 38/schema 57 and older
guests must rebuild and reinstall. Generic API 2 remains unchanged.

API 38 requires schema 57 and adds persisted per-instance door/window opening
poses to native Model transport. API 37 required schema 56 and added
project-owned Length parameter entities and opening-type dimension bindings.
Native API 37 and older guests must rebuild and reinstall. Generic API 2 remains
unchanged and does not gain these native commands. IFC export explicitly rejects
project Length semantics rather than discarding drivers or bindings.

API 36 required model schema 55 and carried analytic `WallPath` data plus
persistent native opening host-end clearances in full-model requests. API 35
introduced analytic wall paths; circular paths are never approximated into
endpoint chords for storage or native geometry. Generic API 2 remains the independent straight-wall
DTO protocol; installed API-2 Wall commands are explicitly rejected for arc
authoring. API 35/schema 54 and older full-model guests must rebuild and reinstall.

API 34 required schema 53 and added outer-wall-face dimension anchors with wall
UUID, Left/Right side and metric station. The model resolver and plan snapshot
use current effective thickness; Angular references remain endpoints. Native
API 33 and older model guests must rebuild and reinstall. Generic API 2 and
the Wasm buffer ABI are unchanged. Version gates cover API 33 rejection and
current schema agreement in `os-plugin-api` and `os-plugin-host` tests.

API 35/schema 54 added required Fixed/Sliding/Casement window-symbol metadata;
older models migrate existing types to Fixed. At that increment native full-model
transport used API 50/schema 69; current transport uses API 63/schema 83. Native
API 50 and older guests must rebuild and reinstall. Generic API 2 and container 2
are unchanged.

API 32 requires model schema 51. It retains required `ScheduleParams.group_by`
(ordered, at most two distinct Level/Kind/Type/Width/Height/Sill keys),
`ScheduleParams.phase`, and version-4 plan settings with required independent
`visibility.doors` / `visibility.windows` booleans. The explicit 48→49 migration
defaults both category visibilities on and advances native headers. Older
schema-47 saved schedules still migrate as `LegacyUnphased` through 47→48.
Door/window schedules own their target/filter; category visibility is per-plan
and does not remove host cutouts. See [native phases](native-phases.md) and
[opening schedules](native-hosted-openings.md). API 32 and older native guests
must rebuild; generic API 2 and container 2 are unchanged.

The schema also retains the explicit nullable opening
`pane_position_override` and `lite_side_override` fields. Only typed windows may
store Center/LeftFace/RightFace pane positions; only typed two-bay doors/windows
may pin a Start/End lite side. Null inherits from the assigned type. API 32 and
older native guests must rebuild.
Generic API 2 and container 2 are unchanged. Native mirrors use atomic companion
opening edits; installed Wall providers that cannot express them reject safely.

The host, plugin API, and bundled examples are licensed under Apache 2.0; see
[LICENSE](../LICENSE). Plugin authors retain ownership of their independently
authored code and choose its license. Contributions submitted to this repository
follow [CONTRIBUTING.md](../CONTRIBUTING.md).

Current compatibility boundary: native Wall guests that receive the full model
must use API 63 with model schema 83. API-62 and older native guests are rejected and must be
rebuilt/reinstalled; API 2 remains the independent generic DTO protocol. The
[B/C baseline audit](bc-baseline-audit.md) verifies independently
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
host/model/document/kernel dependencies. The Cargo feature named `legacy-v1` is
historical: it enables the native message structs now transported as API 38. See the separately resolved
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
numeric `api_version = 38` for native Model guests or `api_version = 2` for
generic DTO guests, dependencies with exact versions, entrypoint,
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
SDK expansion. Native headers in the retired API-1 wall replies used model schema 8;
old guests that construct schema-1 or schema-2 headers failed validation without mutation and
had to be rebuilt for the then-current development model. The API-1 full-model
wire is now retired; API 9 used schemas 27 and 28. API 10 used schema 29
with required serialized `OpeningParams.sill_override`. API 11 used schema 30,
adding required `width_override` and `height_override` fields. Each optional
number preserves inherit versus pin intent; geometry consumers resolve effective
values through `Model::resolve_opening`. API 12 required schema 31 and its
view-owned `opening_tags` collection. API 14 required schema 33,
including room finish codes and RoomFinish schedule definitions alongside
version-4 opening-family material assignments. API 18 required schema 37 with
the shared material RGB field and nullable room material references. API 21
required schema 40, including live room-bound ceiling sources. API 22 required
schema 41 for persisted opening-tag label presets. API 23 required schema 42
with typed `ScheduleParams.filters`. API 24 required schema 43 with tagged
wall-endpoint and opening-jamb dimension references; API 25 required schema 44
with project phases and element lifecycles. API 27 required schema 46 with nullable
window pane overrides and version-3 plan phase settings. API 32 required schema
51 with saved schedule phase, independent door/window plan visibility, version-5
opening families with required nullable `side_lite`, and required nullable
instance `lite_side_override`. API 33 required schema 52 and added
type-level window operation metadata that affects plan symbols only. Native API 33 and
older guests must rebuild and reinstall. Generic API 2, native container 2 and the Wasm
buffer ABI are unchanged. There is no stable independent model
DTO compatibility promise in this wall-specific v1 protocol. The Wasm buffer ABI
and geometry-only probe remain unchanged. Do not advertise arbitrary third-party
element authoring yet; the generic negotiated SDK must resolve model DTO independence.

Native opening type consumers use the schema-8 host model API:

```rust,ignore
// Persisted instance intent; all fields are public.
OpeningParams { name: String, host: Id, offset: f64, definition: OpeningDefinition, width_override: Option<f64>, height_override: Option<f64>, sill_override: Option<f64>, pane_position_override: Option<WindowPanePosition>, lite_side_override: Option<LiteSide>, hinge: DoorHinge, swing: DoorSwing }
OpeningDefinition::Legacy { kind: OpeningKind, width: f64, height: f64, sill: f64 }
OpeningDefinition::Typed { type_id: Id }
// Project-owned Entity<OpeningTypeParams>, with header type core.opening_type.
OpeningTypeParams { name: String, kind: OpeningKind, width: f64, height: f64, sill: f64 }

Model::resolve_opening(&self, opening: &OpeningParams) -> Result<ResolvedOpening>
ResolvedOpening {
    name: String, host: Id, offset: f64,
    kind: OpeningKind, width: f64, height: f64, sill: f64,
    type_id: Option<Id>, type_name: Option<String>,
}
```

`ResolvedOpening` is an owned transient value without serialization traits.
`name` is the instance name; `type_name` is the current type name, present only
for typed instances. Resolution validates dimensions and type existence but
does not require a placed host. `ResolvedOpening::validate_host(&WallParams)`
checks its offset and effective dimensions against a wall;
`OpeningParams::validate(&Model)` also checks the instance name and host reference.
Whole-model validation additionally checks separation between neighboring
openings. `OpeningParams::type_id()` returns the optional type reference.
Geometry consumers must resolve from the current model before generating host
apertures or opening symbols; they must not cache dimensions on a typed instance.
The new core commands do not add a plugin wire operation or change extension
payload schemas. Native migration preserves all plugin payloads verbatim.
