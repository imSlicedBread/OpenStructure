# Plan snap controls

The plan toolbar's **Snaps** menu now exposes Enable snapping, Endpoints,
Intersections, Midpoints, and Nearest lines and grid axes. All default to enabled.
The [perpendicular follow-up](plan-perpendicular-snapping.md) also adds
Perpendicular from anchor, between intersection and midpoint priority.
An additional [Line axis extensions](plan-axis-extension-snapping.md) option
defaults off and has last priority, below finite-feature snapping.
The disabled master state is visibly labelled **Snaps off**. Options remain
accessible during a wall draft, including after a dense intersection query
reports its limit. Disabling intersections lets the existing bounded nearest/
endpoint query proceed without that pair search.

These are application-session preferences retained across plan/document changes,
not model settings, persisted office defaults or history entries. Each query
contains the actual enabled flags, so a result from another configuration is
stale. Exact numeric input stays authoritative. The menu states the fixed
priority order; it does not yet let users reorder priorities, cycle tied
candidates, configure acquisition radius, or separate grid nearest from line
nearest. No unimplemented snap kinds are offered.

## Verification (2026-09-12)

- Real egui input tests open the menu at 1280×800/100% and 1000×650/150%, verify
  the options are visible, switch all snapping off/on, and compare raw versus
  endpoint-acquired wall starts. Model and revision stay unchanged on cancel.
- The crossing-wall input test switches intersections off during a draft,
  observes Nearest, re-enables intersections and observes Intersection, then
  commits from that exact point and undoes once.
- Full offline all-feature workspace tests, strict Clippy, formatting and default
  workspace build are checked with this checkpoint. Native-window inspection of
  the menu remains open.

This advances E01.34's override subset, not the complete production snapping or
independent plugin plan contract.

Follow-up [native inspection](native-offset-save.md) verifies visible controls,
the master-off indicator and session-state retention through project reopen.
