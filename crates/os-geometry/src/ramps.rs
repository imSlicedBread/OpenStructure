//! Closed straight ramp slabs with vertical thickness below the walk surface.
use crate::{Mesh, SurfaceIdentity, Vec3};
use os_core::Result;
use os_model::RampParams;

/// Generate one closed, outward-wound rectangular ramp slab (8 vertices,
/// 12 triangles). `start` is the lower walk-edge centre at `lower_z`; `end`
/// is the upper walk-edge centre at `upper_z`. End faces are vertical.
/// Thickness is vertical, so the slab volume is run * width * thickness.
/// Level and material references are resolved by the caller; geometry inputs
/// and coordinate bounds are validated here. No landings or railings are added.
pub fn ramp_mesh(params: &RampParams, lower_z: f64, upper_z: f64) -> Result<Mesh> {
    let dimensions = params.dimensions(lower_z, upper_z)?;
    let dx = (params.end.x - params.start.x) / dimensions.run;
    let dy = (params.end.y - params.start.y) / dimensions.run;
    let half_width = params.width * 0.5;
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(8),
        triangles: Vec::with_capacity(12),
        surfaces: Vec::new(),
    };
    // Counterclockwise in plan: lower right, upper right, upper left, lower left.
    // The bottom and top share their XY coordinates and differ only in Z.
    for offset in [-params.structural_thickness, 0.0] {
        for (point, z, side) in [
            (params.start, lower_z, -half_width),
            (params.end, upper_z, -half_width),
            (params.end, upper_z, half_width),
            (params.start, lower_z, half_width),
        ] {
            mesh.vertices.push(Vec3::new(
                point.x - dy * side,
                point.y + dx * side,
                z + offset,
            ));
        }
    }
    mesh.triangles
        .extend([[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7]]);
    for i in 0..4 {
        let j = (i + 1) % 4;
        mesh.triangles.extend([[i, j, j + 4], [i, j + 4, i + 4]]);
    }
    mesh.surfaces.resize(
        mesh.triangles.len(),
        SurfaceIdentity {
            layer: None,
            material: params.material,
        },
    );
    mesh.validate()?;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_core::{Id, Point2};
    use std::collections::BTreeMap;

    fn params() -> RampParams {
        RampParams {
            name: "Ramp".into(),
            lower_level: Id::new(),
            upper_level: Id::new(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(12.0, 0.0),
            width: 1.5,
            structural_thickness: 0.2,
            material: Some(Id::new()),
        }
    }

    fn assert_closed(mesh: &Mesh) {
        let mut edges = BTreeMap::new();
        for &[a, b, c] in &mesh.triangles {
            for edge in [(a, b), (b, c), (c, a)] {
                *edges.entry(edge).or_insert(0) += 1;
            }
        }
        for (&(a, b), &count) in &edges {
            assert_eq!(count, 1);
            assert_eq!(edges.get(&(b, a)), Some(&1));
        }
        assert_eq!(
            mesh.vertices.len() + mesh.triangles.len(),
            edges.len() / 2 + 2
        );
    }

    #[test]
    fn closed_outward_deterministic_and_material_preserving_in_every_direction() {
        let mut p = params();
        p.start = Point2::new(7.0, -5.0);
        for (dx, dy) in [(12.0, 0.0), (-12.0, 0.0), (0.0, 12.0), (7.2, -9.6)] {
            p.end = Point2::new(p.start.x + dx, p.start.y + dy);
            for material in [Some(Id::new()), None] {
                p.material = material;
                for reversed in [false, true] {
                    let mut directed = p.clone();
                    if reversed {
                        std::mem::swap(&mut directed.start, &mut directed.end);
                    }
                    let mesh = ramp_mesh(&directed, -2.0, 1.0).unwrap();
                    assert_eq!(mesh, ramp_mesh(&directed, -2.0, 1.0).unwrap());
                    assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (8, 12));
                    assert_closed(&mesh);
                    let expected = directed.run() * directed.width * directed.structural_thickness;
                    assert!((mesh.signed_volume() - expected).abs() < expected * 1e-10);
                    assert_eq!(mesh.surfaces.len(), mesh.triangles.len());
                    assert!(
                        mesh.surfaces
                            .iter()
                            .all(|s| s.material == material && s.layer.is_none())
                    );
                    // Every triangle points away from an interior point of this convex slab.
                    let centre = Vec3::new(
                        (directed.start.x + directed.end.x) * 0.5,
                        (directed.start.y + directed.end.y) * 0.5,
                        -0.5 - directed.structural_thickness * 0.5,
                    );
                    for t in &mesh.triangles {
                        let [a, b, c] = t.map(|i| mesh.vertices[i as usize]);
                        let u = Vec3::new(b.x - a.x, b.y - a.y, b.z - a.z);
                        let v = Vec3::new(c.x - a.x, c.y - a.y, c.z - a.z);
                        let dot = (u.y * v.z - u.z * v.y) * (a.x - centre.x)
                            + (u.z * v.x - u.x * v.z) * (a.y - centre.y)
                            + (u.x * v.y - u.y * v.x) * (a.z - centre.z);
                        assert!(dot > 0.0);
                    }
                }
            }
        }
    }

    #[test]
    fn elevations_width_and_vertical_end_faces_match_authored_dimensions() {
        let mut p = params();
        p.start = Point2::new(7.0, -5.0);
        p.end = Point2::new(14.2, 4.6);
        let mesh = ramp_mesh(&p, 2.0, 5.0).unwrap();
        let dx = (p.end.x - p.start.x) / p.run();
        let dy = (p.end.y - p.start.y) / p.run();
        for i in 0..4 {
            let bottom = mesh.vertices[i];
            let top = mesh.vertices[i + 4];
            assert_eq!((bottom.x, bottom.y), (top.x, top.y));
            assert!((top.z - bottom.z - p.structural_thickness).abs() < 1e-12);
            let (station, elevation) = if i == 0 || i == 3 {
                (0.0, 2.0)
            } else {
                (p.run(), 5.0)
            };
            assert_eq!(top.z, elevation);
            let x = top.x - p.start.x;
            let y = top.y - p.start.y;
            assert!((dx * x + dy * y - station).abs() < 1e-12);
            let side = if i < 2 { -p.width / 2.0 } else { p.width / 2.0 };
            assert!((-dy * x + dx * y - side).abs() < 1e-12);
        }
    }

    #[test]
    fn rejects_invalid_geometry_and_accepts_full_coordinate_envelope() {
        let p = params();
        for case in 0..8 {
            let mut bad = p.clone();
            match case {
                0 => bad.end = bad.start,
                1 => bad.width = f64::NAN,
                2 => bad.width = -1.0,
                3 => bad.structural_thickness = 0.0,
                4 => bad.end.x = f64::INFINITY,
                5 => bad.end.y = 1e7,
                6 => {
                    bad.start.y = 1e6;
                    bad.end.y = 1e6;
                }
                _ => bad.upper_level = bad.lower_level,
            }
            assert!(ramp_mesh(&bad, 0.0, 3.0).is_err(), "case {case}");
        }
        for (lower, upper) in [
            (0.0, 0.0),
            (3.0, 0.0),
            (0.0, f64::NAN),
            (f64::INFINITY, 3.0),
            (0.0, 1e7),
            (-1e6, 0.0),
        ] {
            assert!(ramp_mesh(&p, lower, upper).is_err());
        }
        for sign in [-1.0, 1.0] {
            let mut edge = p.clone();
            edge.start = Point2::new(sign * 999_980.0, sign * (1e6 - edge.width / 2.0));
            edge.end = Point2::new(sign * 999_992.0, edge.start.y);
            let (lower, upper) = if sign < 0.0 {
                (-1e6 + edge.structural_thickness, -999_990.0)
            } else {
                (999_990.0, 1e6)
            };
            let mut mesh = ramp_mesh(&edge, lower, upper).unwrap();
            assert_closed(&mesh);
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.x.abs() <= 1e6 && v.y.abs() <= 1e6 && v.z.abs() <= 1e6)
            );
            // Shift to the origin before the volume sum to avoid large-coordinate cancellation.
            let origin = mesh.vertices[0];
            for v in &mut mesh.vertices {
                v.x -= origin.x;
                v.y -= origin.y;
                v.z -= origin.z;
            }
            let expected = edge.run() * edge.width * edge.structural_thickness;
            assert!((mesh.signed_volume() - expected).abs() < expected * 1e-8);
        }
    }
}
