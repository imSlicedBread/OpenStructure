//! Bounded upward hidden-surface projection of native closed solids.
//! XY stays aligned with the floor plan (the reflected drawing convention).
//! Only underside surfaces and material at the cut plane can face the viewer.
use crate::{
    Mesh, SurfaceIdentity, Vec3,
    plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRole},
};
use os_core::{Id, Point2, Result, ensure};
use std::collections::BTreeMap;

const EPS: f64 = 1e-9;
const MAX_FACES: usize = 10_000;
const MAX_WORK: usize = 2_000_000;
type Regions = BTreeMap<Id, Vec<(SurfaceIdentity, PlanFootprint)>>;
pub struct Projection {
    pub regions: Regions,
    pub seams: BTreeMap<Id, Vec<(Point2, Point2)>>,
}
#[derive(Clone)]
struct Face {
    id: Id,
    surface: SurfaceIdentity,
    role: PlanRole,
    polygon: Vec<Point2>,
    // Height = ax + by + c, in drawing coordinates.
    height: [f64; 3],
}
fn cross(a: Point2, b: Point2, p: Point2) -> f64 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}
fn clip(polygon: &[Point2], distance: impl Fn(Point2) -> f64) -> Vec<Point2> {
    let mut out = Vec::new();
    for (a, b) in polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1))
        .take(polygon.len())
    {
        let (da, db) = (distance(a), distance(b));
        if da >= 0. {
            out.push(a);
        }
        if (da >= 0.) != (db >= 0.) {
            let t = da / (da - db);
            out.push(Point2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)));
        }
    }
    out.dedup_by(|a, b| a.distance(*b) < EPS);
    if out.len() > 1 && out[0].distance(*out.last().unwrap()) < EPS {
        out.pop();
    }
    out
}
fn area(p: &[Point2]) -> f64 {
    crate::floors::signed_area(p).abs()
}
fn overlaps(a: &[Point2], b: &[Point2]) -> bool {
    let bounds = |p: &[Point2]| {
        p.iter().fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |r, p| [r[0].min(p.x), r[1].min(p.y), r[2].max(p.x), r[3].max(p.y)],
        )
    };
    let (a, b) = (bounds(a), bounds(b));
    a[0] < b[2] - EPS && b[0] < a[2] - EPS && a[1] < b[3] - EPS && b[1] < a[3] - EPS
}
fn subtract(polygon: Vec<Point2>, mask: &[Point2]) -> Vec<Vec<Point2>> {
    let mut inside = polygon;
    let mut out = Vec::new();
    for (a, b) in mask
        .iter()
        .copied()
        .zip(mask.iter().copied().cycle().skip(1))
        .take(mask.len())
    {
        let outside = clip(&inside, |p| -cross(a, b, p));
        if area(&outside) > EPS {
            out.push(outside);
        }
        inside = clip(&inside, |p| cross(a, b, p));
        if area(&inside) <= EPS {
            break;
        }
    }
    out
}
fn contains(ring: &[Point2], p: Point2) -> bool {
    let mut inside = false;
    for (a, b) in ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
    {
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Absolute heights satisfy cut <= top <= depth. Top separates primary
/// projection from the lighter depth band. Bottom is a model settings bound;
/// solids wholly below cut do not appear in this upward view.
pub fn project(
    meshes: &[(Id, Mesh)],
    cut: f64,
    top: f64,
    depth: f64,
    basis: HorizontalBasis,
    crop: Option<PlanCrop>,
) -> Result<Projection> {
    ensure(
        [cut, top, depth].iter().all(|v| v.is_finite())
            && cut <= top
            && top <= depth
            && depth > cut,
        "invalid upward projection range",
    )?;
    basis.validate()?;
    if let Some(crop) = crop {
        crop.validate()?;
    }
    let mut faces = Vec::new();
    for (id, mesh) in meshes {
        ensure(
            mesh.triangles.len() <= crate::section::MAX_SECTION_INPUT_TRIANGLES,
            "RCP mesh triangle limit exceeded",
        )?;
        mesh.validate()?;
        for (index, triangle) in mesh.triangles.iter().enumerate() {
            let world = triangle.map(|i| mesh.vertices[i as usize]);
            let xy = world.map(|v| Point2::new(v.x, v.y));
            // The underside's outward normal points down. Reject sides and tops.
            if cross(xy[0], xy[1], xy[2]) >= -EPS {
                continue;
            }
            let mut p = xy
                .into_iter()
                .map(|v| basis.world_to_plane(v))
                .collect::<Result<Vec<_>>>()?;
            let den = cross(p[0], p[1], p[2]);
            let a = ((world[1].z - world[0].z) * (p[2].y - p[0].y)
                - (world[2].z - world[0].z) * (p[1].y - p[0].y))
                / den;
            let b = ((p[1].x - p[0].x) * (world[2].z - world[0].z)
                - (p[2].x - p[0].x) * (world[1].z - world[0].z))
                / den;
            let c = world[0].z - a * p[0].x - b * p[0].y;
            p.reverse();
            for (low, high, role) in [
                (cut, top, PlanRole::Projected),
                (top, depth, PlanRole::Depth),
            ] {
                if high <= low {
                    continue;
                }
                let min = world.iter().map(|v| v.z).fold(f64::INFINITY, f64::min);
                let max = world.iter().map(|v| v.z).fold(f64::NEG_INFINITY, f64::max);
                if max < low - EPS
                    || min > high + EPS
                    || (role == PlanRole::Depth && max <= low + EPS)
                {
                    continue;
                }
                let polygon = clip(&clip(&p, |v| a * v.x + b * v.y + c - low), |v| {
                    high - a * v.x - b * v.y - c
                });
                if area(&polygon) > EPS {
                    faces.push(Face {
                        id: *id,
                        surface: mesh.surfaces.get(index).copied().unwrap_or_default(),
                        role,
                        polygon,
                        height: [a, b, c],
                    });
                }
            }
        }
        // Rotate the horizontal cut into the established closed-mesh section
        // solver, retaining its topology checks, holes and tolerance policy.
        let min = mesh
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::INFINITY, f64::min);
        let max = mesh
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::NEG_INFINITY, f64::max);
        if min < cut - EPS && max > cut + EPS {
            let mut rotated = mesh.clone();
            for v in &mut rotated.vertices {
                *v = Vec3::new(v.x, v.z, -v.y);
            }
            let contours = crate::section::vertical_section(
                &rotated,
                crate::section::VerticalSectionPlane {
                    origin: Point2::new(0., cut),
                    direction: Point2::new(1., 0.),
                },
            )?;
            for outer in contours.iter().filter(|c| c.nesting_depth % 2 == 0) {
                let boundary = outer
                    .points
                    .iter()
                    .map(|p| Point2::new(p.x, -p.y))
                    .collect::<Vec<_>>();
                let holes = contours
                    .iter()
                    .filter(|h| {
                        h.nesting_depth == outer.nesting_depth + 1
                            && contains(&outer.points, h.points[0])
                    })
                    .map(|h| h.points.iter().map(|p| Point2::new(p.x, -p.y)).collect())
                    .collect::<Vec<Vec<_>>>();
                let cap = crate::floor_holes::triangulate_floor_rings(&boundary, &holes)?;
                for tri in cap.triangles {
                    let polygon = tri
                        .into_iter()
                        .map(|i| basis.world_to_plane(cap.vertices[i as usize]))
                        .collect::<Result<Vec<_>>>()?;
                    faces.push(Face {
                        id: *id,
                        surface: mesh.surfaces.first().copied().unwrap_or_default(),
                        role: PlanRole::Cut,
                        polygon,
                        height: [0., 0., cut],
                    });
                }
            }
        }
        ensure(faces.len() <= MAX_FACES, "RCP surface limit exceeded")?;
    }
    // Crop before occlusion, reducing both work and numeric range.
    if let Some(crop) = crop {
        for f in &mut faces {
            for (axis, value, sign) in [
                (0, crop.min.x, 1.),
                (0, crop.max.x, -1.),
                (1, crop.min.y, 1.),
                (1, crop.max.y, -1.),
            ] {
                f.polygon = clip(&f.polygon, |p| {
                    ((if axis == 0 { p.x } else { p.y }) - value) * sign
                });
            }
        }
    }
    faces.retain(|f| area(&f.polygon) > EPS);
    let mut visible = Vec::new();
    let mut work = 0usize;
    for (i, face) in faces.iter().enumerate() {
        let mut pieces = vec![face.polygon.clone()];
        for (j, other) in faces.iter().enumerate() {
            if i == j {
                continue;
            }
            work += 1;
            ensure(work <= MAX_WORK, "RCP occlusion work limit exceeded")?;
            if !overlaps(&face.polygon, &other.polygon) {
                continue;
            }
            let delta = [
                face.height[0] - other.height[0],
                face.height[1] - other.height[1],
                face.height[2] - other.height[2],
            ];
            let equal = delta.iter().all(|d| d.abs() < EPS);
            if equal && j > i {
                continue;
            }
            let mask = if equal {
                other.polygon.clone()
            } else {
                clip(&other.polygon, |p| {
                    delta[0] * p.x + delta[1] * p.y + delta[2] - EPS
                })
            };
            if area(&mask) <= EPS {
                continue;
            }
            pieces = pieces
                .into_iter()
                .flat_map(|p| {
                    if overlaps(&p, &mask) {
                        subtract(p, &mask)
                    } else {
                        vec![p]
                    }
                })
                .collect();
            ensure(
                pieces.len() + visible.len() <= MAX_FACES,
                "RCP fragment limit exceeded",
            )?;
            if pieces.is_empty() {
                break;
            }
        }
        for polygon in pieces {
            visible.push(Face {
                polygon,
                ..face.clone()
            });
        }
    }
    let mut regions: Regions = BTreeMap::new();
    for face in visible {
        for pair in face.polygon[1..].windows(2) {
            let p = vec![face.polygon[0], pair[0], pair[1]];
            if area(&p) > EPS
                && let Some(footprint) = PlanFootprint::from_convex(face.role, p, None)?
            {
                regions
                    .entry(face.id)
                    .or_default()
                    .push((face.surface, footprint));
            }
        }
    }
    // Remove shared (including partially split) triangulation edges. Exterior
    // boundaries and opening edges have no material on their opposite side.
    let mut seams = BTreeMap::new();
    for (id, parts) in &regions {
        let mut shared = Vec::new();
        for (i, (_, a)) in parts.iter().enumerate() {
            let av = a.vertices();
            for (j, (_, b)) in parts.iter().enumerate().skip(i + 1) {
                let _ = j;
                work += 1;
                ensure(work <= MAX_WORK, "RCP seam work limit exceeded")?;
                if a.role != b.role {
                    continue;
                }
                let bv = b.vertices();
                for n in 0..av.len() {
                    for m in 0..bv.len() {
                        let (p, q, r, s) =
                            (av[n], av[(n + 1) % av.len()], bv[m], bv[(m + 1) % bv.len()]);
                        if cross(p, q, r).abs() > EPS || cross(p, q, s).abs() > EPS {
                            continue;
                        }
                        let dx = q.x - p.x;
                        let dy = q.y - p.y;
                        let len = dx * dx + dy * dy;
                        let t = |v: Point2| ((v.x - p.x) * dx + (v.y - p.y) * dy) / len;
                        let (u, v) = (t(r).min(t(s)).max(0.), t(r).max(t(s)).min(1.));
                        if v - u > EPS {
                            shared.push((
                                Point2::new(p.x + u * dx, p.y + u * dy),
                                Point2::new(p.x + v * dx, p.y + v * dy),
                            ));
                        }
                    }
                }
            }
        }
        seams.insert(*id, shared);
    }
    Ok(Projection { regions, seams })
}
