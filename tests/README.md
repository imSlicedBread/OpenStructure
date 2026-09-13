# Acceptance tests

Cargo integration tests live in `crates/os-ui/tests/vertical_slice.rs` so they are
discovered by `cargo test --workspace`. They use the same Editor controller as the
desktop shell. Tests cover plugin load, wall creation, every required wall edit,
level elevation invalidation, geometry regeneration, identity, undo/redo, deletion,
atomic failures, save/reopen and failed-open recovery.

The `os-ui` desktop tests also drive real egui pointer, scroll, and keyboard
events through the native app's frame function. They cover ribbon/Properties
command routing, browser and viewport selection, Manage actions, File-menu path
editing, modal cancellation/replacement, invalid drafts, and save/reopen. Layout
tests exercise both 1280 × 800 and 1000 × 650 logical windows at 1.0, 1.25, and
1.5 pixels per logical point, including scrolling to Endpoints while the Apply
action stays visible. These are headless interaction checks, separate from
native Windows visual inspection.

Crate tests cover units, graph integrity, solid topology, manifest and permission
validation, hostile plugin responses, archive corruption, migration and picking.
`cargo run -p os-app -- --smoke <new-path.osb>` additionally exercises the complete
executable without a display and leaves a native project for desktop inspection.

The IFC slice adds Rust semantic/geometry round trips and executable exchange
safety tests, plus independent schema/EXPRESS and geometry validation of frozen
and freshly exported IFC fixtures. External-viewer acceptance passed locally;
see `docs/ifc-viewer-acceptance.md`. Desktop tests cover the File-menu exchange
workflow, loss consent, replacement races, cancellation/failure preservation,
staged snapshots, modal shortcut blocking and imported-document save state.

Depth-buffer tests cover crossing planes, draw-order-independent visible pixels,
coplanar identity ties, shared-edge coverage, clipping, invalid geometry, DPI
scaling and allocation limits. Actual crossing prisms are compared against an
independent ray/axis-aligned-box oracle at four camera angles. A desktop test
checks displayed-pixel selection, edit/undo, real drag-to-orbit, resize/DPI changes
and absence of redundant texture uploads on unchanged frames.

The opt-in Wasm spike adds 12 transport/installation tests in
`crates/os-plugin-host/tests/wasm_transport.rs`, enabled by `--all-features` or
`cargo test -p os-plugin-host --features wasm`. They cover guest resource limits,
imports/ABI/ranges, independent replies, permissions and history/storage integrity.
CI separately assembles and loads the WAT geometry probe. This does not complete
the external Rust Wall/column SDK acceptance. See `docs/wasm-runtime-spike.md`.

Container 2 tests in `os-storage/tests/auxiliary_files.rs` exercise opaque byte
preservation, portable paths, CRC/symlink/size failures, frozen container-1 input,
and deterministic archive mutation. Editor and CLI tests cover save/open recovery
and explicit auxiliary-file loss on IFC export. See `docs/opaque-files-verification.md`.

Wasm worker tests now cover session/revision/view validity, cancellation/deadlines,
revoked infinite guests, one-time command delivery, safe unload/reload and bounded
slot lifetime. The standalone probe accepts `--worker` to exercise the installed
binary off-thread. See `docs/plugin-workers.md` for precise limits and evidence.
