//! Replaceable geometry kernel with plain serializable boundary types.
use os_core::{Error, Point2, Result, ensure};
use serde::{Deserialize, Serialize};
pub mod columns;
pub mod floors;
pub mod openings;
pub mod plan;
pub mod rooms;
pub mod section;
pub mod walls;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub vertices: Vec<Point2>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation_z: f64,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::default(),
            rotation_z: 0.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Solid {
    pub profile: Profile,
    pub height: f64,
    pub transform: Transform,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    /// Empty for unclassified geometry, otherwise one identity per triangle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<SurfaceIdentity>,
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceIdentity {
    pub layer: Option<os_core::Id>,
    pub material: Option<os_core::Id>,
}
impl SurfaceIdentity {
    /// Stable display swatch; actual material UUID remains available to consumers.
    pub fn color(self) -> Option<[u8; 3]> {
        self.material.or(self.layer).map(|id| {
            let b = id.0.as_bytes();
            [140 + b[0] % 80, 140 + b[1] % 80, 140 + b[2] % 80]
        })
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum BooleanOperation {
    Union,
    Difference,
    Intersection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionResult {
    pub polylines: Vec<Vec<Vec3>>,
}

pub trait GeometryKernel {
    fn tessellate(&self, solid: &Solid) -> Result<Mesh>;
    fn boolean(&self, _a: &Solid, _b: &Solid, _op: BooleanOperation) -> Result<Solid> {
        Err(Error::Unsupported(
            "geometry booleans require a kernel adapter".into(),
        ))
    }
    fn section(&self, _solid: &Solid, _elevation: f64) -> Result<SectionResult> {
        Err(Error::Unsupported(
            "sections require a kernel adapter".into(),
        ))
    }
}

/// The first kernel accepts counter-clockwise rectangular profiles only.
#[derive(Default)]
pub struct PrismKernel;
impl GeometryKernel for PrismKernel {
    fn tessellate(&self, solid: &Solid) -> Result<Mesh> {
        let p = &solid.profile.vertices;
        ensure(
            p.len() == 4 && p.iter().all(|p| p.is_finite()),
            "kernel requires four finite profile points",
        )?;
        ensure(
            solid.height.is_finite() && solid.height > 1e-6,
            "invalid extrusion height",
        )?;
        ensure(
            solid.transform.translation.is_finite() && solid.transform.rotation_z.is_finite(),
            "invalid transform",
        )?;
        let edges: Vec<_> = (0..4)
            .map(|i| Point2::new(p[(i + 1) % 4].x - p[i].x, p[(i + 1) % 4].y - p[i].y))
            .collect();
        for i in 0..4 {
            let a = edges[i];
            let b = edges[(i + 1) % 4];
            let norm = a.x.hypot(a.y) * b.x.hypot(b.y);
            ensure(
                norm.is_finite()
                    && norm > 1e-12
                    && a.x * b.y - a.y * b.x > 0.0
                    && (a.x * b.x + a.y * b.y).abs() <= norm * 1e-8,
                "profile must be a nondegenerate counter-clockwise rectangle",
            )?;
        }
        let (s, c) = solid.transform.rotation_z.sin_cos();
        let t = solid.transform.translation;
        let mut vertices = Vec::with_capacity(8);
        for z in [0.0, solid.height] {
            for p in p {
                vertices.push(Vec3::new(
                    c * p.x - s * p.y + t.x,
                    s * p.x + c * p.y + t.y,
                    z + t.z,
                ));
            }
        }
        let mesh = Mesh {
            surfaces: vec![],
            vertices,
            triangles: vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        };
        mesh.validate()?;
        Ok(mesh)
    }
}
impl Mesh {
    /// Combine disjoint rectangular cells without filling the apertures between them.
    /// Cell boundaries retain internal faces; this is not a welded boolean B-rep.
    pub fn from_prisms(solids: &[Solid]) -> Result<Self> {
        let mut result = Self::default();
        for solid in solids {
            let mesh = PrismKernel.tessellate(solid)?;
            let offset = u32::try_from(result.vertices.len())
                .map_err(|_| Error::Invalid("mesh too large".into()))?;
            result
                .triangles
                .extend(mesh.triangles.iter().map(|t| t.map(|i| i + offset)));
            result.vertices.extend(mesh.vertices);
        }
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.surfaces.is_empty() || self.surfaces.len() == self.triangles.len(),
            "mesh surface identity count mismatch",
        )?;
        ensure(
            !self.vertices.is_empty() && !self.triangles.is_empty(),
            "empty mesh",
        )?;
        ensure(
            self.vertices.iter().all(|v| v.is_finite()),
            "non-finite mesh vertex",
        )?;
        for t in &self.triangles {
            ensure(
                t.iter().all(|i| (*i as usize) < self.vertices.len()),
                "mesh index out of bounds",
            )?;
            ensure(
                t[0] != t[1] && t[1] != t[2] && t[0] != t[2],
                "degenerate triangle",
            )?;
            let [a, b, c] = t.map(|i| self.vertices[i as usize]);
            let u = Vec3::new(b.x - a.x, b.y - a.y, b.z - a.z);
            let v = Vec3::new(c.x - a.x, c.y - a.y, c.z - a.z);
            let area = (u.y * v.z - u.z * v.y)
                .hypot(u.z * v.x - u.x * v.z)
                .hypot(u.x * v.y - u.y * v.x);
            ensure(
                area.is_finite() && area > 0.0,
                "zero-area or overflowing triangle",
            )?;
        }
        Ok(())
    }
    /// Signed volume for closed outward-wound triangle meshes.
    pub fn signed_volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.vertices[i as usize]);
                (a.x * (b.y * c.z - b.z * c.y)
                    + a.y * (b.z * c.x - b.x * c.z)
                    + a.z * (b.x * c.y - b.y * c.x))
                    / 6.0
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prism_is_closed_outward_and_has_expected_volume() {
        let solid = Solid {
            profile: Profile {
                vertices: vec![
                    Point2::new(0.0, -0.1),
                    Point2::new(5.0, -0.1),
                    Point2::new(5.0, 0.1),
                    Point2::new(0.0, 0.1),
                ],
            },
            height: 3.0,
            transform: Transform::default(),
        };
        let mesh = PrismKernel.tessellate(&solid).unwrap();
        assert!((mesh.signed_volume() - 3.0).abs() < 1e-9);
        let mut edges = std::collections::BTreeMap::new();
        for [a, b, c] in &mesh.triangles {
            for (a, b) in [(*a, *b), (*b, *c), (*c, *a)] {
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        for ((a, b), count) in &edges {
            assert_eq!(*count, 1);
            assert_eq!(edges.get(&(*b, *a)), Some(&1));
        }
        let mut bad = solid;
        bad.profile.vertices.swap(1, 2);
        assert!(PrismKernel.tessellate(&bad).is_err());
    }
}
