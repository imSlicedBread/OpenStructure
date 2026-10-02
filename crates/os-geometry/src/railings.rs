//! Checked stair-hosted railings as independent closed members.
use crate::{Mesh, SurfaceIdentity, Vec3};
use os_core::{Id, Point2, Result, ensure};
use os_model::{Model, StairRailingParams};

/// A railing is kept as independent member meshes for reliable sectioning.
/// `mesh` is only a convenient combined scene representation.
#[derive(Clone, Debug, PartialEq)]
pub struct StairRailingGeometry {
    pub members: Vec<Mesh>,
    pub mesh: Mesh,
}

/// Generate a continuous sloped top rail and one vertical post at each tread
/// station, including the lower and upper landings. Side is defined relative to
/// travel from the stair's lower start to its upper end.
pub fn railing_geometry(
    parameters: &StairRailingParams,
    model: &Model,
) -> Result<StairRailingGeometry> {
    parameters.validate_in(model)?;
    let stair = &model.stairs[&parameters.stair].parameters;
    let ty = &model.railing_types[&parameters.railing_type].parameters;
    let dimensions = stair.dimensions_in(model)?;
    let run = dimensions.run;
    let direction = Point2::new(
        (stair.end.x - stair.start.x) / run,
        (stair.end.y - stair.start.y) / run,
    );
    // Keep a right-handed local frame (forward, left, up); the side only
    // changes the offset of the member center from the stair centerline.
    let normal = Point2::new(-direction.y, direction.x);
    let side = match parameters.side {
        os_model::StairRailingSide::Left => 1.0,
        os_model::StairRailingSide::Right => -1.0,
    };
    let material = ty.material;
    let center_offset = side * (stair.width * 0.5 - ty.top_rail_width * 0.5);
    let n = stair.riser_count as usize;
    let mut members = Vec::with_capacity(n + 2);

    // The rail's lower face follows the stair's level-to-level slope. Its top
    // elevation is the authored height above that reference line.
    members.push(oriented_box(OrientedBox {
        p0: stair.start,
        p1: stair.end,
        normal,
        side_offset: center_offset,
        across_width: ty.top_rail_width,
        bottom_z0: lower_elevation(model, stair) + ty.top_rail_height - ty.top_rail_depth,
        bottom_slope: dimensions.rise,
        vertical_depth: ty.top_rail_depth,
        material,
    })?);

    // `max_post_spacing` is validated against one tread going, so this bounded
    // host-based array never leaves a post floating between stepped surfaces.
    let post_height = ty.top_rail_height - ty.top_rail_depth;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let center = Point2::new(
            stair.start.x + (stair.end.x - stair.start.x) * t,
            stair.start.y + (stair.end.y - stair.start.y) * t,
        );
        let half = ty.post_width * 0.5;
        let start = Point2::new(center.x - direction.x * half, center.y - direction.y * half);
        let end = Point2::new(center.x + direction.x * half, center.y + direction.y * half);
        members.push(oriented_box(OrientedBox {
            p0: start,
            p1: end,
            normal,
            side_offset: side * (stair.width * 0.5 - ty.post_depth * 0.5),
            across_width: ty.post_depth,
            bottom_z0: lower_elevation(model, stair) + dimensions.rise * t,
            bottom_slope: 0.0,
            vertical_depth: post_height,
            material,
        })?);
    }

    ensure(
        members.len() <= os_model::MAX_STAIR_RAILING_POSTS as usize + 1,
        "railing member budget exceeded",
    )?;
    let mut mesh = Mesh::default();
    for member in &members {
        append_disconnected(&mut mesh, member)?;
    }
    mesh.validate()?;
    Ok(StairRailingGeometry { members, mesh })
}

fn lower_elevation(model: &Model, stair: &os_model::StairParams) -> f64 {
    model.levels[&stair.lower_level].parameters.elevation
}

struct OrientedBox {
    p0: Point2,
    p1: Point2,
    normal: Point2,
    side_offset: f64,
    across_width: f64,
    bottom_z0: f64,
    bottom_slope: f64,
    vertical_depth: f64,
    material: Option<Id>,
}

fn oriented_box(spec: OrientedBox) -> Result<Mesh> {
    let OrientedBox {
        p0,
        p1,
        normal,
        side_offset,
        across_width,
        bottom_z0,
        bottom_slope,
        vertical_depth,
        material,
    } = spec;
    ensure(
        [p0, p1].iter().all(|p| p.is_finite())
            && [
                normal.x,
                normal.y,
                side_offset,
                across_width,
                bottom_z0,
                bottom_slope,
                vertical_depth,
            ]
            .iter()
            .all(|v| v.is_finite())
            && across_width > 0.0
            && vertical_depth > 0.0,
        "invalid railing member geometry",
    )?;
    let half = across_width * 0.5;
    let mut vertices = Vec::with_capacity(8);
    for top in [false, true] {
        for (t, s) in [(0.0, -1.0), (1.0, -1.0), (1.0, 1.0), (0.0, 1.0)] {
            let origin = Point2::new(p0.x + (p1.x - p0.x) * t, p0.y + (p1.y - p0.y) * t);
            let z = bottom_z0 + bottom_slope * t + if top { vertical_depth } else { 0.0 };
            vertices.push(Vec3::new(
                origin.x + normal.x * (side_offset + s * half),
                origin.y + normal.y * (side_offset + s * half),
                z,
            ));
        }
    }
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2], // bottom
        [4, 5, 6],
        [4, 6, 7], // top
        [0, 1, 5],
        [0, 5, 4], // left side
        [3, 7, 6],
        [3, 6, 2], // right side
        [0, 4, 7],
        [0, 7, 3], // start cap
        [1, 2, 6],
        [1, 6, 5], // end cap
    ];
    let mesh = Mesh {
        surfaces: vec![
            SurfaceIdentity {
                layer: None,
                material
            };
            triangles.len()
        ],
        vertices,
        triangles,
    };
    mesh.validate()?;
    Ok(mesh)
}

fn append_disconnected(target: &mut Mesh, part: &Mesh) -> Result<()> {
    let base = u32::try_from(target.vertices.len())
        .map_err(|_| os_core::Error::Invalid("railing vertex budget exceeded".into()))?;
    ensure(
        target.vertices.len().saturating_add(part.vertices.len()) <= 8 * 514,
        "railing vertex budget exceeded",
    )?;
    target.vertices.extend_from_slice(&part.vertices);
    for triangle in &part.triangles {
        target
            .triangles
            .push([triangle[0] + base, triangle[1] + base, triangle[2] + base]);
    }
    target.surfaces.extend_from_slice(&part.surfaces);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{
        Level, LevelParams, Stair, StairParams, StairRailing, StairRailingSide, StairRailingType,
        StairRailingTypeParams,
    };
    use std::collections::BTreeMap;

    fn fixture() -> (Model, StairRailing) {
        let mut model = Model::new("Stair railings");
        let lower = model.levels.values().next().unwrap().clone();
        let upper = Level::new(
            "core.level",
            LevelParams {
                name: "Upper".into(),
                elevation: 3.0,
                building: lower.parameters.building,
            },
        );
        let upper_id = upper.id();
        model.levels.insert(upper_id, upper);
        let stair = Stair::new(
            "core.stair",
            StairParams {
                name: "Flight".into(),
                lower_level: lower.id(),
                upper_level: upper_id,
                start: Point2::new(2.0, 3.0),
                end: Point2::new(5.6, 5.7),
                width: 1.2,
                riser_count: 12,
                structural_thickness: 0.2,
                material: None,
            },
        );
        let stair_id = stair.id();
        model.stairs.insert(stair_id, stair);
        let ty = StairRailingType::new(
            "core.railing_type",
            StairRailingTypeParams {
                name: "Guardrail".into(),
                top_rail_height: 0.95,
                top_rail_width: 0.05,
                top_rail_depth: 0.05,
                post_width: 0.08,
                post_depth: 0.05,
                max_post_spacing: 0.5,
                material: None,
            },
        );
        let type_id = ty.id();
        model.railing_types.insert(type_id, ty);
        let railing = StairRailing::new(
            "core.railing",
            StairRailingParams {
                name: "Left guardrail".into(),
                stair: stair_id,
                railing_type: type_id,
                side: StairRailingSide::Left,
            },
        );
        (model, railing)
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
    }

    #[test]
    fn railing_generates_closed_sloped_rail_and_tread_posts_deterministically() {
        let (model, railing) = fixture();
        let geometry = railing_geometry(&railing.parameters, &model).unwrap();
        assert_eq!(geometry.members.len(), 14);
        assert_eq!(geometry.members[0].vertices.len(), 8);
        assert!(geometry.members.iter().all(|member| {
            assert_closed(member);
            member
                .surfaces
                .iter()
                .all(|surface| surface.material.is_none())
        }));
        assert_eq!(
            geometry,
            railing_geometry(&railing.parameters, &model).unwrap()
        );
        assert_eq!(geometry.mesh.vertices.len(), 14 * 8);
        assert_eq!(geometry.mesh.triangles.len(), 14 * 12);
        assert_closed(&geometry.mesh);
    }

    #[test]
    fn railing_side_reverses_across_stair_and_type_validation_is_host_aware() {
        let (model, mut railing) = fixture();
        let left = railing_geometry(&railing.parameters, &model).unwrap();
        railing.parameters.side = StairRailingSide::Right;
        let right = railing_geometry(&railing.parameters, &model).unwrap();
        let left_post = left.members[1].vertices[0];
        let right_post = right.members[1].vertices[0];
        let normal = Point2::new(-0.6, 0.8);
        let displacement = Point2::new(right_post.x - left_post.x, right_post.y - left_post.y);
        assert!(displacement.x * normal.x + displacement.y * normal.y < 0.0);
        let mut ty = model
            .railing_types
            .values()
            .next()
            .unwrap()
            .parameters
            .clone();
        ty.top_rail_height = ty.top_rail_depth;
        assert!(ty.validate().is_err());
    }
}
