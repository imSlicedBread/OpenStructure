# Native ceilings and reflected ceiling plans

Model schema 39 added the first-class `Model.ceilings` collection. A ceiling has
a stable entity UUID, associated level, optional project material, straight
outer and opening loops, thickness, and underside offset relative to its level.
The checked geometry path emits a closed perforated shell. Scene regeneration,
building sections, native reflected-ceiling drawings, and sheet/vector-PDF
projection all derive from that same geometry and preserve the opening loops.

Reflected ceiling plans are saved `PlanViewType::ReflectedCeilingPlan` views.
Their range is explicitly ordered bottom → cut → top → depth while looking
upward; the host maps it to the geometry kernel's ordered vertical interval.
They are not floor plans with a display flip. Ceiling visibility is stored in
the version-2 plan settings. Create one from the saved-view picker’s
“New reflected ceiling plan” command. Schema 38→39 migration creates an empty
ceiling map,
advances native headers, and upgrades prior plan settings without changing the
saved floor-plan range.

In a plan, Draw ceiling starts a transient outer-loop sketch. Click to add
snap-aware vertices and click near the first point or use Finish ceiling loop.
Schema 40 adds an optional stable room UUID as a live boundary source; manual
ceilings remain independently sketched. The 39→40 migration initializes prior
ceilings as unassociated and advances native headers. In a reflected ceiling
plan, Ceiling from room creates a ready-to-review draft from the selected
resolved same-level room and carries over its ceiling material assignment. Once
committed, a reflected ceiling plan resolves room-backed outlines against that
view's target phase, independent of its graphic Phase filter. Plan authoring,
opening-fit validation and detachment use the same target-phase boundary. The
section and 3D pipelines remain unphased and use their existing full-model
topology. The ceiling remains an independent entity with its own material,
thickness, elevation and opening loops.

If the source room is deleted, moved to another level, loses its accepted
boundary at the consuming view's analysis phase, or becomes incompatible with a
ceiling opening, the linked ceiling is omitted from that derived view without
blocking wall or room edits. Its last saved boundary is retained for repair or
explicit detachment. Properties reports the unresolved source; detaching uses
the active plan's target-phase room boundary when resolvable, otherwise the
retained outline. Properties supports exact boundary/opening
coordinates, material, level, thickness and underside offset. Edited openings
must fit the current resolved room boundary before a linked edit can commit.
Detachment freezes the current outline even when an existing opening no longer
fits, allowing the opening to be repaired against that footprint. Apply commits one
undoable command. Escape, stale view/document/provider context, or leaving the
active plan cancels a draft. Preview does not edit model state or regenerate the
3D scene; committed edits regenerate through the existing scene path. Existing
ceilings can be selected in the reflected plan or Project Browser and edited.

This is an initial native horizontal-ceiling slice, not Revit parity. Slope/
compound assemblies, ceiling grids, fixtures, MEP coordination, ceiling
schedules/quantities, generic plugin
pointer gestures, and IFC covering exchange remain unsupported. IFC import/export
explicitly refuses coverings/native ceilings rather than silently dropping them.
Native-window and physical-print qualification remain open.

Focused implementation evidence lives in `crates/os-model/src/ceilings.rs`,
`crates/os-geometry/src/ceilings.rs`, `crates/os-document/src/tests/ceilings.rs`,
`crates/os-ui/src/plan_workspace/ceilings.rs`, and
`crates/os-ui/src/plan_workspace/ceiling_tests.rs`
(`ceiling_pointer_preview_and_apply_work_at_both_desktop_profiles` exercises
1280×800/100% and 1000×650/150%, including room-bound plan/section/scene
regeneration and unresolved-source detachment;
`linked_ceiling_openings_validate_live_boundary_and_detach_current_room_outline`
checks live opening validation and detachment after a source-boundary edit),
`crates/os-geometry/tests/room_phase_enclosures.rs` and
`crates/os-ui/src/plan/phase_tests.rs::reflected_ceiling_resolves_room_boundaries_at_the_view_phase`
cover phase-keyed cache isolation and target-phase plan behavior,
`crates/os-storage/src/tests/ceilings.rs`, and
`crates/os-ifc/tests/exchange.rs`.
