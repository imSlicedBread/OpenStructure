//! Lift the shared perforated slab mesh with an affine shear (determinant one).
use crate::Mesh;
use os_core::{Point2, Result};
use os_model::RoofParams;

pub struct RoofPlan {
    pub footprints: Vec<crate::plan::PlanFootprint>,
    /// Triangulation diagonals, excluded from visible outlines and snapping.
    pub seams: Vec<(Point2, Point2)>,
}

/// Clip each cap triangle by actual top elevation. The vertical roof interval
/// is [top-thickness, top]; holes remain absent from every clipped region.
pub fn roof_plan(
    p: &RoofParams,
    level_z: f64,
    range: crate::plan::PlanRange,
    basis: crate::plan::HorizontalBasis,
    crop: Option<crate::plan::PlanCrop>,
) -> Result<RoofPlan> {
    use crate::plan::{PlanFootprint, PlanRole};
    p.validate_at(level_z)?;
    range.validate()?;
    basis.validate()?;
    if let Some(crop) = crop {
        crop.validate()?;
    }
    let cap = crate::floor_holes::triangulate_floor_rings(&p.boundary, &p.holes)?;
    let mut edges = std::collections::BTreeMap::new();
    let mut footprints = Vec::new();
    // Disjoint intervals of top-surface elevation; cut is where the solid
    // straddles the cut plane, including the portion whose top is above it.
    let bands = [
        (range.depth, range.bottom, PlanRole::Depth),
        (range.bottom, range.cut, PlanRole::Projected),
        (
            range.cut,
            (range.cut + p.thickness).min(range.top + p.thickness),
            PlanRole::Cut,
        ),
        (
            range.cut + p.thickness,
            range.top + p.thickness,
            PlanRole::Projected,
        ),
    ];
    for [a, b, c] in cap.triangles {
        for (u, v) in [(a, b), (b, c), (c, a)] {
            *edges.entry((u.min(v), u.max(v))).or_insert(0) += 1;
        }
        for (low, high, role) in bands {
            if high <= low {
                continue;
            }
            let triangle = vec![
                cap.vertices[a as usize],
                cap.vertices[b as usize],
                cap.vertices[c as usize],
            ];
            let minimum = triangle
                .iter()
                .map(|v| p.top_at(*v, level_z))
                .fold(f64::INFINITY, f64::min);
            let maximum = triangle
                .iter()
                .map(|v| p.top_at(*v, level_z))
                .fold(f64::NEG_INFINITY, f64::max);
            if maximum <= low || minimum > high {
                continue;
            }
            let polygon = clip_height(
                clip_height(triangle, p, level_z, low, true),
                p,
                level_z,
                high,
                false,
            );
            if polygon.len() < 3 || crate::floors::signed_area(&polygon).abs() <= 1e-10 {
                continue;
            }
            let plane = polygon
                .into_iter()
                .map(|v| basis.world_to_plane(v))
                .collect::<Result<Vec<_>>>()?;
            if let Some(region) = PlanFootprint::from_convex(role, plane, crop)? {
                footprints.push(region);
            }
        }
    }
    let seams = edges
        .into_iter()
        .filter(|(_, count)| *count == 2)
        .map(|((a, b), _)| {
            Ok((
                basis.world_to_plane(cap.vertices[a as usize])?,
                basis.world_to_plane(cap.vertices[b as usize])?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RoofPlan { footprints, seams })
}

fn clip_height(
    points: Vec<Point2>,
    p: &RoofParams,
    level_z: f64,
    height: f64,
    above: bool,
) -> Vec<Point2> {
    let mut out = Vec::new();
    for (a, b) in points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .take(points.len())
    {
        let za = p.top_at(a, level_z) - height;
        let zb = p.top_at(b, level_z) - height;
        let inside_a = if above { za >= 0.0 } else { za <= 0.0 };
        let inside_b = if above { zb >= 0.0 } else { zb <= 0.0 };
        if inside_a {
            out.push(a);
        }
        if inside_a != inside_b {
            let t = za / (za - zb);
            let v = Point2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
            if out.last().is_none_or(|last| last.distance(v) > 1e-9) {
                out.push(v);
            }
        }
    }
    out.dedup_by(|a, b| a.distance(*b) <= 1e-9);
    if out.len() > 1 && out[0].distance(*out.last().unwrap()) <= 1e-9 {
        out.pop();
    }
    out
}

pub fn roof_mesh(parameters: &RoofParams, level_z: f64) -> Result<Mesh> {
    parameters.validate_at(level_z)?;
    let mut mesh = crate::floor_holes::extrude_floor_rings(
        &parameters.boundary,
        &parameters.holes,
        0.0,
        parameters.thickness,
    )?;
    for vertex in &mut mesh.vertices {
        vertex.z += parameters.top_at(Point2::new(vertex.x, vertex.y), level_z);
    }
    mesh.surfaces.resize(
        mesh.triangles.len(),
        crate::SurfaceIdentity {
            layer: None,
            material: parameters.material,
        },
    );
    mesh.validate()?;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{HorizontalBasis, PlanCrop, PlanRange, PlanRole};
    use crate::section::{VerticalSectionPlane, vertical_section};
    fn params() -> RoofParams {
        RoofParams {
            name: "Roof".into(),
            level: os_core::Id::new(),
            material: Some(os_core::Id::new()),
            boundary: [(0., 0.), (8., 0.), (8., 6.), (0., 6.)]
                .map(|(x, y)| Point2::new(x, y))
                .to_vec(),
            holes: vec![
                [(2., 2.), (4., 2.), (4., 4.), (2., 4.)]
                    .map(|(x, y)| Point2::new(x, y))
                    .to_vec(),
            ],
            thickness: 0.2,
            top_offset: 0.,
            slope_start: Point2::new(0., 0.),
            slope_end: Point2::new(2., 0.),
            rise_per_run: 0.5,
        }
    }
    #[test]
    fn roof_solid_is_closed_outward_deterministic_with_openings_and_signed_slope() {
        let mut p = params();
        for slope in [-0.5, 0., 0.5] {
            p.rise_per_run = slope;
            for reverse in [false, true] {
                if reverse {
                    p.boundary.reverse();
                    p.holes[0].reverse();
                }
                let mesh = roof_mesh(&p, 3.).unwrap();
                assert_eq!(mesh, roof_mesh(&p, 3.).unwrap());
                assert!((mesh.signed_volume() - 44. * 0.2).abs() < 1e-9);
                let mut edges = std::collections::BTreeMap::new();
                for [a, b, c] in &mesh.triangles {
                    for edge in [(*a, *b), (*b, *c), (*c, *a)] {
                        *edges.entry(edge).or_insert(0) += 1;
                    }
                }
                for ((a, b), count) in &edges {
                    assert_eq!(*count, 1);
                    assert_eq!(edges.get(&(*b, *a)), Some(&1));
                }
                assert!(mesh.surfaces.iter().all(|s| s.material == p.material));
                let length_independent = RoofParams {
                    slope_end: Point2::new(20., 0.),
                    ..p.clone()
                };
                assert_eq!(mesh, roof_mesh(&length_independent, 3.).unwrap());
                for (origin, direction, area) in [
                    (Point2::new(0., 3.), Point2::new(1., 0.), 1.2),
                    (Point2::new(3., 0.), Point2::new(0., 1.), 0.8),
                ] {
                    let contours =
                        vertical_section(&mesh, VerticalSectionPlane { origin, direction })
                            .unwrap();
                    assert_eq!(contours.len(), 2);
                    let actual: f64 = contours
                        .iter()
                        .map(|c| crate::floors::signed_area(&c.points).abs())
                        .sum();
                    assert!((actual - area).abs() < 1e-9, "{actual} != {area}");
                }
            }
        }
    }
    #[test]
    fn roof_projection_clips_actual_slope_holes_crop_and_roles() {
        let p = params();
        let range = PlanRange {
            depth: 0.,
            bottom: 0.5,
            cut: 1.,
            top: 1.5,
        };
        let plan = roof_plan(&p, 0., range, HorizontalBasis::default(), None).unwrap();
        assert!((plan.footprints.iter().map(|f| f.area()).sum::<f64>() - 17.6).abs() < 1e-9);
        for role in [PlanRole::Depth, PlanRole::Cut, PlanRole::Projected] {
            assert!(plan.footprints.iter().any(|f| f.role == role));
        }
        assert!(
            !plan
                .footprints
                .iter()
                .any(|f| f.contains(Point2::new(3., 3.)))
        );
        assert!(
            !plan
                .footprints
                .iter()
                .any(|f| f.contains(Point2::new(4., 1.)))
        );
        let crop = Some(PlanCrop {
            min: Point2::new(1., 1.),
            max: Point2::new(3., 5.),
        });
        let clipped = roof_plan(&p, 0., range, HorizontalBasis::default(), crop).unwrap();
        assert!((clipped.footprints.iter().map(|f| f.area()).sum::<f64>() - 6.).abs() < 1e-9);
        assert!(
            roof_plan(&p, 100., range, HorizontalBasis::default(), None)
                .unwrap()
                .footprints
                .is_empty()
        );
    }
    #[test]
    fn roof_validation_bounds_references_rings_and_derived_heights() {
        let mut model = os_model::Model::new("Roof");
        let mut p = params();
        p.level = *model.levels.keys().next().unwrap();
        p.material = None;
        p.validate_in(&model).unwrap();
        for case in 0..10 {
            let mut bad = p.clone();
            match case {
                0 => bad.slope_end = bad.slope_start,
                1 => bad.rise_per_run = f64::NAN,
                2 => bad.rise_per_run = 101.,
                3 => bad.top_offset = 1e6,
                4 => bad.holes[0][0] = Point2::new(-1., -1.),
                5 => bad.level = os_core::Id::new(),
                6 => bad.material = Some(os_core::Id::new()),
                7 => bad.thickness = 0.,
                8 => bad.boundary[0] = Point2::new(1e7, 0.),
                _ => bad.holes.push(bad.holes[0].clone()),
            }
            assert!(bad.validate_in(&model).is_err(), "case {case}");
        }
        let before = model.estimated_memory_bytes();
        let roof = os_model::Roof::new("core.roof", p);
        model.roofs.insert(roof.id(), roof);
        model.validate().unwrap();
        assert!(model.estimated_memory_bytes() > before + std::mem::size_of::<os_model::Roof>());
    }
}
