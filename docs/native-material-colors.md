# Shared native material colors

Schema 34 adds one required `MaterialParams.color: [u8; 3]`: opaque display RGB
channels from 0 through 255. Edit the color picker under **Project materials /
density / color** in the existing wall type and material layers dialog, then
**Save type / layers**. The draft remains local until Save; its material changes
commit in the existing transaction and support Undo/Redo and stale-draft rejection.

The 3D viewport resolves the current shared color for wall layers and door/window
panel and frame material IDs. Family preview lines and assignment swatches use
the same lookup. Document revision changes invalidate the viewport texture even
when all mesh vertices, triangles and surface IDs remain identical. Selection
tint takes priority; no-material and missing lookup fallbacks retain their prior
behavior. Plan material fills and sheet cut fills also resolve the shared RGB.
Existing 3D lighting still shades the chosen color.

The atomic 33→34 migration derives each material's color from its existing UUID:
`[140 + byte0 % 80, 140 + byte1 % 80, 140 + byte2 % 80]`, exactly the previous
`SurfaceIdentity.color()` swatch. It advances native headers and the model root,
leaves opaque extension data alone, and rejects any pre-existing `color` field
in schema 33, even a valid one. Missing or malformed current-schema RGB arrays
are rejected. Current native full-model plugins require API 32/schema 51; generic
API 2 and container 2 are unchanged. Older native guests must rebuild/reinstall.

## Automated coverage

- `os-model/tests/material_colors.rs`: exact three-byte shape, endpoint values,
  missing/null/invalid channels and duplicate fields when deserializing parameters.
- `os-storage/tests/material_colors.rs`: frozen schema-33 fixture with independently
  specified old swatches, exact preservation of earlier fields, save/reopen, and
  atomic rejection after an earlier material has already been processed.
- `os-ui/src/opening_profile_tests.rs`: shared layered wall and two instances each
  of doors/windows; panel/frame pixel colors and native family preview line colors;
  unchanged geometry, surface IDs, depth/picking, assignments, quantities, selection
  tint, one-step Undo/Redo and save/reopen.
- `os-ui/tests/wall_types.rs`: both assigned walls retain geometry and quantities;
  plan colors and sheet cut-fill marks resolve the edited material; PDF generation.
- `os-ui/src/desktop_tests/wall_type_tests.rs`: headless native viewport texture
  refresh for color-only edits and Undo/Redo at 1280×800/1.0 and 1000×650/1.5.
- `os-ui/src/wall_type_tools.rs`: staged material edit, atomic editor transaction,
  stale-draft rejection and Undo/Redo.
- `os-render/tests/depth_buffer.rs`: assigned RGB, unchanged unassigned/missing
  lookup fallback and selection priority.

This is bounded display appearance. It adds no textures, transparency, PBR,
hatch standards, appearance styles, finish material quantities, or
IFC associations. Schema 35 adds independent room finish material references and
subdued active-plan floor fills; see [native rooms](native-rooms.md).
Room finish strings and opening-family assignments keep their
existing meanings. Headless drawing and pixel tests do not qualify a native
window, production visual appearance, physical printing or color management.
