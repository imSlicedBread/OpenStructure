//! Bounded annular display shells; quantities remain analytic.
use super::*;
use crate::plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, PlanRole};

impl NativeWall {
    fn arc_segments(&self) -> Result<usize> {
        self.parameters.validate()?;
        ensure(
            self.openings.is_empty() && self.interfaces.is_empty(),
            "circular walls cannot host openings or joins",
        )?;
        self.parameters
            .path
            .display_segments(self.parameters.thickness / 2.0)
    }
    fn arc_quad(&self, layer: &os_model::ResolvedWallLayer, i: usize, count: usize) -> Vec<Point2> {
        let path = self.parameters.path;
        let a = path.length() * i as f64 / count as f64;
        let b = path.length() * (i + 1) as f64 / count as f64;
        vec![
            path.offset_point(a, layer.min),
            path.offset_point(b, layer.min),
            path.offset_point(b, layer.max),
            path.offset_point(a, layer.max),
        ]
    }
    pub(super) fn arc_layer_mesh(&self, layer: &os_model::ResolvedWallLayer) -> Result<Mesh> {
        let count = self.arc_segments()?;
        let mut mesh = Mesh::default();
        for i in 0..=count {
            let station = self.parameters.length() * i as f64 / count as f64;
            for z in [self.elevation, self.elevation + self.parameters.height] {
                for side in [layer.min, layer.max] {
                    let p = self.parameters.path.offset_point(station, side);
                    mesh.vertices.push(Vec3::new(p.x, p.y, z));
                }
            }
        }
        let mut quad = |a, b, c, d| {
            mesh.triangles.extend([[a, b, c], [a, c, d]]);
        };
        for i in 0..count as u32 {
            let a = i * 4;
            let b = a + 4;
            quad(a, a + 1, b + 1, b); // bottom
            quad(a + 2, b + 2, b + 3, a + 3); // top
            quad(a, b, b + 2, a + 2); // right face
            quad(a + 1, a + 3, b + 3, b + 1); // left face
        }
        quad(0, 2, 3, 1);
        let e = count as u32 * 4;
        quad(e, e + 1, e + 3, e + 2);
        mesh.validate()?;
        Ok(mesh)
    }
    pub(super) fn arc_quantities(&self) -> Result<Vec<LayerQuantity>> {
        self.arc_segments()?;
        let os_model::WallPath::CircularArc {
            radius,
            signed_sweep_rad,
            ..
        } = self.parameters.path
        else {
            unreachable!()
        };
        self.layers
            .iter()
            .map(|layer| {
                let r0 = radius - signed_sweep_rad.signum() * layer.min;
                let r1 = radius - signed_sweep_rad.signum() * layer.max;
                // Factored difference avoids cancellation for thin layers on large radii.
                let area = (r0 + r1) * (layer.max - layer.min) * signed_sweep_rad.abs() / 2.0;
                let volume_m3 = area * self.parameters.height;
                let mass_kg = layer.density_kg_m3.map(|d| d * volume_m3);
                ensure(
                    volume_m3.is_finite() && mass_kg.is_none_or(f64::is_finite),
                    "arc quantity overflow",
                )?;
                Ok(LayerQuantity {
                    layer: layer.id,
                    material: layer.material,
                    name: layer.name.clone(),
                    volume_m3,
                    mass_kg,
                })
            })
            .collect()
    }
    pub(super) fn arc_plan_footprints(
        &self,
        range: PlanRange,
        basis: HorizontalBasis,
        crop: Option<PlanCrop>,
    ) -> Result<Vec<(os_model::ResolvedWallLayer, Vec<PlanFootprint>)>> {
        let count = self.arc_segments()?;
        range.validate()?;
        basis.validate()?;
        if let Some(c) = crop {
            c.validate()?;
        }
        let base = self.elevation;
        let top = base + self.parameters.height;
        let t = crate::plan::PLAN_TOLERANCE;
        let role = if top <= range.depth + t || base >= range.top - t {
            None
        } else if base <= range.cut + t && top > range.cut + t {
            Some(PlanRole::Cut)
        } else if top <= range.cut + t && top > range.bottom + t {
            Some(PlanRole::Projected)
        } else if top <= range.bottom + t && top > range.depth + t {
            Some(PlanRole::Depth)
        } else {
            None
        };
        self.layers
            .iter()
            .map(|layer| {
                let mut parts = Vec::new();
                if let Some(role) = role {
                    for i in 0..count {
                        let vertices = self
                            .arc_quad(layer, i, count)
                            .into_iter()
                            .map(|p| basis.world_to_plane(p))
                            .collect::<Result<Vec<_>>>()?;
                        if let Some(fp) = PlanFootprint::from_convex(role, vertices, crop)? {
                            parts.push(fp);
                        }
                    }
                }
                Ok((layer.clone(), parts))
            })
            .collect()
    }
}
