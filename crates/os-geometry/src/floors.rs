//! Deterministic polygon triangulation and closed horizontal slab extrusion.
//! Independent of the rectangular prism kernel and semantic entity storage.
use crate::{Mesh, Vec3};
use os_core::{Point2, Result, ensure};

fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Signed area, translated to the first vertex to retain large-coordinate detail.
pub fn signed_area(boundary: &[Point2]) -> f64 {
    let Some(&origin) = boundary.first() else {
        return 0.0;
    };
    boundary[1..]
        .windows(2)
        .map(|p| cross(origin, p[0], p[1]))
        .sum::<f64>()
        * 0.5
}

/// Return CCW triangles indexing the unmodified authored ring. Both input windings
/// and collinear forward vertices are accepted. Ear order is stable by ring order.
pub fn triangulate_floor(boundary: &[Point2]) -> Result<Vec<[u32; 3]>> {
    let p = boundary;
    ensure((3..=256).contains(&p.len()), "floor needs 3-256 vertices")?;
    ensure(
        p.iter()
            .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
        "invalid floor coordinates",
    )?;
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            ensure(p[i].distance(p[j]) > 1e-6, "duplicate floor vertices")?;
            if j == i + 1 || (i == 0 && j == p.len() - 1) {
                continue;
            }
            let (a, b, c, d) = (p[i], p[(i + 1) % p.len()], p[j], p[(j + 1) % p.len()]);
            let overlap = a.x.min(b.x) <= c.x.max(d.x) + 1e-9
                && c.x.min(d.x) <= a.x.max(b.x) + 1e-9
                && a.y.min(b.y) <= c.y.max(d.y) + 1e-9
                && c.y.min(d.y) <= a.y.max(b.y) + 1e-9;
            ensure(
                !(overlap
                    && cross(a, b, c) * cross(a, b, d) <= 0.0
                    && cross(c, d, a) * cross(c, d, b) <= 0.0),
                "floor boundary self-intersects",
            )?;
        }
        let (a, b, c) = (p[(i + p.len() - 1) % p.len()], p[i], p[(i + 1) % p.len()]);
        ensure(
            cross(a, b, c).abs() > 1e-12
                || (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y) > 0.0,
            "floor boundary doubles back",
        )?;
    }
    let area = signed_area(p);
    ensure(area.abs() > 1e-8, "floor area is negligible")?;
    let mut ring: Vec<usize> = (0..p.len()).collect();
    if area < 0.0 {
        ring.reverse();
    }
    let mut triangles = Vec::with_capacity(p.len() - 2);
    while ring.len() > 3 {
        let ear = (0..ring.len())
            .find(|&i| {
                let [a, b, c] = [
                    ring[(i + ring.len() - 1) % ring.len()],
                    ring[i],
                    ring[(i + 1) % ring.len()],
                ];
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
            .ok_or_else(|| os_core::Error::Invalid("floor triangulation failed".into()))?;
        triangles.push([
            ring[(ear + ring.len() - 1) % ring.len()] as u32,
            ring[ear] as u32,
            ring[(ear + 1) % ring.len()] as u32,
        ]);
        ring.remove(ear);
    }
    ensure(
        cross(p[ring[0]], p[ring[1]], p[ring[2]]) > 0.0,
        "degenerate floor triangle",
    )?;
    triangles.push([ring[0] as u32, ring[1] as u32, ring[2] as u32]);
    Ok(triangles)
}

/// Extrude down from absolute `top_z` by positive `thickness`. Vertex indices
/// 0..n are the bottom ring and n..2n the top ring, in authored order. All faces
/// wind outward, including for clockwise input. No welding/booleans are involved.
pub fn extrude_floor(boundary: &[Point2], top_z: f64, thickness: f64) -> Result<Mesh> {
    ensure(
        thickness.is_finite()
            && thickness > 1e-6
            && thickness <= 1e6
            && top_z.is_finite()
            && (top_z - thickness).is_finite()
            && top_z - thickness < top_z,
        "invalid floor extrusion heights",
    )?;
    let caps = triangulate_floor(boundary)?;
    let n = boundary.len() as u32;
    let mut mesh = Mesh::default();
    for z in [top_z - thickness, top_z] {
        mesh.vertices
            .extend(boundary.iter().map(|p| Vec3::new(p.x, p.y, z)));
    }
    for [a, b, c] in caps {
        mesh.triangles.push([a, c, b]);
        mesh.triangles.push([a + n, b + n, c + n]);
    }
    let ccw = signed_area(boundary) > 0.0;
    for i in 0..n {
        let (a, b) = if ccw {
            (i, (i + 1) % n)
        } else {
            ((i + 1) % n, i)
        };
        mesh.triangles.extend([[a, b, b + n], [a, b + n, a + n]]);
    }
    mesh.validate()?;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ring() -> Vec<Point2> {
        [(0., 0.), (4., 0.), (4., 1.), (1., 1.), (1., 4.), (0., 4.)]
            .map(|(x, y)| Point2::new(x, y))
            .to_vec()
    }
    #[test]
    fn concave_mesh_is_deterministic_closed_and_outward_for_both_windings() {
        let mut p = ring();
        for _ in 0..2 {
            let m = extrude_floor(&p, 2.0, 0.25).unwrap();
            assert_eq!(m, extrude_floor(&p, 2.0, 0.25).unwrap());
            assert_eq!(m.vertices.len(), 12);
            assert_eq!(m.triangles.len(), 20);
            assert!((m.signed_volume() - 1.75).abs() < 1e-10);
            let caps = triangulate_floor(&p).unwrap();
            assert!(
                (caps
                    .iter()
                    .map(|t| cross(p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]) * 0.5)
                    .sum::<f64>()
                    - 7.0)
                    .abs()
                    < 1e-10
            );
            let mut edges = std::collections::BTreeMap::new();
            for [a, b, c] in &m.triangles {
                for e in [(*a, *b), (*b, *c), (*c, *a)] {
                    *edges.entry(e).or_insert(0) += 1;
                }
            }
            for ((a, b), count) in &edges {
                assert_eq!(*count, 1);
                assert_eq!(edges.get(&(*b, *a)), Some(&1));
            }
            p.reverse();
        }
    }
    #[test]
    fn collinear_vertices_large_coordinates_and_limits() {
        let mut p = ring();
        p.insert(1, Point2::new(2., 0.));
        for v in &mut p {
            v.x += 999_990.;
            v.y += 999_990.;
        }
        extrude_floor(&p, 0., 0.2).unwrap();
        p.reverse();
        extrude_floor(&p, 0., 0.2).unwrap();
        assert!(extrude_floor(&p, 0., -0.2).is_err());
        assert!(extrude_floor(&p, f64::MAX, 0.2).is_err());
        assert!(triangulate_floor(&vec![Point2::new(0., 0.); 257]).is_err());
        assert!(
            triangulate_floor(&[
                Point2::new(0., 0.),
                Point2::new(2., 2.),
                Point2::new(0., 2.),
                Point2::new(2., 0.)
            ])
            .is_err()
        );
        let circle: Vec<_> = (0..256)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 256.0;
                Point2::new(a.cos(), a.sin())
            })
            .collect();
        assert_eq!(triangulate_floor(&circle).unwrap().len(), 254);
    }
    #[test]
    fn concave_radial_rings_conserve_area_and_volume_across_vertex_counts() {
        for n in [5, 8, 17, 64, 255, 256] {
            let mut p: Vec<_> = (0..n)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / n as f64;
                    let r = if i % 2 == 0 { 4.0 } else { 2.0 };
                    Point2::new(r * a.cos(), r * a.sin())
                })
                .collect();
            for _ in 0..2 {
                let area = signed_area(&p).abs();
                let mesh = extrude_floor(&p, 3.0, 0.3).unwrap();
                assert!((mesh.signed_volume() - area * 0.3).abs() < 1e-9);
                assert_eq!(mesh.triangles.len(), 4 * n - 4);
                p.reverse();
            }
        }
    }
}
