# Moving-sash accessory validation — 2026-10-11

Bounded scope: rectangular Casement and Sliding Start-sash accessory extrusions
on straight hosts, with opening-fixed accessories retained. Family/package 8,
native schema 72/API 53. See [the feature contract](native-hosted-openings.md#moving-sash-accessories-schema-72--family-8).
All pre-existing drawing-issue implementation changes were preserved. No reset,
discard, commit, or full-workspace validation was performed. The coordinator owns
the final workspace gate.

## Evidence

Twelve new test functions: model 2, document 3, geometry 2, storage 2, UI 3.
The geometry matrix independently checks 864 poses: Casement 0/45/90 degrees
and Sliding 0/.5/1 travel, four host directions, two widths, frame/no frame,
Start/End/no lite, and three pane positions. It checks exact vertices, volumes,
normal offsets, parent effective depth, surface material/solid identities and
stationary End sash/lite/frame geometry.

Real headless egui input at 1280×800/100% and 1000×650/150% covers attachment
choices, profile dragging, model/scene/history-neutral drafts, Apply, one-step
undo/redo, invalid Apply, Escape, Cancel, revision and same-revision/new-session
staleness. A crop-hidden narrow instance blocks the whole update. Actual package
import checks fresh type/solid/material UUIDs, collision-safe exact material
remapping and undo/redo. Production plan/section/elevation/facade drawings retain
opening-owner picking and solid/material provenance. Persisted linked sheets
and generated vector PDF streams contain the projected accessory edges in each
view; this is automated geometry/PDF evidence, not native display or print
qualification. Native save/reopen, strict legacy migrations, instance pins/poses,
unchanged host aperture geometry and fail-closed IFC export are covered.

## Exact successful validation commands

PowerShell, repository root. Every command below exited 0. Storage tests used:

```powershell
$env:TMP='C:\fKey Labs\OpenStructure\work\issue-test-temp'
$env:TEMP=$env:TMP
```

| Command | Result |
| --- | --- |
| `./tools/cargo.ps1 test -p os-model -p os-document -p os-geometry -p os-storage --test moving_sash_accessories --locked --offline --quiet` | 9 passed: document 3, geometry 2, model 2, storage 2; 0 failed/ignored |
| `./tools/cargo.ps1 test -p os-model -p os-document -p os-geometry --locked --offline --quiet --no-fail-fast` | All selected crate tests and doctests passed; 0 failed |
| `./tools/cargo.ps1 test -p os-storage --locked --offline --quiet --no-fail-fast` | All storage tests and doctests passed; 0 failed |
| `./tools/cargo.ps1 test -p os-ui --all-features opening --lib --locked --offline --quiet` | 241 passed, 0 failed/ignored, 287 filtered; 43.36 s |
| `./tools/cargo.ps1 test -p os-ui --all-features moving_sash_accessory --lib --locked --offline --quiet` | 3 passed, 0 failed/ignored, 525 filtered; 0.83 s |
| `./tools/cargo.ps1 test -p os-plugin-api --locked --offline --quiet` | 6 passed, 0 failed/ignored; includes rejection of API 52 |
| `./tools/cargo.ps1 test -p os-plugin-host --all-features --test wasm_transport legacy_model_guest_is_rejected_before_activation_or_model_dispatch --locked --offline --quiet` | 1 passed, 0 failed/ignored, 25 filtered |
| `./tools/cargo.ps1 test -p os-plugin-host --all-features --test wasm_transport loading_errors_leave_no_registrations_or_plugins --locked --offline --quiet` | 1 passed, 0 failed/ignored, 25 filtered |
| `./tools/cargo.ps1 clippy -p os-model -p os-document -p os-geometry -p os-storage -p os-ui -p os-plugin-api --all-features --all-targets --locked --offline '--' '-Dwarnings'` | Passed, warnings denied |
| `git -c core.safecrlf=false diff --check` | Passed |

Earlier regression runs found stale expected family/package/schema versions and
the old blanket rejection of Sliding accessories; those assertions were updated,
retaining rejection of incompatible PrimaryPanel attachments. The UI future-family
rejection test now uses 9 instead of the supported 8. Final reruns above passed.
The sandbox initially denied rustfmt/Clippy configuration access and the legacy
guest test's file access; scoped elevated retries passed. Test builds emitted
incremental-cache hard-link fallback warnings. There is no remaining known
bounded-slice blocker.

Formatting was checked successfully without traversing unrelated modules:

```powershell
$sashFiles = @(
    'crates/os-model/src/lib.rs',
    'crates/os-model/src/opening_family.rs',
    'crates/os-model/src/openings.rs',
    'crates/os-model/tests/moving_sash_accessories.rs',
    'crates/os-document/tests/moving_sash_accessories.rs',
    'crates/os-geometry/src/openings.rs',
    'crates/os-geometry/src/openings_section_tests.rs',
    'crates/os-geometry/tests/moving_sash_accessories.rs',
    'crates/os-plugin-api/src/lib.rs',
    'crates/os-plugin-host/tests/wasm_transport.rs',
    'crates/os-storage/src/lib.rs',
    'crates/os-storage/src/opening_type_package.rs',
    'crates/os-storage/src/tests/openings.rs',
    'crates/os-storage/tests/arc_dimensions.rs',
    'crates/os-storage/tests/common/mod.rs',
    'crates/os-storage/tests/dimension_anchors.rs',
    'crates/os-storage/tests/elevations.rs',
    'crates/os-storage/tests/facades.rs',
    'crates/os-storage/tests/opening_extrusions.rs',
    'crates/os-storage/tests/opening_lengths.rs',
    'crates/os-storage/tests/opening_materials.rs',
    'crates/os-storage/tests/opening_type_package.rs',
    'crates/os-storage/tests/room_materials.rs',
    'crates/os-storage/tests/two_bay_families.rs',
    'crates/os-storage/tests/moving_sash_accessories.rs',
    'crates/os-ui/src/opening_family_editor.rs',
    'crates/os-ui/src/opening_type_tools.rs',
    'crates/os-ui/src/opening_profile_tests.rs',
    'crates/os-ui/src/moving_sash_accessory_tests.rs'
)
rustfmt --check --edition 2024 --config skip_children=true @sashFiles
```

## Changed files in this slice

The Rust files are listed exactly in `$sashFiles` above. They include production
model/geometry/UI/migration/version changes, new tests, and current-version
expectations in existing regression tests. Other files changed or added here:

```text
README.md
fixtures/wasm-probe/plugin.toml
fixtures/wasm-probe/probe.wat
plugins/walls/plugin.toml
crates/os-storage/tests/fixtures/package-7-sash-accessories.osot
crates/os-storage/tests/fixtures/schema-71-sash-accessories.json
docs/2d-coverage.md
docs/file-format.md
docs/plugin-api.md
docs/native-hosted-openings.md
docs/native-opening-type-packages.md
docs/native-opening-materials.md
docs/native-sections.md
docs/native-material-patterns.md
docs/native-stairs.md
docs/native-phases.md
docs/native-dimensions.md
docs/native-wall-types.md
docs/native-material-colors.md
docs/native-curtain-systems.md
docs/native-rooms.md
docs/native-opening-tags.md
docs/native-ramps.md
docs/native-elevations.md
docs/ai-and-residential-mep-plan.md
docs/semantic-extension-durability.md
docs/decisions/0015-architectural-grid-datums.md
docs/moving-sash-validation.md
```

The older documentation edits update only current provider/API/schema claims
or clarify historical baselines; historical introduction versions remain intact.
Other dirty files, including drawing-issue model/document/storage/render/UI code,
belong to the pre-existing work and are not claimed as this slice's changes.

## Deliberate limits

No End-sash, paired-door, curved-host or nonrectangular accessory attachments;
no new constraints, sweeps/voids, clash solving or IFC family exchange. The
16-authored-solid budget and existing metric bounds remain. No native manual
display/print acceptance or full-workspace qualification is claimed.
