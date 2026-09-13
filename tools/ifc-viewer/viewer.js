/* global OV */
const status = document.getElementById('status');
window.addEventListener('unhandledrejection', e => {status.textContent='FAIL: '+String(e.reason);});
let current;
const viewer = new OV.EmbeddedViewer(document.getElementById('viewport'), {
  backgroundColor: new OV.RGBAColor(238,241,244,255),
  defaultColor: new OV.RGBColor(80,140,190),
  onModelLoaded() {
    const model = viewer.GetModel();
    status.textContent = `${current}: ${model.MeshCount()} meshes, ${model.TriangleCount()} triangles — rendered by upstream viewer`;
  },
  onModelLoadFailed() {status.textContent = 'FAIL: upstream viewer could not load the fixture';},
});
function load(name) {current=name;status.textContent='Loading '+name;viewer.LoadModelFromUrlList(['/'+name]);}
document.getElementById('single').onclick = () => load('wall-exchange.ifc');
document.getElementById('rotated').onclick = () => load('rotated-walls.ifc');
