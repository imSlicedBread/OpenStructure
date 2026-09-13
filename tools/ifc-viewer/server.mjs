// Local-only independent-viewer acceptance harness, not an application service.
import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const deps = path.join(root, 'work/local-ifc-viewer/node_modules');
const files = new Map([
  ['/', [path.join(here, 'index.html'), 'text/html']],
  ['/viewer.js', [path.join(here, 'viewer.js'), 'text/javascript']],
  ['/vendor/o3dv.js', [path.join(deps, 'online-3d-viewer/build/engine/o3dv.min.js'), 'text/javascript']],
  ['/vendor/web-ifc-api-iife.js', [path.join(deps, 'web-ifc/web-ifc-api-iife.js'), 'text/javascript']],
  ['/vendor/web-ifc.wasm', [path.join(deps, 'web-ifc/web-ifc.wasm'), 'application/wasm']],
  ['/wall-exchange.ifc', [path.join(root, 'fixtures/wall-exchange.ifc'), 'application/octet-stream']],
  ['/rotated-walls.ifc', [path.join(root, 'fixtures/rotated-walls.ifc'), 'application/octet-stream']],
]);
const server = http.createServer(async (req, res) => {
  if (req.method !== 'GET') {res.writeHead(405);res.end();return;}
  const entry = files.get(req.url);
  if (!entry) {res.writeHead(404);res.end();return;}
  try {
    let bytes = await readFile(entry[0]);
    if (req.url === '/vendor/o3dv.js') {
      const remote = 'https://cdn.jsdelivr.net/npm/web-ifc@0.0.68/web-ifc-api-iife.js';
      const text = bytes.toString();
      if (!text.includes(remote)) throw new Error('Unexpected viewer version; review asset URL mapping');
      // Only asset routing changes; upstream parsing/rendering code is unchanged.
      bytes = Buffer.from(text.replaceAll(remote, '/vendor/web-ifc-api-iife.js'));
    }
    res.writeHead(200, {'Content-Type': entry[1], 'Cache-Control': 'no-store',
      // Emscripten's embind glue requires Function(); keep assets/connections local.
      'Content-Security-Policy': "default-src 'self'; script-src 'self' 'unsafe-eval'; connect-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; worker-src 'self' blob:; object-src 'none'"});
    res.end(bytes);
  } catch (error) {console.error(error);res.writeHead(500);res.end('Local asset unavailable');}
});
server.listen(8137, '127.0.0.1', () => console.log('Independent IFC viewer: http://127.0.0.1:8137 (GET-only fixture allowlist)'));
