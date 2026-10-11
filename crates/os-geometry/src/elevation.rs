//! Orthographic hidden-line elevations of one native straight-wall assembly.
//!
//! X always follows the current directed host start -> end; Y is absolute Z.
//! Left observes from the positive host normal, right from the negative normal.
//! Opaque triangle surfaces occlude edges (including glazing). No tessellation
//! diagonals or coplanar cell seams are emitted. Geometry is captured for worker
//! use; recapture after model edits. Curved hosts are explicitly unsupported.
use crate::{Mesh, SurfaceIdentity, Vec3, openings, walls::NativeWall};
use os_core::{Error, Id, Point2, Result, ensure};
use os_model::{Model, ResolvedOpening};
use std::collections::BTreeSet;

pub const ELEVATION_TOLERANCE: f64 = 1e-8;
pub const MAX_ELEVATION_TRIANGLES: usize = 4096;
pub const MAX_ELEVATION_SEGMENTS: usize = 4096;
pub const MAX_ELEVATION_COORDINATE: f64 = 1e6;
const MAX_WORK: usize = 40_000_000;
const T: f64 = ELEVATION_TOLERANCE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevationSegment {
    pub entity: Id,
    pub start: Point2,
    pub end: Point2,
    pub surface: SurfaceIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevationBounds {
    pub min_station: f64,
    pub max_station: f64,
    pub bottom_elevation: f64,
    pub top_elevation: f64,
}
impl ElevationBounds {
    pub fn validate(self) -> Result<()> {
        ensure(
            [self.bottom_elevation, self.top_elevation]
                .into_iter()
                .all(within)
                && [self.min_station, self.max_station]
                    .into_iter()
                    .all(|v| v.is_finite() && v.abs() <= 1e7)
                && self.max_station - self.min_station > T
                && self.top_elevation - self.bottom_elevation > T,
            "invalid elevation bounds",
        )
    }
}

/// Frozen native assembly for the existing background view worker.
#[derive(Clone, Debug)]
pub struct ElevationSnapshot {
    wall: NativeWall,
    openings: Vec<(Id, ResolvedOpening)>,
    left_side: bool,
}
pub type WallElevation = ElevationSnapshot;

impl ElevationSnapshot {
    pub fn capture(model: &Model, host: Id, left_side: bool) -> Result<Self> {
        Self::from_model(model, host, left_side)
    }
    pub fn from_model(model: &Model, host: Id, left_side: bool) -> Result<Self> {
        let host_wall = model
            .walls
            .get(&host)
            .ok_or_else(|| Error::Invalid("elevation host wall missing".into()))?;
        if !host_wall.parameters.path.is_straight() {
            return Err(Error::Unsupported(
                "curved-host elevations are unsupported".into(),
            ));
        }
        let instances: Vec<_> = model
            .openings
            .values()
            .filter(|o| o.parameters.host == host)
            .take(33)
            .collect();
        ensure(instances.len() <= 32, "elevation opening limit exceeded")?;
        let wall = NativeWall::from_model(model, host)?;
        let openings: Vec<_> = instances
            .into_iter()
            .map(|o| Ok((o.id(), model.resolve_opening(&o.parameters)?)))
            .collect::<Result<_>>()?;
        for material in wall.layers.iter().filter_map(|l| l.material).chain(
            openings
                .iter()
                .flat_map(|(_, o)| {
                    [
                        o.family.panel_material,
                        o.family.frame_material,
                        o.family.side_lite.as_ref().and_then(|l| l.material),
                    ]
                })
                .flatten(),
        ) {
            ensure(
                model.materials.contains_key(&material),
                "elevation material missing",
            )?;
        }
        let result = Self {
            wall,
            openings,
            left_side,
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<()> {
        self.wall.parameters.validate()?;
        ensure(
            self.wall.parameters.path.is_straight(),
            "elevation requires a straight host",
        )?;
        let w = &self.wall.parameters;
        ensure(
            [
                w.start().x,
                w.start().y,
                w.end().x,
                w.end().y,
                self.wall.elevation,
                self.wall.elevation + w.height,
            ]
            .into_iter()
            .all(within),
            "elevation host outside coordinate envelope",
        )?;
        let profile_vertices: usize = self
            .openings
            .iter()
            .map(|(_, o)| o.family.cut_profile.len() + o.family.profile.len())
            .sum();
        ensure(
            self.openings.len() <= 32 && profile_vertices <= 512 && self.wall.layers.len() <= 32,
            "elevation assembly limit exceeded",
        )?;
        // Count the native wall's station-strip cells before allocating its
        // meshes. A strip contains one wall region plus one per void span.
        let mut xs = vec![self.wall.stations.0, self.wall.stations.1];
        for (_, o) in &self.openings {
            o.validate_host(w)?;
            xs.extend(
                o.family
                    .cut_profile
                    .iter()
                    .map(|p| o.offset + p.x * o.width),
            );
        }
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        let mut cells = 0usize;
        for pair in xs.windows(2) {
            let mid = (pair[0] + pair[1]) * 0.5;
            let mut hits = 0;
            for (_, o) in &self.openings {
                let ring = &o.family.cut_profile;
                for (a, b) in ring
                    .iter()
                    .zip(ring.iter().cycle().skip(1))
                    .take(ring.len())
                {
                    let a = o.offset + a.x * o.width;
                    let b = o.offset + b.x * o.width;
                    hits += usize::from(a.min(b) < mid && mid < a.max(b));
                }
            }
            cells += 1 + hits / 2;
        }
        ensure(
            cells
                .saturating_mul(self.wall.layers.len())
                .saturating_mul(12)
                <= MAX_ELEVATION_TRIANGLES,
            "elevation wall mesh budget exceeded",
        )
    }

    /// Return the whole assembly, with no synthetic border around a crop.
    pub fn derive(self) -> Result<Vec<ElevationSegment>> {
        self.derive_lines()
    }

    fn derive_lines(&self) -> Result<Vec<ElevationSegment>> {
        project(
            &self.meshes()?,
            self.wall.parameters.start(),
            self.wall.parameters.path.tangent(0.),
            self.left_side,
        )
    }

    pub(crate) fn meshes(&self) -> Result<Vec<(Id, Mesh)>> {
        self.validate()?;
        let mut meshes = vec![(self.wall.entity, self.wall.mesh()?)];
        for (entity, opening) in &self.openings {
            for mesh in
                openings::component_solids(opening, &self.wall.parameters, self.wall.elevation)?
            {
                meshes.push((*entity, mesh));
                ensure(
                    meshes.iter().map(|(_, m)| m.triangles.len()).sum::<usize>()
                        <= MAX_ELEVATION_TRIANGLES,
                    "elevation triangle limit exceeded",
                )?;
            }
        }
        Ok(meshes)
    }

    /// Conservative allocation preflight, including all native opening parts.
    pub(crate) fn triangle_bound(&self) -> usize {
        let stations = 2 + self
            .openings
            .iter()
            .map(|(_, o)| o.family.cut_profile.len())
            .sum::<usize>();
        let wall = stations
            .saturating_sub(1)
            .saturating_mul(1 + self.openings.len())
            .saturating_mul(self.wall.layers.len())
            .saturating_mul(12);
        let parts = self
            .openings
            .iter()
            .map(|(_, o)| {
                16 * (o.family.profile.len().max(4) * 4 + 8)
                    + o.family
                        .extrusions
                        .iter()
                        .map(|s| s.profile.len() * 4 + 8)
                        .sum::<usize>()
            })
            .sum::<usize>();
        wall.saturating_add(parts)
    }

    /// Clip visible lines to station/absolute-elevation limits. Cropping does
    /// not create cut edges and cannot reveal geometry hidden by off-crop faces.
    pub fn derive_clipped(&self, bounds: ElevationBounds) -> Result<Vec<ElevationSegment>> {
        bounds.validate()?;
        Ok(self
            .derive_lines()?
            .into_iter()
            .filter_map(|mut s| {
                let mut interval = (0., 1.);
                for (a, b) in [
                    (s.start.x - bounds.min_station, s.end.x - bounds.min_station),
                    (bounds.max_station - s.start.x, bounds.max_station - s.end.x),
                    (
                        s.start.y - bounds.bottom_elevation,
                        s.end.y - bounds.bottom_elevation,
                    ),
                    (
                        bounds.top_elevation - s.start.y,
                        bounds.top_elevation - s.end.y,
                    ),
                ] {
                    if !clip_positive(&mut interval, a, b) {
                        return None;
                    }
                }
                let end = lerp(s.start, s.end, interval.1);
                s.start = lerp(s.start, s.end, interval.0);
                s.end = end;
                (s.start.distance(s.end) > T).then_some(s)
            })
            .collect())
    }
}

fn within(x: f64) -> bool {
    x.is_finite() && x.abs() <= MAX_ELEVATION_COORDINATE
}
fn sub(a: Point2, b: Point2) -> Point2 {
    Point2::new(a.x - b.x, a.y - b.y)
}
fn cross(a: Point2, b: Point2) -> f64 {
    a.x * b.y - a.y * b.x
}
fn lerp(a: Point2, b: Point2, t: f64) -> Point2 {
    Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
fn point(v: Vec3) -> Point2 {
    Point2::new(v.x, v.y)
}
fn compare(a: Point2, b: Point2) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y))
}

#[derive(Clone)]
struct Face {
    p: [Point2; 3],
    depth: [f64; 3],
    gradient: Point2,
    entity: Id,
    surface: SurfaceIdentity,
}
impl Face {
    fn at(&self, p: Point2) -> f64 {
        self.depth[0]
            + self.gradient.x * (p.x - self.p[0].x)
            + self.gradient.y * (p.y - self.p[0].y)
    }
    // Symbolic infinitesimal: at an exact edge, choose the triangle on the
    // requested side without sampling an arbitrary pixel/metre offset.
    fn covers_side(&self, p: Point2, normal: Point2) -> bool {
        (0..3).all(|i| {
            let a = self.p[i];
            let d = sub(self.p[(i + 1) % 3], a);
            let scale = d.x.hypot(d.y);
            let distance = cross(d, sub(p, a)) / scale;
            distance >= -T && (distance > T || cross(d, normal) >= -T * scale)
        })
    }
    fn interval(&self, a: Point2, b: Point2) -> Option<(f64, f64)> {
        let mut result = (0., 1.);
        for i in 0..3 {
            let p = self.p[i];
            let d = sub(self.p[(i + 1) % 3], p);
            let scale = d.x.hypot(d.y);
            if !clip_positive(
                &mut result,
                cross(d, sub(a, p)) / scale,
                cross(d, sub(b, p)) / scale,
            ) {
                return None;
            }
        }
        Some(result)
    }
}

fn clip_positive(interval: &mut (f64, f64), a: f64, b: f64) -> bool {
    if a < -T && b < -T {
        return false;
    }
    if a < 0. && b > 0. {
        interval.0 = interval.0.max(a / (a - b));
    }
    if a > 0. && b < 0. {
        interval.1 = interval.1.min(a / (a - b));
    }
    interval.1 >= interval.0
}

struct Work(usize);
impl Work {
    fn add(&mut self, n: usize) -> Result<()> {
        self.0 = self.0.saturating_add(n);
        ensure(
            self.0 <= MAX_WORK,
            "elevation visibility work limit exceeded",
        )
    }
}

fn project(
    meshes: &[(Id, Mesh)],
    origin: Point2,
    direction: Point2,
    left: bool,
) -> Result<Vec<ElevationSegment>> {
    project_window(meshes, origin, direction, left, None)
}

/// A world-oriented view volume. Depth is distance looking away from the
/// baseline into the scene; the viewer's side determines the fixed normal.
#[derive(Clone, Copy)]
pub struct FacadeVolume {
    pub origin: Point2,
    pub axis: Point2,
    pub left_side: bool,
    pub bounds: ElevationBounds,
    pub near: f64,
    pub far: f64,
}
impl FacadeVolume {
    pub fn validate(self) -> Result<()> {
        self.bounds.validate()?;
        ensure(
            self.origin.is_finite()
                && within(self.origin.x)
                && within(self.origin.y)
                && self.axis.is_finite()
                && (self.axis.x.hypot(self.axis.y) - 1.0).abs() <= T
                && within(self.near)
                && within(self.far)
                && self.near >= 0.0
                && self.far > self.near,
            "invalid facade world frame or near/far depths",
        )
    }
    fn accepts(self, p: Point2, depth: f64) -> bool {
        p.x >= self.bounds.min_station - T
            && p.x <= self.bounds.max_station + T
            && p.y >= self.bounds.bottom_elevation - T
            && p.y <= self.bounds.top_elevation + T
            && depth >= -self.far - T
            && depth <= -self.near + T
    }
    fn clip(self, a: Vec3, b: Vec3) -> Option<(f64, f64)> {
        let mut interval = (0., 1.);
        for (a, b) in [
            (a.x - self.bounds.min_station, b.x - self.bounds.min_station),
            (self.bounds.max_station - a.x, self.bounds.max_station - b.x),
            (
                a.y - self.bounds.bottom_elevation,
                b.y - self.bounds.bottom_elevation,
            ),
            (
                self.bounds.top_elevation - a.y,
                self.bounds.top_elevation - b.y,
            ),
            (a.z + self.far, b.z + self.far),
            (-self.near - a.z, -self.near - b.z),
        ] {
            if !clip_positive(&mut interval, a, b) {
                return None;
            }
        }
        Some(interval)
    }
}

/// One shared visibility solve, not concatenated per-object elevations.
/// Clips original mesh edges only; volume planes never invent cut contours.
pub fn project_facade(
    meshes: &[(Id, Mesh)],
    volume: FacadeVolume,
) -> Result<Vec<ElevationSegment>> {
    volume.validate()?;
    let mut lines = project_window(
        meshes,
        volume.origin,
        volume.axis,
        volume.left_side,
        Some(volume),
    )?;
    // The solve accepts boundary values within T. Canonicalize that numerical
    // fringe before strict downstream crop/picking so rotated boundary edges
    // cannot disappear. This never introduces a clip-plane edge.
    for line in &mut lines {
        for p in [&mut line.start, &mut line.end] {
            p.x =
                p.x.clamp(volume.bounds.min_station, volume.bounds.max_station);
            p.y =
                p.y.clamp(volume.bounds.bottom_elevation, volume.bounds.top_elevation);
        }
    }
    Ok(lines)
}

fn project_window(
    meshes: &[(Id, Mesh)],
    origin: Point2,
    direction: Point2,
    left: bool,
    window: Option<FacadeVolume>,
) -> Result<Vec<ElevationSegment>> {
    ensure(
        meshes.iter().map(|(_, m)| m.triangles.len()).sum::<usize>() <= MAX_ELEVATION_TRIANGLES,
        "elevation triangle limit exceeded",
    )?;
    let sign = if left { 1. } else { -1. };
    let mut faces = Vec::new();
    let mut edges = Vec::new();
    for (entity, mesh) in meshes {
        mesh.validate()?;
        ensure(
            mesh.vertices
                .iter()
                .all(|v| within(v.x) && within(v.y) && within(v.z)),
            "elevation mesh outside coordinate envelope",
        )?;
        let vertices: Vec<_> = mesh
            .vertices
            .iter()
            .map(|v| {
                let x = v.x - origin.x;
                let y = v.y - origin.y;
                Vec3::new(
                    x * direction.x + y * direction.y,
                    v.z,
                    sign * (-x * direction.y + y * direction.x),
                )
            })
            .collect();
        let mut used = BTreeSet::new();
        for (index, tri) in mesh.triangles.iter().enumerate() {
            let mut v = tri.map(|i| vertices[i as usize]);
            if let Some(w) = window {
                // Conservative spatial rejection. Partially intersecting faces
                // remain occluders; no new depth/crop-plane edges are generated.
                if [
                    (0, w.bounds.min_station, w.bounds.max_station),
                    (1, w.bounds.bottom_elevation, w.bounds.top_elevation),
                    (2, -w.far, -w.near),
                ]
                .into_iter()
                .any(|(axis, lo, hi)| {
                    let values = v.map(|p| [p.x, p.y, p.z][axis]);
                    values.iter().all(|x| *x < lo - T) || values.iter().all(|x| *x > hi + T)
                }) {
                    continue;
                }
            }
            let area = cross(sub(point(v[1]), point(v[0])), sub(point(v[2]), point(v[0])));
            // Edge-on faces occlude no area; their boundary also belongs to
            // neighboring non-edge-on faces of these closed native solids.
            let longest = point(v[0])
                .distance(point(v[1]))
                .max(point(v[1]).distance(point(v[2])))
                .max(point(v[2]).distance(point(v[0])));
            if area.abs() <= T * longest {
                continue;
            }
            if area < 0. {
                v.swap(1, 2);
            }
            let a = sub(point(v[1]), point(v[0]));
            let b = sub(point(v[2]), point(v[0]));
            let dz1 = v[1].z - v[0].z;
            let dz2 = v[2].z - v[0].z;
            let determinant = cross(a, b);
            faces.push(Face {
                p: v.map(point),
                depth: v.map(|v| v.z),
                gradient: Point2::new(
                    (dz1 * b.y - dz2 * a.y) / determinant,
                    (a.x * dz2 - b.x * dz1) / determinant,
                ),
                entity: *entity,
                surface: mesh.surfaces.get(index).copied().unwrap_or_default(),
            });
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let key = (a.min(b), a.max(b));
                if used.insert(key)
                    && point(vertices[a as usize]).distance(point(vertices[b as usize])) > T
                {
                    let (a, b) = (vertices[a as usize], vertices[b as usize]);
                    let interval = window.map_or(Some((0., 1.)), |w| w.clip(a, b));
                    if let Some((lo, hi)) = interval {
                        let at = |t| {
                            Vec3::new(
                                a.x + (b.x - a.x) * t,
                                a.y + (b.y - a.y) * t,
                                a.z + (b.z - a.z) * t,
                            )
                        };
                        if (hi - lo) * point(a).distance(point(b)) > T {
                            edges.push((at(lo), at(hi)));
                        }
                    }
                }
            }
        }
    }
    ensure(
        edges.len() <= MAX_ELEVATION_TRIANGLES * 3,
        "facade aggregate candidate segment budget exceeded",
    )?;
    let mut work = Work(0);
    let mut result = Vec::new();
    for (a, b) in edges {
        let (pa, pb) = (point(a), point(b));
        let length = pa.distance(pb);
        let normal = Point2::new(-(pb.y - pa.y) / length, (pb.x - pa.x) / length);
        let mut splits = vec![0., 1.];
        work.add(faces.len())?;
        // Only faces intersecting this projected edge can affect its visibility
        // or either symbolic side. Bound the broad phase once per edge, then
        // avoid rescanning unrelated building faces at every split interval.
        let nearby: Vec<_> = faces
            .iter()
            .filter(|f| f.interval(pa, pb).is_some())
            .cloned()
            .collect();
        for f in &nearby {
            if let Some((mut lo, mut hi)) = f.interval(pa, pb) {
                if let Some(w) = window {
                    let mut interval = (lo, hi);
                    if !clip_positive(&mut interval, f.at(pa) + w.far, f.at(pb) + w.far)
                        || !clip_positive(&mut interval, -w.near - f.at(pa), -w.near - f.at(pb))
                    {
                        continue;
                    }
                    (lo, hi) = interval;
                }
                splits.extend([lo, hi]);
                let da = f.at(pa) - a.z;
                let db = f.at(pb) - b.z;
                if (da > T && db < -T) || (da < -T && db > T) {
                    let t = da / (da - db);
                    if t > lo && t < hi {
                        splits.push(t);
                    }
                }
            }
        }
        splits.sort_by(f64::total_cmp);
        splits.dedup_by(|a, b| (*a - *b).abs() * length <= T);
        for pair in splits.windows(2) {
            if (pair[1] - pair[0]) * length <= T {
                continue;
            }
            let t = (pair[0] + pair[1]) * 0.5;
            let p = lerp(pa, pb, t);
            let depth = a.z + (b.z - a.z) * t;
            work.add(nearby.len() * 2)?;
            let l = front(&nearby, p, normal, window);
            let r = front(&nearby, p, Point2::new(-normal.x, -normal.y), window);
            let visible = [l, r]
                .into_iter()
                .flatten()
                .any(|f| (f.at(p) - depth).abs() <= T);
            if !visible {
                continue;
            }
            if let (Some(l), Some(r)) = (l, r)
                && l.entity == r.entity
                && l.surface == r.surface
                && (l.at(p) - r.at(p)).abs() <= T
                && l.gradient.distance(r.gradient) <= T
            {
                continue;
            }
            let owner = match (l, r) {
                (Some(l), Some(r)) => {
                    if l.at(p) >= r.at(p) - T {
                        l
                    } else {
                        r
                    }
                }
                (Some(f), None) | (None, Some(f)) => f,
                (None, None) => continue,
            };
            insert_segment(
                &mut result,
                ElevationSegment {
                    entity: owner.entity,
                    surface: owner.surface,
                    start: lerp(pa, pb, pair[0]),
                    end: lerp(pa, pb, pair[1]),
                },
                &mut work,
            )?;
        }
    }
    result.sort_by(|a, b| {
        a.entity
            .cmp(&b.entity)
            .then_with(|| compare(a.start, b.start))
            .then_with(|| compare(a.end, b.end))
    });
    Ok(result)
}

fn front(faces: &[Face], p: Point2, normal: Point2, window: Option<FacadeVolume>) -> Option<&Face> {
    let mut result: Option<&Face> = None;
    for f in faces {
        if window.is_none_or(|w| w.accepts(p, f.at(p)))
            && f.covers_side(p, normal)
            && result.is_none_or(|old| f.at(p) > old.at(p) + T)
        {
            result = Some(f);
        }
    }
    result
}

fn insert_segment(
    result: &mut Vec<ElevationSegment>,
    mut segment: ElevationSegment,
    work: &mut Work,
) -> Result<()> {
    // Merge collinear overlapping/touching fragments only when their semantic
    // identity agrees. This also removes front/back coincident duplicates.
    if compare(segment.start, segment.end).is_gt() {
        std::mem::swap(&mut segment.start, &mut segment.end);
    }
    let mut i = 0;
    while i < result.len() {
        work.add(1)?;
        let other = result[i];
        let delta = sub(segment.end, segment.start);
        let length = delta.x.hypot(delta.y);
        let d = Point2::new(delta.x / length, delta.y / length);
        let project = |p| {
            let v = sub(p, segment.start);
            v.x * d.x + v.y * d.y
        };
        if other.entity == segment.entity
            && other.surface == segment.surface
            && cross(d, sub(other.start, segment.start)).abs() <= T
            && cross(d, sub(other.end, segment.start)).abs() <= T
            && project(other.start).min(project(other.end)) <= length + T
            && project(other.start).max(project(other.end)) >= -T
        {
            let mut points = [segment.start, segment.end, other.start, other.end];
            points.sort_by(|a, b| compare(*a, *b));
            segment.start = points[0];
            segment.end = points[3];
            result.swap_remove(i);
            i = 0;
        } else {
            i += 1;
        }
    }
    ensure(
        result.len() < MAX_ELEVATION_SEGMENTS,
        "elevation segment limit exceeded",
    )?;
    result.push(segment);
    Ok(())
}

#[cfg(test)]
#[path = "elevation_tests.rs"]
mod tests;
