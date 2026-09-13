use crate::Triangle;
use os_core::{Id, Result, ensure};
use std::collections::BTreeMap;

pub const MAX_PIXELS: usize = 2_000_000;
const MAX_SIDE: usize = 4096;
const EMPTY: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterStyle {
    pub normal: [u8; 3],
    pub selected: [u8; 3],
}
impl Default for RasterStyle {
    fn default() -> Self {
        Self {
            normal: [171, 182, 197],
            selected: [75, 132, 237],
        }
    }
}

/// All buffers use the same pixel centres. Transparent pixels have no owner.
pub struct RasterFrame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<[u8; 4]>,
    depths: Vec<f64>,
    owners: Vec<u32>,
    entities: Vec<Id>,
    logical_size: [f32; 2],
}

pub(super) fn sample_depth(t: &Triangle, x: f64, y: f64) -> Option<f64> {
    let weights = barycentric(t, x, y)?;
    let [a, b, c] = t.points;
    // Relative interpolation preserves constant-depth planes exactly.
    let d = a.depth + weights[1] * (b.depth - a.depth) + weights[2] * (c.depth - a.depth);
    d.is_finite().then_some(d)
}
fn barycentric(t: &Triangle, x: f64, y: f64) -> Option<[f64; 3]> {
    if !x.is_finite()
        || !y.is_finite()
        || !t
            .points
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite() && p.depth.is_finite())
    {
        return None;
    }
    let [a, b, c] = t.points;
    let (ax, ay, bx, by, cx, cy) = (
        f64::from(a.x),
        f64::from(a.y),
        f64::from(b.x),
        f64::from(b.y),
        f64::from(c.x),
        f64::from(c.y),
    );
    let denominator = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
    if !denominator.is_finite() || denominator.abs() < 1e-12 {
        return None;
    }
    let u = ((by - cy) * (x - cx) + (cx - bx) * (y - cy)) / denominator;
    let v = ((cy - ay) * (x - cx) + (ax - cx) * (y - cy)) / denominator;
    let w = 1. - u - v;
    // Shared-edge samples may be visited twice; the stable depth/identity tie
    // rule makes coverage deterministic and prevents cracks at the diagonal.
    ([u, v, w].iter().all(|v| *v >= -1e-12 && *v <= 1. + 1e-12)).then_some([u, v, w])
}

impl RasterFrame {
    pub fn render(
        triangles: &[Triangle],
        logical_size: [f32; 2],
        scale: f32,
        selected: Option<Id>,
        style: RasterStyle,
    ) -> Result<Self> {
        ensure(
            logical_size.iter().all(|x| x.is_finite() && *x > 0.)
                && scale.is_finite()
                && scale > 0.,
            "invalid viewport dimensions or resolution scale",
        )?;
        let w = f64::from(logical_size[0]) * f64::from(scale);
        let h = f64::from(logical_size[1]) * f64::from(scale);
        let reduction = (MAX_PIXELS as f64 / (w * h))
            .sqrt()
            .min(MAX_SIDE as f64 / w)
            .min(MAX_SIDE as f64 / h)
            .min(1.);
        let width = (w * reduction).floor().max(1.) as usize;
        let height = (h * reduction).floor().max(1.) as usize;
        let count = width
            .checked_mul(height)
            .filter(|n| *n <= MAX_PIXELS)
            .ok_or_else(|| {
                os_core::Error::Invalid("framebuffer exceeds allocation limit".into())
            })?;
        let index: BTreeMap<Id, u32> = triangles
            .iter()
            .map(|t| t.entity)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .enumerate()
            .map(|(i, id)| (id, i as u32))
            .collect();
        let mut frame = Self {
            width,
            height,
            rgba: vec![[0; 4]; count],
            depths: vec![f64::NEG_INFINITY; count],
            owners: vec![EMPTY; count],
            entities: index.keys().copied().collect(),
            logical_size,
        };
        let sx = width as f64 / f64::from(logical_size[0]);
        let sy = height as f64 / f64::from(logical_size[1]);
        for t in triangles {
            if !t.shade.is_finite()
                || !t
                    .points
                    .iter()
                    .all(|p| p.x.is_finite() && p.y.is_finite() && p.depth.is_finite())
            {
                continue;
            }
            let min_x = t
                .points
                .iter()
                .map(|p| f64::from(p.x) * sx)
                .fold(f64::INFINITY, f64::min)
                .floor()
                .max(0.)
                .min(width as f64) as usize;
            let max_x = t
                .points
                .iter()
                .map(|p| f64::from(p.x) * sx)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .max(0.)
                .min(width as f64) as usize;
            let min_y = t
                .points
                .iter()
                .map(|p| f64::from(p.y) * sy)
                .fold(f64::INFINITY, f64::min)
                .floor()
                .max(0.)
                .min(height as f64) as usize;
            let max_y = t
                .points
                .iter()
                .map(|p| f64::from(p.y) * sy)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .max(0.)
                .min(height as f64) as usize;
            let owner = index[&t.entity];
            let base = if selected == Some(t.entity) {
                style.selected
            } else {
                style.normal
            };
            let mut color = [0; 4];
            for i in 0..3 {
                color[i] = (f32::from(base[i]) * t.shade.clamp(0., 1.)) as u8;
            }
            color[3] = 255;
            for py in min_y..max_y {
                for px in min_x..max_x {
                    let Some(depth) =
                        sample_depth(t, (px as f64 + 0.5) / sx, (py as f64 + 0.5) / sy)
                    else {
                        continue;
                    };
                    let i = py * width + px;
                    if depth > frame.depths[i]
                        || (depth == frame.depths[i]
                            && (owner < frame.owners[i]
                                || (owner == frame.owners[i] && color < frame.rgba[i])))
                    {
                        frame.depths[i] = depth;
                        frame.owners[i] = owner;
                        frame.rgba[i] = color;
                    }
                }
            }
        }
        Ok(frame)
    }
    pub fn pick(&self, x: f32, y: f32) -> Option<Id> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.
            || y < 0.
            || x >= self.logical_size[0]
            || y >= self.logical_size[1]
        {
            return None;
        }
        let px =
            (f64::from(x) / f64::from(self.logical_size[0]) * self.width as f64).floor() as usize;
        let py =
            (f64::from(y) / f64::from(self.logical_size[1]) * self.height as f64).floor() as usize;
        self.entities
            .get(self.owners[py * self.width + px] as usize)
            .copied()
    }
    pub fn depth_at(&self, x: usize, y: usize) -> Option<f64> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let d = self.depths[y * self.width + x];
        d.is_finite().then_some(d)
    }
}
