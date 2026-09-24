//! Pure room faces from straight native boundary segments; all coordinates are metres.
//!
//! Coordinates and intersections round to a 1 micrometre lattice (halfway ties
//! away from zero). There is no nearest-face fallback. Overlaps and unsupported
//! topology fail explicitly. Input order and endpoint reversal are immaterial.
use os_core::{Id, Point2};
use std::collections::{BTreeMap, BTreeSet};

pub const ROOM_TOLERANCE: f64 = 1e-6;
pub const MAX_ROOM_SEGMENTS: usize = 1024;
const MAX_SPLITS: usize = 16384;
type Vertex = (i64, i64);
pub type BoundarySegment = (Id, Point2, Point2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundaryDiagnostic {
    InvalidSegment(Id),
    DuplicateBoundary(Id),
    OverlappingBoundaries(Id, Id),
    AmbiguousTopology,
    /// Nested disconnected loops require hole-aware boundaries, a later increment.
    NestedLoops,
    ResourceLimit,
    /// These dangling segments do not enclose space; closed faces remain usable.
    OpenChains(Vec<Id>),
}

/// Canonical directed boundary cycle, independent of dimensions and input direction.
/// Not a room UUID. Retain the accepted key across edits to detect face changes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FaceKey(Vec<(Id, bool)>);

impl FaceKey {
    /// Reconstruct a persisted topology signature without a model dependency.
    /// Rotation is canonicalized; directed sides must not be duplicated.
    pub fn from_signature(sides: &[(Id, bool)]) -> Option<Self> {
        let unique: BTreeSet<_> = sides.iter().copied().collect();
        (sides.len() >= 3
            && sides.len() <= MAX_ROOM_SEGMENTS
            && unique.len() == sides.len()
            && sides.iter().all(|(id, _)| !id.0.is_nil()))
        .then(|| cycle_key(sides.to_vec()))
    }

    /// Ordered `(boundary UUID, forward)` pairs. Model UUIDs are globally unique,
    /// so a wall and a room-separation line need no source-kind field here.
    /// Forward means lexicographically smaller quantized endpoint toward larger.
    pub fn as_signature(&self) -> &[(Id, bool)] {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomFace {
    pub key: FaceKey,
    /// Counterclockwise, without repeating the first vertex.
    pub boundary: Vec<Point2>,
    pub area_m2: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomFaces {
    pub faces: Vec<RoomFace>,
    pub diagnostics: Vec<BoundaryDiagnostic>,
    segments: Vec<BoundarySegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeedDiagnostic {
    InvalidSeed,
    OnBoundary,
    NotEnclosed,
    AmbiguousBoundary,
    FaceChanged,
}

impl RoomFaces {
    /// Explicit initial placement. Never moves a seed or selects a nearby face.
    pub fn assign_seed(&self, seed: Point2) -> Result<&RoomFace, SeedDiagnostic> {
        if quantize(seed).is_none() {
            return Err(SeedDiagnostic::InvalidSeed);
        }
        if self
            .segments
            .iter()
            .any(|(_, a, b)| distance(seed, *a, *b) <= ROOM_TOLERANCE)
        {
            return Err(SeedDiagnostic::OnBoundary);
        }
        let mut candidates = self.faces.iter().filter(|f| inside(seed, &f.boundary));
        let face = candidates.next().ok_or(SeedDiagnostic::NotEnclosed)?;
        if candidates.next().is_some() {
            return Err(SeedDiagnostic::AmbiguousBoundary);
        }
        Ok(face)
    }

    /// Re-derive using the last accepted assignment, including after unresolved
    /// edits. Crossing/deleting a partition returns `FaceChanged`, not a remap.
    /// Reconstruct `expected` from the room's persisted boundary signature after
    /// reopening. Keep that accepted signature unchanged through unresolved edits.
    pub fn resolve_seed(
        &self,
        seed: Point2,
        expected: &FaceKey,
    ) -> Result<&RoomFace, SeedDiagnostic> {
        let face = self.assign_seed(seed)?;
        if &face.key != expected {
            return Err(SeedDiagnostic::FaceChanged);
        }
        Ok(face)
    }
}

fn quantize(p: Point2) -> Option<Vertex> {
    (p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6).then(|| {
        (
            (p.x / ROOM_TOLERANCE).round() as i64,
            (p.y / ROOM_TOLERANCE).round() as i64,
        )
    })
}
fn point(p: Vertex) -> Point2 {
    Point2::new(p.0 as f64 * ROOM_TOLERANCE, p.1 as f64 * ROOM_TOLERANCE)
}
fn sub(a: Point2, b: Point2) -> Point2 {
    Point2::new(a.x - b.x, a.y - b.y)
}
fn cross(a: Point2, b: Point2) -> f64 {
    a.x * b.y - a.y * b.x
}
fn parameter(p: Point2, a: Point2, b: Point2) -> f64 {
    let d = sub(b, a);
    let p = sub(p, a);
    (p.x * d.x + p.y * d.y) / (d.x * d.x + d.y * d.y)
}
fn distance(p: Point2, a: Point2, b: Point2) -> f64 {
    let t = parameter(p, a, b).clamp(0.0, 1.0);
    p.distance(Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t))
}
fn inside(p: Point2, ring: &[Point2]) -> bool {
    let mut hit = false;
    for (a, b) in ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
    {
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            hit = !hit;
        }
    }
    hit
}
fn cycle_key(mut sides: Vec<(Id, bool)>) -> FaceKey {
    sides.dedup();
    if sides.len() > 1 && sides.first() == sides.last() {
        sides.pop();
    }
    let best = (0..sides.len())
        .min_by(|a, b| {
            (0..sides.len())
                .map(|i| sides[(a + i) % sides.len()])
                .cmp((0..sides.len()).map(|i| sides[(b + i) % sides.len()]))
        })
        .unwrap_or(0);
    sides.rotate_left(best);
    FaceKey(sides)
}

/// Derive bounded simple faces from any supported native boundary segments.
/// Splits crossings/T-junctions and removes dangling
/// chains. Rejects collinear overlap, nested loops and non-simple bounded walks.
/// Fatal errors return no partial faces. Work is bounded by 1024 input segments,
/// 16384 split entries and coordinates within +/- 1,000,000 metres.
pub fn derive_faces(input: &[BoundarySegment]) -> Result<RoomFaces, BoundaryDiagnostic> {
    if input.len() > MAX_ROOM_SEGMENTS {
        return Err(BoundaryDiagnostic::ResourceLimit);
    }
    let mut segments = BTreeMap::new();
    for (id, a, b) in input {
        let a = quantize(*a).ok_or(BoundaryDiagnostic::InvalidSegment(*id))?;
        let b = quantize(*b).ok_or(BoundaryDiagnostic::InvalidSegment(*id))?;
        if a == b {
            return Err(BoundaryDiagnostic::InvalidSegment(*id));
        }
        if segments.insert(*id, (a.min(b), a.max(b))).is_some() {
            return Err(BoundaryDiagnostic::DuplicateBoundary(*id));
        }
    }
    let segments: Vec<_> = segments
        .into_iter()
        .map(|(id, (a, b))| (id, a, b))
        .collect();
    let mut splits: Vec<BTreeSet<Vertex>> = segments
        .iter()
        .map(|(_, a, b)| BTreeSet::from([*a, *b]))
        .collect();
    let mut split_count = segments.len() * 2;
    // Exact i128 orientation on lattice coordinates avoids unstable parallel tests.
    let determinant = |a: Vertex, b: Vertex| {
        i128::from(a.0) * i128::from(b.1) - i128::from(a.1) * i128::from(b.0)
    };
    let difference = |a: Vertex, b: Vertex| (a.0 - b.0, a.1 - b.1);
    for i in 0..segments.len() {
        let (id, a, b) = segments[i];
        let d = difference(b, a);
        for j in i + 1..segments.len() {
            let (other, c, e) = segments[j];
            let v = difference(e, c);
            let delta = difference(c, a);
            let denom = determinant(d, v);
            if denom == 0 {
                if determinant(delta, d) == 0 {
                    let lo = a.max(c);
                    let hi = b.min(e);
                    if lo < hi {
                        return Err(BoundaryDiagnostic::OverlappingBoundaries(id, other));
                    }
                    if lo == hi {
                        splits[i].insert(lo);
                        splits[j].insert(lo);
                    }
                }
            } else {
                let t = determinant(delta, v);
                let u = determinant(delta, d);
                let in_range = |n: i128| {
                    if denom > 0 {
                        n >= 0 && n <= denom
                    } else {
                        n <= 0 && n >= denom
                    }
                };
                if in_range(t) && in_range(u) {
                    let t = t as f64 / denom as f64;
                    let p = (
                        (a.0 as f64 + d.0 as f64 * t).round() as i64,
                        (a.1 as f64 + d.1 as f64 * t).round() as i64,
                    );
                    split_count +=
                        usize::from(splits[i].insert(p)) + usize::from(splits[j].insert(p));
                }
            }
            if split_count > MAX_SPLITS {
                return Err(BoundaryDiagnostic::ResourceLimit);
            }
        }
    }
    let mut edges = BTreeMap::new();
    let mut adjacency: BTreeMap<Vertex, BTreeSet<Vertex>> = BTreeMap::new();
    for (i, split) in splits.iter().enumerate() {
        let vertices: Vec<_> = split.iter().copied().collect();
        for pair in vertices.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if edges.insert((a, b), segments[i].0).is_some() {
                return Err(BoundaryDiagnostic::AmbiguousTopology);
            }
            adjacency.entry(a).or_default().insert(b);
            adjacency.entry(b).or_default().insert(a);
        }
    }
    let mut pending: Vec<_> = adjacency
        .iter()
        .filter(|(_, n)| n.len() == 1)
        .map(|(v, _)| *v)
        .collect();
    let mut open = BTreeSet::new();
    while let Some(a) = pending.pop() {
        if adjacency[&a].len() != 1 {
            continue;
        }
        let b = *adjacency[&a].first().unwrap();
        adjacency.get_mut(&a).unwrap().clear();
        adjacency.get_mut(&b).unwrap().remove(&a);
        open.insert(edges[&(a.min(b), a.max(b))]);
        if adjacency[&b].len() == 1 {
            pending.push(b);
        }
    }
    let ordered: BTreeMap<_, Vec<_>> = adjacency
        .iter()
        .map(|(a, neighbors)| {
            let mut neighbors: Vec<_> = neighbors.iter().copied().collect();
            neighbors.sort_by(|b, c| {
                let b = difference(*b, *a);
                let c = difference(*c, *a);
                let upper = |v: Vertex| v.1 > 0 || (v.1 == 0 && v.0 >= 0);
                upper(c)
                    .cmp(&upper(b))
                    .then_with(|| determinant(c, b).cmp(&0))
            });
            (*a, neighbors)
        })
        .collect();
    let mut walked = BTreeSet::new();
    let mut faces = Vec::new();
    for (a, neighbors) in &ordered {
        for b in neighbors {
            let start = (*a, *b);
            if walked.contains(&start) {
                continue;
            }
            let mut current = start;
            let mut ring = Vec::new();
            let mut sides = Vec::new();
            let mut seen = BTreeSet::new();
            let mut simple = true;
            loop {
                if !walked.insert(current) {
                    return Err(BoundaryDiagnostic::AmbiguousTopology);
                }
                let (u, v) = current;
                simple &= seen.insert(u);
                ring.push(point(u));
                sides.push((edges[&(u.min(v), u.max(v))], u < v));
                let neighbors = &ordered[&v];
                let incoming = neighbors.iter().position(|p| *p == u).unwrap();
                current = (
                    v,
                    neighbors[(incoming + neighbors.len() - 1) % neighbors.len()],
                );
                if current == start {
                    break;
                }
            }
            let area_m2 = ring
                .windows(2)
                .map(|p| cross(sub(p[0], ring[0]), sub(p[1], ring[0])))
                .sum::<f64>()
                * 0.5;
            if area_m2 > ROOM_TOLERANCE * ROOM_TOLERANCE {
                if !simple {
                    return Err(BoundaryDiagnostic::AmbiguousTopology);
                }
                faces.push(RoomFace {
                    key: cycle_key(sides),
                    boundary: ring,
                    area_m2,
                });
            }
        }
    }
    for (i, face) in faces.iter().enumerate() {
        for other in faces.iter().skip(i + 1) {
            // Shared-boundary neighboring faces are not nested.
            let strictly_in = |ring: &[Point2], p: Point2| {
                inside(p, ring)
                    && ring
                        .iter()
                        .zip(ring.iter().cycle().skip(1))
                        .take(ring.len())
                        .all(|(a, b)| distance(p, *a, *b) > ROOM_TOLERANCE)
            };
            if face
                .boundary
                .iter()
                .any(|p| strictly_in(&other.boundary, *p))
                || other
                    .boundary
                    .iter()
                    .any(|p| strictly_in(&face.boundary, *p))
            {
                return Err(BoundaryDiagnostic::NestedLoops);
            }
        }
    }
    faces.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(RoomFaces {
        faces,
        diagnostics: if open.is_empty() {
            vec![]
        } else {
            vec![BoundaryDiagnostic::OpenChains(open.into_iter().collect())]
        },
        segments: segments
            .into_iter()
            .map(|(id, a, b)| (id, point(a), point(b)))
            .collect(),
    })
}
