# Independent contract boundary

Historical isolation-slice report. The later [generic v2 command slice](generic-plugin-commands.md)
adds independent command/descriptor DTOs and a callable host path. Statements below
about generic operations not yet existing describe the earlier isolation baseline.

The `os-plugin-api` crate can now be consumed without linking any other
OpenStructure crate. Use:

```toml
os-plugin-api = { path = "<checkout>/crates/os-plugin-api", default-features = false }
```

This exposes manifests, permissions, capabilities, registrations and contract
validation/errors. It does **not** yet expose a generic modeling protocol,
descriptor renderer or complete Rust guest SDK. API version remains 1; the
required separately installed Wall/column authoring acceptance remains open.

Default builds enable `legacy-v1`, retaining the old wall-specific JSON messages
and built-in Rust trait. Only that feature depends on core/model/document/geometry.
The application and current Wasm probe therefore keep their working path while
new generic contracts can be implemented independently of internal model layouts.

`Manifest::from_toml` and `validate` now return `ProtocolError`, an independent
error type. Legacy consumers can convert it into `os_core::Error` with `?` or
`.map_err(Into::into)`. Explicit old result-type annotations may need that mapping.
No wire schema or native file version changes in this slice.

Manifest input is capped at 64 KiB; plugin IDs at 128 bytes, registration IDs at
256, names at 256, entrypoints at 1,024, dependencies at 128 and registrations at
256. Names reject control characters. Versions are canonical numeric triples
without leading zeros, matching persisted requirement records. Unknown manifest,
registration and dependency fields are rejected rather than ignoring misspelled
security-sensitive metadata. Existing valid bundled and Wasm fixtures pass.

## Reproduction and evidence

`examples/contract-consumer` is deliberately outside the host workspace, with its
own lockfile. Merely testing `--no-default-features` in a larger workspace could
allow feature unification to conceal host dependencies. The separate example reads
the real Wall manifest and checks its element registration/read permission.
It is a native metadata consumer, not a Wasm modeling plugin or installation test.

```powershell
.\tools\cargo.ps1 test -p os-plugin-api --no-default-features --locked --offline
.\tools\cargo.ps1 run --manifest-path examples/contract-consumer/Cargo.toml --locked --offline '--' plugins/walls/plugin.toml
.\tools\cargo.ps1 metadata --manifest-path examples/contract-consumer/Cargo.toml --locked --offline --format-version 1 | C:\Python314\python.exe tools/check-contract-dependencies.py
.\tools\cargo.ps1 clippy --manifest-path examples/contract-consumer/Cargo.toml --all-targets --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --manifest-path examples/contract-consumer/Cargo.toml '--' --check
.\tools\cargo.ps1 test --workspace --all-features --locked --offline
.\tools\cargo.ps1 clippy --workspace --all-targets --all-features --locked --offline '--' -D warnings
.\tools\cargo.ps1 fmt --all '--' --check
.\tools\cargo.ps1 build --workspace --locked --offline --target-dir work/completion-build
```

Observed on Windows x64/Rust 1.98.1: separate consumer PASS, 23 package versions
(22 distinct names),
no other `os-*` packages, UUID runtime, Wasmi or eframe. Contract-only tests pass
(2). Full workspace suite passes 102 tests, including the document doctest.
The parser-bound test rejects oversized/unknown metadata and excessive counts;
existing manifest/host/Wasm tests preserve version/grant/collision enforcement.

Both workspace and independent-consumer Clippy/formatting passed. The desktop
build and wall edit/history/save/reopen smoke passed, producing
`outputs/independent-contract-verified.osb`. Workspace license metadata remains
347 packages with publishing disabled; the separate consumer's 23-package license
metadata check also passed. The separate lockfile is retained and its
generated target directory is ignored. No dependency installation was required.

No desktop interaction changed, so native UI checks were not repeated for this
feature-gating slice. No schema/IFC mapping changed. CI now repeats the standalone
build, graph check, formatting and Clippy on its Windows/Linux matrix; hosted CI
has not run. The source is still an untracked initial scaffold with no HEAD or
remote. Nothing was committed/published and licensing remains pending.

## Remaining sequence

1. Implement negotiated generic messages, validated descriptors, scoped snapshots
   and commands in this independent surface, including required 2D contract design.
2. Connect runtime/worker/lifecycle UI, real descriptor-driven authoring and
   separately compiled Wall/column examples, including migrations and failure tests.
3. Pass B/C before linked floor-plan D, then full E1–E4 and G1–G6. This dependency
   cleanup does not qualify architectural, companion-BIM or production workflows.

Owner deployment/project/output/performance targets remain undecided. L1/L2
remain post-E4 follow-ons. See [coverage](2d-coverage.md) and [ADR 0010](decisions/0010-independent-contract-feature.md).
