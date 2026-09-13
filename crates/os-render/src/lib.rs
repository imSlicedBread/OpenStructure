//! Orthographic projection and bounded depth-buffered rendering/picking.
use os_core::Id;
use os_geometry::{Mesh, Vec3};
use std::collections::BTreeMap;

pub type Scene = BTreeMap<Id, Mesh>;
pub mod plan;
mod raster;
pub mod snapping;
pub use raster::{MAX_PIXELS, RasterFrame, RasterStyle};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    pub pixels_per_metre: f64,
    pub target: Vec3,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: -0.65,
            pitch: 0.55,
            pixels_per_metre: 65.0,
            target: Vec3::new(2.5, 0.0, 1.0),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectedPoint {
    pub x: f32,
    pub y: f32,
    pub depth: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Triangle {
    pub entity: Id,
    pub points: [ProjectedPoint; 3],
    pub shade: f32,
}
impl Camera {
    pub fn project(&self, v: Vec3, width: f32, height: f32) -> ProjectedPoint {
        let x = v.x - self.target.x;
        let y = v.y - self.target.y;
        let z = v.z - self.target.z;
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let right = cy * x + sy * y;
        let horizontal_depth = -sy * x + cy * y;
        ProjectedPoint {
            x: width / 2.0 + (right * self.pixels_per_metre) as f32,
            y: height / 2.0 - ((z * cp - horizontal_depth * sp) * self.pixels_per_metre) as f32,
            depth: horizontal_depth * cp + z * sp,
        }
    }
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw += f64::from(dx) * 0.008;
        self.pitch = (self.pitch + f64::from(dy) * 0.008).clamp(0.08, 1.5);
    }
    pub fn zoom(&mut self, delta: f32) {
        self.pixels_per_metre =
            (self.pixels_per_metre * (f64::from(delta) * 0.002).exp()).clamp(0.001, 10000.0);
    }
    pub fn fit(&mut self, scene: &Scene, width: f32, height: f32) {
        let vertices: Vec<_> = scene.values().flat_map(|m| m.vertices.iter()).collect();
        if vertices.is_empty() {
            return;
        }
        let mut low = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut high = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for v in vertices {
            low.x = low.x.min(v.x);
            low.y = low.y.min(v.y);
            low.z = low.z.min(v.z);
            high.x = high.x.max(v.x);
            high.y = high.y.max(v.y);
            high.z = high.z.max(v.z);
        }
        self.target = Vec3::new(
            low.x / 2.0 + high.x / 2.0,
            low.y / 2.0 + high.y / 2.0,
            low.z / 2.0 + high.z / 2.0,
        );
        let diameter = (high.x - low.x)
            .hypot(high.y - low.y)
            .hypot(high.z - low.z)
            .max(1.0);
        self.pixels_per_metre =
            (f64::from(width.min(height)) * 0.8 / diameter).clamp(0.001, 10000.0);
    }
}

pub fn project_scene(scene: &Scene, camera: &Camera, width: f32, height: f32) -> Vec<Triangle> {
    let mut triangles = Vec::new();
    let (sy, cy) = camera.yaw.sin_cos();
    let (sp, cp) = camera.pitch.sin_cos();
    let toward_camera = Vec3::new(-sy * cp, cy * cp, sp);
    for (id, mesh) in scene {
        for indices in &mesh.triangles {
            let [Some(a), Some(b), Some(c)] =
                indices.map(|i| mesh.vertices.get(i as usize).copied())
            else {
                continue;
            };
            let u = Vec3::new(b.x - a.x, b.y - a.y, b.z - a.z);
            let v = Vec3::new(c.x - a.x, c.y - a.y, c.z - a.z);
            let n = Vec3::new(
                u.y * v.z - u.z * v.y,
                u.z * v.x - u.x * v.z,
                u.x * v.y - u.y * v.x,
            );
            // Cull closed outward-wound solids' hidden back faces.
            if !n.is_finite()
                || n.x * toward_camera.x + n.y * toward_camera.y + n.z * toward_camera.z <= 0.0
            {
                continue;
            }
            let norm = n.x.hypot(n.y).hypot(n.z).max(1e-12);
            let light = ((n.x * 0.3 - n.y * 0.4 + n.z * 0.85) / norm).max(0.0);
            let points = [a, b, c].map(|v| camera.project(v, width, height));
            if !points
                .iter()
                .all(|p| p.x.is_finite() && p.y.is_finite() && p.depth.is_finite())
            {
                continue;
            }
            triangles.push(Triangle {
                entity: *id,
                points,
                shade: (0.52 + 0.48 * light) as f32,
            });
        }
    }
    triangles
}

/// Analytical visible-surface query. Desktop clicks use RasterFrame::pick so
/// they exactly match the displayed pixel, including resolution scaling.
pub fn pick(triangles: &[Triangle], x: f32, y: f32) -> Option<Id> {
    triangles
        .iter()
        .filter_map(|t| raster::sample_depth(t, f64::from(x), f64::from(y)).map(|d| (d, t.entity)))
        .max_by(|(da, ia), (db, ib)| da.total_cmp(db).then_with(|| ib.cmp(ia)))
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_closed_prism_only_draws_its_three_camera_facing_sides() {
        use os_core::Point2;
        use os_geometry::{GeometryKernel, PrismKernel, Profile, Solid, Transform};
        let solid = Solid {
            profile: Profile {
                vertices: vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(5.0, 0.0),
                    Point2::new(5.0, 0.2),
                    Point2::new(0.0, 0.2),
                ],
            },
            height: 3.0,
            transform: Transform::default(),
        };
        let id = Id::new();
        let scene = Scene::from([(id, PrismKernel.tessellate(&solid).unwrap())]);
        for yaw in [-2.5, -0.65, 0.5, 2.5] {
            let camera = Camera {
                yaw,
                ..Camera::default()
            };
            let triangles = project_scene(&scene, &camera, 800.0, 600.0);
            assert_eq!(triangles.len(), 6);
            let center = camera.project(Vec3::new(2.5, 0.1, 1.5), 800.0, 600.0);
            assert_eq!(pick(&triangles, center.x, center.y), Some(id));
        }
    }
    #[test]
    fn projection_and_pick() {
        let camera = Camera::default();
        let center = camera.project(camera.target, 800.0, 600.0);
        assert_eq!((center.x, center.y), (400.0, 300.0));
        let id = Id::new();
        let p = |x, y| ProjectedPoint { x, y, depth: 0.0 };
        let tris = vec![Triangle {
            entity: id,
            points: [p(0.0, 0.0), p(100.0, 0.0), p(0.0, 100.0)],
            shade: 1.0,
        }];
        assert_eq!(pick(&tris, 20.0, 20.0), Some(id));
        assert_eq!(pick(&tris, 90.0, 90.0), None);
    }
}
