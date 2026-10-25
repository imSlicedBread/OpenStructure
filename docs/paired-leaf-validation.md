# Paired-leaf accessory validation — 2026-10-11

Scope: authored ActiveLeaf/InactiveLeaf accessories on straight-host rectangular
paired doors, using independent existing leaf poses. Model schema 73, native API
54, opening family 9 and `.osot` package 9. Container 2 and generic API 2 are unchanged.
Earlier drawing-issue and MovingSash changes are preserved; no reset, discard or
commit was performed. Full workspace gates belong to the coordinator.

## Evidence

The geometry test independently derives vertices over 864 paired configurations:
unequal 63/37 leaves, all independent 0/30/90-degree combinations, both hinges and
swings, four host directions, two widths and no/Start/End side lite. It checks
exact depth/offset transforms, signed volume, material/solid provenance, stationary
frame/lite/fixed solids and isolation of the assigned leaf's pose.

Model/document tests cover explicit Single/Paired reassignment, each leaf's actual
bounds, the 16-solid cap, hidden narrow width pins, missing materials, atomic
preflight, unchanged instances/hosts/clearance locks and one-step undo/redo.

Existing desktop acceptance is extended, rather than copied: real headless egui
input at 1280×800/100% and 1000×650/150% exercises both leaf choices, neutral
model/history/scene previews, Apply, undo/redo, invalid Apply, Escape, Cancel,
revision and same-revision/new-session staleness. Both leaf attachments retain
opening-owner picking and material/solid identity in plan/section/elevation/facade;
linked sheet marks and vector PDF streams contain the projected accessory edges.
Package import verifies fresh type/solid/material IDs, exact collision remapping
and history. Native save/reopen preserves the model, and IFC fails closed.
Casement and Sliding MovingSash cases remain in the same regression tests.

Frozen schema72/package8 fixtures prove exact migration preservation and atomic
rejection of ActiveLeaf/InactiveLeaf or paired accessory semantics in legacy wires.
Old PrimaryPanel, OpeningFixed and MovingSash meanings are preserved.

## Checks

Commands below ran from the repository root and exited 0. The recorded storage
and focused UI runs used `TEMP` and `TMP` set to
`C:\fKey Labs\OpenStructure\target\issue-bundle-temp`; the coordinator's broader
opening UI rerun used `C:\fKey Labs\OpenStructure\target\paired-leaf-test-temp`.

| Command | Result |
| --- | --- |
| `cargo test -p os-model --test moving_sash_accessories --locked --offline` | 2 passed |
| `cargo test -p os-geometry --test moving_sash_accessories --locked --offline` | 2 passed |
| `cargo test -p os-geometry --test paired_leaf_accessories -p os-model --test paired_leaf_accessories --test opening_extrusions --locked --offline` | 3 passed |
| `cargo test -p os-storage --test paired_leaf_accessories -p os-document --test paired_leaf_accessories --test moving_sash_accessories --test opening_extrusions --locked --offline` | 11 passed (document 5, storage 6) |
| `cargo test -p os-ui --lib --all-features moving_sash_accessory --locked --offline -- --test-threads=1` | 3 passed, 525 filtered; includes both leaves and both DPI settings |
| `cargo test -p os-ui --all-features opening --lib --locked --offline -- --test-threads=1` | 241 passed, 0 failed/ignored; 287 filtered; 199.22 s; includes paired-door assignment and existing door/window regressions |
| `cargo test -p os-plugin-api --all-features --locked --offline` | 6 passed; native API 53 rejected, generic API 2 retained |
| `cargo clippy -p os-model -p os-geometry -p os-document -p os-storage -p os-plugin-api -p os-ui --all-features --lib --tests --locked --offline -- -D warnings` | Passed |
| `rustfmt --edition 2024 --config skip_children=true --check <changed Rust files listed below>` | Passed |
| `git -c core.safecrlf=false diff --check` | Passed |

The focused storage package regression also passed 16 `opening_type_package`
tests and 2 `paired_doors` tests. Its initial combined run caught a new test fixture
using a window pane-position value for a door; the fixture was corrected and the
paired storage suite rerun successfully. Initial sandboxed Clippy/rustfmt attempts
hit Windows configuration-access errors; approved runs outside that sandbox passed.
A new geometry test's range-loop Clippy warning was corrected before the passing run.

## Changed files for this increment

Implementation/version gates:

- `crates/os-model/src/opening_family.rs`, `crates/os-model/src/lib.rs`
- `crates/os-geometry/src/openings.rs`
- `crates/os-storage/src/lib.rs`, `crates/os-storage/src/opening_type_package.rs`
- `crates/os-plugin-api/src/lib.rs`, `crates/os-plugin-host/tests/wasm_transport.rs`
- `crates/os-ui/src/opening_family_editor.rs`
- `fixtures/wasm-probe/plugin.toml`, `fixtures/wasm-probe/probe.wat`, `plugins/walls/plugin.toml`

New tests/fixtures:

- `crates/os-model/tests/paired_leaf_accessories.rs`
- `crates/os-geometry/tests/paired_leaf_accessories.rs`
- `crates/os-document/tests/paired_leaf_accessories.rs`
- `crates/os-storage/tests/paired_leaf_accessories.rs`
- `crates/os-storage/tests/fixtures/schema-72-leaf-accessories.json`
- `crates/os-storage/tests/fixtures/package-8-leaf-accessories.osot`

Extended acceptance/version expectations (included in changed-file formatting):

- `crates/os-ui/src/moving_sash_accessory_tests.rs`, `crates/os-ui/src/opening_profile_tests.rs`
- `crates/os-document/tests/moving_sash_accessories.rs`
- `crates/os-storage/src/tests/openings.rs`
- `crates/os-storage/tests/common/mod.rs`
- `crates/os-storage/tests/arc_dimensions.rs`, `crates/os-storage/tests/dimension_anchors.rs`
- `crates/os-storage/tests/elevations.rs`, `crates/os-storage/tests/facades.rs`
- `crates/os-storage/tests/moving_sash_accessories.rs`, `crates/os-storage/tests/opening_extrusions.rs`
- `crates/os-storage/tests/opening_materials.rs`, `crates/os-storage/tests/opening_type_package.rs`
- `crates/os-storage/tests/opening_lengths.rs`, `crates/os-storage/tests/two_bay_families.rs`

Documentation: `README.md`, `docs/file-format.md`, `docs/plugin-api.md`,
`docs/native-hosted-openings.md`, `docs/native-opening-type-packages.md`,
`docs/2d-coverage.md` (including M01.30/M02.01), and this file.
Current native compatibility references only were advanced in
`docs/native-curtain-systems.md`, `docs/native-dimensions.md`,
`docs/native-elevations.md`, `docs/native-material-colors.md`,
`docs/native-material-patterns.md`, `docs/native-opening-materials.md`,
`docs/native-opening-tags.md`, `docs/native-phases.md`, `docs/native-ramps.md`,
`docs/native-rooms.md`, `docs/native-sections.md`, `docs/native-stairs.md`, and
`docs/native-wall-types.md`; historical version notes are retained.

## Remaining limits

No curved hosts, nonrectangular parent components/cuts, nested families, voids,
formulas, hardware catalogs, clearance/clash solving or new IFC support. End-sash
attachments remain unsupported. Accessories do not change host apertures or nominal
schedule dimensions. This is automated geometry/egui/PDF evidence; native display,
physical plot and full workspace qualification remain outside this focused run.
