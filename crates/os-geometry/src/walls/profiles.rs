//! Exact elevation partitioning. Every strip is bounded by authored vertices;
//! no sampling, staircase approximation or boolean kernel is involved.
use super::*;
use crate::plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, PlanRole};

impl NativeWall {
    pub fn has_profile_cuts(&self) -> bool {
        self.openings.iter().any(|o| !o.family.rectangular_cut())
    }

    /// Convex wall-material regions in (host station, elevation above level).
    fn elevation_regions(&self) -> Result<Vec<Vec<Point2>>> {
        super::validate_station_limits(&self.parameters, self.stations)?;
        ensure(self.elevation.is_finite(), "invalid wall elevation")?;
        let mut openings: Vec<_> = self.openings.iter().collect();
        openings.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        let mut cursor = self.stations.0;
        let mut rings = Vec::new();
        let mut xs = vec![self.stations.0, self.stations.1];
        for o in openings {
            o.validate_host(&self.parameters)?;
            ensure(
                o.offset >= cursor + 0.001 && o.offset + o.width <= self.stations.1 - 0.001,
                "opening overlaps another opening or trimmed wall end",
            )?;
            cursor = o.offset + o.width;
            let ring: Vec<_> = o
                .family
                .cut_profile
                .iter()
                .map(|p| Point2::new(o.offset + p.x * o.width, o.sill + p.y * o.height))
                .collect();
            xs.extend(ring.iter().map(|p| p.x));
            rings.push(ring);
        }
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        let mut regions = Vec::new();
        for pair in xs.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            let mid = left + (right - left) * 0.5;
            let mut edges = Vec::new();
            for ring in &rings {
                for (a, b) in ring
                    .iter()
                    .zip(ring.iter().cycle().skip(1))
                    .take(ring.len())
                {
                    if a.x.min(b.x) < mid && mid < a.x.max(b.x) {
                        let y = |x| a.y + (x - a.x) * (b.y - a.y) / (b.x - a.x);
                        edges.push((y(mid), y(left), y(right)));
                    }
                }
            }
            edges.sort_by(|a, b| a.0.total_cmp(&b.0));
            ensure(edges.len() % 2 == 0, "unpaired opening cut intersections")?;
            let mut bottom = (0., 0.);
            for cut in edges.chunks_exact(2) {
                push_region(&mut regions, left, right, bottom, (cut[0].1, cut[0].2))?;
                bottom = (cut[1].1, cut[1].2);
            }
            push_region(
                &mut regions,
                left,
                right,
                bottom,
                (self.parameters.height, self.parameters.height),
            )?;
        }
        Ok(regions)
    }

    fn region_mesh(&self, ring: &[Point2], layer: &os_model::ResolvedWallLayer) -> Result<Mesh> {
        let mut mesh = crate::floors::extrude_floor(ring, layer.max, layer.max - layer.min)?;
        for p in &mut mesh.vertices {
            let xy = crate::openings::world(&self.parameters, p.x, p.z);
            *p = Vec3::new(xy.x, xy.y, self.elevation + p.y);
        }
        // Elevation/depth axis permutation reverses handedness.
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
        mesh.validate()?;
        Ok(mesh)
    }

    /// Closed convex regions for sectioning; callers reduce shared contours.
    pub fn layer_mesh_regions(&self) -> Result<Vec<(os_model::ResolvedWallLayer, Vec<Mesh>)>> {
        if !self.has_profile_cuts() {
            return self
                .layer_cells()?
                .into_iter()
                .map(|(layer, cells)| {
                    Ok((
                        layer,
                        cells
                            .iter()
                            .map(|c| crate::PrismKernel.tessellate(c))
                            .collect::<Result<_>>()?,
                    ))
                })
                .collect();
        }
        let regions = self.elevation_regions()?;
        self.layers
            .iter()
            .map(|layer| {
                Ok((
                    layer.clone(),
                    regions
                        .iter()
                        .map(|r| self.region_mesh(r, layer))
                        .collect::<Result<_>>()?,
                ))
            })
            .collect()
    }

    pub(super) fn profile_mesh(&self) -> Result<Mesh> {
        let regions = self.elevation_regions()?;
        let mut result = Mesh::default();
        for layer in &self.layers {
            for ring in &regions {
                // Join contacts cannot intersect an opening envelope (model
                // validation). Rectangular end regions reuse seam suppression.
                let mesh = if ring.len() == 4 && ring[0].y == ring[1].y && ring[2].y == ring[3].y {
                    let mut cell = wall_solid(&self.parameters, self.elevation + ring[0].y)?;
                    cell.profile.vertices = vec![
                        Point2::new(ring[0].x, layer.min),
                        Point2::new(ring[1].x, layer.min),
                        Point2::new(ring[1].x, layer.max),
                        Point2::new(ring[0].x, layer.max),
                    ];
                    cell.height = ring[2].y - ring[0].y;
                    self.mesh_solids(vec![cell])?
                } else {
                    self.region_mesh(ring, layer)?
                };
                append_mesh(&mut result, mesh);
            }
        }
        result.validate()?;
        Ok(result)
    }

    pub(super) fn profile_quantities(&self) -> Result<Vec<LayerQuantity>> {
        let area: f64 = self
            .elevation_regions()?
            .iter()
            .map(|r| crate::floors::signed_area(r).abs())
            .sum();
        self.layers
            .iter()
            .map(|layer| {
                let volume_m3 = area * (layer.max - layer.min);
                let mass_kg = layer.density_kg_m3.map(|d| d * volume_m3);
                ensure(
                    volume_m3.is_finite() && mass_kg.is_none_or(f64::is_finite),
                    "layer quantity overflow",
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

    /// Horizontal cuts and downward projections of exact elevation regions.
    /// Clipping a convex elevation polygon at range heights yields an exact
    /// station interval, extruded through the resolved layer width in plan.
    pub fn layer_plan_footprints(
        &self,
        range: PlanRange,
        basis: HorizontalBasis,
        crop: Option<PlanCrop>,
    ) -> Result<Vec<(os_model::ResolvedWallLayer, Vec<PlanFootprint>)>> {
        range.validate()?;
        if !self.has_profile_cuts() {
            return self
                .layer_cells()?
                .into_iter()
                .map(|(layer, cells)| {
                    let footprints = cells
                        .iter()
                        .map(|c| crate::plan::rectangular_plan(c, range, basis, crop))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect();
                    Ok((layer, footprints))
                })
                .collect();
        }
        let regions = self.elevation_regions()?;
        self.layers
            .iter()
            .map(|layer| {
                let mut footprints = Vec::new();
                let mut intervals: [Vec<(f64, f64)>; 3] = Default::default();
                for ring in &regions {
                    for (lo, hi, role) in [
                        (range.depth, range.bottom, PlanRole::Depth),
                        (range.bottom, range.cut, PlanRole::Projected),
                        (range.cut, range.cut, PlanRole::Cut),
                    ] {
                        let lo = lo - self.elevation;
                        let hi = hi - self.elevation;
                        let top = ring.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
                        let bottom = ring.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
                        if top <= lo + 1e-9 || bottom >= range.top - self.elevation - 1e-9 {
                            continue;
                        }
                        let clipped = clip_height(&clip_height(ring, lo, true), hi, false);
                        if clipped.is_empty() {
                            continue;
                        }
                        let a = clipped.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
                        let b = clipped
                            .iter()
                            .map(|p| p.x)
                            .fold(f64::NEG_INFINITY, f64::max);
                        if b - a <= 1e-9 {
                            continue;
                        }
                        intervals[match role {
                            PlanRole::Depth => 0,
                            PlanRole::Projected => 1,
                            PlanRole::Cut => 2,
                        }]
                        .push((a, b));
                    }
                }
                for (role, mut spans) in [PlanRole::Depth, PlanRole::Projected, PlanRole::Cut]
                    .into_iter()
                    .zip(intervals)
                {
                    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let mut merged: Vec<(f64, f64)> = Vec::new();
                    for (a, b) in spans {
                        if let Some(last) = merged.last_mut()
                            && a <= last.1 + 1e-9
                        {
                            last.1 = last.1.max(b);
                        } else {
                            merged.push((a, b));
                        }
                    }
                    for (a, b) in merged {
                        let vertices = [
                            (a, layer.min),
                            (b, layer.min),
                            (b, layer.max),
                            (a, layer.max),
                        ]
                        .into_iter()
                        .map(|(x, y)| {
                            basis.world_to_plane(crate::openings::world(&self.parameters, x, y))
                        })
                        .collect::<Result<Vec<_>>>()?;
                        if let Some(fp) = PlanFootprint::from_convex(role, vertices, crop)? {
                            footprints.push(fp);
                        }
                    }
                }
                Ok((layer.clone(), footprints))
            })
            .collect()
    }
}

fn push_region(
    regions: &mut Vec<Vec<Point2>>,
    a: f64,
    b: f64,
    bottom: (f64, f64),
    top: (f64, f64),
) -> Result<()> {
    let mut ring = vec![
        Point2::new(a, bottom.0),
        Point2::new(b, bottom.1),
        Point2::new(b, top.1),
        Point2::new(a, top.0),
    ];
    ring.dedup_by(|a, b| a.distance(*b) < 1e-10);
    if ring.len() > 1 && ring[0].distance(*ring.last().unwrap()) < 1e-10 {
        ring.pop();
    }
    if ring.len() < 3 {
        return Ok(());
    }
    ensure(
        crate::floors::signed_area(&ring) > 0.,
        "invalid wall elevation region",
    )?;
    regions.push(ring);
    Ok(())
}

fn clip_height(input: &[Point2], height: f64, above: bool) -> Vec<Point2> {
    let mut out = Vec::new();
    for (a, b) in input
        .iter()
        .zip(input.iter().cycle().skip(1))
        .take(input.len())
    {
        let inside = |p: &Point2| if above { p.y >= height } else { p.y <= height };
        if inside(a) {
            out.push(*a);
        }
        if inside(a) != inside(b) {
            out.push(Point2::new(
                a.x + (height - a.y) * (b.x - a.x) / (b.y - a.y),
                height,
            ));
        }
    }
    out
}

fn append_mesh(result: &mut Mesh, part: Mesh) {
    let n = result.vertices.len() as u32;
    result
        .triangles
        .extend(part.triangles.iter().map(|t| t.map(|i| i + n)));
    result.vertices.extend(part.vertices);
}
