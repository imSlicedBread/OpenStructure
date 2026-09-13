# Independent IFC viewer acceptance — 2026-09-11

Passed using the upstream **Online 3D Viewer 0.18.0** embedded viewer and
**web-ifc 0.0.68**, rendered interactively in an isolated Chromium browser.
This is a separate IFC parser and rendering stack from OpenStructure's Rust
reader/prism renderer and from the earlier IfcOpenShell/Open CASCADE validator.

## Evidence

| Fixture | Viewer result | Visual inspection |
| --- | --- | --- |
| `fixtures/wall-exchange.ifc` | 1 mesh, 12 triangles | Solid rectangular wall, thickness/top edge visible |
| `fixtures/rotated-walls.ifc` | 2 meshes, 24 triangles | Both separated, differently oriented walls visible; orbit changes view correctly |

Screenshots retained locally in `outputs/ifc-external-single.png`,
`outputs/ifc-external-rotated.png` and `outputs/ifc-external-orbit.png`. All three
were visually inspected. Browser error output was empty on the successful run.
The resource list contained no off-origin requests. Exact metre dimensions,
elevations and volumes are independently checked by `tools/validate-ifc.py`;
screen appearance is not used to claim numerical precision.

Fixture SHA-256 at inspection:

```text
wall-exchange.ifc  1618a088ccfd7559aac3268310b2a37a9e437389219fddbf651a6df062991fb5
rotated-walls.ifc  974a0cf54c1d183686483cd0dda6603939a65b2ac3e5bc16b7e2b58c65b2b7d3
```

The public-site file-loading operation was blocked by security review and was
not performed. The successful run used a local installation of the external
viewer's library through its documented `EmbeddedViewer` API. The harness serves
only the two fixtures and named assets, accepts GET only, binds 127.0.0.1 and
blocks external connections through CSP. It does not upload files. The only
upstream bundle substitution redirects the pinned web-ifc asset URL to localhost;
parsing and rendering code is unchanged. The page's controls/status text are
test harness code, not an OpenStructure rendering implementation.

## Reproduce

```sh
npm install --prefix work/local-ifc-viewer online-3d-viewer@0.18.0 web-ifc@0.0.68
node tools/ifc-viewer/server.mjs
```

Open `http://127.0.0.1:8137`, choose **Single wall**, then **Rotated walls**.
Wait for the mesh/triangle counts and visible solid geometry. Drag to orbit.
Stop the local server when done. It is a development-only acceptance harness,
not a distributed service or application dependency. Both verification browser
sessions and the local server were stopped after inspection.

The browser-verification skills required a real rendered-page check and a retry
after diagnosing a CSP initialization error: Emscripten's embind uses `Function`,
so the local page permits `unsafe-eval` while keeping script assets and network
connections local. This permission applies only to the development harness.

## Dependency limitation

The temporary npm stack reports two moderate entries for one transitive issue:
fflate ZIP64 denial of service (GHSA-px8p-9vwx-vf98), propagated to the viewer.
Only the two generated plain STEP IFC fixtures were served, not ZIP or arbitrary
files. The viewer is not bundled into the Rust desktop application. Do not deploy
this harness as a general file-upload/viewing service or call its dependencies
warning-free. Upstream viewer license metadata is MIT. The installed web-ifc
package has no license field; inspect upstream licensing before any redistribution.

This closes the external-viewer acceptance gate for the documented wall subset,
not general IFC conformance or arbitrary third-party IFC import.
