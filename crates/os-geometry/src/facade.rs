//! Frozen multi-object facade inputs and one bounded cross-object visibility solve.
use crate::{
    Mesh, SurfaceIdentity,
    elevation::{
        ElevationBounds, ElevationSegment, ElevationSnapshot, FacadeVolume,
        MAX_ELEVATION_TRIANGLES, project_facade,
    },
};
use os_core::{Id, Point2, Result, ensure};
use os_model::{ElevationSide, FacadeViewSettings, FloorParams, Model, RoofParams};

pub struct FacadeSnapshot {
    volume: FacadeVolume,
    walls: Vec<ElevationSnapshot>,
    floors: Vec<(Id, FloorParams, f64)>,
    roofs: Vec<(Id, RoofParams, f64)>,
}
impl FacadeSnapshot {
    pub fn capture(model: &Model, marker_level: Id, settings: FacadeViewSettings) -> Result<Self> {
        settings.validate()?;
        let level = model
            .levels
            .get(&marker_level)
            .ok_or_else(|| os_core::Error::Invalid("facade marker level missing".into()))?;
        let building = level.parameters.building;
        let volume = FacadeVolume {
            origin: settings.origin,
            axis: settings.axis,
            left_side: settings.side == ElevationSide::Left,
            bounds: ElevationBounds {
                min_station: settings.min_station,
                max_station: settings.max_station,
                bottom_elevation: settings.bottom_elevation,
                top_elevation: settings.top_elevation,
            },
            near: settings.near_depth,
            far: settings.far_depth,
        };
        volume.validate()?;
        let in_building = |id: Id| {
            model
                .levels
                .get(&id)
                .is_some_and(|l| l.parameters.building == building)
        };
        // These categories have no certified facade volume bounds. Fail
        // conservatively for this building, rather than silently omit them.
        for (present, category) in [
            (!model.extensions.is_empty(), "external plugin geometry"),
            (
                model
                    .curtain_systems
                    .values()
                    .any(|e| in_building(e.parameters.level)),
                "curtain systems",
            ),
            (
                model
                    .columns
                    .values()
                    .any(|e| in_building(e.parameters.level)),
                "columns",
            ),
            (
                model
                    .casework
                    .values()
                    .any(|e| in_building(e.parameters.level)),
                "casework",
            ),
            (
                model
                    .ceilings
                    .values()
                    .any(|e| in_building(e.parameters.level)),
                "ceilings",
            ),
            (
                model
                    .stairs
                    .values()
                    .any(|e| in_building(e.parameters.lower_level)),
                "stairs/railings",
            ),
            (
                model
                    .ramps
                    .values()
                    .any(|e| in_building(e.parameters.lower_level)),
                "ramps",
            ),
        ] {
            ensure(
                !present,
                format!(
                    "Facade incomplete: unsupported {category}; category has no certified facade bounds"
                ),
            )?;
        }
        let mut snapshot = Self {
            volume,
            walls: Vec::new(),
            floors: Vec::new(),
            roofs: Vec::new(),
        };
        let mut budget = 0usize;
        for wall in model
            .walls
            .values()
            .filter(|w| in_building(w.parameters.level))
        {
            let p = model.resolve_wall(wall.id())?.parameters;
            ensure(
                p.path.is_straight(),
                format!(
                    "Facade incomplete: curved wall {} is unsupported",
                    wall.id()
                ),
            )?;
            let z = model.levels[&p.level].parameters.elevation;
            let pad = model
                .openings
                .values()
                .filter(|o| o.parameters.host == wall.id())
                .map(|o| {
                    model.resolve_opening(&o.parameters).map(|o| {
                        o.width
                            + o.family
                                .extrusions
                                .iter()
                                .map(|s| s.normal_offset.abs() + s.depth / 2.)
                                .fold(0., f64::max)
                    })
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .fold(p.thickness, f64::max);
            let ring = [
                Point2::new(
                    p.start().x.min(p.end().x) - pad,
                    p.start().y.min(p.end().y) - pad,
                ),
                Point2::new(
                    p.start().x.max(p.end().x) + pad,
                    p.start().y.min(p.end().y) - pad,
                ),
                Point2::new(
                    p.start().x.max(p.end().x) + pad,
                    p.start().y.max(p.end().y) + pad,
                ),
                Point2::new(
                    p.start().x.min(p.end().x) - pad,
                    p.start().y.max(p.end().y) + pad,
                ),
            ];
            if !intersects(volume, &ring, z, z + p.height) {
                continue;
            }
            let assembly = ElevationSnapshot::capture(model, wall.id(), volume.left_side)?;
            budget = budget.saturating_add(assembly.triangle_bound());
            check_budget(budget)?;
            snapshot.walls.push(assembly);
        }
        for floor in model
            .floors
            .values()
            .filter(|f| in_building(f.parameters.level))
        {
            let p = &floor.parameters;
            let z = model.levels[&p.level].parameters.elevation + p.top_offset;
            if !intersects(volume, &p.boundary, z - p.thickness, z) {
                continue;
            }
            budget = budget.saturating_add(ring_bound(&p.boundary, &p.holes));
            check_budget(budget)?;
            snapshot.floors.push((floor.id(), p.clone(), z));
        }
        for roof in model
            .roofs
            .values()
            .filter(|r| in_building(r.parameters.level))
        {
            let p = &roof.parameters;
            let z = model.levels[&p.level].parameters.elevation;
            let top = p
                .boundary
                .iter()
                .map(|pnt| p.top_at(*pnt, z))
                .fold(f64::NEG_INFINITY, f64::max);
            let bottom = p
                .boundary
                .iter()
                .map(|pnt| p.top_at(*pnt, z) - p.thickness)
                .fold(f64::INFINITY, f64::min);
            if !intersects(volume, &p.boundary, bottom, top) {
                continue;
            }
            budget = budget.saturating_add(ring_bound(&p.boundary, &p.holes));
            check_budget(budget)?;
            snapshot.roofs.push((roof.id(), p.clone(), z));
        }
        Ok(snapshot)
    }
    pub fn derive(self) -> Result<Vec<ElevationSegment>> {
        let mut meshes: Vec<(Id, Mesh)> = Vec::new();
        for wall in self.walls {
            meshes.extend(wall.meshes()?);
            check_meshes(&meshes)?;
        }
        for (id, p, z) in self.floors {
            let mut mesh =
                crate::floor_holes::extrude_floor_rings(&p.boundary, &p.holes, z, p.thickness)?;
            mesh.surfaces.resize(
                mesh.triangles.len(),
                SurfaceIdentity {
                    layer: None,
                    material: p.material,
                },
            );
            meshes.push((id, mesh));
            check_meshes(&meshes)?;
        }
        for (id, p, z) in self.roofs {
            meshes.push((id, crate::roofs::roof_mesh(&p, z)?));
            check_meshes(&meshes)?;
        }
        project_facade(&meshes, self.volume)
    }
}
fn check_budget(n: usize) -> Result<()> {
    ensure(
        n <= MAX_ELEVATION_TRIANGLES,
        "facade aggregate triangle preflight exceeded; reduce crop/depth or model complexity",
    )
}
fn check_meshes(meshes: &[(Id, Mesh)]) -> Result<()> {
    check_budget(meshes.iter().map(|(_, m)| m.triangles.len()).sum())
}
fn ring_bound(boundary: &[Point2], holes: &[Vec<Point2>]) -> usize {
    (boundary.len() + holes.iter().map(Vec::len).sum::<usize>() + holes.len() * 2) * 4
}
fn intersects(v: FacadeVolume, ring: &[Point2], bottom: f64, top: f64) -> bool {
    if top < v.bounds.bottom_elevation || bottom > v.bounds.top_elevation {
        return false;
    }
    let sign = if v.left_side { -1. } else { 1. };
    let points: Vec<_> = ring
        .iter()
        .map(|p| {
            let (x, y) = (p.x - v.origin.x, p.y - v.origin.y);
            (
                x * v.axis.x + y * v.axis.y,
                sign * (-x * v.axis.y + y * v.axis.x),
            )
        })
        .collect();
    !points.iter().all(|p| p.0 < v.bounds.min_station)
        && !points.iter().all(|p| p.0 > v.bounds.max_station)
        && !points.iter().all(|p| p.1 < v.near)
        && !points.iter().all(|p| p.1 > v.far)
}

#[cfg(test)]
mod tests;
