//! Shared positive component evaluator. Does not perform host booleans.
use crate::{
    GeometryKernel, Mesh,
    floors::{extrude_floor, triangulate_floor},
};
use os_core::{Point2, Result};
use os_model::{
    DoorHinge, DoorSwing, OpeningKind, ResolvedOpening, WallParams, WindowPanePosition,
};

pub fn depth(p: &ResolvedOpening, wall: &WallParams) -> f64 {
    p.family.depth.min(wall.thickness * 0.2).min(p.width * 0.2)
}

pub fn pane_offset(p: &ResolvedOpening, wall: &WallParams) -> f64 {
    let offset = (wall.thickness - depth(p, wall)) / 2.;
    match p.pane_position {
        WindowPanePosition::Center => 0.,
        WindowPanePosition::LeftFace => offset,
        WindowPanePosition::RightFace => -offset,
    }
}

pub fn frame_thickness(p: &ResolvedOpening, wall: &WallParams) -> f64 {
    p.family.frame_depth.min(wall.thickness).min(p.width)
}

pub fn frame_offset(p: &ResolvedOpening, wall: &WallParams) -> f64 {
    if p.kind == OpeningKind::Door {
        return 0.;
    }
    let face_offset = (wall.thickness - frame_thickness(p, wall)) / 2.;
    match p.pane_position {
        WindowPanePosition::Center => 0.,
        WindowPanePosition::LeftFace => face_offset,
        WindowPanePosition::RightFace => -face_offset,
    }
}

/// Component coordinates in the host's XY frame, u measured from the hinge/start.
pub fn component_point(p: &ResolvedOpening, wall: &WallParams, u: f64) -> Point2 {
    match p.kind {
        OpeningKind::Window => Point2::new(p.offset + u * p.width, pane_offset(p, wall)),
        OpeningKind::Door => {
            let side = if p.swing == DoorSwing::Left { 1. } else { -1. };
            Point2::new(
                p.offset
                    + if p.hinge == DoorHinge::End {
                        p.width
                    } else {
                        0.
                    },
                side * (u * p.width - wall.thickness / 2.),
            )
        }
    }
}

pub fn world(wall: &WallParams, x: f64, y: f64) -> Point2 {
    let dx = (wall.end.x - wall.start.x) / wall.length();
    let dy = (wall.end.y - wall.start.y) / wall.length();
    Point2::new(
        wall.start.x + dx * x - dy * y,
        wall.start.y + dy * x + dx * y,
    )
}

pub fn component_mesh(p: &ResolvedOpening, wall: &WallParams, elevation: f64) -> Result<Mesh> {
    p.validate_host(wall)?;
    p.family.validate_for(p.width, p.height, p.kind)?;
    let thickness = depth(p, wall);
    if p.family.frame_width == 0.0 && p.family.profile == os_model::OpeningFamily::default().profile
    {
        let start = component_point(p, wall, 0.);
        let end = component_point(p, wall, 1.);
        let mut panel = wall.clone();
        panel.start = world(wall, start.x, start.y);
        panel.end = world(wall, end.x, end.y);
        panel.height = p.height;
        panel.thickness = thickness;
        return crate::PrismKernel
            .tessellate(&crate::walls::wall_solid(&panel, elevation + p.sill)?);
    }
    // Triangulate normalized coordinates so small/large type dimensions do not
    // change polygon predicates. Rotate the extrusion into the vertical plane.
    let profile = p.family.component_profile(p.width, p.height, p.kind);
    let mut mesh = extrude_floor(&profile, thickness / 2., thickness)?;
    let a = component_point(p, wall, 0.);
    let b = component_point(p, wall, 1.);
    let (dx, dy) = ((b.x - a.x) / p.width, (b.y - a.y) / p.width);
    for v in &mut mesh.vertices {
        let center = component_point(p, wall, v.x);
        let xy = world(wall, center.x + dy * v.z, center.y - dx * v.z);
        *v = crate::Vec3::new(xy.x, xy.y, elevation + p.sill + v.y * p.height);
    }
    if p.family.frame_width == 0.0 {
        mesh.validate()?;
        return Ok(mesh);
    }

    let frame_x = p.family.frame_width / p.width;
    let frame_y = p.family.frame_width / p.height;
    let mut bars = vec![
        rect_profile(0., 0., frame_x, 1.),
        rect_profile(1. - frame_x, 0., 1., 1.),
        rect_profile(frame_x, 1. - frame_y, 1. - frame_x, 1.),
    ];
    if p.kind == OpeningKind::Window {
        bars.push(rect_profile(frame_x, 0., 1. - frame_x, frame_y));
    }
    let frame_depth = frame_thickness(p, wall);
    let frame_offset = frame_offset(p, wall);
    for bar in bars {
        let mut frame = extrude_floor(&bar, frame_depth / 2., frame_depth)?;
        for v in &mut frame.vertices {
            let xy = world(wall, p.offset + v.x * p.width, frame_offset + v.z);
            *v = crate::Vec3::new(xy.x, xy.y, elevation + p.sill + v.y * p.height);
        }
        // Elevation (x,z) plus wall-normal extrusion swaps two axes.
        for triangle in &mut frame.triangles {
            triangle.swap(1, 2);
        }
        append_mesh(&mut mesh, frame)?;
    }
    mesh.validate()?;
    Ok(mesh)
}

fn rect_profile(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point2> {
    vec![
        Point2::new(x0, y0),
        Point2::new(x1, y0),
        Point2::new(x1, y1),
        Point2::new(x0, y1),
    ]
}

fn append_mesh(target: &mut Mesh, addition: Mesh) -> Result<()> {
    let offset = u32::try_from(target.vertices.len())
        .map_err(|_| os_core::Error::Invalid("opening family mesh is too large".into()))?;
    target.triangles.extend(
        addition
            .triangles
            .into_iter()
            .map(|t| t.map(|i| i + offset)),
    );
    target.vertices.extend(addition.vertices);
    Ok(())
}

/// Cut the authored elevation profile at the plan cut, or project its visible
/// part if the component is wholly below the cut. Intervals are normalized u.
pub fn plan_spans(
    p: &ResolvedOpening,
    elevation: f64,
    cut: f64,
    depth: f64,
) -> Result<Vec<(f64, f64)>> {
    p.family.validate_for(p.width, p.height, p.kind)?;
    let profile = p.family.component_profile(p.width, p.height, p.kind);
    profile_plan_spans(p, &profile, elevation, cut, depth)
}

pub fn cut_plan_spans(
    p: &ResolvedOpening,
    elevation: f64,
    cut: f64,
    depth: f64,
) -> Result<Vec<(f64, f64)>> {
    p.family.validate_for(p.width, p.height, p.kind)?;
    profile_plan_spans(p, &p.family.cut_profile, elevation, cut, depth)
}

fn profile_plan_spans(
    p: &ResolvedOpening,
    profile: &[Point2],
    elevation: f64,
    cut: f64,
    depth: f64,
) -> Result<Vec<(f64, f64)>> {
    let cut = (cut - elevation - p.sill) / p.height;
    let low = (depth - elevation - p.sill) / p.height;
    let top = profile
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    if cut < top {
        return Ok(section_spans(profile, cut));
    }
    // Projection is the union of triangle projections clipped to view depth.
    let mut spans = Vec::new();
    for triangle in triangulate_floor(profile)? {
        let ring = triangle.map(|i| profile[i as usize]);
        let mut xs = Vec::new();
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(3) {
            if a.y >= low {
                xs.push(a.x);
            }
            if (a.y < low && b.y > low) || (b.y < low && a.y > low) {
                xs.push(a.x + (low - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }
        if !xs.is_empty() {
            spans.push((
                xs.iter().copied().fold(f64::INFINITY, f64::min),
                xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            ));
        }
    }
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (a, b) in spans {
        if b - a <= 1e-9 {
            continue;
        }
        if let Some(last) = merged.last_mut()
            && a <= last.1 + 1e-9
        {
            last.1 = last.1.max(b);
        } else {
            merged.push((a, b));
        }
    }
    Ok(merged)
}

fn section_spans(profile: &[Point2], height: f64) -> Vec<(f64, f64)> {
    let top = profile
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let y = if height == top {
        height - 1e-10
    } else {
        height
    };
    let mut xs = Vec::new();
    for (a, b) in profile
        .iter()
        .zip(profile.iter().cycle().skip(1))
        .take(profile.len())
    {
        if (a.y <= y && y < b.y) || (b.y <= y && y < a.y) {
            xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.chunks_exact(2)
        .filter(|p| p[1] - p[0] > 1e-9)
        .map(|p| (p[0], p[1]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_opening_frames_create_closed_door_and_window_assemblies() {
        let mut model = os_model::Model::new("Framed openings");
        let level = *model.levels.keys().next().unwrap();
        let wall = WallParams {
            name: "Host".into(),
            start: Point2::new(0., 0.),
            end: Point2::new(8., 0.),
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        };
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let family = os_model::OpeningFamily {
                frame_width: 0.05,
                frame_depth: 0.08,
                ..Default::default()
            };
            let ty = os_model::OpeningType::new(
                "core.opening_type",
                os_model::OpeningTypeParams {
                    family,
                    name: format!("{kind:?}"),
                    kind,
                    width: 1.,
                    height: 1.2,
                    sill: if kind == OpeningKind::Door { 0. } else { 0.8 },
                    pane_position: if kind == OpeningKind::Window {
                        WindowPanePosition::RightFace
                    } else {
                        WindowPanePosition::Center
                    },
                },
            );
            let type_id = ty.id();
            model.opening_types.insert(type_id, ty);
            let p = model
                .resolve_opening(&os_model::OpeningParams {
                    name: format!("{kind:?} instance"),
                    host: level,
                    offset: 1.,
                    definition: os_model::OpeningDefinition::Typed { type_id },
                    hinge: DoorHinge::Start,
                    swing: DoorSwing::Left,
                })
                .unwrap();
            let mesh = component_mesh(&p, &wall, 0.).unwrap();
            mesh.validate().unwrap();

            let width = p.width;
            let height = p.height;
            let rail_count = if kind == OpeningKind::Window { 2. } else { 1. };
            let panel_height = height - rail_count * 0.05;
            let panel_volume = (width - 0.1) * panel_height * depth(&p, &wall);
            let frame_volume = (2. * 0.05 * height + rail_count * (width - 0.1) * 0.05) * 0.08;
            assert!((mesh.signed_volume() - panel_volume - frame_volume).abs() < 1e-10);
            let mut directed_edges = std::collections::BTreeMap::new();
            for triangle in &mesh.triangles {
                for edge in 0..3 {
                    *directed_edges
                        .entry((triangle[edge], triangle[(edge + 1) % 3]))
                        .or_insert(0) += 1;
                }
            }
            for (&(a, b), &count) in &directed_edges {
                assert_eq!(count, 1);
                assert_eq!(directed_edges.get(&(b, a)), Some(&1));
            }
            let spans = plan_spans(&p, 0., p.sill + height * 0.5, 2.).unwrap();
            assert_eq!(spans.len(), 1);
            assert!((spans[0].0 - 0.05).abs() < 1e-10);
            assert!((spans[0].1 - 0.95).abs() < 1e-10);
        }
    }

    #[test]
    fn opening_family_extrusion_and_cut_projection_match_bounds_for_all_orientations() {
        let mut model = os_model::Model::new("Profile geometry");
        let level = *model.levels.keys().next().unwrap();
        let wall = WallParams {
            name: "Host".into(),
            start: Point2::new(3., 4.),
            end: Point2::new(7., 7.),
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        };
        let ty = os_model::OpeningType::new(
            "core.opening_type",
            os_model::OpeningTypeParams {
                family: os_model::OpeningFamily {
                    profile: vec![
                        Point2::new(0., 0.),
                        Point2::new(1., 0.),
                        Point2::new(0.5, 1.),
                    ],
                    ..Default::default()
                },
                name: "Triangle".into(),
                kind: OpeningKind::Door,
                width: 1.,
                height: 2.,
                sill: 0.,
                pane_position: WindowPanePosition::Center,
            },
        );
        let type_id = ty.id();
        model.opening_types.insert(type_id, ty);
        for hinge in [DoorHinge::Start, DoorHinge::End] {
            for swing in [DoorSwing::Left, DoorSwing::Right] {
                let p = model
                    .resolve_opening(&os_model::OpeningParams {
                        name: "Door".into(),
                        host: level,
                        offset: 1.,
                        definition: os_model::OpeningDefinition::Typed { type_id },
                        hinge,
                        swing,
                    })
                    .unwrap();
                let mesh = component_mesh(&p, &wall, 2.5).unwrap();
                assert!((mesh.signed_volume() - 0.025).abs() < 1e-10);
                assert_eq!(plan_spans(&p, 2.5, 3.5, 2.).unwrap(), vec![(0.25, 0.75)]);
                assert_eq!(plan_spans(&p, 2.5, 5., 2.).unwrap(), vec![(0., 1.)]);
                assert!(plan_spans(&p, 2.5, 2., 1.).unwrap().is_empty());
                assert!(plan_spans(&p, 2.5, 6., 5.).unwrap().is_empty());
            }
        }
    }
}
