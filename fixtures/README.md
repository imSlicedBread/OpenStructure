# Fixtures

`schema-0-model.json` is a fixed, independently authored old-schema model used to
test migration. Its UUIDs are fixed for repeatability. The file includes a level
at 2.5 metres using the old `z` field.

Generate a current native example with the executable's `--smoke` command.
`wall-exchange.ifc` is exported from the native smoke workflow: a 7 × 0.3 × 3.5 m
wall at elevation 3 m. `rotated-walls.ifc` contains two 5 × 0.3 × 3.5 m rotated
walls on levels at 0 and 4.2 m, generated with
`cargo run -p os-ifc --example rotated_fixture -- <new-output.ifc>`.
Both pass independent IfcOpenShell schema/EXPRESS and Open CASCADE geometry checks;
run `python tools/validate-ifc.py <fixture-path>`. These are exchange fixtures,
not complete native backups; see `docs/ifc-roadmap.md` for losses and restrictions.

`intersecting-walls.osb` contains east-west, north-south and diagonal walls with
different heights, all crossing at the origin. Generate a new copy with
`cargo run -p os-app --example intersecting_walls -- <new-output.osb>`.
It exercises depth-buffer visibility and selection, not boolean joins.

`wasm-probe/` contains an owned WAT source and v1 manifest for an opt-in runtime
compatibility test. It returns a fixed unit prism and is not a modeling plugin.
The `os-plugin-host` `wasm_probe` example assembles a fresh external installation;
only its subsequent loader reads the installed binary. No binary fixture needs
to be checked in. See `docs/wasm-runtime-spike.md` for ABI and limits.
