# OpenStructure UI design guide

Current D addition: [native plan workspace](native-plan-workspace.md) documents
the plan picker, create action, Browser entries, split panes and navigation above
the canvas. The initial-release descriptions below are historical where they say
floor plans or IFC controls are unavailable; use that report and the IFC guide for
current supported workflows. [Detailed plan settings](plan-settings-form.md) now
use a validated modal draft. [Bundled wall pointer authoring](native-pointer-walls.md)
now has endpoint snapping and native workflow evidence. [Persisted grids](grid-plan-integration.md)
now draw/select/snap in plans, with read-only Properties and headless input tests.
[Grid authoring/edit forms](grid-authoring-forms.md) now expose model XY coordinates,
direction/length preview and transactional Apply/Cancel. Native inspection and
independent plugin pointer authoring remain pending.

The desktop workspace is a compact, light interface for BIM authoring. Familiar
task grouping and selection-driven properties help users move between modeling,
inspection, and project management. OpenStructure uses its own name, original
line icons, and independently authored layouts and text.

This guide describes the implemented first release. Numeric wall creation,
wall editing, levels, and the existing 3D view are available. Direct wall drawing,
snapping, floor plans, sheets, floating docking, and IFC exchange are future work.

## Workspace anatomy

```text
+--------------------------------------------------------------------------+
| OS  Save  Undo  Redo              Project name  Unsaved  OpenStructure     |
| File | Architecture | View | Manage | Modify / Walls (when selected)      |
| Task commands, grouped with captions                                     |
+--------------------------+-----------------------------------------------+
| Properties               | 3D | Model view                               |
| Create wall / Instance    +-----------------------------------------------+
|                          |                                               |
| Identity                 |                                               |
| Constraints              |                 3D canvas                     |
| Dimensions       scroll  |                                               |
| Endpoints                |                                               |
| Volume                   |                                               |
| [Create / Apply changes] |                                               |
|========== drag ==========|                                               |
| Project Browser          |                                               |
| Project                  |                                               |
|   Views > 3D             |                                               |
|   Levels         scroll  |                                               |
|   Walls                  +-----------------------------------------------+
|                          | Fit model | Navigation hints                  |
+--------------------------+-----------------------------------------------+
| Ready / Error | Message                  Active level | m | Count | Details|
+--------------------------------------------------------------------------+
```

All measurements below are logical pixels; egui applies display scaling.

| Region | Size and behavior |
| --- | --- |
| Quick access | 30 px; Save, Undo, Redo, committed project name, unsaved state |
| Ribbon tabs | 28 px; File menu, Architecture, View, Manage, contextual Modify |
| Command area | 80 px; original 26 px line icons with text and group captions |
| Left column | Initially 320 px; drag the right edge between 280 and 420 px |
| Palette split | Initially 55% Properties / 45% Browser; drag the divider |
| Palette minimums | 160 px Properties and 120 px Browser at supported sizes |
| View tab / controls | 30 px each, above and below the canvas |
| Status bar | 28 px; truncate lengthy text and expose the complete message in Details |

The application starts at 1280 × 800 with a 1000 × 650 minimum. Palette sizes
remain local to the current session and do not affect the project file. Each
palette scrolls separately. Property actions remain outside the scrolling fields.
Command-area overflow scrolls horizontally. Names and status text truncate
instead of pushing the canvas or controls out of the window.

## Visual tokens

The source of truth is the `os-ui` theme module. Update this table when changing
its tokens; do not add isolated hard-coded UI colors in new components.

| Token | Value | Use |
| --- | --- | --- |
| Background | `#F3F4F6` | Application chrome, separators and tabs |
| Surface | `#FFFFFF` | Ribbon, palettes, menus and dialogs |
| Canvas | `#FAFBFC` | Modeling background |
| Border | `#D1D5DB` | 1 px separators and control outlines |
| Text | `#20252B` | Primary labels and values |
| Muted text | `#5B6573` | Captions, units and secondary instructions |
| Accent | `#2563EB` | Focus, active tools and selection |
| Selected surface | `#E2EDFF` | Selected rows and primary action background |
| Error | `#B42318` | Explicit Error label and message |
| Grid | `#DDE2E8` | Subdued canvas grid |

Use the existing bundled proportional font: body and button text 13 px,
captions and ribbon command text 12 px, palette titles 14 px. Stable IDs use
11 px secondary text. Use a 4 px spacing unit, 8 px palette padding, 24 px
standard controls, and 2 px corner radii. Keep fields rectangular and compact.

Icons are small line drawings authored in the theme module, not font glyphs
or vendor artwork. Always pair a command icon with a text label. Use restrained
blue-gray wall shading and blue shading for the selected wall. The empty canvas
has a bordered instruction card so the grid does not interfere with the text.

## Commands and editing rules

| Location | Command and effect |
| --- | --- |
| Quick access | Save, Undo and Redo; unavailable history actions are disabled |
| File | Editable `.osb` path, New, Open, Save, and disabled IFC exchange |
| Architecture / Build | Wall clears selection and shows the numeric creation form |
| Architecture / Datum | Add level creates a level 3 m above the highest level and makes it active |
| View / Navigate | Fit model frames all walls |
| Manage / Project | Edit the project-name draft, then Rename project to commit |
| Manage / Active level | Edit the active level elevation; the existing immediate transaction regenerates dependent walls |
| Manage / Plugins | Displays the loaded built-in Walls plugin |
| Modify / Edit selection | Apply changes or Delete wall for the selected wall |
| Properties footer | Create wall or Apply changes using the same command handler as the ribbon |
| View controls | Fit model and drag/scroll/click navigation instructions |

Properties contains Identity (name), Constraints (level and height), Dimensions
(length and thickness), and Endpoints (start/end X and Y). Length changes preserve
the start point and direction. Volume is calculated from the current draft.
The selected wall's stable ID appears below the editable fields.

Wall property changes remain drafts until Create wall or Apply changes. A
failed validation retains the draft and leaves the committed model intact.
Selecting another wall reloads its committed parameters; clearing selection
loads the default creation parameters on the active level. Unapplied wall drafts
are not saved and do not trigger the committed-document unsaved prompt.

The browser has one project root and Views, Levels, and Walls branches. Only
the existing 3D view is presented. Selecting a wall in the browser or canvas
loads its properties and activates Modify | Walls. Other ribbon tabs remain
available while the wall stays selected. Clicking an empty part of the canvas
clears the selection. Selecting a level changes the active level; it does not
silently reassign a selected wall. Reassignment uses the Properties level field
and Apply changes.

Retain drag-to-orbit, scroll-to-zoom, and click-to-select. Ctrl+Z/Ctrl+Y control
model history and Ctrl+S saves when no text field is focused. Modal confirmations
block global model shortcuts. Native file operations keep their current semantics:
New/Open/Close ask before discarding committed unsaved changes, saving to a
different existing path asks before replacement, and a failed open preserves
the working document. The File menu stays open while its path is edited.

## States and component rules

Documents containing extension envelopes show a persistent **Incomplete view**
warning. **Inspect preserved data** opens Details with a virtualized read-only
entity list, owner/version availability, stable IDs and a bounded JSON preview.
The preview is limited to 4,096 characters; all original payload data stays in the
native document. Generic providers are not callable yet, even for loaded owners.
An extension-only project explicitly says it is not empty, rather than showing
the create-your-first-wall message. These entities cannot be edited in this UI.
Native save remains available; unsupported extension geometry blocks IFC export.

| State | Presentation |
| --- | --- |
| Empty project | Architecture active, creation form visible, browser says No walls yet; canvas explains how to create a wall |
| Selected wall | Blue model shading and selected browser row; Modify tab and Wall · Instance properties |
| Draft edit | Fields and calculated volume change; model geometry changes only after Apply |
| Unsaved document | An explicit Unsaved label in quick access |
| Invalid input / failed operation | Red Error label plus meaningful text; Details contains the full message |
| Hover | Pale blue surface, blue border, descriptive tooltip |
| Keyboard focus | Visible blue outline; a focused command remains identifiable without hovering |
| Unavailable command | Disabled control; IFC supplies its unavailable explanation on hover |

Use concise tooltips to explain consequences and shortcuts. Do not communicate
error, selection, or availability only through color. Do not add enabled buttons
for unimplemented workflows. Keep technical diagnostics in Details and stable
identity below the main property fields.

## Implementation and acceptance

Presentation is split into theme, ribbon, palettes, and viewport modules inside
`os-ui`. The startup code applies the theme once. Commands use the existing
Editor and document transactions; the UI stores ribbon selection, panel split,
drafts, and operation feedback. This redesign does not change the model schema,
plugin protocol, or `.osb` format. `os_ui::theme::apply` is the additive theme
setup entry point; existing controller APIs remain compatible.

Run the workspace tests, desktop build, Clippy, and formatting check. Desktop
tests feed real egui pointer, scroll, and keyboard events through the same frame
function as the native application. Check creation, all wall fields, selection,
level creation/reassignment/elevation, renaming, deletion/undo, save/reopen,
invalid drafts, and file confirmations. Check independently scrolling palettes,
the divider and width drag handles, and fixed property actions at 1280 × 800 and
1000 × 650 with 1.0, 1.25, and 1.5 pixels per logical point. Headless layout tests
complement native visual inspection; they do not prove Windows display settings
or native compositor behavior.
