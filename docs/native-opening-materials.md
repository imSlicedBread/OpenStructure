# Native door and window material assignments

Reusable native opening types can assign separate project materials to the
door leaf or window pane and to generated frame rails. This advances the family
authoring workflow but does not yet provide a full physically based material
editor or Revit-like family system.

## Editing and behavior

- In the opening type editor, choose **Materials…** in the footer to open the
  material-assignment page. Choose **Panel material** and **Frame material**
  from the project material catalog, or choose **No material** for the existing
  default swatch. **Back to profile** returns to type/profile editing. The
  project material catalog can be managed in the wall-type and material-layers
  editor.
- Panel material applies to the door leaf/window pane extrusion. Frame material
  applies to the generated jamb, head, and (for windows) sill rails. A frame
  assignment may be set before a frame is enabled.
- Type changes are draft-only until Apply. Validation checks both referenced
  material IDs exist. A successful shared-type change preflights every instance
  and regenerates their component meshes in one undoable transaction; opening
  IDs, hosts, and per-instance dimension/sill overrides remain unchanged.
- Component and frame triangles carry distinct `SurfaceIdentity.material` IDs.
  The current 3D viewport and family preview resolve editable shared RGB colors
  from those IDs. The material picker is in the wall-type/material-layers editor.
  Color changes preserve geometry and assignments; see
  [shared material colors](native-material-colors.md) for migration and evidence.
  Texture, transparency, reflectance and physical-finish properties remain absent.
- Assignments affect the opening component mesh only. Host wall cuts and plan
  symbols remain unchanged. IFC export does not yet emit opening material
  associations, and material assignment does not add instance overrides.

## Compatibility

Opening-family payload version 4 adds required nullable `panel_material` and
`frame_material` fields. Model schema 32 performs an explicit atomic 31→32
migration: every version-3 family must not already contain either field; both
are inserted as null, family versions advance to 4, and native entity headers
advance to 32. Old families therefore retain their prior default appearance.
Malformed, ambiguous, or inconsistent headers are rejected without partial
migration. At introduction, native full-model plugin API 13 required schema 32.
Current native API 26 requires schema 45; generic API 2 and container version 2
remain unchanged.

## Evidence and remaining work

Acceptance coverage checks distinct panel/frame triangle identities for doors
and windows, unchanged component geometry, validation of missing IDs, atomic
document rejection, type preview without model/scene/history mutation, shared
type propagation across instances with pinned dimensions, one-step undo/redo,
save/reopen, and schema-31 family-v3 migration with malformed/ambiguous
rejection. `opening_family_panel_and_frame_material_selectors_work_at_both_dpis`
checks the material page and swatches at 1280×800/1.0 and 1000×650/1.5;
`opening_materials_shared_types_preview_failure_history_and_reopen` covers
shared type behavior and persistence. The full `cargo test --workspace
--all-features --locked --offline --quiet` suite and
`cargo clippy --workspace --all-targets --all-features --locked --offline --
-D warnings` both pass.

Appearance is bounded to shared display RGB: per-instance material overrides,
textures, transparency, PBR, IFC material export, and material schedules remain
unimplemented. Single-type versioned packages now support file-based transfer;
searchable catalogs, office libraries, constrained sketches and nested
components remain. See [native opening type packages](native-opening-type-packages.md)
and the [coverage ledger](2d-coverage.md) for scope and evidence.
