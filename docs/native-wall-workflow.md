# Native independent Wall workflow evidence

Local Windows, 1280×800, untracked working tree with no HEAD/remote. The independent
Rust Wasm Wall installation in `outputs/rust-wall-install` declares API 2 and
provider version 0.1.0. The `wall_desktop` developer harness explicitly replaces
the bundled provider with that installation and test-owned model.read/model.write/
ui.tool grants. It shares setup code with `plugin_desktop`. This is not a production
installation/grant UX test and does not load code suggested by a project file.

## Observed workflow

1. Architecture → Wall, then Properties → Review Wall parameters opens the
   registered external create form. Review endpoints (0,0)–(5,0), thickness 0.2 m
   and height 3 m; scroll to Apply plugin command.
2. Apply creates one native Wall and generated geometry. Browser selection opens
   Modify | Walls and retains ID `7418f54a-06d6-4da9-b678-2f9ce4c9f5ab` in the form.
3. Review the selected Wall's registered edit form. Change End X to 8 and Height
   to 4 in one draft, then Apply. Geometry changes and the same ID remains selected.
4. One Undo restores the earlier geometry; Redo restores the edited geometry,
   without another interaction needed to finish repainting.
5. Native Save creates `outputs/native-independent-wall.osb`. Native Open reopens
   it with regenerated geometry and reports zero unavailable plugin elements.
6. Click the reopened wall face in the viewport: it highlights and the matching
   Browser Wall row is selected. The clean acceptance window then closes.

Read-only ZIP/JSON inspection confirms the same ID under native `walls`, type
`org.openstructure.walls.wall`, header schema 2, endpoint (8,0), start (0,0),
thickness 0.2, height 4 and provider requirement `org.openstructure.walls = 0.1.0`.
Ground level is `328d9bdb-4e8b-4cd3-8278-3b29486d9c23`. This artifact is generated
evidence, not a committed reference project or production qualification fixture.

## Reproduce and limits

Build/install the separate guest using the [Wall example](../examples/rust-wall/README.md), then:

```powershell
.\tools\cargo.ps1 run -p os-ui --features external-plugins --example wall_desktop --locked --offline '--' outputs/rust-wall-install
```

Use a new native save path when repeating. Scrolling is required to reach the
six-field form's Apply button at this resolution. A collapsed Plugin tools window
does not automatically expand when Review Wall parameters is pressed; manually
expand its header. This usability issue remains. Native thickness editing, level
reassignment, deletion, missing-provider reload and permission changes were not
performed here. Existing automated probes cover additional operations, but those
do not count as native UI observations. No security/permission controls were automated.

The source changes in this turn are a shared acceptance-harness extraction and
the `wall_desktop` entry point; no public API, dependency, wire ABI or native format
changed. All 151 workspace tests passed with
`cargo test --workspace --all-features --locked --offline --target-dir work/completion-build`
to avoid the earlier unsaved migration harness's executable lock. Strict Clippy,
format check, default build and smoke at `outputs/native-wall-verified.osb` passed.
The older single-type migration harness was not modified or closed. No remote,
commit, hosted CI result, publication or license grant is claimed.

Next: re-audit the B/C requirements against current native and automated evidence;
close concrete remaining lifecycle/form/durability gaps; then implement D's linked
floor-plan authoring. E1–E4/G1–G6 remain incomplete. Production targets remain
undecided; L1/L2 follow later. B/C is not declared complete by this single workflow.
