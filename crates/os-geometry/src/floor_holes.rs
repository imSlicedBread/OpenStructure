//! Bounded, deterministic triangulation and downward extrusion of perforated slabs.
//!
//! Rings omit the repeated closing point. Each ring has 3..=256 vertices; there
//! are 1..=16 holes. Coordinates are metres, finite and bounded by +/-1e6.
//! Edges, distinct vertices, nonadjacent edges, and separate rings must have more
//! than 1e-6 m clearance. Straight forward collinear vertices are retained.
//!
//! Internal visible bridges join CCW outer and CW inner rings into a weakly
//! simple contour. Its repeated bridge endpoints are handled here, not passed
//! to the simple-ring triangulator. Ear clipping uses stable authored-index
//! tie breaks, introduces no Steiner vertices, and preserves every boundary
//! edge. Worst-case work is cubic in the bounded total vertex count (4352).
//! Predicates use f64, not exact arithmetic: numerically unresolved geometry
//! returns an error rather than a partial mesh. Determinism means identical
//! ordered inputs on the same floating-point target, not winding-invariant
//! triangle ordering. No schema, model, or UI integration is performed here.

use crate::{Mesh, Vec3};
use os_core::{Point2, Result, ensure};
use std::collections::BTreeMap;

pub const MAX_HOLES: usize = 16;
pub const MAX_RING_VERTICES: usize = 256;
pub const CLEARANCE: f64 = 1e-6;
pub const MIN_NET_AREA: f64 = 1e-8;

/// CCW triangles indexing outer vertices followed by each hole's vertices, all
/// in the original authored order. There are no added or duplicated vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct FloorTriangulation {
    pub vertices: Vec<Point2>,
    pub triangles: Vec<[u32; 3]>,
    pub net_area: f64,
}

fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn area(ring: &[Point2]) -> f64 {
    ring[1..]
        .windows(2)
        .map(|p| cross(ring[0], p[0], p[1]))
        .sum::<f64>()
        * 0.5
}

fn edges(ring: &[Point2]) -> impl Iterator<Item = (Point2, Point2)> + '_ {
    ring.iter()
        .copied()
        .zip(ring.iter().copied().cycle().skip(1))
        .take(ring.len())
}

fn point_segment_distance(p: Point2, a: Point2, b: Point2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

fn intersects(a: Point2, b: Point2, c: Point2, d: Point2) -> bool {
    a.x.min(b.x) <= c.x.max(d.x)
        && c.x.min(d.x) <= a.x.max(b.x)
        && a.y.min(b.y) <= c.y.max(d.y)
        && c.y.min(d.y) <= a.y.max(b.y)
        && cross(a, b, c) * cross(a, b, d) <= 0.0
        && cross(c, d, a) * cross(c, d, b) <= 0.0
}

fn segment_distance(a: Point2, b: Point2, c: Point2, d: Point2) -> f64 {
    if intersects(a, b, c, d) {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

// Callers check boundary clearance separately; this is the open-region test.
fn inside(p: Point2, ring: &[Point2]) -> bool {
    let mut contained = false;
    for (a, b) in edges(ring) {
        if (a.y > p.y) != (b.y > p.y) && p.x - a.x < (b.x - a.x) * ((p.y - a.y) / (b.y - a.y)) {
            contained = !contained;
        }
    }
    contained
}

fn validate_ring(ring: &[Point2]) -> Result<()> {
    ensure(
        (3..=MAX_RING_VERTICES).contains(&ring.len()),
        "floor ring needs 3-256 vertices",
    )?;
    ensure(
        ring.iter()
            .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
        "invalid floor ring coordinates",
    )?;
    for i in 0..ring.len() {
        let (a, b, c) = (
            ring[i],
            ring[(i + 1) % ring.len()],
            ring[(i + 2) % ring.len()],
        );
        ensure(a.distance(b) > CLEARANCE, "floor ring edge is too short")?;
        ensure(
            cross(a, b, c) != 0.0 || (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y) > 0.0,
            "floor ring doubles back",
        )?;
        for j in i + 1..ring.len() {
            ensure(
                a.distance(ring[j]) > CLEARANCE,
                "duplicate floor ring vertices",
            )?;
            if j == i + 1 || (i == 0 && j == ring.len() - 1) {
                continue;
            }
            ensure(
                segment_distance(a, b, ring[j], ring[(j + 1) % ring.len()]) > CLEARANCE,
                "floor ring crosses, touches, or lacks clearance",
            )?;
        }
    }
    ensure(area(ring).abs() > 0.0, "floor ring has zero area")
}

fn separated(a: &[Point2], b: &[Point2]) -> bool {
    edges(a).all(|(p, q)| edges(b).all(|(r, s)| segment_distance(p, q, r, s) > CLEARANCE))
}

fn oriented_indices(ring: &[Point2], offset: usize, ccw: bool) -> Vec<usize> {
    let mut indices: Vec<_> = (offset..offset + ring.len()).collect();
    if (area(ring) > 0.0) != ccw {
        indices.reverse();
    }
    indices
}

// Material lies to the left of both directed boundary edges. Strict inequalities
// exclude bridges along an incident edge, including previously installed bridges.
fn in_cone(p: &[Point2], ring: &[usize], i: usize, q: Point2) -> bool {
    let (a, b, c) = (
        p[ring[(i + ring.len() - 1) % ring.len()]],
        p[ring[i]],
        p[ring[(i + 1) % ring.len()]],
    );
    let (left_in, left_out) = (cross(a, b, q) > 0.0, cross(b, c, q) > 0.0);
    if cross(a, b, c) >= 0.0 {
        left_in && left_out
    } else {
        left_in || left_out
    }
}

fn bridge_clear(p: &[Point2], a: usize, b: usize, c: usize, d: usize) -> bool {
    if a == c || a == d || b == c || b == d {
        // Shared endpoints are allowed, collinear overlap is not.
        let other = if a == c || b == c { d } else { c };
        return point_segment_distance(p[other], p[a], p[b]) > CLEARANCE;
    }
    segment_distance(p[a], p[b], p[c], p[d]) > CLEARANCE
}

fn ear_clip(p: &[Point2], mut ring: Vec<usize>) -> Result<Vec<[u32; 3]>> {
    let mut triangles = Vec::with_capacity(ring.len() - 2);
    let mut cursor = 0;
    while ring.len() > 3 {
        let ear = (0..ring.len())
            .map(|step| (cursor + step) % ring.len())
            .find(|&i| {
                let (a, b, c) = (
                    ring[(i + ring.len() - 1) % ring.len()],
                    ring[i],
                    ring[(i + 1) % ring.len()],
                );
                cross(p[a], p[b], p[c]) > 0.0
                    && !ring.iter().any(|&q| {
                        q != a
                            && q != b
                            && q != c
                            && cross(p[a], p[b], p[q]) >= 0.0
                            && cross(p[b], p[c], p[q]) >= 0.0
                            && cross(p[c], p[a], p[q]) >= 0.0
                    })
            })
            .ok_or_else(|| {
                os_core::Error::Invalid("hole triangulation could not resolve an ear".into())
            })?;
        triangles.push([
            ring[(ear + ring.len() - 1) % ring.len()] as u32,
            ring[ear] as u32,
            ring[(ear + 1) % ring.len()] as u32,
        ]);
        ring.remove(ear);
        cursor = (ear + ring.len() - 1) % ring.len();
    }
    ensure(
        cross(p[ring[0]], p[ring[1]], p[ring[2]]) > 0.0,
        "degenerate hole cap triangle",
    )?;
    triangles.push([ring[0] as u32, ring[1] as u32, ring[2] as u32]);
    Ok(triangles)
}

/// Validate and triangulate a simple outer ring with 1..=16 disjoint holes.
/// Either winding is accepted independently for every ring. Failure is atomic.
pub fn triangulate_floor_with_holes(
    outer: &[Point2],
    holes: &[Vec<Point2>],
) -> Result<FloorTriangulation> {
    ensure(
        (1..=MAX_HOLES).contains(&holes.len()),
        "floor needs 1-16 holes",
    )?;
    validate_ring(outer)?;
    for (i, hole) in holes.iter().enumerate() {
        validate_ring(hole)?;
        ensure(
            separated(outer, hole) && inside(hole[0], outer),
            "hole must be strictly inside the floor with clearance",
        )?;
        for previous in &holes[..i] {
            ensure(
                separated(previous, hole)
                    && !inside(hole[0], previous)
                    && !inside(previous[0], hole),
                "holes cross, touch, nest, or lack clearance",
            )?;
        }
    }
    let net_area = area(outer).abs() - holes.iter().map(|h| area(h).abs()).sum::<f64>();
    ensure(net_area > MIN_NET_AREA, "floor net area is negligible")?;
    let mut vertices = outer.to_vec();
    let mut ring = oriented_indices(outer, 0, true);
    let mut inner = Vec::with_capacity(holes.len());
    for hole in holes {
        inner.push(oriented_indices(hole, vertices.len(), false));
        vertices.extend_from_slice(hole);
    }
    // Process leftmost holes first; all ties use the original flat vertex index.
    let key = |a: usize, b: usize| {
        vertices[a]
            .x
            .total_cmp(&vertices[b].x)
            .then(vertices[a].y.total_cmp(&vertices[b].y))
            .then(a.cmp(&b))
    };
    for h in &mut inner {
        let start = (0..h.len()).min_by(|&a, &b| key(h[a], h[b])).unwrap();
        h.rotate_left(start);
    }
    inner.sort_by(|a, b| key(a[0], b[0]));
    let boundaries: Vec<_> = std::iter::once(oriented_indices(outer, 0, true))
        .chain(inner.iter().cloned())
        .collect();
    for hole in inner {
        let h = hole[0];
        let candidate = (0..ring.len())
            .filter(|&i| {
                let v = ring[i];
                if !in_cone(&vertices, &ring, i, vertices[h])
                    || !in_cone(&vertices, &hole, 0, vertices[v])
                {
                    return false;
                }
                let midpoint = Point2::new(
                    (vertices[h].x + vertices[v].x) * 0.5,
                    (vertices[h].y + vertices[v].y) * 0.5,
                );
                inside(midpoint, outer)
                    && !holes.iter().any(|r| inside(midpoint, r))
                    && boundaries.iter().chain(std::iter::once(&ring)).all(|r| {
                        (0..r.len())
                            .all(|j| bridge_clear(&vertices, h, v, r[j], r[(j + 1) % r.len()]))
                    })
            })
            .min_by(|&a, &b| {
                vertices[h]
                    .distance(vertices[ring[a]])
                    .total_cmp(&vertices[h].distance(vertices[ring[b]]))
                    .then(ring[a].cmp(&ring[b]))
                    .then(a.cmp(&b))
            })
            .ok_or_else(|| {
                os_core::Error::Invalid("hole triangulation could not find a clear bridge".into())
            })?;
        let mut joined = Vec::with_capacity(ring.len() + hole.len() + 2);
        joined.extend_from_slice(&ring[..=candidate]);
        joined.extend_from_slice(&hole);
        joined.extend([h, ring[candidate]]);
        joined.extend_from_slice(&ring[candidate + 1..]);
        ring = joined;
    }
    let triangles = ear_clip(&vertices, ring)?;
    // Postconditions guard both numeric failure and accidental dropped boundary
    // vertices. Every cap interior edge has one opposite mate; each authored
    // boundary edge occurs once in its material-left direction.
    let mut counts = BTreeMap::<(u32, u32), usize>::new();
    let mut triangle_area = 0.0;
    for &[a, b, c] in &triangles {
        triangle_area += cross(
            vertices[a as usize],
            vertices[b as usize],
            vertices[c as usize],
        ) * 0.5;
        for e in [(a, b), (b, c), (c, a)] {
            *counts.entry(e).or_default() += 1;
        }
    }
    for r in boundaries {
        for i in 0..r.len() {
            ensure(
                counts.remove(&(r[i] as u32, r[(i + 1) % r.len()] as u32)) == Some(1),
                "hole cap lost a boundary edge",
            )?;
        }
    }
    ensure(
        counts
            .iter()
            .all(|(&(a, b), &n)| n == 1 && counts.get(&(b, a)) == Some(&1)),
        "hole cap is not manifold",
    )?;
    ensure(
        (triangle_area - net_area).abs() <= net_area * 1e-9,
        "hole cap area mismatch",
    )?;
    Ok(FloorTriangulation {
        vertices,
        triangles,
        net_area,
    })
}

/// Extrude down from `top_z` by positive `thickness` (>1e-6 and <=1e6 m).
/// Z coordinates are finite and bounded by +/-1e6. Bottom vertices precede top
/// vertices, each in the triangulation's authored flat order. Caps and all outer
/// and inner side faces wind outward (inner normals point into the holes).
pub fn extrude_floor_with_holes(
    outer: &[Point2],
    holes: &[Vec<Point2>],
    top_z: f64,
    thickness: f64,
) -> Result<Mesh> {
    let bottom = top_z - thickness;
    ensure(
        top_z.is_finite()
            && top_z.abs() <= 1e6
            && bottom.is_finite()
            && bottom.abs() <= 1e6
            && thickness.is_finite()
            && thickness > CLEARANCE
            && thickness <= 1e6
            && bottom < top_z,
        "invalid floor extrusion heights",
    )?;
    let cap = triangulate_floor_with_holes(outer, holes)?;
    let n = cap.vertices.len() as u32;
    let mut mesh = Mesh::default();
    for z in [bottom, top_z] {
        mesh.vertices
            .extend(cap.vertices.iter().map(|p| Vec3::new(p.x, p.y, z)));
    }
    for [a, b, c] in cap.triangles {
        mesh.triangles.extend([[a, c, b], [a + n, b + n, c + n]]);
    }
    let mut offset = 0;
    for (i, boundary) in std::iter::once(outer)
        .chain(holes.iter().map(Vec::as_slice))
        .enumerate()
    {
        let ring = oriented_indices(boundary, offset, i == 0);
        for j in 0..ring.len() {
            let (a, b) = (ring[j] as u32, ring[(j + 1) % ring.len()] as u32);
            mesh.triangles.extend([[a, b, b + n], [a, b + n, a + n]]);
        }
        offset += boundary.len();
    }
    mesh.validate()?;
    Ok(mesh)
}

/// Triangulate an ordinary floor with the existing simple-ring kernel, or a
/// perforated floor with the bridge/ear-clipping kernel. Vertices are always
/// flattened in authored ring order so renderers can use one index space.
pub fn triangulate_floor_rings(
    outer: &[Point2],
    holes: &[Vec<Point2>],
) -> Result<FloorTriangulation> {
    if holes.is_empty() {
        let triangles = crate::floors::triangulate_floor(outer)?;
        return Ok(FloorTriangulation {
            vertices: outer.to_vec(),
            triangles,
            net_area: crate::floors::signed_area(outer).abs(),
        });
    }
    triangulate_floor_with_holes(outer, holes)
}

/// Keep holeless slabs on the established extrusion path; openings use the
/// closed, hole-aware mesh kernel above.
pub fn extrude_floor_rings(
    outer: &[Point2],
    holes: &[Vec<Point2>],
    top_z: f64,
    thickness: f64,
) -> Result<Mesh> {
    if holes.is_empty() {
        return crate::floors::extrude_floor(outer, top_z, thickness);
    }
    extrude_floor_with_holes(outer, holes, top_z, thickness)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polygon(points: &[(f64, f64)]) -> Vec<Point2> {
        points.iter().map(|&(x, y)| Point2::new(x, y)).collect()
    }
    fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point2> {
        polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
    }

    // Clip a triangle against a CCW triangle. A positive intersection area proves
    // an interior overlap; mere shared boundaries produce zero area.
    fn intersection_area(mut subject: Vec<Point2>, clip: [Point2; 3]) -> f64 {
        for (a, b) in edges(&clip) {
            let mut output = Vec::new();
            for (p, q) in edges(&subject) {
                let (dp, dq) = (cross(a, b, p), cross(a, b, q));
                if (dp >= 0.0) != (dq >= 0.0) {
                    let t = dp / (dp - dq);
                    output.push(Point2::new(p.x + t * (q.x - p.x), p.y + t * (q.y - p.y)));
                }
                if dq >= 0.0 {
                    output.push(q);
                }
            }
            subject = output;
            if subject.len() < 3 {
                return 0.0;
            }
        }
        area(&subject).abs()
    }

    fn verify(outer: &[Point2], holes: &[Vec<Point2>]) {
        let cap = triangulate_floor_with_holes(outer, holes).unwrap();
        assert_eq!(cap, triangulate_floor_with_holes(outer, holes).unwrap());
        assert_eq!(
            cap.vertices,
            outer
                .iter()
                .chain(holes.iter().flatten())
                .copied()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            cap.triangles.len(),
            cap.vertices.len() + 2 * holes.len() - 2
        );
        let expected = area(outer).abs() - holes.iter().map(|h| area(h).abs()).sum::<f64>();
        let hole_triangles: Vec<_> = holes
            .iter()
            .flat_map(|h| {
                crate::floors::triangulate_floor(h)
                    .unwrap()
                    .into_iter()
                    .map(|t| t.map(|i| h[i as usize]))
            })
            .collect();
        let outer_triangles = crate::floors::triangulate_floor(outer).unwrap();
        let mut actual = 0.0;
        for t in &cap.triangles {
            let points = t.map(|i| cap.vertices[i as usize]);
            let a = cross(points[0], points[1], points[2]) * 0.5;
            assert!(a > 0.0);
            actual += a;
            for &ht in &hole_triangles {
                assert!(
                    intersection_area(points.to_vec(), ht) < 1e-9,
                    "triangle enters hole"
                );
            }
            let contained: f64 = outer_triangles
                .iter()
                .map(|t| intersection_area(points.to_vec(), t.map(|i| outer[i as usize])))
                .sum();
            assert!(
                (contained - a).abs() < 1e-8,
                "triangle leaves outer boundary"
            );
        }
        assert!((actual - expected).abs() < expected * 1e-10);
        let mesh = extrude_floor_with_holes(outer, holes, 7.0, 0.3).unwrap();
        assert_eq!(
            mesh,
            extrude_floor_with_holes(outer, holes, 7.0, 0.3).unwrap()
        );
        assert!((mesh.signed_volume() - expected * 0.3).abs() < expected * 1e-9);
        let mut counts = BTreeMap::<(u32, u32), usize>::new();
        for &[a, b, c] in &mesh.triangles {
            for e in [(a, b), (b, c), (c, a)] {
                *counts.entry(e).or_default() += 1;
            }
        }
        for (&(a, b), &n) in &counts {
            assert_eq!(n, 1);
            assert_eq!(counts.get(&(b, a)), Some(&1));
        }
        for t in &mesh.triangles {
            let [a, b, c] = t.map(|i| mesh.vertices[i as usize]);
            let normal = Vec3::new(
                (b.y - a.y) * (c.z - a.z) - (b.z - a.z) * (c.y - a.y),
                (b.z - a.z) * (c.x - a.x) - (b.x - a.x) * (c.z - a.z),
                (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x),
            );
            if a.z == b.z && b.z == c.z {
                assert!(if a.z == 7.0 {
                    normal.z > 0.0
                } else {
                    normal.z < 0.0
                });
            } else {
                let length = normal.x.hypot(normal.y);
                let probe = Point2::new(
                    (a.x + b.x + c.x) / 3.0 + normal.x / length * 1e-5,
                    (a.y + b.y + c.y) / 3.0 + normal.y / length * 1e-5,
                );
                assert!(
                    !inside(probe, outer) || holes.iter().any(|h| inside(probe, h)),
                    "side normal points into slab"
                );
            }
        }
    }

    #[test]
    fn concave_outer_multiple_concave_holes_all_winding_combinations() {
        let outer = polygon(&[
            (0., 0.),
            (12., 0.),
            (12., 12.),
            (8., 12.),
            (8., 6.),
            (5., 6.),
            (5., 12.),
            (0., 12.),
        ]);
        let holes = [
            rect(1., 1., 2., 2.),
            polygon(&[
                (8., 1.),
                (11., 1.),
                (11., 4.),
                (10., 4.),
                (10., 2.),
                (8., 2.),
            ]),
            rect(1., 7., 2., 2.),
        ];
        for mask in 0..16 {
            let mut o = outer.clone();
            let mut hs = holes.to_vec();
            if mask & 1 != 0 {
                o.reverse();
            }
            for (i, h) in hs.iter_mut().enumerate() {
                if mask & (2 << i) != 0 {
                    h.reverse();
                }
            }
            verify(&o, &hs);
        }
    }

    #[test]
    fn collinear_boundaries_and_aligned_holes_preserve_every_edge() {
        let outer = polygon(&[(0., 0.), (5., 0.), (10., 0.), (10., 10.), (0., 10.)]);
        let holes = vec![
            polygon(&[(1., 1.), (2., 1.), (3., 1.), (3., 3.), (1., 3.)]),
            rect(5., 1., 2., 2.),
            rect(1., 5., 2., 2.),
            rect(5., 5., 2., 2.),
        ];
        verify(&outer, &holes);
        let mut reversed = holes;
        reversed.reverse();
        verify(&outer, &reversed);
    }

    #[test]
    fn rejects_invalid_rings_containment_clearance_and_nesting() {
        let outer = rect(0., 0., 10., 10.);
        let good = rect(2., 2., 2., 2.);
        for bad in [
            rect(-1., 2., 2., 2.),
            rect(0., 2., 2., 2.),
            rect(CLEARANCE * 0.5, 2., 2., 2.),
            rect(20., 20., 1., 1.),
            polygon(&[(1., 1.), (4., 4.), (1., 4.), (4., 1.)]),
            polygon(&[(1., 1.), (4., 1.), (2., 1.), (2., 4.)]),
            vec![Point2::new(1., 1.); 3],
            polygon(&[(1., 1.), (2., 1.), (3., 1.)]),
        ] {
            assert!(triangulate_floor_with_holes(&outer, &[bad]).is_err());
        }
        for second in [
            good.clone(),
            rect(3., 3., 2., 2.),
            rect(4., 2., 2., 2.),
            rect(4. + CLEARANCE * 0.5, 2., 2., 2.),
            rect(2.5, 2.5, 0.5, 0.5),
            rect(1., 1., 4., 4.),
        ] {
            assert!(triangulate_floor_with_holes(&outer, &[good.clone(), second]).is_err());
        }
        let concave = polygon(&[
            (0., 0.),
            (10., 0.),
            (10., 10.),
            (6., 10.),
            (6., 3.),
            (4., 3.),
            (4., 10.),
            (0., 10.),
        ]);
        assert!(triangulate_floor_with_holes(&concave, &[rect(2., 2., 6., 3.)]).is_err());
        for value in [f64::NAN, f64::INFINITY, -f64::INFINITY, 1e6 + 1.] {
            let mut bad = good.clone();
            bad[0].x = value;
            assert!(triangulate_floor_with_holes(&outer, &[bad.clone()]).is_err());
            assert!(triangulate_floor_with_holes(&bad, std::slice::from_ref(&good)).is_err());
        }
        assert!(triangulate_floor_with_holes(&outer, &[]).is_err());
        assert!(triangulate_floor_with_holes(&outer, &vec![good.clone(); 17]).is_err());
        assert!(triangulate_floor_with_holes(&outer[..2], std::slice::from_ref(&good)).is_err());
        assert!(triangulate_floor_with_holes(&outer, &[good[..2].to_vec()]).is_err());
        assert!(
            triangulate_floor_with_holes(
                &vec![Point2::new(0., 0.); 257],
                std::slice::from_ref(&good)
            )
            .is_err()
        );
        assert!(triangulate_floor_with_holes(&outer, &[vec![Point2::new(1., 1.); 257]]).is_err());
    }

    #[test]
    fn rejects_negligible_net_area_and_invalid_extrusions() {
        let outer = rect(0., 0., 0.0002, 0.0002);
        let hole = rect(2e-6, 2e-6, 0.000196, 0.000196);
        assert!(triangulate_floor_with_holes(&outer, &[hole]).is_err());
        let outer = rect(0., 0., 10., 10.);
        let holes = vec![rect(2., 2., 2., 2.)];
        // The 1e-8 area threshold applies to the remaining slab, not each hole.
        let small = triangulate_floor_with_holes(&outer, &[rect(2., 2., 1e-5, 1e-5)]).unwrap();
        assert_eq!(small.triangles.len(), 8);
        assert_eq!(small.vertices.len(), 8);
        assert!((small.net_area - (100. - 1e-10)).abs() < 1e-13);
        for (top, thickness) in [
            (0., 0.),
            (0., -1.),
            (0., CLEARANCE),
            (0., f64::NAN),
            (0., f64::INFINITY),
            (0., 1e6 + 1.),
            (f64::NAN, 1.),
            (f64::INFINITY, 1.),
            (1e6 + 1., 1.),
            (-1e6, 1.),
        ] {
            assert!(extrude_floor_with_holes(&outer, &holes, top, thickness).is_err());
        }
    }

    #[test]
    fn deterministic_radial_cases_and_sixteen_holes() {
        for n in [5, 8, 17, 64, 256] {
            let outer: Vec<_> = (0..n)
                .map(|i| {
                    let angle = i as f64 * std::f64::consts::TAU / n as f64;
                    let r = if i % 2 == 0 { 20. } else { 15. };
                    Point2::new(r * angle.cos(), r * angle.sin())
                })
                .collect();
            verify(&outer, &[rect(-2., -2., 1., 1.), rect(2., 2., 1., 1.)]);
        }
        let holes: Vec<_> = (0..16)
            .map(|i| rect(1. + (i % 4) as f64 * 2., 1. + (i / 4) as f64 * 2., 1., 1.))
            .collect();
        verify(&rect(0., 0., 10., 10.), &holes);
    }

    #[test]
    fn coordinates_near_bound_and_translation_preserve_caps() {
        let outer = rect(0., 0., 10., 10.);
        let holes = vec![rect(1., 1., 3., 3.), rect(6., 6., 2., 2.)];
        let cap = triangulate_floor_with_holes(&outer, &holes).unwrap();
        for delta in [-999_999., 999_989.] {
            let shift = |ring: &[Point2]| {
                ring.iter()
                    .map(|p| Point2::new(p.x + delta, p.y + delta))
                    .collect::<Vec<_>>()
            };
            let shifted = triangulate_floor_with_holes(
                &shift(&outer),
                &holes.iter().map(|h| shift(h)).collect::<Vec<_>>(),
            )
            .unwrap();
            assert_eq!(cap.triangles, shifted.triangles);
            assert_eq!(cap.net_area, shifted.net_area);
        }
    }

    #[test]
    fn varied_hole_shapes_orders_and_bridges() {
        let mut seed = 81_u64;
        let mut random = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as f64 / u32::MAX as f64
        };
        for case in 0..48 {
            let mut outer = rect(-10., -10., 20., 20.);
            let mut holes = Vec::new();
            for i in 0..1 + case % 16 {
                let n = 3 + (random() * 10.) as usize;
                let (x, y) = (-6. + (i % 4) as f64 * 4., -6. + (i / 4) as f64 * 4.);
                let mut ring: Vec<_> = (0..n)
                    .map(|j| {
                        let a = j as f64 * std::f64::consts::TAU / n as f64;
                        let r = 0.5 + random();
                        Point2::new(x + r * a.cos(), y + r * a.sin())
                    })
                    .collect();
                if random() < 0.5 {
                    ring.reverse();
                }
                ring.rotate_left(case % n);
                holes.push(ring);
            }
            if case % 2 == 0 {
                outer.reverse();
                holes.reverse();
            }
            verify(&outer, &holes);
        }
    }

    #[test]
    fn maximum_vertex_and_hole_counts_produce_closed_caps_and_sides() {
        let circle = |x: f64, y: f64, r: f64| {
            (0..MAX_RING_VERTICES)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / MAX_RING_VERTICES as f64;
                    Point2::new(x + r * a.cos(), y + r * a.sin())
                })
                .collect::<Vec<_>>()
        };
        let outer = circle(0., 0., 30.);
        let holes: Vec<_> = (0..MAX_HOLES)
            .map(|i| circle(-6. + (i % 4) as f64 * 4., -6. + (i / 4) as f64 * 4., 1.))
            .collect();
        let cap = triangulate_floor_with_holes(&outer, &holes).unwrap();
        assert_eq!(cap.vertices.len(), 4352);
        assert_eq!(cap.triangles.len(), 4382);
        let mesh = extrude_floor_with_holes(&outer, &holes, 1., 0.5).unwrap();
        assert!((mesh.signed_volume() - cap.net_area * 0.5).abs() < 1e-8);
        let mut counts = BTreeMap::<(u32, u32), usize>::new();
        for &[a, b, c] in &mesh.triangles {
            for e in [(a, b), (b, c), (c, a)] {
                *counts.entry(e).or_default() += 1;
            }
        }
        assert!(
            counts
                .iter()
                .all(|(&(a, b), &n)| n == 1 && counts.get(&(b, a)) == Some(&1))
        );
    }

    #[test]
    fn no_openings_keep_the_existing_simple_floor_triangulation_and_extrusion() {
        let outer = polygon(&[(0., 0.), (6., 0.), (6., 2.), (2., 2.), (2., 6.), (0., 6.)]);
        let cap = triangulate_floor_rings(&outer, &[]).unwrap();
        assert_eq!(cap.vertices, outer);
        assert_eq!(
            cap.triangles,
            crate::floors::triangulate_floor(&outer).unwrap()
        );
        assert_eq!(cap.net_area, crate::floors::signed_area(&outer).abs());
        assert_eq!(
            extrude_floor_rings(&outer, &[], 3.0, 0.25).unwrap(),
            crate::floors::extrude_floor(&outer, 3.0, 0.25).unwrap()
        );
    }
}
