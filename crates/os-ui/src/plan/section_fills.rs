//! Independently tessellated solids in deterministic section compositing order.
use super::*;
use os_geometry::{Mesh, SurfaceIdentity, section::CutContour};
use os_render::plan::PlanSectionFill;

#[derive(Default)]
pub(super) struct SectionFills {
    groups: BTreeMap<(u8, Id), Vec<PlanSectionFill>>,
    triangle_count: usize,
}

impl SectionFills {
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

    pub(super) fn finish(self) -> Vec<PlanSectionFill> {
        self.groups.into_values().flatten().collect()
    }
}

pub(super) fn material_surface(material: Option<Id>) -> SurfaceIdentity {
    SurfaceIdentity {
        layer: None,
        material,
    }
}
