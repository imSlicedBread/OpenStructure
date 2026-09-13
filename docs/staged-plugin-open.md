# Staged plugin project opening

Verified locally on Windows x64, 2026-09-12. This is an opt-in Editor controller
slice, not completed desktop plugin acceptance or production qualification.

## Workflow and guarantees

With `os-ui/external-plugins`, obtain unsaved-change consent, then call
`Editor::start_plugin_open(path, timeout)` with a positive timeout at most 30 seconds.
Poll `poll_plugin_open` until it returns `Some(OpenedProject)` or an error; use
`cancel_plugin_open` to abandon it. The ordinary `poll_plugin_work` reports busy
while opening but does not advance the candidate: callers must poll the open API.
Only after success should a desktop caller adopt the returned path and reset its
selection/property drafts. `OpenedProject::unavailable` lists preserved extension
IDs without a loaded API-2 provider. No project installs or loads executable code.

The candidate document and scene are separate from current work. External v1 Wall
and API-2 Wall/extension geometry uses bounded workers, with one job per available
provider and the existing global four-slot limit. Each job has at most five seconds
within the total open deadline. Successful completion replaces document, scene and
saved state together, gives the document a fresh session, and starts empty history.
File reads/validation and trusted built-in geometry remain synchronous; this is not
a fully asynchronous file loader or a hard-real-time frame guarantee.

Cancellation, worker rejection, elapsed deadline, document session/revision changes,
and plugin activation changes discard the candidate. Worker replies lose authority;
running workers keep their permits until they drain under existing fuel limits.
The current model, saved state, history and scene are not replaced on failure.
New tool starts and saves are blocked during staging. Core edits remain possible
but invalidate the candidate. Missing optional extension data and auxiliary bytes
are preserved; a loaded provider that cannot produce required geometry fails open.
Native walls still require the explicit built-in or external Wall provider.

## Reproduction and observed results

Build/install the independent guests as documented in
[Rust Wall](../examples/rust-wall/README.md) and
[Rust column](../examples/rust-column/README.md). The existing copied installations
`outputs/rust-wall-install` and `outputs/rust-column-geometry-install` were used;
no guest or host source changes were needed to switch installed providers.

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_editor_probe --locked --offline
.\target\debug\examples\plugin_editor_probe.exe outputs/rust-wall-install
.\target\debug\examples\plugin_editor_probe.exe outputs/rust-column-geometry-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 test -p os-plugin-api --no-default-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/staged-plugin-open-verified.osb
```

Both installed probes pass create/edit/undo, generated mesh volume, pending-save
rejection, cancellation, saved native data, staged worker reopening, stable geometry
after adoption, and unload invalidation. Five new unit tests cover deferred adoption,
cancel/history, preparation failure, stale document/activation, deadline rejection,
unavailable data/auxiliary round trips, and refusal to invoke an unbounded provider.
The workspace passes 138 tests, including the document doctest (previous baseline
133); the contract-only build passes 5. Strict Clippy, formatting, default build and
the named smoke output pass. License metadata check passes for 347 packages;
publication stays disabled. Reuse a new smoke output name for subsequent runs.

One initial test incorrectly expected storage to omit its standard empty assets/
and previews/ directories; corrected the assertion to compare with the stored
candidate. Strict Clippy also required placing the new test module last. Both
were corrected and the full checks rerun successfully.

## Compatibility, release status and next tasks

Adds feature-gated Editor methods and `OpenedProject`; no wire ABI, dependency,
container-2/model-2/envelope-1 version or unsafe-policy change. Legacy `Editor::open`
and default visible desktop behavior are unchanged. Wasm is a bounded in-process
interpreter, not a process security boundary. Existing native IFC4 Wall restrictions
remain. No native visual acceptance is claimed for this controller-only change.

1. Connect descriptor forms, selection and pending/error states to desktop frames.
2. Connect explicit plugin management/grants and staged Open controls; implement
   plugin migrations and complete installed Wall/column desktop B/C acceptance.
3. Complete D's linked floor plans and independent 2D providers, then E1–E4 and
   G1–G6 qualification. L1/L2 remain later and unimplemented.

See the [coverage ledger](2d-coverage.md): architectural documentation, the four BIM
foundations and production gates remain incomplete, with no issued reference-project
or plotting qualification. Jurisdiction, project size/concurrency, supported output
versions/platforms and plot targets remain owner decisions; “not sure” is retained
as open, not converted to support promises. Only local Windows was observed.
The entire scaffold remains untracked with no HEAD or remote; no GitHub URL,
commit or hosted CI result exists. CI already runs the extended Editor probe for
both guests. Publication and project-license approval remain outstanding.

## Desktop Open follow-up

The opt-in DesktopApp now routes `open_path` and File/Open through staging when
an API-2 catalog or external worker provider is loaded. `open_path` returning Ok
then means **staging started**, not adoption completed; the desktop frame loop polls
it. Built-in-only opening remains synchronous. Caller-obtained unsaved-change
consent is still required; the File menu uses the existing confirmation workflow.

Pending work displays an Opening project modal with Cancel project open. Polling
pauses during another document/exchange confirmation. On completion the desktop
adopts the candidate path, clears selection/form drafts/old viewport cache, refreshes
document fields and requests camera fit. Failure preserves the prior opened path,
selection and current document. The File menu's path input may still show the
attempted path; it is distinct from the successfully opened path. Parsing and
trusted built-in tessellation remain synchronous and bounded, as described above.

Reproduce using the [plugin desktop harness](plugin-forms.md): explicitly load an
installed guest, create/save an element, then use File/Open on that saved `.osb`.
Review the unsaved-change prompt if applicable; progress/cancellation belongs to
the candidate, not the current document. Native end-to-end visual acceptance of
this workflow remains pending; the earlier computer-use launch approval timed out.

Current workspace suite passes **143 tests**, up from 141. Two new tests exercise
actual egui confirmation input and direct frame polling: cancelled consent preserves
the working session, success adopts only after confirmation/completion, and an
unbounded-provider failure preserves document identity/data, selected ID and opened
path. An initial test needed a second layout frame before the modal's Cancel button
was visible; this was corrected and tests rerun. They do not prove native input
acceptance of the progress Cancel button. Existing controller tests/probes cover
candidate cancellation and both real installed Wasm geometry paths separately.

Strict Clippy, formatting, default build, desktop harness build and both installed
Editor probes pass. Smoke passes at `outputs/desktop-staged-open-verified.osb` using
the commands above with that unused output name. No wire, file schema, dependency,
license or publishing change. The feature-gated asynchronous `open_path` behavior
is now documented in its Rust API comment. No remote, commit or hosted CI result.

Next: explicit plugin management/grants/loading; native full Wall/column acceptance;
plugin migrations followed by remaining B/C and D/E1–E4/G1–G6. L1/L2 remain later.
All production-profile, architectural/BIM coverage and qualification limits above
still apply. This is progress toward BUILD_PROMPT, not its completion.
