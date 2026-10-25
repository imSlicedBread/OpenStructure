# Typed opening parameters and bounded formulas

This M02/M03 increment adds project-owned Length and Scalar drivers for supported
typed door/window components. It advances the full production specification;
it does not redefine or complete the Revit-parity goal.

Model schema 77 / native Model API 58 / opening family 10 / `.osot` package 10.
Container 2 and generic API 2 are unchanged. The existing `length_parameters`
map and `core.length_parameter` identity are retained for migration continuity.
Its unit is `Metres` (Length) or `Scalar`; each definition has a required nullable
`formula` and a finite literal fallback. Names never identify references.

Formulas accept decimal literals (`0.02m` for Length, `2` for Scalar), UUID
references (`@{uuid}`), unary minus, parentheses, and `+ - * /`. Addition and
subtraction require equal dimensions. Multiplication requires at least one
Scalar. Division by Scalar preserves dimension; Length / Length gives Scalar.
Other dimensions, functions, strings, reporting values and arbitrary code are
unsupported. Zero division, missing references, cycles and unit mismatch fail
closed. Literal and evaluated values must be finite within ±1,000,000; binding
targets enforce their narrower existing geometric ranges.

Limits: 64 definitions, 2048 formula bytes, 128 AST nodes, 32 syntax/AST levels,
32 dependency levels and 4096 evaluation steps for the entire parameter graph.
Dependency depth is checked independently of evaluation caching and UUID order.
AST formatting preserves precedence without redundant AST nodes. Package import
preserves original expression tokens/whitespace and substitutes only compact
32-hex UUID references, so remapping cannot grow a valid source formula. A
48-reference package formula near the 2048-byte limit covers this boundary.
Formula strings are parsed into an explicit bounded AST;
they are never interpreted by a general-purpose language runtime.

Existing width/height/window-sill bindings require Length and keep instance
override → project driver → type literal precedence. Each authored extrusion
can bind frame X, frame Y (vertical), depth and normal offset independently.
Profile, attachment and material remain authored literals. Removing an existing
binding freezes its current effective value. Changing a shared definition can
drive multiple types; reverse dependencies propagate through formulas, types,
instances and the existing derived-view/sheet graph.

The Manage parameter dialog stages definitions, units and formulas. It lists
UUIDs, formula dependents, direct property bindings, evaluated values, and an
always-visible affected-type/placement preview. Scalar values are not labelled
as metres. The type editor offers Length selectors beside extrusion properties.
Candidate evaluation checks all instances and host/component meshes before
Apply. Cancel and invalid Apply leave model, scene and history intact. Apply is
one document transaction and preserves authored instance overrides and locks.

Packages include exactly the transitive driver dependencies plus existing exact
material dependencies. Import allocates new driver IDs before rewriting formula
references and every binding; matching names/source IDs do not overwrite project
drivers. Schema 76→77 and package/family 9→10 migrations add literal-only defaults,
preserve existing IDs/payloads and reject ambiguous new fields in legacy input.
Frozen fixtures: `schema-76-parameters.json` and `package-9-parameters.osot` under
`crates/os-storage/tests/fixtures`.

## Evidence

Focused tests live in model/storage `tests/parameter_formulas.rs`, document
`tests/opening_lengths.rs`, UI `desktop_tests/opening_length_tests.rs`,
`opening_type_package_tests.rs`, `opening_tag_tests.rs`, and
`opening_extrusion_tests.rs`. They cover analytic units/budgets/order, atomic
shared drivers and freeze-on-unbind, legacy/current persistence, remapping,
two-DPI editor history, geometry, tags/schedules and cross-view paper/PDF output.

All commands use `./tools/cargo.ps1`, `--locked --offline`; test commands also
use `-- --test-threads=1` via a PowerShell argument array. `TMP`/`TEMP` point to
workspace-local `work/issue-test-temp`. Final focused results:

| Arguments | Result |
| --- | --- |
| `test -p os-model --test parameter_formulas` | 3 passed: arithmetic/dimensions, cycles/missing references/budgets, UUID-order permutations and near-budget remapping |
| `test -p os-document --test opening_lengths` | 3 passed: shared two-type drivers, pins/locks, freeze-on-unbind, rejected depth/dimension/cycle edits, history |
| `test -p os-storage --test opening_type_package --test parameter_formulas --test opening_lengths` | 20 passed: strict current/legacy wire, exact dependency closure, package boundaries and native save/reopen |
| `test -p os-ui --all-features --lib shared_lengths` | 3 passed: parameter editor, formula tags/schedule/sheet/scene, near-budget formula package remap with colliding source IDs |
| `test -p os-ui --all-features --lib typed_formulas` | 1 passed: two types/instances, immutable preview/cancel, invalid Apply, geometry, preserved pin, one undo/redo at both DPIs |
| `test -p os-ui --all-features --lib authored_extrusions` | 3 passed: real-egui solid editing, formula-driven Plan/3D/Section/Elevation/Facade/sheet/vector-PDF, save/reopen, ownership/materials/import |
| `test -p os-ui --all-features --lib drafting` | 5 passed: Section drafting regression after correcting duplicate provider-line snap attachment |
| `test -p os-plugin-api --lib` | 6 passed: API 58 compatibility and bounded contracts |
| `check -p os-ui --all-features` | Passed |

44 focused tests passed. No full-workspace suite ran. A final single-test package
duplicate-key check also verifies its mutation uses the current version constant.
After the byte-budget remapping fix, model `parameter_formulas` was rerun (3
passed), as was UI `shared_lengths_package` (1 passed), including the 48-reference
near-limit formula and exact source/import evaluated-value equality at both DPIs.
Rustfmt on the new formula module returned `Access is denied` before formatting.
Scoped library Clippy failed configuration discovery with the same access error;
its reported assignment-spacing warning was fixed, but the retry still failed
on configuration access. Formatting and Clippy remain unverified. No sandbox
escalation, reset or commit was performed; unrelated dirty work was preserved.

## Remaining specification work

M02/M03 remain partial: general categories/applicability and instance formula
bindings, other units/dimensions, reporting parameters, constraint authoring,
constrained profiles, nested/void/revolve/sweep/loft families, parameter-specific
schedule fields, shared library version/update policy and full interoperability
are still open. The existing supported-host restrictions remain. No visual or
print qualification and no Revit-parity completion claim is made.
