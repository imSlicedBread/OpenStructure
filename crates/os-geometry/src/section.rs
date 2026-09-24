//! Bounded vertical cuts of closed, indexed triangle surfaces.
//!
//! This is a pure mesh utility, not a boolean/repair kernel. Input must share
//! vertex indices across each shell's faces. We verify closed, consistently wound
//! edges and manifold vertex fans in addition to `Mesh::validate`. Surface
//! self-intersections away from the section are the caller's responsibility;
//! intersecting, touching, or overlapping cut contours are rejected. In
//! particular, touching cells from `Mesh::from_prisms` need a union before they
//! can be interpreted as one section region. Separated cells are supported.
//!
//! Classification uses an absolute metre tolerance, without rounding coordinates
//! or welding unrelated vertices. Tangent points/edges contribute no area.
//! Coplanar face patches contribute their boundary, without triangle diagonals.
//! Ambiguities below the tolerance fail rather than producing repaired contours.
//! Nested rings use even-odd material semantics, independent of shell winding.
use crate::Mesh;
use os_core::{Point2, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// Absolute plane and contour comparison tolerance in metres.
pub const SECTION_TOLERANCE: f64 = 1e-8;
/// Bounds apply before topology allocation/validation, including for missed cuts.
pub const MAX_SECTION_INPUT_VERTICES: usize = 65_536;
pub const MAX_SECTION_INPUT_TRIANGLES: usize = 131_072;
/// Maximum raw segments and therefore total returned contour vertices.
pub const MAX_SECTION_SEGMENTS: usize = 4_096;
pub const MAX_SECTION_CONTOURS: usize = 256;
/// Architectural coordinate envelope, in metres, for vertices and plane origin.
pub const MAX_SECTION_COORDINATE: f64 = 1e6;

/// A vertical plane. Direction is normalized internally; its magnitude is not a
/// scale. Positive section X follows direction, and section Y is world Z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalSectionPlane {
    pub origin: Point2,
    pub direction: Point2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutContourKind {
    Outer,
    Hole,
}

/// A simple, implicitly closed ring: the first point is NOT repeated at the end.
/// Points are `(distance along normalized direction, world elevation)`, metres.
/// Outer rings wind CCW, holes CW. Each starts at its lexicographic minimum;
/// contours are sorted lexicographically by their points. Collinear triangle
/// subdivisions are removed. Depth zero is outside all other contours; even
/// depths are material and odd depths are holes (including nested islands).
#[derive(Clone, Debug, PartialEq)]
pub struct CutContour {
    pub points: Vec<Point2>,
    pub kind: CutContourKind,
    pub nesting_depth: usize,
}

type Edge = (u32, u32);

#[derive(Clone, Copy)]
struct EdgeUse {
    forward: bool,
    opposite: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Vertex(u32),
    Edge(Edge),
}

#[derive(Clone, Copy)]
struct Projected {
    point: Point2,
    distance: f64,
    side: i8,
}

fn ordered<T: Ord>(a: T, b: T) -> (T, T) {
    if a < b { (a, b) } else { (b, a) }
}

fn invalid(message: &str) -> os_core::Error {
    os_core::Error::Invalid(message.into())
}

/// Cut a closed mesh into semantic contours. A miss or a zero-area tangent cut
/// returns an empty vector. Invalid topology, non-finite/out-of-envelope inputs,
/// ambiguous or non-manifold cuts, and exceeded limits return `Error::Invalid`.
/// No partial result is returned. Does not mutate the mesh or plane.
///
/// Work is O(T log T + S²), storage O(V + T + S), with all counts capped above.
/// Intersections are keyed by source vertex/edge, so shared triangle edges are
/// computed exactly once and traversal is independent of triangle ordering.
pub fn vertical_section(mesh: &Mesh, plane: VerticalSectionPlane) -> Result<Vec<CutContour>> {
    ensure(
        mesh.vertices.len() <= MAX_SECTION_INPUT_VERTICES
            && mesh.triangles.len() <= MAX_SECTION_INPUT_TRIANGLES,
        "section input limit exceeded",
    )?;
    let within = |v: f64| v.is_finite() && v.abs() <= MAX_SECTION_COORDINATE;
    ensure(
        within(plane.origin.x) && within(plane.origin.y) && plane.direction.is_finite(),
        "invalid section plane",
    )?;
    // Scaling first accepts both subnormal and very large finite directions.
    let scale = plane.direction.x.abs().max(plane.direction.y.abs());
    ensure(scale > 0.0, "zero section direction")?;
    let dx = plane.direction.x / scale;
    let dy = plane.direction.y / scale;
    let length = dx.hypot(dy);
    let direction = Point2::new(dx / length, dy / length);
    ensure(
        mesh.vertices
            .iter()
            .all(|v| within(v.x) && within(v.y) && within(v.z)),
        "section mesh outside finite coordinate envelope",
    )?;
    mesh.validate()?;
    let projected: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| {
            let x = v.x - plane.origin.x;
            let y = v.y - plane.origin.y;
            let distance = -direction.y * x + direction.x * y;
            Projected {
                point: Point2::new(direction.x * x + direction.y * y, v.z),
                distance,
                side: if distance.abs() <= SECTION_TOLERANCE {
                    0
                } else if distance > 0.0 {
                    1
                } else {
                    -1
                },
            }
        })
        .collect();
    let edges = closed_topology(mesh, &projected)?;
    let mut segments = BTreeSet::new();
    for triangle in &mesh.triangles {
        let on_plane = triangle
            .iter()
            .filter(|&&i| projected[i as usize].side == 0)
            .count();
        if on_plane >= 2 {
            // Exact edges and coplanar patches need both incident faces below.
            continue;
        }
        let mut hits = Vec::with_capacity(2);
        for &i in triangle {
            if projected[i as usize].side == 0 {
                hits.push(Node::Vertex(i));
            }
        }
        let [a, b, c] = *triangle;
        for (a, b) in [(a, b), (b, c), (c, a)] {
            if projected[a as usize].side * projected[b as usize].side == -1 {
                hits.push(Node::Edge(ordered(a, b)));
            }
        }
        if hits.len() == 2 {
            insert_segment(&mut segments, hits[0], hits[1])?;
        }
    }
    for (&(a, b), uses) in &edges {
        if projected[a as usize].side != 0 || projected[b as usize].side != 0 {
            continue;
        }
        let s = uses.map(|u| projected[u.opposite as usize].side);
        // Keep a transverse exact edge once, or a coplanar patch boundary.
        // Same-side tangencies and interior coplanar diagonals have no boundary.
        if s[0] * s[1] == -1 || (s[0] == 0) != (s[1] == 0) {
            insert_segment(&mut segments, Node::Vertex(a), Node::Vertex(b))?;
        }
    }
    let mut positions = BTreeMap::new();
    let mut graph: BTreeMap<Node, Vec<Node>> = BTreeMap::new();
    for &(a, b) in &segments {
        for node in [a, b] {
            positions.entry(node).or_insert_with(|| match node {
                Node::Vertex(i) => projected[i as usize].point,
                Node::Edge((a, b)) => {
                    let (a, b) = (projected[a as usize], projected[b as usize]);
                    let t = a.distance / (a.distance - b.distance);
                    Point2::new(
                        a.point.x + (b.point.x - a.point.x) * t,
                        a.point.y + (b.point.y - a.point.y) * t,
                    )
                }
            });
        }
        ensure(
            positions[&a].distance(positions[&b]) > SECTION_TOLERANCE,
            "section segment is below tolerance",
        )?;
        graph.entry(a).or_default().push(b);
        graph.entry(b).or_default().push(a);
    }
    ensure(
        graph.values().all(|neighbors| neighbors.len() == 2),
        "open or non-manifold section contour",
    )?;
    validate_intersections(&segments, &positions)?;
    let mut visited = BTreeSet::new();
    let mut rings = Vec::new();
    for &start in graph.keys() {
        if visited.contains(&start) {
            continue;
        }
        ensure(
            rings.len() < MAX_SECTION_CONTOURS,
            "section contour limit exceeded",
        )?;
        let mut ring = Vec::new();
        let mut current = start;
        let mut previous = None;
        loop {
            ensure(visited.insert(current), "section contour revisits a vertex")?;
            ring.push(positions[&current]);
            let neighbors = &graph[&current];
            let next = if Some(neighbors[0]) == previous {
                neighbors[1]
            } else {
                neighbors[0]
            };
            if next == start {
                break;
            }
            previous = Some(current);
            current = next;
        }
        simplify(&mut ring);
        ensure(
            ring.len() >= 3 && signed_area(&ring).abs() > SECTION_TOLERANCE.powi(2),
            "section contour has negligible area",
        )?;
        rings.push(ring);
    }
    let mut result = Vec::with_capacity(rings.len());
    for (i, ring) in rings.iter().enumerate() {
        let depth = rings
            .iter()
            .enumerate()
            .filter(|(j, other)| *j != i && contains(other, ring[0]))
            .count();
        let outer = depth % 2 == 0;
        let mut points = ring.clone();
        if (signed_area(&points) > 0.0) != outer {
            points.reverse();
        }
        let first = (0..points.len())
            .min_by(|&a, &b| compare_points(points[a], points[b]))
            .ok_or_else(|| invalid("empty section contour"))?;
        points.rotate_left(first);
        result.push(CutContour {
            points,
            kind: if outer {
                CutContourKind::Outer
            } else {
                CutContourKind::Hole
            },
            nesting_depth: depth,
        });
    }
    result.sort_by(|a, b| {
        a.points
            .iter()
            .zip(&b.points)
            .map(|(&a, &b)| compare_points(a, b))
            .find(|order| !order.is_eq())
            .unwrap_or_else(|| a.points.len().cmp(&b.points.len()))
    });
    Ok(result)
}

fn closed_topology(mesh: &Mesh, projected: &[Projected]) -> Result<BTreeMap<Edge, [EdgeUse; 2]>> {
    let mut edges: BTreeMap<Edge, Vec<EdgeUse>> = BTreeMap::new();
    let mut faces = BTreeSet::new();
    let mut links = vec![Vec::new(); mesh.vertices.len()];
    for &[a, b, c] in &mesh.triangles {
        let mut face = [a, b, c];
        face.sort_unstable();
        ensure(faces.insert(face), "duplicate section mesh triangle")?;
        for (a, b, opposite) in [(a, b, c), (b, c, a), (c, a, b)] {
            let uses = edges.entry(ordered(a, b)).or_default();
            ensure(uses.len() < 2, "non-manifold section mesh edge")?;
            uses.push(EdgeUse {
                forward: a < b,
                opposite,
            });
            links[a as usize].push((b, opposite));
        }
    }
    let mut closed = BTreeMap::new();
    for (edge, uses) in edges {
        ensure(uses.len() == 2, "open section mesh edge")?;
        ensure(
            uses[0].forward != uses[1].forward,
            "inconsistently wound section mesh",
        )?;
        closed.insert(edge, [uses[0], uses[1]]);
    }
    // A whole shell inside the plane band is unresolved, not an empty cut.
    // This also rejects planar closed triangle soups with zero enclosed volume.
    let mut visited = vec![false; links.len()];
    for start in 0..links.len() {
        if visited[start] {
            continue;
        }
        let mut pending = vec![start];
        visited[start] = true;
        let mut off_plane = false;
        while let Some(i) = pending.pop() {
            off_plane |= projected[i].side != 0;
            for &(a, b) in &links[i] {
                for j in [a as usize, b as usize] {
                    if !visited[j] {
                        visited[j] = true;
                        pending.push(j);
                    }
                }
            }
        }
        ensure(off_plane, "section shell collapses within plane tolerance")?;
    }
    // Edge manifoldness alone misses two otherwise closed fans sharing a vertex.
    // With consistent winding each vertex link must be one directed cycle.
    for link in links {
        let mut next = BTreeMap::new();
        for (a, b) in link {
            ensure(
                next.insert(a, b).is_none(),
                "non-manifold section mesh vertex",
            )?;
        }
        let &start = next
            .keys()
            .next()
            .ok_or_else(|| invalid("unused section mesh vertex"))?;
        let mut current = start;
        for step in 0..next.len() {
            current = *next
                .get(&current)
                .ok_or_else(|| invalid("open section mesh vertex fan"))?;
            ensure(
                current != start || step + 1 == next.len(),
                "disconnected section mesh vertex fan",
            )?;
        }
        ensure(current == start, "open section mesh vertex fan")?;
    }
    Ok(closed)
}

fn insert_segment(segments: &mut BTreeSet<(Node, Node)>, a: Node, b: Node) -> Result<()> {
    let segment = ordered(a, b);
    if !segments.contains(&segment) {
        ensure(
            segments.len() < MAX_SECTION_SEGMENTS,
            "section segment limit exceeded",
        )?;
        segments.insert(segment);
    }
    Ok(())
}

fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn signed_area(ring: &[Point2]) -> f64 {
    ring[1..]
        .windows(2)
        .map(|p| cross(ring[0], p[0], p[1]))
        .sum::<f64>()
        * 0.5
}

fn compare_points(a: Point2, b: Point2) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
}

fn validate_intersections(
    segments: &BTreeSet<(Node, Node)>,
    positions: &BTreeMap<Node, Point2>,
) -> Result<()> {
    let segments: Vec<_> = segments
        .iter()
        .map(|&(a, b)| (a, b, positions[&a], positions[&b]))
        .collect();
    for (i, &(a, b, pa, pb)) in segments.iter().enumerate() {
        for &(c, d, pc, pd) in &segments[i + 1..] {
            let shared = [a, b].into_iter().find(|n| *n == c || *n == d);
            if let Some(shared) = shared {
                let (p, q) = if a == shared { (pa, pb) } else { (pb, pa) };
                let r = if c == shared { pd } else { pc };
                let dot = (q.x - p.x) * (r.x - p.x) + (q.y - p.y) * (r.y - p.y);
                ensure(
                    dot <= 0.0
                        || cross(p, q, r).abs()
                            > SECTION_TOLERANCE * p.distance(q).max(p.distance(r)),
                    "overlapping adjacent section segments",
                )?;
            } else {
                ensure(
                    !segments_touch(pa, pb, pc, pd),
                    "intersecting or touching section contours",
                )?;
            }
        }
    }
    Ok(())
}

fn segments_touch(a: Point2, b: Point2, c: Point2, d: Point2) -> bool {
    let e = SECTION_TOLERANCE;
    if a.x.min(b.x) > c.x.max(d.x) + e
        || c.x.min(d.x) > a.x.max(b.x) + e
        || a.y.min(b.y) > c.y.max(d.y) + e
        || c.y.min(d.y) > a.y.max(b.y) + e
    {
        return false;
    }
    let separated = |a: Point2, b: Point2, c: Point2, d: Point2| {
        let tolerance = e * a.distance(b);
        let (c, d) = (cross(a, b, c), cross(a, b, d));
        (c > tolerance && d > tolerance) || (c < -tolerance && d < -tolerance)
    };
    !separated(a, b, c, d) && !separated(c, d, a, b)
}

fn simplify(ring: &mut Vec<Point2>) {
    loop {
        let remove = (0..ring.len()).find(|&i| {
            let (a, b, c) = (
                ring[(i + ring.len() - 1) % ring.len()],
                ring[i],
                ring[(i + 1) % ring.len()],
            );
            let dot = (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y);
            dot >= 0.0 && cross(a, b, c).abs() <= SECTION_TOLERANCE * a.distance(c)
        });
        if ring.len() <= 3 {
            break;
        }
        if let Some(i) = remove {
            ring.remove(i);
        } else {
            break;
        }
    }
}

fn contains(ring: &[Point2], p: Point2) -> bool {
    let mut inside = false;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (b.x - a.x) * ((p.y - a.y) / (b.y - a.y)) {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GeometryKernel, PrismKernel, Profile, Solid, Transform, Vec3, floors::extrude_floor,
    };

    fn plane(x: f64, y: f64, dx: f64, dy: f64) -> VerticalSectionPlane {
        VerticalSectionPlane {
            origin: Point2::new(x, y),
            direction: Point2::new(dx, dy),
        }
    }

    fn slab(x0: f64, x1: f64, y0: f64, y1: f64, bottom: f64, top: f64) -> Mesh {
        extrude_floor(
            &[
                Point2::new(x0, y0),
                Point2::new(x1, y0),
                Point2::new(x1, y1),
                Point2::new(x0, y1),
            ],
            top,
            top - bottom,
        )
        .unwrap()
    }

    fn cube() -> Mesh {
        slab(0., 2., 0., 2., 0., 2.)
    }

    fn append(a: &mut Mesh, b: Mesh) {
        let offset = a.vertices.len() as u32;
        a.vertices.extend(b.vertices);
        a.triangles
            .extend(b.triangles.into_iter().map(|t| t.map(|i| i + offset)));
    }

    fn rectangle(c: &CutContour, x0: f64, x1: f64, z0: f64, z1: f64) {
        assert_eq!(c.kind, CutContourKind::Outer);
        assert_eq!(c.points.len(), 4, "{:?}", c.points);
        for (actual, (x, z)) in c
            .points
            .iter()
            .zip([(x0, z0), (x1, z0), (x1, z1), (x0, z1)])
        {
            assert!(
                actual.distance(Point2::new(x, z)) < 1e-8,
                "{actual:?}, {x}, {z}"
            );
        }
    }

    #[test]
    fn rotated_wall_has_metric_distance_absolute_elevation_and_no_triangle_seams() {
        let angle: f64 = 0.73;
        let mesh = PrismKernel
            .tessellate(&Solid {
                profile: Profile {
                    vertices: vec![
                        Point2::new(0., -0.1),
                        Point2::new(4., -0.1),
                        Point2::new(4., 0.1),
                        Point2::new(0., 0.1),
                    ],
                },
                height: 3.,
                transform: Transform {
                    translation: Vec3::new(10., 20., 5.),
                    rotation_z: angle,
                },
            })
            .unwrap();
        let original = mesh.clone();
        let cut =
            vertical_section(&mesh, plane(10., 20., 7. * angle.cos(), 7. * angle.sin())).unwrap();
        assert_eq!(cut.len(), 1);
        rectangle(&cut[0], 0., 4., 5., 8.);
        assert_eq!(mesh, original);
    }

    #[test]
    fn concave_slab_can_cut_into_two_disjoint_regions() {
        let boundary = [
            (0., 0.),
            (4., 0.),
            (4., 4.),
            (3., 4.),
            (3., 1.),
            (1., 1.),
            (1., 4.),
            (0., 4.),
        ]
        .map(|(x, y)| Point2::new(x, y));
        let mesh = extrude_floor(&boundary, 3., 0.3).unwrap();
        let cut = vertical_section(&mesh, plane(0., 2., 1., 0.)).unwrap();
        assert_eq!(cut.len(), 2);
        rectangle(&cut[0], 0., 1., 2.7, 3.);
        rectangle(&cut[1], 3., 4., 2.7, 3.);
    }

    #[test]
    fn exact_edges_coplanar_faces_and_tangencies_are_not_triangle_soup() {
        let mesh = cube();
        let diagonal = vertical_section(&mesh, plane(0., 0., 1., 1.)).unwrap();
        assert_eq!(diagonal.len(), 1);
        rectangle(&diagonal[0], 0., 8.0_f64.sqrt(), 0., 2.);
        for y in [0., 2.] {
            let face = vertical_section(&mesh, plane(0., y, 1., 0.)).unwrap();
            assert_eq!(face.len(), 1);
            rectangle(&face[0], 0., 2., 0., 2.);
        }
        for p in [
            plane(0., 0., 1., -1.),
            plane(2., 2., 1., -1.),
            plane(0., 3., 1., 0.),
        ] {
            assert!(vertical_section(&mesh, p).unwrap().is_empty());
        }
    }

    #[test]
    fn exact_vertex_intersection_is_a_closed_triangle_and_point_tangent_is_empty() {
        let mesh = Mesh {
            surfaces: vec![],
            vertices: vec![
                Vec3::new(0., 0., 2.),
                Vec3::new(0., -1., 0.),
                Vec3::new(2., 1., 0.),
                Vec3::new(-2., 1., 0.),
            ],
            triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        };
        let cut = vertical_section(&mesh, plane(0., 0., 1., 0.)).unwrap();
        assert_eq!(cut.len(), 1);
        assert_eq!(
            cut[0].points,
            vec![
                Point2::new(-1., 0.),
                Point2::new(1., 0.),
                Point2::new(0., 2.)
            ]
        );
        assert!(
            vertical_section(&mesh, plane(2., 0., 0., 1.))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn nested_shells_have_even_odd_holes_and_islands() {
        let mut mesh = slab(0., 6., -3., 3., 0., 6.);
        append(&mut mesh, slab(1., 5., -2., 2., 1., 5.));
        append(&mut mesh, slab(2., 4., -1., 1., 2., 4.));
        let cut = vertical_section(&mesh, plane(0., 0., 1., 0.)).unwrap();
        assert_eq!(cut.len(), 3);
        for (depth, c) in cut.iter().enumerate() {
            assert_eq!(c.nesting_depth, depth);
            assert_eq!(signed_area(&c.points) > 0., depth % 2 == 0);
            assert_eq!(
                c.kind,
                if depth == 1 {
                    CutContourKind::Hole
                } else {
                    CutContourKind::Outer
                }
            );
        }
    }

    #[test]
    fn one_connected_hollow_prism_returns_outer_and_hole() {
        let mut mesh = Mesh::default();
        for y in [-1., 1.] {
            for (x, z) in [
                (0., 0.),
                (4., 0.),
                (4., 4.),
                (0., 4.),
                (1., 1.),
                (3., 1.),
                (3., 3.),
                (1., 3.),
            ] {
                mesh.vertices.push(Vec3::new(x, y, z));
            }
        }
        let mut quad = |[a, b, c, d]: [u32; 4]| mesh.triangles.extend([[a, b, c], [a, c, d]]);
        for i in 0..4 {
            let j = (i + 1) % 4;
            quad([i, j, j + 4, i + 4]);
            quad([i + 8, i + 12, j + 12, j + 8]);
            quad([i, i + 8, j + 8, j]);
            quad([i + 4, j + 4, j + 12, i + 12]);
        }
        let cut = vertical_section(&mesh, plane(0., 0., 1., 0.)).unwrap();
        assert_eq!(cut.len(), 2);
        rectangle(&cut[0], 0., 4., 0., 4.);
        assert_eq!(cut[1].kind, CutContourKind::Hole);
        assert_eq!(cut[1].nesting_depth, 1);
        assert_eq!(
            cut[1].points,
            vec![
                Point2::new(1., 1.),
                Point2::new(1., 3.),
                Point2::new(3., 3.),
                Point2::new(3., 1.)
            ]
        );
    }

    #[test]
    fn manifold_saddle_cut_rejects_branching_at_exact_vertex() {
        // A closed solid between a saddle-shaped upper surface and a lower
        // pyramid. The Y=0 cut has four branches at vertex zero.
        let mut mesh = Mesh {
            surfaces: vec![],
            vertices: vec![
                Vec3::new(0., 0., 0.),
                Vec3::new(0., -3., 0.),
                Vec3::new(1., 1., 0.),
                Vec3::new(0., -1., 1.),
                Vec3::new(-1., 1., 0.),
                Vec3::new(0., -1., -1.),
            ],
            triangles: Vec::new(),
        };
        for i in 0..4 {
            let (a, b) = (i + 2, (i + 1) % 4 + 2);
            mesh.triangles.extend([[0, a, b], [1, b, a]]);
        }
        mesh.validate().unwrap();
        let error = vertical_section(&mesh, plane(0., 0., 1., 0.)).unwrap_err();
        assert!(
            error.to_string().contains("non-manifold section contour"),
            "{error}"
        );
    }

    #[test]
    fn shell_wholly_inside_tolerance_is_rejected_even_beside_a_valid_cut() {
        let mut thin = cube();
        for v in &mut thin.vertices {
            v.y = (v.y - 1.) * SECTION_TOLERANCE / 2.;
        }
        thin.validate().unwrap();
        let p = plane(0., 0., 1., 0.);
        assert!(
            vertical_section(&thin, p)
                .unwrap_err()
                .to_string()
                .contains("plane tolerance")
        );
        append(&mut thin, slab(4., 6., -1., 1., 0., 2.));
        assert!(
            vertical_section(&thin, p)
                .unwrap_err()
                .to_string()
                .contains("plane tolerance")
        );
    }

    #[test]
    fn order_winding_and_positive_direction_scale_do_not_change_output() {
        let mut mesh = cube();
        let expected = vertical_section(&mesh, plane(0., 1., 1., 0.)).unwrap();
        mesh.triangles.reverse();
        for t in &mut mesh.triangles {
            t.rotate_left(1);
        }
        assert_eq!(
            vertical_section(&mesh, plane(0., 1., f64::MAX, 0.)).unwrap(),
            expected
        );
        for t in &mut mesh.triangles {
            t.swap(0, 1);
        }
        assert_eq!(
            vertical_section(&mesh, plane(0., 1., f64::MIN_POSITIVE, 0.)).unwrap(),
            expected
        );
        let reversed = vertical_section(&mesh, plane(0., 1., -1., 0.)).unwrap();
        rectangle(&reversed[0], -2., 0., 0., 2.);
    }

    #[test]
    fn malformed_geometry_and_invalid_planes_fail_even_when_plane_misses() {
        let p = plane(0., 9., 1., 0.);
        let mut cases = vec![Mesh::default()];
        let mut m = cube();
        m.triangles.pop();
        cases.push(m);
        let mut m = cube();
        m.triangles.push(m.triangles[0]);
        cases.push(m);
        let mut m = cube();
        m.triangles[0].swap(0, 1);
        cases.push(m);
        let mut m = cube();
        m.triangles[0][0] = 999;
        cases.push(m);
        let mut m = cube();
        m.vertices[0].x = f64::NAN;
        cases.push(m);
        let mut m = cube();
        m.vertices[0].z = f64::INFINITY;
        cases.push(m);
        let mut m = cube();
        m.vertices[0] = m.vertices[1];
        cases.push(m);
        let mut m = cube();
        m.vertices.push(Vec3::default());
        cases.push(m);
        let mut m = cube();
        m.vertices[0].x = MAX_SECTION_COORDINATE + 1.;
        cases.push(m);
        for m in cases {
            assert!(vertical_section(&m, p).is_err());
        }
        for p in [
            plane(0., 0., 0., 0.),
            plane(f64::NAN, 0., 1., 0.),
            plane(0., 0., 1., f64::INFINITY),
            plane(1e7, 0., 1., 0.),
        ] {
            assert!(vertical_section(&cube(), p).is_err());
        }
    }

    #[test]
    fn closed_edge_counts_do_not_hide_a_non_manifold_vertex() {
        let mut mesh = cube();
        let other = slab(-2., 0., -2., 0., -2., 0.);
        // Weld only the two origins, creating two disconnected vertex fans.
        let shared = other
            .vertices
            .iter()
            .position(|v| *v == Vec3::default())
            .unwrap();
        let mut ids = Vec::new();
        for (i, v) in other.vertices.into_iter().enumerate() {
            if i == shared {
                ids.push(0);
            } else {
                ids.push(mesh.vertices.len() as u32);
                mesh.vertices.push(v);
            }
        }
        mesh.triangles.extend(
            other
                .triangles
                .into_iter()
                .map(|t| t.map(|i| ids[i as usize])),
        );
        mesh.validate().unwrap();
        assert!(
            vertical_section(&mesh, plane(0., 9., 1., 0.))
                .unwrap_err()
                .to_string()
                .contains("vertex fan")
        );
    }

    #[test]
    fn intersecting_touching_and_duplicate_shell_cuts_are_rejected() {
        for second in [
            slab(1., 3., 0., 2., 1., 3.),
            slab(2., 4., 0., 2., 0., 2.),
            cube(),
        ] {
            let mut mesh = cube();
            append(&mut mesh, second);
            assert!(vertical_section(&mesh, plane(0., 1., 1., 0.)).is_err());
        }
    }

    #[test]
    fn bounds_reject_without_returning_partial_geometry() {
        let mut mesh = cube();
        mesh.vertices
            .resize(MAX_SECTION_INPUT_VERTICES + 1, Vec3::default());
        assert!(
            vertical_section(&mesh, plane(0., 1., 1., 0.))
                .unwrap_err()
                .to_string()
                .contains("input limit")
        );
        let mut mesh = cube();
        mesh.triangles
            .resize(MAX_SECTION_INPUT_TRIANGLES + 1, [0, 1, 2]);
        assert!(
            vertical_section(&mesh, plane(0., 1., 1., 0.))
                .unwrap_err()
                .to_string()
                .contains("input limit")
        );
        for (count, message) in [
            (MAX_SECTION_CONTOURS + 1, "contour limit"),
            (MAX_SECTION_SEGMENTS / 8 + 1, "segment limit"),
        ] {
            let mut mesh = Mesh::default();
            for i in 0..count {
                append(
                    &mut mesh,
                    slab(3. * i as f64, 3. * i as f64 + 2., -1., 1., 0., 2.),
                );
            }
            assert!(
                vertical_section(&mesh, plane(0., 0., 1., 0.))
                    .unwrap_err()
                    .to_string()
                    .contains(message)
            );
        }
    }

    #[test]
    fn large_coordinates_and_plane_tolerance_are_explicit() {
        let mesh = slab(999_990., 999_992., 999_990., 999_992., -20., -19.75);
        let cut = vertical_section(&mesh, plane(999_990., 999_991., 1., 0.)).unwrap();
        rectangle(&cut[0], 0., 2., -20., -19.75);
        let face = vertical_section(&cube(), plane(0., -SECTION_TOLERANCE / 2., 1., 0.)).unwrap();
        rectangle(&face[0], 0., 2., 0., 2.);
        assert!(
            vertical_section(&cube(), plane(0., -2. * SECTION_TOLERANCE, 1., 0.))
                .unwrap()
                .is_empty()
        );
    }
}
