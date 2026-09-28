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
    let mut width = p.width;
    if let Ok(Some(bays)) = p.family.bays(p.width) {
        width = (bays.primary.1 - bays.primary.0).min(bays.lite.1 - bays.lite.0);
    }
    p.family.depth.min(wall.thickness * 0.2).min(width * 0.2)
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
    let dx = (wall.end().x - wall.start().x) / wall.length();
    let dy = (wall.end().y - wall.start().y) / wall.length();
    Point2::new(
        wall.start().x + dx * x - dy * y,
        wall.start().y + dy * x + dx * y,
    )
}

pub fn component_mesh(p: &ResolvedOpening, wall: &WallParams, elevation: f64) -> Result<Mesh> {
    p.validate_host(wall)?;
    p.family.validate_for(p.width, p.height, p.kind)?;
    if let Some(bays) = p.family.bays(p.width)? {
        return two_bay_mesh(p, wall, elevation, bays);
    }
    let thickness = depth(p, wall);
    if p.family.frame_width == 0.0 && p.family.profile == os_model::OpeningFamily::default().profile
    {
        let start = component_point(p, wall, 0.);
        let end = component_point(p, wall, 1.);
        let mut panel = wall.clone();
        panel.path = os_model::WallPath::Straight {
            start: world(wall, start.x, start.y),
            end: world(wall, end.x, end.y),
        };
        panel.height = p.height;
        panel.thickness = thickness;
        let mut mesh = crate::PrismKernel
            .tessellate(&crate::walls::wall_solid(&panel, elevation + p.sill)?)?;
        assign_material(&mut mesh, p.family.panel_material);
        return Ok(mesh);
    }
    // Triangulate normalized coordinates so small/large type dimensions do not
    // change polygon predicates. Rotate the extrusion into the vertical plane.
    let profile = p.family.component_profile(p.width, p.height, p.kind);
    let mut mesh = extrude_floor(&profile, thickness / 2., thickness)?;
    assign_material(&mut mesh, p.family.panel_material);
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
        assign_material(&mut frame, p.family.frame_material);
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

/// Derived component only: the model opening and its cut retain their outer bounds.
pub fn primary_bay(p: &ResolvedOpening) -> Result<ResolvedOpening> {
    let mut primary = p.clone();
    if let Some(bays) = p.family.bays(p.width)? {
        primary.offset += bays.primary.0;
        primary.width = bays.primary.1 - bays.primary.0;
        primary.family.side_lite = None;
        primary.family.frame_width = 0.;
        primary.family.profile = os_model::OpeningFamily::rectangle();
        let bottom = if p.kind == OpeningKind::Window {
            p.family.frame_width
        } else {
            0.
        };
        primary.sill += bottom;
        primary.height -= bottom + p.family.frame_width;
    }
    Ok(primary)
}

fn two_bay_mesh(
    p: &ResolvedOpening,
    wall: &WallParams,
    elevation: f64,
    bays: os_model::OpeningBays,
) -> Result<Mesh> {
    let primary = primary_bay(p)?;
    let thickness = depth(p, wall);
    let make_panel = |component: &ResolvedOpening| -> Result<Mesh> {
        let mut part = extrude_floor(
            &os_model::OpeningFamily::rectangle(),
            thickness / 2.,
            thickness,
        )?;
        assign_material(&mut part, component.family.panel_material);
        let mut component = component.clone();
        component.family.depth = thickness;
        let a = component_point(&component, wall, 0.);
        let b = component_point(&component, wall, 1.);
        let (dx, dy) = ((b.x - a.x) / component.width, (b.y - a.y) / component.width);
        for v in &mut part.vertices {
            let center = component_point(&component, wall, v.x);
            let xy = world(wall, center.x + dy * v.z, center.y - dx * v.z);
            *v = crate::Vec3::new(
                xy.x,
                xy.y,
                elevation + component.sill + v.y * component.height,
            );
        }
        Ok(part)
    };
    let mut mesh = make_panel(&primary)?;
    let mut lite = primary;
    lite.kind = OpeningKind::Window;
    lite.offset = p.offset + bays.lite.0;
    lite.width = bays.lite.1 - bays.lite.0;
    lite.family.panel_material = p
        .family
        .side_lite
        .as_ref()
        .and_then(|l| l.material)
        .or(p.family.panel_material);
    append_mesh(&mut mesh, make_panel(&lite)?)?;
    let f = p.family.frame_width;
    let bottom = if p.kind == OpeningKind::Window { f } else { 0. };
    let mut bars = vec![rect_profile(
        bays.mullion.0 / p.width,
        bottom / p.height,
        bays.mullion.1 / p.width,
        1. - f / p.height,
    )];
    if f > 0. {
        bars.extend([
            rect_profile(0., 0., f / p.width, 1.),
            rect_profile(1. - f / p.width, 0., 1., 1.),
            rect_profile(f / p.width, 1. - f / p.height, 1. - f / p.width, 1.),
        ]);
        if p.kind == OpeningKind::Window {
            bars.push(rect_profile(
                f / p.width,
                0.,
                1. - f / p.width,
                f / p.height,
            ));
        }
    }
    for bar in bars {
        let thickness = frame_thickness(p, wall);
        let mut part = extrude_floor(&bar, thickness / 2., thickness)?;
        assign_material(&mut part, p.family.frame_material);
        for v in &mut part.vertices {
            let xy = world(wall, p.offset + v.x * p.width, frame_offset(p, wall) + v.z);
            *v = crate::Vec3::new(xy.x, xy.y, elevation + p.sill + v.y * p.height);
        }
        for t in &mut part.triangles {
            t.swap(1, 2);
        }
        append_mesh(&mut mesh, part)?;
    }
    mesh.validate()?;
    Ok(mesh)
}

fn assign_material(mesh: &mut Mesh, material: Option<os_core::Id>) {
    if material.is_some() {
        mesh.surfaces = vec![
            crate::SurfaceIdentity {
                layer: None,
                material
            };
            mesh.triangles.len()
        ];
    }
}

fn append_mesh(target: &mut Mesh, mut addition: Mesh) -> Result<()> {
    let offset = u32::try_from(target.vertices.len())
        .map_err(|_| os_core::Error::Invalid("opening family mesh is too large".into()))?;
    if !target.surfaces.is_empty() || !addition.surfaces.is_empty() {
        target
            .surfaces
            .resize(target.triangles.len(), Default::default());
        addition
            .surfaces
            .resize(addition.triangles.len(), Default::default());
        target.surfaces.extend(addition.surfaces);
    }
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
    fn two_bay_components_materials_frames_alignment_and_reversed_hosts() {
        let model = os_model::Model::new("Bays");
        let level = *model.levels.keys().next().unwrap();
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            for side in [os_model::LiteSide::Start, os_model::LiteSide::End] {
                for reversed in [false, true] {
                    for alignment in [
                        WindowPanePosition::Center,
                        WindowPanePosition::LeftFace,
                        WindowPanePosition::RightFace,
                    ] {
                        let mut wall = WallParams {
                            name: "Host".into(),
                            path: os_model::WallPath::Straight {
                                start: Point2::new(2., 3.),
                                end: Point2::new(2., 11.),
                            },
                            thickness: 0.2,
                            height: 4.,
                            level,
                            material: None,
                        };
                        if reversed {
                            wall.path = os_model::WallPath::Straight {
                                start: wall.end(),
                                end: wall.start(),
                            };
                        }
                        for frame in [0., 0.05] {
                            let panel = os_core::Id::new();
                            let lite = os_core::Id::new();
                            let mullion = os_core::Id::new();
                            let p = ResolvedOpening {
                                window_operation: Default::default(),
                                name: "Bay".into(),
                                host: level,
                                offset: 1.,
                                kind,
                                width: 1.2,
                                height: 2.,
                                sill: if kind == OpeningKind::Door { 0. } else { 0.8 },
                                pane_position: alignment,
                                type_id: None,
                                type_name: None,
                                hinge: DoorHinge::End,
                                swing: DoorSwing::Right,
                                family: os_model::OpeningFamily {
                                    frame_width: frame,
                                    panel_material: Some(panel),
                                    frame_material: Some(mullion),
                                    side_lite: Some(os_model::SideLite {
                                        side,
                                        width_fraction: 0.25,
                                        mullion_width: 0.05,
                                        material: Some(lite),
                                    }),
                                    ..Default::default()
                                },
                            };
                            let mesh = component_mesh(&p, &wall, 0.).unwrap();
                            mesh.validate().unwrap();
                            assert_eq!(
                                mesh.triangles.len(),
                                12 * (3 + if frame == 0. {
                                    0
                                } else if kind == OpeningKind::Door {
                                    3
                                } else {
                                    4
                                })
                            );
                            assert_eq!(
                                mesh.surfaces
                                    .iter()
                                    .filter(|s| s.material == Some(panel))
                                    .count(),
                                12
                            );
                            assert_eq!(
                                mesh.surfaces
                                    .iter()
                                    .filter(|s| s.material == Some(lite))
                                    .count(),
                                12
                            );
                            let separator = &mesh.vertices[16..24];
                            let low = separator.iter().map(|v| v.z).fold(f64::INFINITY, f64::min);
                            let high = separator
                                .iter()
                                .map(|v| v.z)
                                .fold(f64::NEG_INFINITY, f64::max);
                            assert!(
                                (low - p.sill
                                    - if kind == OpeningKind::Window {
                                        frame
                                    } else {
                                        0.
                                    })
                                .abs()
                                    < 1e-10
                            );
                            assert!((high - p.sill - p.height + frame).abs() < 1e-10);
                            assert_eq!(
                                cut_plan_spans(&p, 0., p.sill + 1., 0.).unwrap(),
                                vec![(0., 1.)]
                            );
                            if kind == OpeningKind::Window {
                                let normal = |v: &crate::Vec3| {
                                    if reversed {
                                        v.x - wall.start().x
                                    } else {
                                        wall.start().x - v.x
                                    }
                                };
                                for vertices in [&mesh.vertices[..8], &mesh.vertices[8..16]] {
                                    let low =
                                        vertices.iter().map(normal).fold(f64::INFINITY, f64::min);
                                    let high = vertices
                                        .iter()
                                        .map(normal)
                                        .fold(f64::NEG_INFINITY, f64::max);
                                    assert!(
                                        ((low + high) / 2. - pane_offset(&p, &wall)).abs() < 1e-10
                                    );
                                }
                            }
                            let mut inherited = p.clone();
                            inherited.family.side_lite.as_mut().unwrap().material = None;
                            let inherited = component_mesh(&inherited, &wall, 0.).unwrap();
                            assert_eq!(inherited.vertices, mesh.vertices);
                            assert_eq!(
                                inherited
                                    .surfaces
                                    .iter()
                                    .filter(|s| s.material == Some(panel))
                                    .count(),
                                24
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn authored_opening_frames_create_closed_door_and_window_assemblies() {
        let mut model = os_model::Model::new("Framed openings");
        let level = *model.levels.keys().next().unwrap();
        let wall = WallParams {
            name: "Host".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
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
                    window_operation: Default::default(),
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
                    width_override: None,
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
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
            // Material assignment classifies each triangle without changing geometry.
            for (panel_material, frame_material) in [
                (Some(os_core::Id::new()), Some(os_core::Id::new())),
                (None, Some(os_core::Id::new())),
                (Some(os_core::Id::new()), None),
            ] {
                let mut assigned = p.clone();
                assigned.family.panel_material = panel_material;
                assigned.family.frame_material = frame_material;
                let colored = component_mesh(&assigned, &wall, 0.).unwrap();
                colored.validate().unwrap();
                assert_eq!(colored.vertices, mesh.vertices);
                assert_eq!(colored.triangles, mesh.triangles);
                // The rectangular component is a prism; remaining triangles are rails.
                assert!(
                    colored.surfaces[..12]
                        .iter()
                        .all(|s| s.material == panel_material && s.layer.is_none())
                );
                assert!(
                    colored.surfaces[12..]
                        .iter()
                        .all(|s| s.material == frame_material && s.layer.is_none())
                );
                assert_eq!(colored.surfaces.len(), colored.triangles.len());
                assigned.family.frame_width = 0.;
                let plain = component_mesh(&assigned, &wall, 0.).unwrap();
                assert!(plain.surfaces.iter().all(|s| s.material == panel_material));
                if panel_material.is_some() {
                    assert_eq!(plain.surfaces.len(), plain.triangles.len());
                }
            }

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
            path: os_model::WallPath::Straight {
                start: Point2::new(3., 4.),
                end: Point2::new(7., 7.),
            },
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        };
        let ty = os_model::OpeningType::new(
            "core.opening_type",
            os_model::OpeningTypeParams {
                window_operation: Default::default(),
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
                        width_override: None,
                        height_override: None,
                        sill_override: None,
                        pane_position_override: None,
                        lite_side_override: None,
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
