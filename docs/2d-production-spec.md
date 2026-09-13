# OpenStructure: production architectural 2D and BIM foundations

Status: required product scope and acceptance criteria, not implemented capability.
Reference baseline: Autodesk Revit 2026 architectural documentation workflows;
public documentation reviewed 2026-09-11. This is an independently designed
OpenStructure specification, not an assertion of exhaustive Revit parity.

## 1. Outcome and meaning of production ready

An architecture team must be able to author, coordinate, review, issue, revise,
and archive a complete architectural drawing package in OpenStructure. A working
floor-plan demo, a list of tools, or one exported PDF does not satisfy this goal.
Keep one authoritative building model with linked editable views, alongside
explicitly separate view-owned drafting and annotation data.

All capability groups in section 2 are required for the first production
architectural 2D release, including four companion BIM foundations: parametric
3D component authoring, architectural modeling workflows, centralized parameter/
material management, and model checking/coordination. These do not replace or
reduce the existing 2D requirements. Advanced site modeling and 3D presentation
are separately planned L1/L2 follow-on milestones in section 4, not E4 blockers.

Deliver them incrementally, but do not turn unfinished requirements into optional
plugins or indefinite follow-on work to claim completion. A supported plugin can
implement a required feature only if its version, installation, offline use,
data durability, and complete workflow are part of release testing.

The initial floor-plan milestone remains an internal development milestone. A
limited preview can be released under its exact supported scope; it must not be
advertised as full 2D parity or general firm-wide production readiness.

Before a production commitment, confirm with the owner:

- Country/jurisdiction, office drawing conventions, languages, and unit systems.
- Building types, complexity, largest expected model/drawing set, and linked data.
- Number of concurrent editors, coordination practices, IT deployment constraints.
- Required CAD/PDF versions, consultant deliverables, fonts, and printer/plotter sizes.
- Reference hardware, measurable performance budgets, and release support policy.

Until confirmed, design for metric and imperial architecture, new construction
and renovation, local offline authoring, and an eventual multi-user office
workflow. These are planning assumptions, not verified deployment guarantees.
No automatic building-code compliance, professional approval, or engineering
certification is implied.

Native `.rvt`, `.rfa`, and `.rte` compatibility, proprietary Autodesk assets and
UI replication, cloud services, and specialist structural/MEP analysis are not
required by this scope. Equivalent user outcomes use original formats, content,
and interaction design. Record excluded Revit workflows explicitly and obtain
owner agreement before making any broad equivalence claim. Do not purchase SDKs,
choose unresolved licenses, or publish anything merely because this spec exists.

## 2. Capability requirements

Use the IDs below in implementation tasks, regression tests, and release evidence.
Split each group into individual coverage rows before implementation; one passing
example must not mark an entire group complete.

### V01. Architectural views and navigation

- Floor, roof, reflected ceiling, site, area, and architectural coordination plans;
  interior/exterior elevations, building/wall sections, enlarged plans, detail
  views, and independent drafting views. Reflected ceiling views require their
  own projection and visibility semantics, not an upside-down floor-plan image.
- Level-relative cut/top/bottom/depth ranges, local plan regions for split levels,
  upward/downward underlays, far clipping, crop and annotation-crop boundaries,
  rotated crops, scope boxes, project/true north, and explicit view orientation.
- Independent duplication, duplication with detailing, and dependent views with
  defined shared/overridden settings. Provide matchlines, callouts, reference-only
  callouts, section/elevation markers, and references to placed drawing details.
- Searchable/filterable project browser, view naming and organization, tabs,
  split 2D/3D, saved navigation, shared selection, and navigation from markers,
  annotations, schedule rows, and sheet viewports back to their source objects.

Verify a split-level, multi-storey fixture with ceiling elements and large plans
split across sheets. Moving or renumbering a referenced view must update every
associated drawing reference without manual text correction.

Reference: [dependent views](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-EF8CCDA0-9946-49FA-B7AB-2B94470525D2.htm),
[plan regions](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-231E2653-8369-4F02-A78E-8A66AF4E4CEE.htm),
[underlays](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-77184183-E245-4F3B-8486-617E9A9FB296.htm).

### V02. Graphics and office view standards

- Category/subcategory styles; per-element and rule-based overrides; material
  cut/surface patterns; scale-aware line weights, line styles and dash patterns;
  hidden/overhead lines, joins, halftone, transparency, color and monochrome.
- Coarse/medium/fine representations; model-scale and paper-scale hatch patterns
  with editable origin, rotation and spacing; compound-layer cut graphics; masking
  and draw order. Different detail levels must produce intentional representations.
- Linked view templates with controlled fields, apply-once templates, batch
  assignment, transfer of standards, and documented override precedence across
  templates, phases, options, links, categories, filters and individual elements.
- Temporary hide/isolate, persistent hide, reveal-hidden diagnostics, and a
  "why is this invisible?" explanation. Temporary display states must not silently
  change an issued drawing set.

Verify template changes across multiple views and scales using deterministic
output fixtures. Confirm matching graphics on screen, in PDF, and in supported
CAD output; record any intentional format-specific differences.

Reference: [view templates](https://help.autodesk.com/cloudhelp/2025/ENU/Revit-Customize/files/GUID-C3B5FB82-3247-48F6-82F0-73011A0F8027.htm).

### M01. Architectural modeling workflows and truthful drawings

- Support the architectural elements needed by the declared project profile:
  layered and curved walls, wall joins, curtain systems, floors/slabs, roofs,
  ceilings, openings, doors/windows, stairs, ramps, railings, architectural
  columns, and placed furniture, casework and fixtures. Site plans require
  coordinates and supported terrain/survey reference representations.
- Provide type/instance parameters, level and host relationships, placement,
  flip/mirror behavior, material layers, geometry and symbolic representations,
  visibility by view/detail level, and consistent quantities where supported.
- Model groups, reusable assemblies and architectural part/layer representations
  need explicit membership, identity and update behavior where used in drawings.
- Changes to hosts, levels, types, joins and openings must regenerate plans,
  ceilings, elevations, sections, dimensions, tags and schedules consistently.
- Provide direct creation and editing in the appropriate plan, elevation, section
  or 3D work plane, with exact inputs, snapping, previews, property editing and
  transactional undo. Each required element class needs a usable workflow, not
  just serialization and a drawing representation.
- Edit wall paths, layers, joins and end/opening wrapping; place and rehost doors/
  windows; sketch and edit floor, roof and ceiling boundaries, slopes and openings;
  edit curtain grids, panels and mullions; create and revise stair runs/landings,
  ramps and host-associated railings. Validate geometry and host relationships
  before applying changes and explain unsupported configurations.

These are real modeling dependencies of production 2D, not an invitation to fake
unsupported geometry with disconnected lines. Stage implementation by element
class; missing required classes keep the corresponding production profile blocked.

Verify a connected architectural fixture: move a joined layered wall containing
an opening, reshape a sloped roof/floor, change a curtain grid, and alter stair
height/landings and their railings. Check affected geometry, plan/section graphics,
host relationships and quantities; undo each edit and repeat after reopening.
Required site plans and survey/terrain references stay in M01/V01/X01; editing
terrain and calculating grading quantities belong to L1.

### M02. Parametric 3D component editor

- Provide an original visual editor for reusable doors, windows, furniture,
  casework and custom components. Users must create and modify content without
  writing Rust or adding executable code for each component.
- Create/edit solid and void forms using supported profile-based operations such
  as extrusion, revolve, sweep and blend/loft. Include dimensioned sketches,
  reference planes, constraints, material assignment and an explicit supported
  operation matrix. Invalid forms must fail without corrupting the definition.
- Define reusable types, instance parameters, safe formulas, nested components,
  host/work-plane relationships, placement and flip/mirror behavior. Track nested
  dependencies and reject recursive definitions or conflicting constraints.
- Author synchronized 2D symbols and detail-level visibility alongside 3D geometry;
  use the same definition and parameter values in plan, section, 3D, tags and
  schedules. Preview type changes before committing them to a project.
- Use versioned local/office content packages shared with A02. Support save/load,
  transfer between projects, type catalogs, collision resolution, instance-aware
  updates and rollback. Preserve stable identities and explicit reference remapping;
  do not require `.rfa` compatibility or bundled proprietary content.

Verify a configurable hosted door with a nested handle, material choices and a
plan swing symbol. Create two types in the editor, place instances, reuse the
package in another project and change its dimensions. Check host opening,
geometry and 2D-symbol agreement, per-instance overrides and E2 tags/schedules.
Reject an invalid void and recursive nesting; test library update, undo/redo,
save/reopen and old-package migration without losing the previous valid state.

Reference: [component families](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-Model/files/GUID-4EBB97AD-C7B6-4828-91EB-BC0E99B81E43.htm).

### M03. Central parameter and material management

- Manage reusable typed parameter definitions with stable IDs, names, units,
  applicability, type/instance bindings and defaults. Keep display names separate
  from identity so renaming does not break formulas, labels or schedule fields.
- Provide project-wide driving/reporting parameters and constraints, unit-aware
  formulas and dependency inspection. Reject dimensional mismatches, cycles,
  invalid/nonfinite values and conflicting constraints with actionable explanations.
  Evaluate bounded expressions, never arbitrary host code.
- Centralize material definitions and library versions, including identity,
  classification, physical data used by supported quantities, layer assignments,
  cut/surface patterns and basic display appearance. Geometry, drawings, tags
  and schedules must reference the same definitions. Advanced rendering is L2.
- Support searching, importing, duplicating, editing and transferring libraries
  between projects with visible conflict handling and an affected-use preview.
  Preserve intentional instance overrides. Library updates and formula-driven
  changes must commit or roll back as a validated document operation.

Verify a project parameter driving multiple component types, a definition rename,
and a material/layer update. Confirm consistent geometry, symbols, hatch patterns,
tags and quantities; compare driving versus reporting behavior. Invalid formulas
and incompatible units must leave document state and undo history unchanged.
Exercise save/reopen, library collisions, migration and undo/redo across projects.

Reference: [global parameters](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-Model/files/GUID-1AA9B2DC-C08B-458E-BA93-C72C109D61C8.htm).

### E01. Accurate drawing and editing

- Lines, polylines, arcs, circles, ellipses, splines, rectangles, polygons and
  reference lines/planes; valid closed-boundary editing for regions and sketches.
- Select/window/crossing selection, cycling overlapping objects, category filters,
  move/copy/rotate/mirror, align, offset, trim/extend, split, fillet, arrays,
  group/ungroup, pin/unpin, and copy/paste aligned across views and levels.
- Endpoint, midpoint, intersection, perpendicular, tangent, nearest, center,
  grid and axis snapping; explicit snap priorities/overrides and screen-space
  acquisition tolerances distinct from model-space geometric tolerances.
- Exact coordinate, length, angle and offset inputs; metric, decimal and
  fractional imperial input; temporary dimensions, live previews and repeat tools.
  Cancel without residue, preserve keyboard focus, and make one gesture one undo.
- Implement documented geometric/dimensional constraints, equality and locked
  alignment with conflict explanations. Failed solves must be transactional;
  never silently move unrelated elements to resolve a constraint.

Test rotated work planes, large coordinates, mixed units, long operation sequences,
copy/paste identity remapping, and constraint conflicts in both linked and draft views.

### A01. Dimensions, text, tags and keynotes

- Associative aligned/linear, chained/baseline, angular, radial, diameter and
  arc-length dimensions; spot elevation, coordinate and slope annotations;
  witness-line editing, prefixes/suffixes, tolerances, units and rounding styles.
- Stable references to semantic features, materials and supported linked objects;
  reference remapping after supported edits and reloads; visible orphan warnings
  and deliberate repair. Never reattach silently to an unrelated nearby edge.
- Separate reporting dimensions from editable/driving constraints. Any displayed
  value override must remain identifiable to reviewers and drawing preflight;
  an override must not falsify the stored measurement or silently drive geometry.
- Rich text, wrapping, lists, symbols, leaders, arrowheads, alignment, find/replace,
  Unicode shaping, and explicit font substitution. Define supported languages and
  test their shaping; do not claim all-language support from accepting Unicode.
- Configurable element/material/room/area tags, multi-element leaders, keynote
  catalogs and legends, general notes and annotation schedules. Labels read
  structured parameters; provide type/instance bindings and orphan handling.

Verify annotation layout at multiple sheet scales, after host deletion and linked
file reload, and on a machine without the original author's optional fonts.

Reference: [keynotes](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-A3161C4C-6171-4C8E-BC2A-CF65E9F716D1.htm).

### A02. Detailing, reusable content and authoring tools

- Independent drafting and model-linked detail views with detail lines, filled
  and masking regions, insulation, break lines, detail components, repeating
  details, detail groups and explicit foreground/background order.
- An original reusable-content editor for parametric detail components, annotation
  symbols, tags, title blocks and model-element 2D symbols. Include reference
  planes, constraints, type catalogs, safe expressions/formulas, nested components,
  visibility parameters, and preview at supported scales and detail levels.
- Extend M02's component/package system and M03's parameter/material definitions
  rather than inventing a disconnected 2D content library. E2 integrates their
  types, symbols and bindings with annotations, legends and schedules; M02/M03
  foundations are required in E1.
- Searchable local/office libraries, versioned packages, loading and updating,
  conflict handling, transfer between projects, and original licensed starter
  content sufficient for supported workflows. No dependency on Autodesk family files.
- Library updates must preview affected instances, preserve edited project data,
  support rollback, and never execute untrusted content expressions as host code.

Verify that an architect can author a door tag, title block and repeating detail,
reuse them in another project, update a type, and issue correct revised output.

Reference: [detailing](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-C7424E33-F884-4EDD-BF42-71585281007F.htm).

### P01. Rooms, areas, phases and design alternatives

- Room/area boundaries, separation lines, identifiers, placement/enclosure
  diagnostics, computation rules, finish parameters, tags, color fills and legends.
  Define treatment of openings, linked boundaries and phase-dependent enclosures.
- Existing/new/demolished/temporary states, ordered phases, view filters and
  graphic overrides; phase-consistent schedules, tags and linked-model mapping.
- Design-option sets with isolated alternatives, dedicated views and schedules,
  clear membership and acceptance/merge behavior. Unsupported cross-option
  references must fail clearly rather than mix alternatives in issued drawings.
- Site/area and room reporting conventions must be configurable and documented;
  computed areas are not a declaration of local regulatory compliance.

Verify a renovation with demolition and new-work sheets, two design alternatives,
and a moved partition that updates room boundaries, tags and finish schedules.

### S01. Schedules, quantities and legends

- Door, window, room, finish and material schedules; key schedules, annotation
  note blocks, sheet/view indexes, and revision schedules; reusable legends and
  supported model-component illustrations that can appear on multiple sheets.
- Typed fields and shared/project parameters, calculated values with unit-aware
  validation, filters, sort/group, totals, conditional formatting, key-driven
  defaults, linked-model inclusion, and phase/option/view/sheet filtering.
- Editable supported cells use the document transaction service; computed cells
  stay read-only. Select/highlight source elements and support undo and validation.
- Column widths, headers, borders, repeated headings, split tables over sheets,
  reliable pagination, and CSV exchange with encoding, units and loss reporting.

Verify model edits update quantities and tags, grouped totals reconcile to raw
entities, and a multi-page schedule remains correct after row counts change.

Reference: [schedule fields and presentation](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-6D4DBBDA-3611-40CD-9A45-BE40EB07188A.htm),
[key schedules](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-CD22DA24-4162-4AB2-B1D3-02CEBE918444.htm).

### S02. Sheets and drawing-set management

- Custom title blocks, sheet sizes and project/sheet metadata, drawing numbers,
  unique-number checks, placeholders, browser organization, indexes and batch edits.
- Place, align, rotate and crop scaled viewports; edit viewport titles; guide grids;
  mixed scales; schedules/legends; sheet duplication with explicit view-copy rules.
- Automatic drawing/sheet cross-references, placed/unplaced status, and navigation
  between sheet and source view. Renumbering must update references atomically.
- Named ordered sheet sets, discipline/package grouping, issue status and revision
  metadata, including superseded/withdrawn sheets without losing issue history.

Verify a mixed-size, mixed-scale package with dependent plans, reused legends,
multi-sheet schedules, and renumbered details against its sheet index.

Reference: [sheet properties](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-AAD676E4-B98A-414C-829E-59143BB958DC.htm).

### I01. Revision control and issue packages

- Revision sequences, project/sheet numbering policies, clouds and tags in views
  and sheets, descriptions/dates/issued-by/issued-to, revision schedules, and
  explicit inclusion/exclusion rules independent of cloud display where needed.
- Review/approve/issue states and recorded transitions; issued revisions cannot
  be silently rewritten. Corrections create an explicit superseding issue record.
- A publish job uses a frozen document/view/template/link/plugin version snapshot.
  Store an issue manifest with drawing list, revisions, file hashes, settings,
  warnings and acknowledgments. Preserve an immutable copy of issued artifacts;
  reopening an old issue must not regenerate it from today's model by accident.
- Preflight broken references, stale results, missing fonts/links/plugins, duplicate
  sheet numbers, unplaced required views, hidden temporary states, dimension
  overrides, and clipping/overflow. Missing geometry, wrong scale and invalid
  references block issue; lesser warnings require a recorded review decision.

Verify an original issue, two revisions, a renumbered sheet and a withdrawn sheet;
reproduce the archived packages independently of later model edits. Do not claim
automatic revision clouds or regulatory approval unless separately implemented.

Reference: [revision workflow](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-312CD63E-12FD-4CA7-A05B-CD7DADBAACA7.htm).

### X01. PDF, printing and consultant exchange

- Vector-first PDF for suitable 2D geometry, explicit raster fallback diagnostics,
  searchable text with lawful font embedding/substitution, line weights, fills,
  transparency, hyperlinks/bookmarks, page boxes, orientation and mixed sizes.
- Single/combined files, saved ordered export sets, parameter-based naming,
  background progress/cancellation, and atomic publish of completed packages.
  A partial or stale job cannot appear as a successfully issued complete set.
- Native print/plot workflow for supported platforms, paper size, margins,
  orientation, color/monochrome, scale and preview. Default issue printing must
  preserve specified scale; warn explicitly before fit-to-page or other scaling.
- PDF/image underlays with page selection, calibration, rotation, crop and reload;
  vector snapping where supported. Label scanned/raster limitations accurately.
- DWG/DXF import/link/export for declared versions and entities, with layers,
  colors, line types, hatches, blocks, text/font mapping, units, coordinates,
  paper/model space and external-reference packaging. Validate in an independent
  CAD application; do not silently substitute DXF when a DWG deliverable is required.
- Linked native/IFC/CAD references: relative paths, shared-coordinate transforms,
  pinning, reload/unload/replace, nested-link/cycle policy, visibility, missing-file
  diagnostics and portable packaging. Preserve references across supported reloads.
  Track imported snapshots separately from live links and identify source versions.
- Keep CAD translators behind adapters. Investigate licensing, redistribution,
  supported versions and dependency cost before selection; seek approval for
  licenses or purchases. An unresolved required translator is a release blocker.

Verify export in at least two independent PDF viewers and the target CAD workflow,
then physically plot representative sheets. Measure paper-scale test geometry;
compare fonts, line weights, masks, crops and hatch density. Round-trips must
include third-party-authored fixtures used with permission, not only our exporter.

Reference: [PDF export](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-E9058256-8A36-4FB8-9809-5AA896FC2237.htm),
[DWG/DXF export](https://help.autodesk.com/cloudhelp/2026/ENU/Revit-DocumentPresent/files/GUID-42C75024-4D71-4831-8910-2747168624A3.htm).

### T01. Safe team use and office deployment

- First make single-writer documents safe: lock ownership/expiry policy, read-only
  opening, external-change detection, deliberate handoff and recoverable backups.
  Two desktops must never silently overwrite each other's project changes.
- Full multi-user office readiness requires an explicit local/self-hosted
  worksharing design with identity, ownership/borrowing, atomic synchronization,
  conflict resolution, permission checks, audit history and recoverable local work.
  Record an ADR; do not assume a shared ZIP, network drive or cloud-sync folder
  provides these guarantees. No mandatory hosted account is needed for local work.
- Test concurrent edits to model, annotation, view, sheet and shared content;
  host/dependent conflicts; user undo after another user's sync; interrupted
  connections; stale ownership; crash/restart; central restore; and version mismatch.
- Model federation and ownership of linked consultant data remain distinct from
  same-model editing. Provide read-only review and a portable project/issue archive.
- Provide an offline installer, supported OS/hardware matrix, controlled upgrades,
  uninstall that preserves user projects, admin-managed content/plugin versions,
  keyboard shortcuts, high-DPI/multi-monitor support, and accessible focus/contrast.

A single-writer preview is acceptable only with that limitation stated and accepted.
It does not satisfy the full team-readiness gate. Define concurrent-user targets
with the firm before promising scale; managed cloud collaboration remains separate.

### Q01. Model checking and linked-model coordination

- Provide configurable checks for geometric clashes, duplicate elements, missing
  hosts and broken constraints. Define eligible categories, scope, geometric
  tolerances and intentional-intersection exclusions. Bounding-box candidate
  overlaps alone must not be reported as confirmed geometric clashes.
- Monitor selected linked levels and grids using stable source/link identities,
  coordinate transforms and source revisions. Detect movement, renaming, deletion
  and replacement on reload; never silently match a different element by name
  or write changes into consultant files.
- Present a searchable model-health/coordination panel with rule ID, severity,
  message, affected entity/link references and document/source revisions. Support
  selection, isolation and navigation to affected geometry, plus recorded issue
  disposition and rechecking after model or link changes.
- Run checks read-only against bounded, revision-tagged snapshots. Mark stale or
  incomplete checks explicitly; an unsupported check is not a passed check.
  Any offered fix is a separate previewed, authorized and undoable transaction
  on editable local data. Preserve T01 ownership and X01 link boundaries.
- Expose callable diagnostic providers through the plugin contract with the same
  scopes, cancellation, limits and validation as other providers. Feed blocking
  results into I01 preflight; issue acknowledgments must refer to the checked
  revision and cannot conceal data corruption or stale geometry.

Verify known clash/non-clash pairs and an intentional joined intersection, a
duplicate, missing-host and constraint diagnostics, and a consultant level/grid
move followed by deletion. Confirm correct navigation and issue lifecycle,
unchanged consultant file hashes, rejection of stale replies, and rechecking
after undo/redo. Preserve monitoring identities/dispositions through save/reopen
and migration, and reject malformed or out-of-scope provider results.

Reference: [coordination tools](https://help.autodesk.com/cloudhelp/2025/ENU/Revit-Collaborate/files/GUID-6324F0AF-48A7-4669-B1F7-D0B37440A29C.htm).

## 3. Architecture commitments to make before feature expansion

1. Distinguish model entities, named views, annotations, drafting entities,
   component definitions/types/instances, parameter/material libraries, monitored
   link relationships, diagnostic records, viewports, sheets, templates and issues.
   Give each persistent object a stable identity, schema and dependency rules.
2. Use explicit model, view-plane and paper coordinate spaces with tested
   transforms. Model geometry uses `f64` and documented tolerances; text size and
   plotted line weight use paper units. Keep camera-relative rendering separate.
3. Build a deterministic 2D drawing representation with paths, curves, text runs,
   patterns, clips, masks, draw order and semantic references. Share it across
   screen/picking and output adapters; do not require pixel-identical rasterizers.
4. Resolve cut/projection/symbolic graphics and template/phase/option/link rules
   before caching. Cache keys include document and view revisions, relevant style,
   font, content, plugin and link versions, and scale/detail settings.
5. Model dependencies drive targeted regeneration. Bound memory and background
   work; discard stale results. Editing may visibly show pending work, but issue
   generation must wait for a consistent validated snapshot or fail explicitly.
6. All model, annotation, sheet, component, parameter/material and schedule edits
   use validated transactions, with explicit history, safe formula dependencies
   and deletion/reference-repair semantics. Persist unknown plugin data and old
   schemas without destructive migrations, including library and monitored-link data.
7. Extend the public plugin contract to versioned component definitions, geometry/
   symbol providers, parameter/material libraries, annotations/tags, snap/graphics,
   schedule fields, diagnostic providers, export adapters and preflight checks.
   Required services need callable examples and failure tests, not registrations
   alone. Host control of permissions, data validation and publishing is mandatory.
   Diagnostic results carry stable affected-object references and checked revisions;
   repair proposals require separate host authorization and document validation.
   Retain the current runtime limits and unknown-data preservation guarantees.

## 4. Delivery sequence and evidence ledger

Keep milestones A-C in `BUILD_PROMPT.md`. Then deliver vertical slices in order:

| Gate | Outcome | Main coverage |
| --- | --- | --- |
| D | Working linked floor-plan editor and independently installed 2D plugin | Initial V01/E01 and shared model/view architecture |
| E1 | Architectural modeling, parametric 3D components, parameter/material foundations and linked views | M01, M02, M03, V01, V02, P01 |
| E2 | Accurate detailing, annotations, shared 2D/3D content and library workflows, and schedules | E01, A01, A02, S01 with M02/M03 integration |
| E3 | Coordinated drawing sets, revisions, exchange, model checks and safe team workflows | S02, I01, X01, T01, Q01 |
| E4 | Production qualification with representative reference projects | All section 2 capabilities, including M01/M02/M03/Q01, and G1-G6; L1/L2 excluded |

These gates specify order, not calendar estimates. Plan engineering effort only
after capability spikes and a source-backed gap audit. Foundational storage,
security and team-contract decisions may run earlier; do not postpone them until
the UI is complete. Keep contributor and CI documentation current throughout.

During implementation maintain `docs/2d-coverage.md`. Each individual requirement
needs an ID, Revit/reference workflow when applicable, OpenStructure behavior,
dependencies, supported limits, implementation status, test/fixture IDs and
review evidence. Allowed statuses: not started, partial, automated-tested,
workflow-validated, or owner-approved exclusion. Workflow validation requires the
end-to-end checks in G1, not external architect review. Seed status from source
and observed behavior, never this specification's wording. A new documentation item is not a
completed feature. Re-audit the relevant public workflow inventory before release
and add omissions; this document is not proof that every Revit command is covered.

### Follow-on milestones after the first production release

L1 and L2 are planned additions, not first-release requirements. Record them in
a separate follow-on portion of the coverage ledger so their unfinished status
does not mask a required E1-E4 gap or block E4. Keep existing site/reference,
basic 2D/3D navigation, material graphics and coordinate workflows required.

| Gate | Planned capability | Dependency |
| --- | --- | --- |
| L1 | Advanced site modeling | After E4; extends M01/V01/X01 |
| L2 | 3D design and presentation | After L1; extends existing model/view/material services |

- L1: editable terrain, survey-point and contour import, contour generation,
  grading, and cut/fill quantities. Define supported source formats, coordinates
  and tolerances; preserve original survey data. Validate against known terrain
  and earthwork fixtures, including large-coordinate and undo/save/reopen cases.
  This does not defer the site plans or survey/terrain reference support already
  required for E1-E3.
- L2: saved cameras, section boxes, walkthroughs, conceptual massing, material
  previews and sun/shadow studies. Keep presentation settings separate from
  authoritative geometry; a camera or clipping change must not mutate the model.
  Test view persistence, clipping/picking agreement, mass-to-model relationships
  where supported and reproducible sun settings. Advanced rendering and analysis
  integrations remain plugin-oriented, with existing runtime/security boundaries
  and no mandatory cloud service for local use.

Do not claim these capabilities at the first release unless independently
implemented and qualified. The deferred presentation work does not relax required
architectural geometry, cut-plane graphics or drawing correctness.

## 5. Production release gates

All gates are required. Record exact build, platform, fixtures, commands, output
and release decisions. Architecture-firm participation and architect-reviewed
project pilots are not release requirements. Do not invent approvals, test
results or measured performance.

### G1. Complete reference-project workflow

Run end-to-end tests on at least two representative reference-project fixtures:
one new multi-storey building and one renovation with existing/demolition/new work.
Use original or synthetic projects, or third-party fixtures used with permission;
no live firm project is required. Include ceiling/roof/site plans, elevations,
sections, enlarged details, schedules, consultant references, and mixed-scale
drawing sets appropriate to those projects.

Create, check, issue, revise and archive the sets in OpenStructure. Compare output
with documented expected geometry, dimensions, schedules, drawing references and
issue manifests; record reproducible results and defects. Combine automated
workflow assertions with the existing native UI and output checks. No external
architect review, firm participation or architect sign-off is required for this gate.

The same fixtures must exercise M01/M02/M03/Q01: create/reuse a configurable 3D
component across projects without Rust, perform architectural creation/editing,
propagate a type/material change to geometry and documentation, reject invalid
formulas/constraints atomically, and detect clashes and linked-datum changes.
Include supported library-version updates, undo/redo, save/reopen and migration.
Use the detailed acceptance scenarios in each group; one example does not prove
coverage of every required element or modeling operation. L1/L2 are not G1 prerequisites.

### G2. Geometric, annotation and output correctness

Use independently calculated reference geometry and quantities, semantic assertions,
numeric tolerances, golden drawing fixtures and physical print checks. Cover
rotated/curved/sloped elements, cut-boundary cases, large coordinates, unit
conversion, joins, rooms, phases, options, host changes and orphan repair.
Include solid/void component operations, nested/hosted instances, parameter units,
material layers, global constraints and agreement between 3D and 2D symbols.
Set explicit model, annotation-placement and paper-scale tolerances from the
documented project precision policy before qualification; visual similarity alone
is insufficient.

No open release-blocking defect may cause wrong dimensions/scale/quantities,
missing geometry, mixed revisions, incorrect cross-references or unreadable
required output. Record lesser defects, impact, workaround and owner acceptance;
do not lower severity to make a release gate pass.

### G3. Data durability and recovery

Provide configurable autosave/recovery checkpoints, versioned backups and restore
UI. Define the recovery-point objective and distinguish saved/confirmed durable
transactions from unsaved in-memory edits. Test forced termination at transaction,
save, migration, link reload, sync and publish boundaries; truncated/corrupt files,
disk full, denied access, external modifications and missing plugins.
Include component/parameter/material package upgrades, ID collision/remapping,
formula failure and monitored-link/diagnostic-state migration. Failed batches must
preserve the last valid document and must not create a partial undo-history entry.

Require zero silent data loss in the test suite and qualification runs. The last
confirmed durable state must survive supported failure scenarios, with a usable recovery
path and explicit warnings about unsaved work. Never repair by overwriting the
only original. Verify portable archives on a clean offline machine.

### G4. Performance and sustained use

Select small, typical and worst-supported projects plus synthetic stress fixtures.
Record hardware, entity/link/view/sheet/annotation counts, visible primitive counts,
file size, and cold versus warm runs. Set approved numerical budgets for p95
pointer feedback, selection, view changes, committed-edit regeneration, open/save,
peak memory, sync and batch publication. Publish exact supported bounds.

As an initial qualification protocol, run at least five eight-hour work sessions
per reference-project profile and repeated edit/undo/save/reopen/publish loops.
These sessions may use scripted workloads; external project pilots are not needed.
Measure memory growth, responsiveness, cancellation and recovery. These are proposed test
requirements, not evidence that the prototype has achieved them. Missing budgets
or representative measurements block a production performance claim.

### G5. Team and interoperability qualification

Run the agreed concurrent-user workload, conflict/restore tests and consultant
exchange workflow on the supported deployment topology. Verify clean offline
installation and reopening on another workstation with managed content/fonts.
Test the declared CAD/PDF versions, print devices and file naming conventions.
Verify Q01 checks and monitored link reloads against known expected findings;
confirm stale results are rejected and consultant source files remain unchanged.
Unresolved mandatory DWG translation or team editing is a blocker, not a hidden
"coming soon" item in a general architecture-firm release.

### G6. Security, licensing and support

Fuzz project, drawing, font, image, CAD and plugin inputs as their parsers are added;
enforce bounded decoding, path validation and runtime isolation. Audit dependencies,
content/font provenance, redistributability and installation/update integrity.
Resolve the application's pending open-source license with the owner before
claiming an open-source production release. Do not promise immunity from claims.

Ship versioned installation/admin/SDK/user guides, tutorials and original example
projects, migration/rollback instructions, known limitations, security reporting,
release notes, reproducible diagnostics with sensitive data excluded by default,
and a named maintenance owner with a stated support policy. No silent updates or
mandatory telemetry. Keep the independently buildable Rust SDK working.

## 6. Release decision

Production readiness is a recorded decision supported by G1-G6 and the complete
coverage ledger. Publish the exact supported profile, unresolved exclusions and
evidence. Do not claim "full Revit 2D" from tool count, screenshots, basic PDF
export or self-generated round-trips. The goal is dependable architectural work;
the claim must remain no broader than the workflows actually qualified.
