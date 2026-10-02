//! Bounded annular display shells; quantities remain analytic.
use super::*;
use crate::plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRole};

impl NativeWall {
    pub(super) fn validate_arc(&self) -> Result<usize> {
        self.parameters.validate()?;
        ensure(
            self.interfaces.is_empty() && self.stations == (0., self.parameters.length()),
            "circular wall joins and trimmed/extended stations are unsupported",
        )?;
        ensure(self.elevation.is_finite(), "invalid wall elevation")?;
        self.parameters
            .path
            .display_segments(self.parameters.thickness / 2.0)
    }
    fn arc_quad(&self, layer: &os_model::ResolvedWallLayer, a: f64, b: f64) -> Vec<Point2> {
        let path = self.parameters.path;
        vec![
            path.offset_point(a, layer.min),
            path.offset_point(b, layer.min),
            path.offset_point(b, layer.max),
            path.offset_point(a, layer.max),
        ]
    }
    pub(super) fn arc_layer_mesh(&self, layer: &os_model::ResolvedWallLayer) -> Result<Mesh> {
        let count = self.validate_arc()?;
        if !self.openings.is_empty() {
            return crate::openings::radial_regions_mesh(
                &self.parameters,
                self.elevation,
                &self.elevation_regions()?,
                layer.min,
                layer.max,
            );
        }
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
    pub(super) fn arc_layer_mesh_regions(
        &self,
        layer: &os_model::ResolvedWallLayer,
    ) -> Result<Vec<Mesh>> {
        self.validate_arc()?;
        if self.openings.is_empty() {
            return Ok(vec![self.arc_layer_mesh(layer)?]);
        }
        // Section consumers already union contours of elevation regions for
        // straight hosts. Keep that contract on arcs: a plane on a radial
        // jamb then cuts individual closed regions instead of asking the
        // section extractor to union a coplanar reveal with a transverse cut.
        // The render mesh above still has one boundary without internal caps.
        self.elevation_regions()?
            .into_iter()
            .map(|region| {
                crate::openings::radial_regions_mesh(
                    &self.parameters,
                    self.elevation,
                    &[region],
                    layer.min,
                    layer.max,
                )
            })
            .collect()
    }
    pub(super) fn arc_quantities(&self) -> Result<Vec<LayerQuantity>> {
        self.validate_arc()?;
        let elevation_area: f64 = self
            .elevation_regions()?
            .iter()
            .map(|r| crate::floors::signed_area(r).abs())
            .sum();
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
                // dV = (1 - sign*y/radius) ds dy dz. The Jacobian does
                // not depend on station or elevation, so this is exact for
                // every authored polygonal cut, including sloping heads.
                let radial_factor = (r0 + r1) * (layer.max - layer.min) / (2. * radius);
                let volume_m3 = elevation_area * radial_factor;
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
    pub(super) fn arc_plan_span(
        &self,
        layer: &os_model::ResolvedWallLayer,
        a: f64,
        b: f64,
        role: PlanRole,
        basis: HorizontalBasis,
        crop: Option<PlanCrop>,
    ) -> Result<Vec<PlanFootprint>> {
        let count = self.validate_arc()?;
        // Keep the common display grid (also used by NativeWall::seams),
        // inserting exact jamb/profile stations at either end of the span.
        let step = self.parameters.length() / count as f64;
        let mut stations = vec![a];
        stations.extend(
            (1..count)
                .map(|i| i as f64 * step)
                .filter(|s| *s > a && *s < b),
        );
        stations.push(b);
        let mut parts = Vec::new();
        for pair in stations.windows(2) {
            let vertices = self
                .arc_quad(layer, pair[0], pair[1])
                .into_iter()
                .map(|p| basis.world_to_plane(p))
                .collect::<Result<Vec<_>>>()?;
            if let Some(fp) = PlanFootprint::from_convex(role, vertices, crop)? {
                parts.push(fp);
            }
        }
        Ok(parts)
    }
}
