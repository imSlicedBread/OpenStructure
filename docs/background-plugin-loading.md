# Background plugin preparation

The opt-in Wasm host now offers `start_wasm_load(directory, grants, timeout)`,
`poll_wasm_load(&mut ticket)` and `PendingLoad::cancel`. This is the loading service
for the upcoming plugin manager, not the manager UI itself.

An explicitly chosen installation directory and grants are copied into a worker.
Bounded directory/manifest/artifact reads, Wasm compilation and API-2 Describe run
there, not on the caller's frame. Requested permissions must be granted before
artifact compilation/guest description. Current host dependency versions, duplicate
IDs and registration ownership are checked at adoption. Describe may therefore run
before a dependency/collision is rejected, but it receives no document or services
and creates no callable host registration. The immutable prepared catalog is not
requested again during adoption. No project file installs or loads code.

Only one load, including an unread prepared result, is admitted per host. Tickets
are host-bound and single-consumption. Cancellation/drop/expiry revokes adoption;
the worker retains capacity until preparation actually exits. Positive deadlines
are at most 30 seconds and gate delivery, **not** forced interruption of filesystem
IO or compiler execution. Runtime fuel applies to Describe. This is not a process
sandbox or a hard wall-time/memory guarantee for compilation. Installation folders
still must not be concurrently writable by an adversary. No unsafe exception or
new dependency was introduced.

## Verification and reproduction

Install guests using the existing [Wall](../examples/rust-wall/README.md) and
[column](../examples/rust-column/README.md) guides, then run:

```powershell
.\tools\cargo.ps1 build -p os-ui --features external-plugins --example plugin_editor_probe --locked --offline
.\target\debug\examples\plugin_editor_probe.exe outputs/rust-wall-install
.\target\debug\examples\plugin_editor_probe.exe outputs/rust-column-geometry-install
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
.\work\completion-build\debug\os-app.exe --smoke outputs/background-plugin-load-verified.osb
```

Both installed Editor probes now load through the asynchronous API and pass their
existing command/geometry/history/persistence/staged-open checks. Three new tests
cover bounded admission, host binding, single activation, concurrent duplicate
installation, cancellation/drain, expired delivery and denied grants. Full suite:
146 passing tests, up from 143. Strict Clippy (after correcting one needless borrow),
formatting, default build and the named smoke pass locally on Windows. Use an unused
smoke filename on subsequent runs. No visible UI changes or native acceptance were
performed in this service slice. CI's existing Editor probes use the updated path;
no hosted run is claimed.

The synchronous developer loader and built-in API remain supported. Wire ABI,
container/model/envelope versions and document format are unchanged. New public
host methods/types are feature-gated. Existing IFC restrictions and missing-plugin
data preservation remain unchanged. All work remains an untracked scaffold with
no HEAD, remote, GitHub URL or hosted CI result. License/publication approval remains
pending; nothing was published.

Next three tasks: (1) explicit manager listing/loading/grants/disable and status,
(2) native installed Wall/column full B/C acceptance and plugin migrations,
(3) D linked plans followed by E1–E4/G1–G6 qualification. L1/L2 remain later work.
The [coverage ledger](2d-coverage.md) still marks architectural/companion BIM and
production gates incomplete. Deployment/jurisdiction/project/concurrency/output
targets remain undecided; no issued reference-project or plotting qualification.
