//! Plan contours of the same authored extrusion solids used in 3D.
use crate::{
    SurfaceIdentity, openings,
    plan::{PlanRange, PlanRole},
};
use os_core::{Point2, Result};
use os_model::{ResolvedOpening, WallParams};

pub type AuthoredPlanEdge = (Point2, Point2, PlanRole, SurfaceIdentity);

pub fn plan_edges(
    p: &ResolvedOpening,
    wall: &WallParams,
    elevation: f64,
    range: PlanRange,
) -> Result<Vec<AuthoredPlanEdge>> {
    range.validate()?;
    let mut edges = Vec::new();
    for (solid, mesh) in p
        .family
        .extrusions
        .iter()
        .zip(openings::authored_solids(p, wall, elevation)?)
    {
        let n = solid.profile.len();
        let min_z = mesh
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::INFINITY, f64::min);
        let max_z = mesh
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::NEG_INFINITY, f64::max);
        if max_z <= range.depth || min_z >= range.top || min_z > range.cut {
            continue;
        }
        let cut = min_z <= range.cut && max_z > range.cut;
        let role = if cut {
            PlanRole::Cut
        } else if max_z > range.bottom {
            PlanRole::Projected
        } else {
            PlanRole::Depth
        };
        let profile: Vec<_> = solid
            .profile
            .iter()
            .enumerate()
            .map(|(i, p)| Point2::new(p.x, mesh.vertices[i].z))
            .collect();
        let mut spans = Vec::new();
        if cut {
            for (a, b) in profile.iter().zip(profile.iter().cycle().skip(1)).take(n) {
                if (a.y <= range.cut && range.cut < b.y) || (b.y <= range.cut && range.cut < a.y) {
                    spans.push(a.x + (range.cut - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            spans.sort_by(f64::total_cmp);
        } else {
            // Union the horizontal extents of triangles clipped to the view's
            // vertical band; a concave profile may yield several disjoint spans.
            let mut intervals = Vec::new();
            for t in crate::floors::triangulate_floor(&solid.profile)? {
                let mut polygon: Vec<_> = t.iter().map(|i| profile[*i as usize]).collect();
                for (z, above) in [(range.depth, true), (range.cut, false)] {
                    let mut clipped = Vec::new();
                    for (a, b) in polygon
                        .iter()
                        .zip(polygon.iter().cycle().skip(1))
                        .take(polygon.len())
                    {
                        let inside = |p: &Point2| if above { p.y >= z } else { p.y <= z };
                        if inside(a) {
                            clipped.push(*a);
                        }
                        if inside(a) != inside(b) {
                            clipped
                                .push(Point2::new(a.x + (z - a.y) * (b.x - a.x) / (b.y - a.y), z));
                        }
                    }
                    polygon = clipped;
                }
                if !polygon.is_empty() {
                    intervals.push((
                        polygon.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
                        polygon
                            .iter()
                            .map(|p| p.x)
                            .fold(f64::NEG_INFINITY, f64::max),
                    ));
                }
            }
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut merged: Vec<(f64, f64)> = Vec::new();
            for (a, b) in intervals {
                if let Some(last) = merged.last_mut()
                    && a <= last.1 + 1e-10
                {
                    last.1 = last.1.max(b);
                } else {
                    merged.push((a, b));
                }
            }
            spans.extend(merged.into_iter().flat_map(|(a, b)| [a, b]));
        }
        let first = mesh.vertices[0];
        let other = (1..n)
            .find(|i| (solid.profile[*i].x - solid.profile[0].x).abs() > 1e-10)
            .unwrap();
        let dx = solid.profile[other].x - solid.profile[0].x;
        let axis = Point2::new(
            (mesh.vertices[other].x - first.x) / dx,
            (mesh.vertices[other].y - first.y) / dx,
        );
        let normal = Point2::new(mesh.vertices[n].x - first.x, mesh.vertices[n].y - first.y);
        let at = |u: f64, side: f64| {
            Point2::new(
                first.x + (u - solid.profile[0].x) * axis.x + side * normal.x,
                first.y + (u - solid.profile[0].x) * axis.y + side * normal.y,
            )
        };
        for pair in spans.chunks_exact(2).filter(|p| p[1] - p[0] > 1e-10) {
            let ring = [
                at(pair[0], 0.),
                at(pair[1], 0.),
                at(pair[1], 1.),
                at(pair[0], 1.),
            ];
            for i in 0..4 {
                edges.push((ring[i], ring[(i + 1) % 4], role, mesh.surfaces[0]));
            }
        }
    }
    os_core::ensure(
        edges.len() <= 4096,
        "authored extrusion plan segment budget exceeded",
    )?;
    Ok(edges)
}
