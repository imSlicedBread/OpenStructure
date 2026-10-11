//! Frozen opening fillings, cut independently from their host and one another.
use super::*;
use os_geometry::{SurfaceIdentity, section::VerticalSectionPlane};
use os_model::{Model, ResolvedOpening, WallParams};
use os_render::plan::PlanLine;

pub(super) struct SectionOpening {
    entity: Id,
    parameters: ResolvedOpening,
    host: WallParams,
    elevation: f64,
}

impl SectionOpening {
    pub(super) fn capture(model: &Model, building: Id) -> Result<Vec<Self>> {
        let mut sources = Vec::new();
        for opening in model.openings.values() {
            let host = model.resolve_wall(opening.parameters.host)?.parameters;
            let level = &model.levels[&host.level].parameters;
            if level.building != building {
                continue;
            }
            let parameters = model.resolve_opening(&opening.parameters)?;
            parameters.validate_host(&host)?;
            sources.push(Self {
                entity: opening.id(),
                parameters,
                host,
                elevation: level.elevation,
            });
            ensure(
                sources.len() <= MAX_PLAN_ELEMENTS,
                "section opening source limit exceeded",
            )?;
        }
        Ok(sources)
    }
}

pub(super) fn append_contours(
    openings: Vec<SectionOpening>,
    plane: VerticalSectionPlane,
    lines: &mut BTreeMap<Id, Vec<PlanLine>>,
    surfaces: &mut BTreeMap<(Id, u32), SurfaceIdentity>,
    raw_segment_count: &mut usize,
    fills: &mut SectionFills,
) -> Result<()> {
    let mut component_count = 0usize;
    for opening in openings {
        let solids = os_geometry::openings::component_solids(
            &opening.parameters,
            &opening.host,
            opening.elevation,
        )?;
        component_count = component_count.saturating_add(solids.len());
        ensure(
            component_count <= MAX_PLAN_ELEMENTS,
            "section opening component limit exceeded",
        )?;
        ensure(
            !lines.contains_key(&opening.entity),
            "duplicate section source identity",
        )?;
        let output = lines.entry(opening.entity).or_default();
        for solid in solids {
            // A supporting plane has no area of cut. This matches other native
            // components and avoids treating a tangent as an opening filling.
            if !section_plane_crosses_component(&solid, plane)? {
                fills.record_projection(
                    opening.entity,
                    solid.surfaces.first().copied().unwrap_or_default(),
                    &solid,
                )?;
                continue;
            }
            let mut edges = Vec::new();
            add_section_contours(
                &mut edges,
                fills.capture(
                    2,
                    opening.entity,
                    solid.surfaces.first().copied().unwrap_or_default(),
                    &solid,
                    plane,
                )?,
                raw_segment_count,
            )?;
            let edges = reduce_section_edges(edges)?;
            ensure(
                edges.len() <= 256,
                "opening component exceeds cut segment limit",
            )?;
            let surface = solid.surfaces.first().copied().unwrap_or_default();
            for segment in edges {
                let feature = u32::try_from(output.len()).map_err(|_| {
                    os_core::Error::Invalid("opening section feature overflow".into())
                })?;
                output.push(PlanLine {
                    entity: opening.entity,
                    feature,
                    start: segment.start,
                    end: segment.end,
                    role: PlanRole::Cut,
                });
                surfaces.insert((opening.entity, feature), surface);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "opening_section_tests.rs"]
mod tests;
