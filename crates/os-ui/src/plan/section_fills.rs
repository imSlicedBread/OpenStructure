//! Independently tessellated solids in deterministic section compositing order.
use super::*;
use os_geometry::{Mesh, SurfaceIdentity, section::CutContour};
use os_render::plan::PlanSectionFill;

#[derive(Default)]
pub(super) struct SectionFills {
    groups: BTreeMap<(u8, Id), Vec<PlanSectionFill>>,
    triangle_count: usize,
    pub(super) projection: Option<os_geometry::elevation::FacadeVolume>,
    meshes: Vec<(Id, Mesh)>,
    mesh_triangles: usize,
    mesh_vertices: usize,
}

impl SectionFills {
    pub(super) fn new(projection: Option<os_geometry::elevation::FacadeVolume>) -> Self {
        Self {
            projection,
            ..Self::default()
        }
    }
    /// Group order: structure, curtain components, opening fillings, railings.
    /// Ring parity is evaluated within this mesh only, before line seam removal.
    pub(super) fn capture(
        &mut self,
        group: u8,
        entity: Id,
        surface: os_geometry::SurfaceIdentity,
        mesh: &Mesh,
        plane: os_geometry::section::VerticalSectionPlane,
    ) -> Result<Vec<CutContour>> {
        self.record_projection(entity, surface, mesh)?;
        let contours = os_geometry::section::vertical_section(mesh, plane)?;
        if !contours.is_empty() && section_plane_crosses_component(mesh, plane)? {
            let remaining = os_geometry::section::MAX_SECTION_FILL_TRIANGLES
                .checked_sub(self.triangle_count)
                .ok_or_else(|| os_core::Error::Invalid("section fill budget exhausted".into()))?;
            let triangles = os_geometry::section::tessellate_contours(&contours, remaining)?;
            self.triangle_count = self
                .triangle_count
                .checked_add(triangles.len())
                .ok_or_else(|| {
                    os_core::Error::Invalid("section fill triangle count overflow".into())
                })?;
            let components = self.groups.entry((group, entity)).or_default();
            components.push(PlanSectionFill {
                entity,
                component: u32::try_from(components.len()).map_err(|_| {
                    os_core::Error::Invalid("section fill component count overflow".into())
                })?,
                surface,
                triangles,
            });
        }
        Ok(contours)
    }

    pub(super) fn record_projection(
        &mut self,
        entity: Id,
        surface: SurfaceIdentity,
        mesh: &Mesh,
    ) -> Result<()> {
        if let Some(volume) = self.projection {
            // Reject certified bounds before allocation or aggregate charging.
            // Depth is positive away from the viewer, unlike solver Z.
            let sign = if volume.left_side { -1.0 } else { 1.0 };
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for v in &mesh.vertices {
                let x = v.x - volume.origin.x;
                let y = v.y - volume.origin.y;
                let local = [
                    x * volume.axis.x + y * volume.axis.y,
                    v.z,
                    sign * (-x * volume.axis.y + y * volume.axis.x),
                ];
                for axis in 0..3 {
                    min[axis] = min[axis].min(local[axis]);
                    max[axis] = max[axis].max(local[axis]);
                }
            }
            if [
                (volume.bounds.min_station, volume.bounds.max_station),
                (volume.bounds.bottom_elevation, volume.bounds.top_elevation),
                (volume.near, volume.far),
            ]
            .into_iter()
            .enumerate()
            .any(|(axis, (lo, hi))| max[axis] < lo - 1e-8 || min[axis] > hi + 1e-8)
            {
                return Ok(());
            }
            self.mesh_triangles = self.mesh_triangles.saturating_add(mesh.triangles.len());
            self.mesh_vertices = self.mesh_vertices.saturating_add(mesh.vertices.len());
            ensure(
                self.mesh_vertices <= 65_536,
                "Section incomplete: projection vertex budget exceeded",
            )?;
            ensure(
                self.mesh_triangles <= os_geometry::elevation::MAX_ELEVATION_TRIANGLES,
                "Section incomplete: projection triangle budget exceeded",
            )?;
            let mut source = mesh.clone();
            if source.surfaces.is_empty() {
                source.surfaces = vec![surface; source.triangles.len()];
            }
            self.meshes.push((entity, source));
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Vec<PlanSectionFill> {
        self.groups.into_values().flatten().collect()
    }
    pub(super) fn surface(&self, entity: Id) -> Option<SurfaceIdentity> {
        self.groups
            .iter()
            .find(|((_, id), _)| *id == entity)
            .and_then(|(_, fills)| fills.first())
            .map(|f| f.surface)
    }

    pub(super) fn projected(&self) -> Result<Vec<os_geometry::elevation::ElevationSegment>> {
        let Some(volume) = self.projection else {
            return Ok(Vec::new());
        };
        let source = os_geometry::elevation::project_facade(&self.meshes, volume)?;
        let masks = self
            .groups
            .values()
            .flatten()
            .flat_map(|f| f.triangles.iter())
            .copied()
            .collect::<Vec<_>>();
        os_geometry::section::mask_projected_segments(source, &masks)
    }
}

pub(super) fn material_surface(material: Option<Id>) -> SurfaceIdentity {
    SurfaceIdentity {
        layer: None,
        material,
    }
}
