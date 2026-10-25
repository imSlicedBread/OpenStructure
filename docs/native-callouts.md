# Native plan callouts (schema 74 / native API 55)

Rectangular callouts are native, model-owned links from one floor plan to
another same-level floor plan. Drawing a rectangle creates one stable marker
and one enlarged plan in a single undoable transaction. The new view copies
the parent's plan settings and graphics binding by value, then receives its
own crop and scale. Walls, openings, floors, and other model entities remain
shared project data; annotations remain owned by their original views.

The rectangle preview is transient. Escape, pointer loss, view/camera changes,
provider changes, and stale document revisions discard it without changing the
model or history. Releasing a valid rectangle commits once. Existing compatible
plans can instead be referenced without creating or owning a target view.
Markers can be moved or have their bounds/target repaired; opening parent,
target, or the referenced sheet navigates without changing model content.
Target crop editing is available when that view already has a crop.

Each sheet viewport has a unique detail number local to its sheet. References
resolve dynamically to `detail_number / sheet_number`, or report `Unplaced` or
`Choose destination` when there is no unique placement. A marker may store an
explicit viewport identity to disambiguate multiple placements. Moving a
viewport between sheets or renumbering it updates the resolved label; frozen
issued PDF/manifest bytes do not change. Plan canvas and sheet/PDF adapters use
the same resolved callout graphic.

Validation requires distinct native floor plans on the same level, valid
rectangular bounds, unique ownership for created enlarged plans, and an
acyclic ownership chain. Referenced views, selected destinations, and sheets
cannot be deleted or transferred in ways that leave dangling references until
the marker is repaired or removed. Drawing-issue preflight blocks unresolved
references and requires every destination sheet to be included in the issue.
Schema-73 migration assigns existing viewport detail numbers in their prior
order and rejects ambiguous new fields in older input. Container 2, generic
plugin API 2, opening families, and `.osot` versions are unchanged.

Focused evidence:

- `cargo test -p os-model -p os-document --test callouts --locked --offline -- --test-threads=1` — 5 passed.
- `cargo test -p os-storage --test callouts --locked --offline -- --test-threads=1` — 3 passed.
- `cargo test -p os-ui --all-features --lib callouts::tests --locked --offline -- --test-threads=1` — 3 passed; creation covers 1280×800/100% and 1000×650/150%.
- `cargo test --workspace --all-features --all-targets --locked --offline -- --test-threads=4` — full workspace passed; UI library 526 passed / 5 ignored.
- `cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.

This increment does not implement sections/elevations, dependent views,
nonrectangular or custom-family markers, linked/external model references,
automated placement layout, or full revision-aware cross-reference workflows.
