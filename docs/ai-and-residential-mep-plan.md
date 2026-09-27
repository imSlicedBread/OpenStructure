# AI features and residential plumbing, electrical, and HVAC generation

Status: proposed roadmap, 2026-09-27. No implementation is claimed.

## Product direction

Add an assistant that understands the current building model and helps users
produce editable design proposals. Add automatic plumbing, electrical, and HVAC
generation for small residential buildings, starting with one level.

Confirmed scope:

- Small residential buildings first.
- Electrical generation includes device placement, circuit assignments, and
  physical cable/conduit routes.
- Plumbing generation includes water supply followed by drainage and venting.
- HVAC generation includes residential equipment, air terminals, and supply,
  return, and exhaust duct networks, starting with one level.

The first release should let a user select rooms, choose a residential preset,
place utility connections and HVAC equipment/terminals, generate a proposal, adjust it, and
apply the result as one undoable operation. Every generated element remains
editable using ordinary modeling tools.

## Existing foundation and missing pieces

The current working tree has model schema 43, stable entity IDs, rooms, walls,
openings, floors, columns, roofs, stairs, ceilings, and reflected ceiling plans.
Document edits use validated atomic command batches with undo/redo. Background
worker and stale-result patterns already exist. Native storage supports schema
migrations; plan, section, 3D, and schedule paths provide integration points.

The model currently has no native MEP equipment, connection ports, pipes,
electrical circuits, physical cable routes, or HVAC duct networks. `os-constraints` handles dependency
invalidation; a routing solver must be added. Existing IFC support is a restricted
architectural subset and does not provide MEP exchange.

This roadmap extends the foundation scope in response to the user's request.
It does not change the architectural production acceptance requirements.

## AI feature priorities

| Priority | Feature | Example | Dependency |
| --- | --- | --- | --- |
| First | Model questions and selection | “Select rooms with unresolved boundaries.” | Typed, read-only model queries |
| First | Explain model issues | Explain an unconnected fixture and highlight its port | Structured diagnostics linked to entity IDs |
| Next | Natural-language edits | “Use this finish in all bedrooms.” | Allowlisted commands, proposal preview, Apply |
| Next | Plumbing/electrical/HVAC assistant | “Route supply air to the selected bedrooms.” | Working manual tools and deterministic generators |
| Later | Compare alternatives | Compare route length, bends, clashes, and unresolved connections | Multiple validated routing candidates |
| Later | Documentation assistance | Draft model-backed schedules and change summaries | MEP quantities, annotations, and provenance |
| Later | Layout suggestions | Suggest fixture/device arrangements for selected rooms | Approved content and placement rules |

Generated answers should cite model objects and state missing inputs. Quantities
come from model queries. AI translates user intent into typed requests and
explains results; routing and validation run through explicit Rust logic.

## User workflow

1. Open a **Systems** workspace with Plumbing, Electrical, and HVAC tools.
2. Select rooms or a level and choose the system to generate.
3. Confirm room uses, fixture/device/equipment types, and existing items to preserve.
4. Place connection points: water entry/heater, drain outlet/stack, panel, or HVAC
   equipment ports, air terminals, and outdoor intake/discharge points.
5. Set allowed routing zones, elevations, keep-out zones, and locked routes.
6. Generate a preview showing proposed objects, connections, quantities, and issues.
7. Move a device, pin a segment, or change preferences and regenerate the proposal.
8. Choose **Apply** to commit one validated batch, or **Cancel** to discard it.

The assistant is available beside these tools. The same generators work through
forms without an AI connection. Preview uses plan and 3D together; schematic
circuit lines and physical installation routes have separate visibility controls.

## Automatic plumbing

### First slice: cold-water supply

- Place a small fixture catalog: sink, basin, toilet, shower, and appliance inlet.
- Give each fixture explicit typed connection ports and mounting coordinates.
- Let users place the supply connection and choose available pipe sizes/materials.
- Generate a connected tree through user-approved service zones.
- Account for pipe envelope and fitting space when testing route clearance.
- Derive editable pipe segments, bends/tees, connection diagnostics, and a schedule.
- Use explicit user-selected sizes initially; label sizing as unevaluated.

### Second slice: hot water

- Add a water heater with distinct inlet/outlet ports.
- Connect only compatible fixture hot-water ports.
- Keep hot/cold network identity and quantities separate.
- Add configurable separation and routing preferences as versioned rules.

### Third slice: drainage and vents

- Require drain destination elevations, pipe sizes, slope inputs, and vent strategy.
- Route drainage with explicit flow direction and continuously checked fall.
- Include fitting geometry, traps, cleanout/access requirements, and vent
  connections as separate modeled components and validation work.
- Report impossible fall, missing vent connections, and blocked paths explicitly.
- Release this slice only with the selected jurisdiction's reviewed rule profile;
  supply-only generation must remain clearly labeled until then.

Hydraulic sizing and pressure analysis are later services. Gas, fire protection,
and site sewer design are outside the first release.

## Automatic electrical and wire generation

### Device placement

- Place outlets, switches, lights, and a small catalog of equipment connection points.
- Suggest positions from room use and explicit placement presets.
- Respect openings, mounting heights, host geometry, and reserved/access zones.
- Support both existing manually placed devices and generated suggestions.
- Associate switches with controlled lights independently of physical routing.

### Circuit proposals

- Require panel location, supply configuration, and known device load data.
- Assign devices to proposed circuits under explicit project rules.
- Represent dedicated equipment circuits and panel assignments.
- Show known connected loads and missing load data in the panel schedule.
- Add demand, protection, conductor sizing, and voltage-drop checks only when the
  necessary inputs and reviewed calculation rules are implemented.

### Physical cable/conduit routes

- Route circuits through permitted wall/ceiling/service zones to the panel.
- Model circuit connectivity separately from cable paths and conduit containment.
- Support junction/pull points and editable waypoints.
- Preserve locked paths and manually placed devices during regeneration.
- Calculate geometric route lengths; show installation allowances separately.
- Produce circuit/device schedules and cable/conduit quantity schedules.
- Report unroutable devices, clashes, and unevaluated checks directly in the preview.

The first electrical release is a coordinated design proposal. Construction-ready
sizing and compliance qualification require a chosen jurisdiction and rule edition.

## Automatic residential HVAC

### Equipment, rooms, and air terminals

- Begin with a user-selected ducted residential system: air handler with linked
  heating/cooling equipment, supply diffusers/registers, return grilles, and
  separate bathroom/general exhaust fans and terminals.
- Group served rooms into explicit HVAC zones without changing architectural room
  identities. Show unassigned rooms and unresolved room boundaries.
- Let users place equipment and terminals manually or preview preset-based
  suggestions in floor and reflected ceiling plans.
- Record equipment dimensions, typed ports, service-access space, electrical
  connection requirements, and condensate connection requirements where applicable.
- Accept user-supplied room airflow targets and duct sizes. Show missing targets
  and unevaluated sizing explicitly; room area alone must not imply a calculated load.

### Automatic duct routing

- Generate separate supply, return, and exhaust networks between compatible ports.
- Route through approved ceiling/service zones at explicit elevations, accounting
  for round or rectangular cross-sections, insulation, elbows, branches, and transitions.
- Connect exhaust networks to user-designated outdoor discharge points; preserve
  system separation and flag missing endpoints.
- Respect beams/columns, walls, floors, ceiling openings, lights, plumbing, and
  cable/conduit routes, plus equipment service-access zones.
- Preserve manual placements, pinned routes, and user-selected dimensions when
  regenerating. Flag blocked routes and required penetrations for review.
- Use the shared preview, Apply/Cancel, undo/redo, and save/reopen workflow.

### Coordination, diagnostics, and output

- Show connectivity and flow direction in plan, reflected ceiling plan, section,
  and 3D; select the same entity from any view or schedule.
- Report disconnected terminals, incompatible ports, unserved rooms, clashes,
  blocked service access, and incomplete airflow inputs.
- Sum known terminal airflow targets by branch/system and compare with an entered
  equipment airflow value when supplied. Label incomplete totals; distinguish
  these bookkeeping checks from verified pressure or capacity calculations.
- Produce equipment, terminal, duct/fitting, and insulation quantity schedules.
- Link equipment electrical and condensate requirements to the electrical and
  plumbing proposals. Display unresolved connections without silently creating
  cross-discipline edits outside the selected generation scope.

### Later HVAC engineering

Add heating/cooling load calculations, duct sizing, pressure-loss calculations,
balancing, ventilation checks, and equipment selection as reviewed services with
explicit inputs, calculation methods, and applicable rule versions. Whole-building
energy simulation is a separate later integration.

The first HVAC milestone delivers editable layout and coordination proposals using
entered sizes and airflow targets. Refrigerant piping, hydronic heating networks,
combustion/flue design, kitchen grease exhaust, smoke control, fabrication, and
multi-level systems require separate follow-on scope.

## Technical design

### Shared model

Introduce typed equipment/devices, connection ports, systems, routed segments,
fittings, circuits, containment, HVAC zones, air terminals, duct networks, routing
zones, and diagnostic references. Ducts need cross-section shape/dimensions,
insulation, and optional airflow targets with explicit units and provenance.
Ports carry system compatibility, flow direction, position/orientation, and dimensions. Networks
store explicit connectivity; nearby geometry alone does not imply a connection.

Persist stable IDs, level/host references, sizes, materials, chosen presets,
manual locks, generation provenance, and ruleset version. Separate validity of
stored model structure from unresolved design conditions so incomplete designs
can be saved and repaired.

Architectural changes invalidate dependent routes and mark them for review.
Regeneration proposes a diff and preserves accepted manual work. Removed or moved
hosts produce actionable diagnostics rather than silently relocated devices.

### Routing service

Add an `os-routing` crate for candidate graphs, obstacle queries, path search,
branching, and route simplification. Start with orthogonal routes in bounded
service zones; add system-specific constraints above this shared layer.

Hard constraints define feasibility: compatible ports, allowed zones, required
clearance, fitting/transition space, locked geometry, and drainage fall where supported. Ranking preferences
include length, bends, shared runs, and access. Clearance checks use the routed
envelope, not just its centerline. Wall/slab crossings require an approved opening
or an explicit penetration proposal; the generator does not assume empty space
inside structural elements.

Coordinate proposed systems against accepted routes from every discipline. For a
combined proposal, route in a visible user-selected priority order, treating
accepted/locked systems as fixed obstacles; report unresolved conflicts. Include
duct insulation and maintenance access in the relevant clearance envelopes.

Use immutable model snapshots, cancellation, operation budgets, and revision
checks. Distinguish “no feasible route found within the search budget” from a
proved constraint conflict. Candidate generation must leave the live model untouched.

### AI boundary

Add an `os-ai` adapter after the typed query/proposal interface exists. Keep model
providers replaceable. Local and hosted providers can implement the same interface;
hosted requests require the user to enable sharing the selected project context.
Send only task-relevant structured data and keep credentials out of `.osb` files.

Validate structured responses, entity references, units, allowed actions, and
scope. Reject arbitrary code and shell commands. Model text and imported notes
are project data, never permission to execute actions. AI failures leave manual
tools and deterministic generation available.

### Integration map

| Area | Planned work |
| --- | --- |
| `os-model` | Typed MEP entities, references, topology, validation |
| `os-document` | Create/edit/remove commands, atomic proposal Apply, history accounting |
| `os-storage` | Schema migration, round-trip persistence, rules/provenance |
| `os-constraints` | Host/system dependencies and route invalidation |
| `os-geometry` | Pipe/conduit/duct/fitting shapes, insulation, and route/access envelopes |
| New `os-routing` | Shared routing service and discipline constraints |
| New `os-ai` | Provider adapters and typed query/proposal requests |
| `os-ui` / `os-render` | Systems tools, previews, selection, plan/section/3D graphics |
| Schedules and sheets | Circuit, device, pipe, HVAC equipment/terminal, duct/fitting, and insulation quantities and presentation |
| Plugin API/host | Deliberate versioned support for catalogs and later discipline services |
| `os-ifc` | Separate later MEP mapping and interoperability acceptance |

Start with native typed MEP data for dependable editing and storage. Keep routing
and analysis behind service boundaries that can later support plugins. Allocate
schema/API versions against the current baseline when implementation begins.

## Delivery sequence and acceptance

| Milestone | Deliverable | Exit condition |
| --- | --- | --- |
| 1. Manual systems foundation | Ports, devices/fixtures, pipe/path segments, system identity, manual editing | A simple network works in plan/3D and survives undo/redo and save/reopen |
| 2. Shared routing preview | Service zones, obstacle/clearance checks, locked segments, candidate Apply | Connected endpoints route around a known obstacle; blocked and canceled cases leave no edits |
| 3. Water supply generator | Cold water, then hot water and heater | Small kitchen/bathroom example generates connected editable networks and matching quantities |
| 4. Electrical generator | Placement, circuit grouping, physical paths, schedules | One-level home example preserves existing devices and shows every unconnected/unroutable device |
| 5. HVAC generator | Equipment/terminal placement, zones, supply/return/exhaust routing, schedules | One-level house has editable connected networks, coordinated service space, and explicit unresolved routes/inputs; plan/RCP/3D, undo/redo, and save/reopen agree |
| 6. AI assistant | Model questions, issue explanation, and typed generation/edit requests across all three disciplines | Requests resolve to model-backed results or reviewable proposals; invalid/stale output is rejected |
| 7. Drainage and vents | Elevation-aware routing and reviewed discipline checks | Fixtures connect with checked fall and vent requirements under the selected ruleset |
| 8. Broader coordination | Multiple levels, risers, richer content, MEP IFC | Cross-level topology and independent exchange checks pass before claiming support |
| 9. HVAC engineering services | Load calculations, duct sizing, pressure loss, and equipment selection | Reviewed reference cases and incomplete-input cases pass for each declared calculation method; whole-building energy simulation remains a separate integration |

The read-only assistant can be developed independently once typed queries exist.
Natural-language generation should use the same proven form-driven workflow.

Each milestone needs positive and negative acceptance cases: invalid topology,
incompatible systems, impossible paths, moved hosts, deleted objects, stale
previews, canceled workers, bounded history, migrations, and schedule consistency.
Include manual visual inspection of plan/3D previews and a complete save/reopen
workflow. Set and measure a reference-model performance budget during milestone 1.

HVAC acceptance includes a house fixture with supply and return terminals plus a
separate exhaust path to outdoors. Check a route blocked by a beam, a ceiling
height change, insulation clearance, a supply/return mismatch, missing airflow
targets, equipment relocation, and preservation of locked plumbing/electrical
routes. A failed or canceled proposal must leave all disciplines unchanged.

## First implementation task

Build a manual cold-water vertical slice: place one supply point and two fixtures,
connect them with editable pipe segments and a tee, show the result in plan and
3D, and verify connectivity, undo/redo, and `.osb` save/reopen. This establishes the
shared entities, ports, and editing path needed by all three automatic generators.
Before automatic HVAC routing, extend that manual workflow with one equipment
unit, supply/return terminals, a separate exhaust fan/discharge pair, and editable
ducts/fittings. Verify typed connectivity and round/rectangular route envelopes.

## Decisions before implementation or qualification

- Choose the first jurisdiction and rule edition before adding code-based placement
  or sizing defaults; keep geometric prototypes explicitly unevaluated meanwhile.
- Confirm residential supply/panel configuration and the initial content catalog.
- Confirm the first HVAC equipment preset, duct construction/insulation choices,
  available ceiling/service zones, and sources for room airflow targets and
  equipment data. Use explicit user inputs until calculation services are qualified.
- Choose whether the first AI provider is local or hosted and define context sharing.
- Select the reference house and measurable interaction/routing performance budget.

These decisions can be made while refining the manual modeling foundation; the
plan does not assume regulatory numbers, provider pricing, or release dates.

## Repository references

- [Architecture](architecture.md)
- [Current coverage ledger](2d-coverage.md)
- [Native rooms](native-rooms.md)
- [Native ceilings](native-ceilings.md)
- [Document/history behavior](history-retention.md)
- [Worker validity patterns](plugin-workers.md)
- [Current IFC scope](ifc-roadmap.md)
