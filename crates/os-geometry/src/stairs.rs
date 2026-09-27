//! A straight stair is one indexed prism with a stepped top and sloped underside.
use crate::{Mesh, SurfaceIdentity, Vec3};
use os_core::Result;
use os_model::StairParams;

/// Generate a closed outward-wound stair, centred on its plan axis. The first
/// riser starts at `start` and `lower_z`; the far edge of the arrival tread is
/// at `end` and `upper_z`. Thickness is measured vertically below the line
/// joining the levels. No landing, railing or slab boolean is generated.
///
/// Each side cap is triangulated in strips with shared indices. Only the outer
/// profile is extruded: there are no internal faces or touching step solids.
/// Work and storage are O(N): 6N+4 vertices, 12N+4 triangles, at most 1540/3076.
/// The current section kernel rejects transverse cuts exactly coplanar with an
/// interior riser (a coplanar patch meets a transverse region along an edge).
pub fn stair_mesh(params: &StairParams, lower_z: f64, upper_z: f64) -> Result<Mesh> {
    let dimensions = params.dimensions(lower_z, upper_z)?;
    let n = params.riser_count as usize;
    let direction = (
        (params.end.x - params.start.x) / dimensions.run,
        (params.end.y - params.start.y) / dimensions.run,
    );
    // Per side: B_i lies on the underside, C_i on the level-to-level line,
    // A_i above C_i at the next riser elevation. A_i -> C_(i+1) is a tread.
    let b = |i: usize| i as u32;
    let c = |i: usize| (n + 1 + i) as u32;
    let a = |i: usize| (2 * n + 2 + i) as u32;
    let side_len = (3 * n + 2) as u32;
    let station = |i: usize| {
        if i == n {
            (params.end, upper_z)
        } else {
            let fraction = i as f64 / n as f64;
            (
                os_core::Point2::new(
                    params.start.x + (params.end.x - params.start.x) * fraction,
                    params.start.y + (params.end.y - params.start.y) * fraction,
                ),
                lower_z + dimensions.rise * fraction,
            )
        }
    };
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(6 * n + 4),
        triangles: Vec::with_capacity(12 * n + 4),
        surfaces: Vec::new(),
    };
    for side in [-params.width * 0.5, params.width * 0.5] {
        let vertex = |i: usize, z: f64| {
            let (point, _) = station(i);
            Vec3::new(
                point.x - direction.1 * side,
                point.y + direction.0 * side,
                z,
            )
        };
        for i in 0..=n {
            mesh.vertices
                .push(vertex(i, station(i).1 - params.structural_thickness));
        }
        for i in 0..=n {
            mesh.vertices.push(vertex(i, station(i).1));
        }
        for i in 0..n {
            mesh.vertices.push(vertex(i, station(i + 1).1));
        }
    }
    for i in 0..n {
        for [u, v, w] in [
            [b(i), b(i + 1), c(i + 1)],
            [b(i), c(i + 1), c(i)],
            [c(i), c(i + 1), a(i)],
        ] {
            mesh.triangles.push([u, v, w]);
            mesh.triangles
                .push([u + side_len, w + side_len, v + side_len]);
        }
    }
    let mut profile: Vec<u32> = (0..=n).map(b).collect();
    profile.push(c(n));
    for i in (0..n).rev() {
        profile.extend([a(i), c(i)]);
    }
    for i in 0..profile.len() {
        let (u, v) = (profile[i], profile[(i + 1) % profile.len()]);
        mesh.triangles
            .extend([[u, v + side_len, v], [u, u + side_len, v + side_len]]);
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
    use crate::section::{CutContourKind, VerticalSectionPlane, vertical_section};
    use os_core::{Id, Point2};
    use os_model::MAX_STAIR_RISERS;
    use std::collections::BTreeMap;

    fn params() -> StairParams {
        StairParams {
            name: "Flight".into(),
            lower_level: Id::new(),
            upper_level: Id::new(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.5, 0.0),
            width: 1.2,
            riser_count: 15,
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
        // One closed genus-zero shell, with all vertices used.
        assert_eq!(
            mesh.vertices.len() + mesh.triangles.len(),
            edges.len() / 2 + 2
        );
    }

    #[test]
    fn stair_is_closed_outward_deterministic_and_bounded() {
        let mut p = params();
        for count in [1, 2, 15, MAX_STAIR_RISERS] {
            p.riser_count = count;
            for end in [
                Point2::new(4.5, 0.0),
                Point2::new(-4.5, 0.0),
                Point2::new(0.0, 4.5),
                Point2::new(2.7, -3.6),
            ] {
                p.end = end;
                let mesh = stair_mesh(&p, -2.0, 1.0).unwrap();
                assert_eq!(mesh, stair_mesh(&p, -2.0, 1.0).unwrap());
                assert_eq!(mesh.vertices.len(), 6 * count as usize + 4);
                assert_eq!(mesh.triangles.len(), 12 * count as usize + 4);
                assert_closed(&mesh);
                // Integral: continuous structural strip plus N triangular wedges.
                let expected =
                    p.width * p.run() * (p.structural_thickness + 3.0 / (2.0 * f64::from(count)));
                assert!((mesh.signed_volume() - expected).abs() < expected * 1e-10);
                assert!(
                    mesh.surfaces
                        .iter()
                        .all(|s| s.material == p.material && s.layer.is_none())
                );
                assert_eq!(mesh.surfaces.len(), mesh.triangles.len());
            }
        }
    }

    #[test]
    fn stair_treads_arrival_underside_and_width_match_authored_dimensions() {
        let mut p = params();
        p.start = Point2::new(7.0, -5.0);
        p.end = Point2::new(9.7, -1.4);
        let mesh = stair_mesh(&p, 2.0, 5.0).unwrap();
        let n = p.riser_count as usize;
        let side_len = 3 * n + 2;
        let (dx, dy) = (
            (p.end.x - p.start.x) / p.run(),
            (p.end.y - p.start.y) / p.run(),
        );
        for side in 0..2 {
            let offset = side * side_len;
            for i in 0..=n {
                let bottom = mesh.vertices[offset + i];
                let line = mesh.vertices[offset + n + 1 + i];
                assert!((bottom.z - (2.0 + i as f64 * 0.2 - p.structural_thickness)).abs() < 1e-12);
                assert!((line.z - bottom.z - p.structural_thickness).abs() < 1e-12);
                let x = line.x - p.start.x;
                let y = line.y - p.start.y;
                assert!((dx * x + dy * y - i as f64 * 0.3).abs() < 1e-12);
                let expected_side = if side == 0 {
                    -p.width / 2.0
                } else {
                    p.width / 2.0
                };
                assert!((-dy * x + dx * y - expected_side).abs() < 1e-12);
                if i < n {
                    let tread_start = mesh.vertices[offset + 2 * n + 2 + i];
                    let tread_end = mesh.vertices[offset + n + 2 + i];
                    assert!((tread_start.z - line.z - 0.2).abs() < 1e-12);
                    assert_eq!(tread_start.z, tread_end.z);
                }
            }
            assert_eq!(mesh.vertices[offset + 3 * n + 1].z, 5.0); // arrival tread
        }
        p.material = None;
        assert!(
            stair_mesh(&p, 2.0, 5.0)
                .unwrap()
                .surfaces
                .iter()
                .all(|s| s.material.is_none())
        );
    }

    #[test]
    fn stair_sections_are_single_contours_longitudinal_and_transverse() {
        let p = params();
        let mesh = stair_mesh(&p, 0.0, 3.0).unwrap();
        for plane in [
            VerticalSectionPlane {
                origin: p.start,
                direction: Point2::new(1.0, 0.0),
            },
            VerticalSectionPlane {
                origin: Point2::new(0.0, p.width / 2.0),
                direction: Point2::new(1.0, 0.0),
            },
            VerticalSectionPlane {
                origin: Point2::new(1.35, 0.0),
                direction: Point2::new(0.0, 1.0),
            },
            VerticalSectionPlane {
                origin: Point2::new(1.5 - 1e-4, 0.0),
                direction: Point2::new(0.0, 1.0),
            },
            VerticalSectionPlane {
                origin: Point2::new(1.5 + 1e-4, 0.0),
                direction: Point2::new(0.0, 1.0),
            },
        ] {
            let contours =
                vertical_section(&mesh, plane).unwrap_or_else(|error| panic!("{plane:?}: {error}"));
            assert_eq!(contours.len(), 1);
            assert_eq!(contours[0].kind, CutContourKind::Outer);
            let area = crate::floors::signed_area(&contours[0].points);
            let expected = if plane.direction.x == 1.0 {
                4.5 * 0.3
            } else {
                let x = plane.origin.x;
                let top = if x > 1.5 { 1.2 } else { 1.0 };
                p.width * (top - (x / 4.5 * 3.0 - p.structural_thickness))
            };
            assert!(
                (area - expected).abs() < 1e-9,
                "{plane:?}: {area} vs {expected}"
            );
        }
    }

    #[test]
    fn stair_exact_riser_cut_reports_existing_coplanar_section_limitation() {
        let mesh = stair_mesh(&params(), 0.0, 3.0).unwrap();
        assert_closed(&mesh);
        let error = vertical_section(
            &mesh,
            VerticalSectionPlane {
                origin: Point2::new(1.5, 0.0),
                direction: Point2::new(0.0, 1.0),
            },
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("open or non-manifold section contour")
        );
    }

    #[test]
    fn stair_mesh_rejects_invalid_input_and_accepts_coordinate_envelope() {
        let p = params();
        for case in 0..8 {
            let mut bad = p.clone();
            match case {
                0 => bad.riser_count = 0,
                1 => bad.riser_count = MAX_STAIR_RISERS + 1,
                2 => bad.end = bad.start,
                3 => bad.width = f64::NAN,
                4 => bad.structural_thickness = 0.0,
                5 => bad.end.x = f64::INFINITY,
                6 => bad.end.y = 1e7,
                _ => bad.width = -1.0,
            }
            assert!(stair_mesh(&bad, 0.0, 3.0).is_err());
        }
        for (lower, upper) in [
            (0.0, 0.0),
            (3.0, 0.0),
            (0.0, f64::NAN),
            (f64::INFINITY, 3.0),
            (0.0, 1e7),
        ] {
            assert!(stair_mesh(&p, lower, upper).is_err());
        }
        let mut large = p;
        large.start = Point2::new(999_990.0, -999_990.0);
        large.end = Point2::new(999_994.5, -999_990.0);
        let mesh = stair_mesh(&large, 999_990.0, 999_993.0).unwrap();
        assert_closed(&mesh);
        vertical_section(
            &mesh,
            VerticalSectionPlane {
                origin: large.start,
                direction: Point2::new(1.0, 0.0),
            },
        )
        .unwrap();
    }
}
