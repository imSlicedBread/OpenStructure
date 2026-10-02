# Native casework

OpenStructure's first casework slice adds reusable cabinet types and placed
instances to the native model. This is a bounded authoring feature, not a
general-purpose family editor.

## Model and authoring

A `CaseworkType` stores a name, width, depth, height, and optional project
material. A `Casework` instance stores its own name, type UUID, level, plan
center, yaw, and base offset. Geometry is currently a single rectangular solid
envelope. Instances refer to shared types, so editing a type regenerates every
instance while preserving instance UUIDs and placement data.

From a native floor plan, open **Place → Casework**, choose an existing type or
create a new one, then click a snapped center. The preview is transient and
does not alter the model or 3D scene. First placement of a new type and its
instance is one transaction. Selected casework exposes instance and shared-type
properties; a type edit affecting multiple instances is also one undoable
transaction. Escape, stale plan/document context, and active drag ownership
cancel placement without committing.

Placed casework participates in plan cropping/range, plan drawing and picking,
selection filters, 3D scene regeneration, undo/redo, and native save/reopen.
Schema 61→62 adds the `casework_types` and `casework` collections; native
full-model plugin compatibility is API 43/schema 62. Generic plugin API 2 and
container 2 are unchanged.

IFC export explicitly refuses projects containing casework or casework types;
it will not silently omit them.

## Current limits

- Only simple rectangular cabinet envelopes are modeled; there are no cabinet
  doors, drawers, interiors, countertop assemblies, or detailed components.
- General furniture and fixture catalogs, user-authored arbitrary families,
  direct grips, generic move/copy/mirror tools, and casework quantities are not
  implemented.
- Casework section graphics and IFC mapping are not implemented.
- This does not add dimensions, annotations, schedules, or a full Revit-like
  family/component system.

Focused evidence: `crates/os-model/src/casework.rs`,
`crates/os-geometry/src/casework.rs`,
`crates/os-document/src/tests/casework.rs`,
`crates/os-render/tests/casework.rs`,
`crates/os-storage/src/tests/casework.rs`,
`crates/os-ui/src/plan_workspace/casework_tests.rs`, and the fail-closed IFC
test in `crates/os-ifc/tests/exchange.rs`.
