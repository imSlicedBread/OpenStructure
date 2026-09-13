# Opt-in plugin manager

The normal application can now be built with `os-app/external-plugins`. Manage →
Plugin manager exposes explicit installation-directory input, grant checkboxes
(initially all denied), background Load/Cancel, load/error status, loaded versions,
capabilities, requested/effective grants, dependency requirements, Disable, and
Restore bundled Wall. Disabled IDs remain listed for the session; reload by choosing
the installation directory again. No automatic startup loading, persistent grant
store, marketplace, directory scanning or project-directed execution is implemented.

```powershell
.\tools\cargo.ps1 build -p os-app --features external-plugins --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe
```

Build/install the independent [Rust column](../examples/rust-column/README.md) or
[Rust Wall](../examples/rust-wall/README.md) first. In Manage → Plugin manager, enter
its installed directory, explicitly choose its required grants and click Load
installed plugin. The reference guests request ModelRead, ModelWrite and UiTool.
For external Wall, first disable the bundled Wall to release its registration IDs;
loading never replaces a provider implicitly. Use the resulting Plugin tools form
to create/edit, then native Save/Open. Cancel revokes load adoption; a draining
preparation retains its slot. Unsatisfied dependencies/collisions appear as load
errors. Effective grants exclude permissions not requested by the manifest.

Disable removes callable registrations without deleting document data, cancels
command/open tickets and invalidates managed meshes. Loaded dependencies can prevent
disable; the host reports why. Running Wasm invocations retain their runtime until
they drain, and their old results cannot commit. Restore bundled Wall explicitly
loads the trusted built-in provider and queues native wall regeneration. New Project
now preserves application plugin installations while replacing document/history;
plugins are not stored inside a project. Application restart still starts from the
bundled provider and requires explicit external loading again.

## Evidence

147 workspace tests pass, up from 146. The new egui-input test opens Manage and the
manager, disables/restores the bundled provider, checks model/revision preservation
and mesh invalidation/regeneration, and verifies New retains its activation identity.
Both real installed Editor probes pass background load, commands, geometry, history
and staged persistence. They do not operate manager grants or perform full native
installed authoring acceptance.

The computer-use skill successfully launched the feature-enabled normal application,
clicked Manage and Plugin manager, and inspected the native window. The directory
input, four unchecked grant boxes, Load, version/capabilities, requested/effective
grants and Disable were visible at the observed 1280×800 client layout. No native
permission settings were changed by automation. The clean verification window was
closed and its absence confirmed. This resolves manager-window visual inspection,
not the earlier full Wall/column native workflow acceptance gap.

```powershell
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\work\completion-build\debug\os-app.exe --smoke outputs/plugin-manager-verified.osb
```

All above checks pass locally on Windows, as do the feature-enabled app build and
installed Editor probes. Choose an unused smoke path when rerunning. No hosted CI
result is claimed. The wire ABI and container/model/envelope versions are unchanged;
the host adds a read-only effective-grants accessor and the application forwards the
existing external-plugins feature. Default builds still omit external loading.

## Remaining work

The later [B/C audit](bc-baseline-audit.md) records completed bounded baseline
authoring/durability/failure-isolation evidence, including native Wall/column
workflows and callable single/mixed-type migrations. The 147-test count and native
inspection above remain the historical manager checkpoint.

Next is D linked plans, then E1–E4/G1–G6. Production manager UX, retained installation/
grant policy and deployment qualification remain open; L1/L2 are later work.

Wasm remains bounded in-process execution, not process isolation; compilation/file
IO deadlines revoke adoption but do not forcibly terminate preparation. The original
IFC restrictions, native durability and unknown-plugin preservation remain. See the
[coverage ledger](2d-coverage.md): architecture/BIM capabilities and production gates
remain incomplete; project/jurisdiction/concurrency/output/platform qualification
targets remain undecided. No issued reference project or plot qualification. Only
Windows observed; Linux CI configured, macOS unverified. All source remains an
untracked scaffold with no HEAD, remote/GitHub URL or hosted CI result. License and
publication approval remain pending; nothing was published.
