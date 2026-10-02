//! Shared positive component evaluator. Does not perform host booleans.
use crate::{
    GeometryKernel, Mesh,
    floors::{extrude_floor, triangulate_floor},
};
use os_core::{Point2, Result};
use os_model::{
    DoorHinge, DoorSwing, OpeningKind, ResolvedOpening, WallParams, WindowOperation,
    WindowPanePosition,
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
            let angle = p.door_angle(wall);
            if angle != 90. {
                let (sin, cos) = angle.to_radians().sin_cos();
                let direction = if p.hinge == DoorHinge::Start { 1. } else { -1. };
                return Point2::new(
                    p.offset
                        + if p.hinge == DoorHinge::End {
                            p.width
                        } else {
                            0.
                        }
                        + direction * u * p.width * cos,
                    side * (u * p.width * sin - wall.thickness / 2.),
                );
            }
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
    if !wall.path.is_straight() {
        return wall.path.offset_point(x, y);
    }
    let dx = (wall.end().x - wall.start().x) / wall.length();
    let dy = (wall.end().y - wall.start().y) / wall.length();
    Point2::new(
        wall.start().x + dx * x - dy * y,
        wall.start().y + dy * x + dx * y,
    )
}

/// Active then inactive leaf. Both geometry and plan symbols use these resolved
/// clear bays and poses, so handedness changes location without exchanging identity.
pub fn door_pair(p: &ResolvedOpening) -> Result<Option<[ResolvedOpening; 2]>> {
    let os_model::DoorLeaves::Paired { active_fraction } = p.family.door_leaves else {
        return Ok(None);
    };
    p.family.validate_for(p.width, p.height, p.kind)?;
    p.validate_open_state()?;
    let (start, end) = p.family.primary_interval(p.width)?;
    let angles = match p.open_state {
        os_model::OpeningState::DoorPairAngles {
            active_degrees,
            inactive_degrees,
        } => [active_degrees, inactive_degrees],
        os_model::OpeningState::DoorAngle(a) => [a, a],
        _ => [90., 90.],
    };
    Ok(Some(std::array::from_fn(|i| {
        let mut leaf = p.clone();
        leaf.width = (end - start)
            * if i == 0 {
                active_fraction
            } else {
                1. - active_fraction
            };
        leaf.hinge = if i == 0 {
            p.hinge
        } else if p.hinge == DoorHinge::Start {
            DoorHinge::End
        } else {
            DoorHinge::Start
        };
        leaf.offset = p.offset
            + if leaf.hinge == DoorHinge::Start {
                start
            } else {
                end - leaf.width
            };
        leaf.height -= p.family.frame_width;
        leaf.open_state = os_model::OpeningState::DoorAngle(angles[i]);
        leaf.family.door_leaves = os_model::DoorLeaves::Single;
        leaf.family.side_lite = None;
        leaf.family.frame_width = 0.;
        leaf.family.profile = os_model::OpeningFamily::rectangle();
        leaf
    })))
}

fn paired_panels(p: &ResolvedOpening, wall: &WallParams, elevation: f64) -> Result<Mesh> {
    let mut mesh = Mesh::default();
    for mut leaf in
        door_pair(p)?.ok_or_else(|| os_core::Error::Invalid("expected paired door".into()))?
    {
        leaf.family.depth = depth(p, wall);
        append_mesh(&mut mesh, component_mesh(&leaf, wall, elevation)?)?;
    }
    Ok(mesh)
}

pub fn component_mesh(p: &ResolvedOpening, wall: &WallParams, elevation: f64) -> Result<Mesh> {
    p.validate_host(wall)?;
    p.family.validate_for(p.width, p.height, p.kind)?;
    if !wall.path.is_straight() {
        return arc_component_mesh(p, wall, elevation);
    }
    if let Some(bays) = p.family.bays(p.width)? {
        return two_bay_mesh(p, wall, elevation, bays);
    }
    let thickness = depth(p, wall);
    let sash = has_closed_sash(p);
    let paired = matches!(p.family.door_leaves, os_model::DoorLeaves::Paired { .. });
    if !paired
        && !sash
        && p.family.frame_width == 0.0
        && p.family.profile == os_model::OpeningFamily::default().profile
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
    let mut mesh = if paired {
        paired_panels(p, wall, elevation)?
    } else if sash {
        let mut clear = p.clone();
        let f = p.family.frame_width;
        clear.offset += f;
        clear.sill += f;
        clear.width -= 2. * f;
        clear.height -= 2. * f;
        closed_sash_mesh(p, &clear, wall, elevation)?
    } else {
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
        mesh
    };
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

// Called only on straight hosts. Recognize equivalent rectangle windings and
// collinear boundary vertices, just as rectangular_cut does after validation.
fn has_closed_sash(p: &ResolvedOpening) -> bool {
    p.kind == OpeningKind::Window
        && p.window_operation != WindowOperation::Fixed
        && p.rectangular_component()
}

/// Pose a primary sash in host coordinates. The Start sliding sash travels
/// (clear width - meeting rail)/2 toward End; the End sash stays fixed.
/// Casements rotate toward +normal about the clear Start jamb, at most 90°.
pub fn sash_point(
    parent: &ResolvedOpening,
    bay: &ResolvedOpening,
    wall: &WallParams,
    track: usize,
    x: f64,
    y: f64,
) -> Point2 {
    if parent.supports_open_state(wall) {
        match parent.open_state {
            os_model::OpeningState::SlidingFraction(f) if track == 0 => {
                let rail = 0.05_f64.min(bay.width * 0.1).min(bay.height * 0.1);
                return Point2::new(x + f * (bay.width - rail) / 2., y);
            }
            os_model::OpeningState::CasementAngle(a) if a != 0. => {
                let center = pane_offset(parent, wall);
                let (sin, cos) = a.to_radians().sin_cos();
                let u = x - bay.offset;
                let v = y - center;
                return Point2::new(bay.offset + u * cos - v * sin, center + u * sin + v * cos);
            }
            _ => {}
        }
    }
    Point2::new(x, y)
}

/// Effective clear primary bay, shared by sash meshes and plan poses.
pub fn sash_bay(p: &ResolvedOpening) -> Result<ResolvedOpening> {
    let mut bay = primary_bay(p)?;
    if p.family.side_lite.is_none() {
        let f = p.family.frame_width;
        bay.offset += f;
        bay.sill += f;
        bay.width -= 2. * f;
        bay.height -= 2. * f;
    }
    Ok(bay)
}

/// Static construction representation in the parent's pane envelope. Derive
/// rails from the effective clear bay, including narrow legal instance pins.
/// Separate sliding tracks overlap in elevation but never in volume.
fn closed_sash_mesh(
    parent: &ResolvedOpening,
    bay: &ResolvedOpening,
    wall: &WallParams,
    elevation: f64,
) -> Result<Mesh> {
    let d = depth(parent, wall);
    let center = pane_offset(parent, wall);
    let rail = 0.05_f64.min(bay.width * 0.1).min(bay.height * 0.1);
    let tracks = if parent.window_operation == WindowOperation::Sliding {
        vec![
            (0., (bay.width + rail) / 2., center - d * 0.275, d * 0.45),
            (
                (bay.width - rail) / 2.,
                bay.width,
                center + d * 0.275,
                d * 0.45,
            ),
        ]
    } else {
        vec![(0., bay.width, center, d)]
    };
    let mut mesh = Mesh::default();
    for (track, (start, end, y, thickness)) in tracks.into_iter().enumerate() {
        let x0 = start / bay.width;
        let x1 = end / bay.width;
        let rx = rail / bay.width;
        let rz = rail / bay.height;
        for (profile, depth, material) in [
            (
                rect_profile(x0, 0., x0 + rx, 1.),
                thickness,
                parent.family.frame_material,
            ),
            (
                rect_profile(x1 - rx, 0., x1, 1.),
                thickness,
                parent.family.frame_material,
            ),
            (
                rect_profile(x0 + rx, 0., x1 - rx, rz),
                thickness,
                parent.family.frame_material,
            ),
            (
                rect_profile(x0 + rx, 1. - rz, x1 - rx, 1.),
                thickness,
                parent.family.frame_material,
            ),
            (
                rect_profile(x0 + rx, rz, x1 - rx, 1. - rz),
                thickness * 0.4,
                parent.family.panel_material,
            ),
        ] {
            let mut part = extrude_floor(&profile, depth / 2., depth)?;
            assign_material(&mut part, material);
            for v in &mut part.vertices {
                let point = sash_point(
                    parent,
                    bay,
                    wall,
                    track,
                    bay.offset + v.x * bay.width,
                    y + v.z,
                );
                let xy = world(wall, point.x, point.y);
                *v = crate::Vec3::new(xy.x, xy.y, elevation + bay.sill + v.y * bay.height);
            }
            for triangle in &mut part.triangles {
                triangle.swap(1, 2);
            }
            append_mesh(&mut mesh, part)?;
        }
    }
    mesh.validate()?;
    Ok(mesh)
}

/// Sweep convex, nonoverlapping material regions in (station, elevation).
/// First split at every authored station and the bounded display grid. Then
/// conform vertical edges before canceling internal region edges. This creates
/// one indexed boundary, with radial reveals and no internal caps/T junctions.
/// Quantities deliberately do not use this display approximation.
pub(crate) fn radial_regions_mesh(
    wall: &WallParams,
    elevation: f64,
    regions: &[Vec<Point2>],
    min: f64,
    max: f64,
) -> Result<Mesh> {
    use os_core::ensure;
    use std::collections::BTreeMap;
    let os_model::WallPath::CircularArc {
        radius,
        signed_sweep_rad,
        ..
    } = wall.path
    else {
        return Err(os_core::Error::Invalid(
            "radial sweep requires an arc".into(),
        ));
    };
    ensure(
        elevation.is_finite() && min.is_finite() && max.is_finite() && max > min,
        "invalid radial extrusion",
    )?;
    ensure(
        radius - signed_sweep_rad.signum() * min > 1e-6
            && radius - signed_sweep_rad.signum() * max > 1e-6,
        "radial extrusion crosses circle centre",
    )?;
    ensure(regions.len() <= 4096, "too many radial material regions")?;
    // All wall layers share the host display grid so their coincident faces
    // also share the same approximation, including when layer widths differ.
    let count = wall
        .path
        .display_segments((wall.thickness / 2.).max(min.abs()).max(max.abs()))?;
    let mut stations: Vec<_> = (0..=count)
        .map(|i| wall.length() * i as f64 / count as f64)
        .collect();
    stations.extend(regions.iter().flatten().map(|p| p.x));
    ensure(
        stations
            .iter()
            .all(|s| s.is_finite() && *s >= 0. && *s <= wall.length()),
        "radial profile exceeds host stations",
    )?;
    stations.sort_by(f64::total_cmp);
    stations.dedup();
    // Each strip endpoint records its station index. Heights computed from
    // opposite sides of a shared edge are canonicalized only at roundoff scale.
    let mut strips: Vec<Vec<(usize, f64)>> = Vec::new();
    let mut heights = vec![Vec::<f64>::new(); stations.len()];
    for region in regions {
        ensure(
            region.len() >= 3 && region.iter().all(|p| p.is_finite()),
            "invalid radial region",
        )?;
        let lo = region.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let hi = region.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        for (i, pair) in stations.windows(2).enumerate() {
            if pair[0] < lo || pair[1] > hi {
                continue;
            }
            let mut ring = clip_station(&clip_station(region, pair[0], true), pair[1], false);
            ring.dedup_by(|a, b| a.distance(*b) < 1e-12);
            if ring.len() > 1 && ring[0].distance(*ring.last().unwrap()) < 1e-12 {
                ring.pop();
            }
            if ring.len() < 3 {
                continue;
            }
            let area = crate::floors::signed_area(&ring);
            if area == 0. {
                continue;
            }
            ensure(
                area > 0.,
                "radial regions must be counterclockwise and convex",
            )?;
            let strip: Vec<_> = ring
                .iter()
                .map(|p| {
                    let index = if p.x == pair[0] { i } else { i + 1 };
                    heights[index].push(p.y);
                    (index, p.y)
                })
                .collect();
            strips.push(strip);
            ensure(
                strips.len() <= 8192,
                "radial display region budget exceeded",
            )?;
        }
    }
    let mut points = Vec::<Point2>::new();
    let mut bases = Vec::new();
    for (i, ys) in heights.iter_mut().enumerate() {
        ys.sort_by(f64::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() <= 1e-10);
        bases.push(points.len());
        points.extend(ys.iter().map(|y| Point2::new(stations[i], *y)));
    }
    let mut rings = Vec::new();
    for strip in strips {
        let mut ring = Vec::new();
        for (&(i, y), &(j, z)) in strip
            .iter()
            .zip(strip.iter().cycle().skip(1))
            .take(strip.len())
        {
            let index = heights[i].partition_point(|v| *v < y - 1e-10);
            ring.push((bases[i] + index) as u32);
            if i == j {
                let mut inside: Vec<_> = heights[i]
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v > y.min(z) + 1e-10 && **v < y.max(z) - 1e-10)
                    .map(|(k, _)| (bases[i] + k) as u32)
                    .collect();
                if z < y {
                    inside.reverse();
                }
                ring.extend(inside);
            }
        }
        rings.push(ring);
    }
    ensure(
        2 * (points.len() + rings.len()) <= crate::section::MAX_SECTION_INPUT_VERTICES,
        "radial display vertex budget exceeded",
    )?;
    let mut mesh = Mesh::default();
    let map = |p: Point2, y| {
        let xy = wall.path.offset_point(p.x, y);
        crate::Vec3::new(xy.x, xy.y, elevation + p.y)
    };
    for p in &points {
        mesh.vertices.extend([map(*p, min), map(*p, max)]);
    }
    let mut edges = BTreeMap::<(u32, u32), usize>::new();
    for ring in rings {
        let center = ring.iter().fold(Point2::default(), |p, &i| {
            Point2::new(
                p.x + points[i as usize].x / ring.len() as f64,
                p.y + points[i as usize].y / ring.len() as f64,
            )
        });
        let c = mesh.vertices.len() as u32;
        mesh.vertices.extend([map(center, min), map(center, max)]);
        for (&a, &b) in ring
            .iter()
            .zip(ring.iter().cycle().skip(1))
            .take(ring.len())
        {
            mesh.triangles
                .extend([[c, 2 * a, 2 * b], [c + 1, 2 * b + 1, 2 * a + 1]]);
            *edges.entry((a, b)).or_default() += 1;
        }
    }
    for (&(a, b), &uses) in &edges {
        ensure(uses == 1, "overlapping radial material regions")?;
        if !edges.contains_key(&(b, a)) {
            mesh.triangles
                .extend([[2 * a, 2 * a + 1, 2 * b + 1], [2 * a, 2 * b + 1, 2 * b]]);
        }
    }
    ensure(
        mesh.triangles.len() <= crate::section::MAX_SECTION_INPUT_TRIANGLES,
        "radial display triangle budget exceeded",
    )?;
    mesh.validate()?;
    Ok(mesh)
}

fn clip_station(input: &[Point2], station: f64, after: bool) -> Vec<Point2> {
    let mut out = Vec::new();
    for (&a, &b) in input
        .iter()
        .zip(input.iter().cycle().skip(1))
        .take(input.len())
    {
        let inside = |p: Point2| {
            if after {
                p.x >= station
            } else {
                p.x <= station
            }
        };
        if inside(a) {
            out.push(a);
        }
        if inside(a) != inside(b) {
            out.push(Point2::new(
                station,
                a.y + (station - a.x) * (b.y - a.y) / (b.x - a.x),
            ));
        }
    }
    out
}

fn arc_component_mesh(p: &ResolvedOpening, wall: &WallParams, elevation: f64) -> Result<Mesh> {
    os_core::ensure(elevation.is_finite(), "invalid opening elevation")?;
    let thickness = depth(p, wall);
    let f = p.family.frame_width;
    let bottom = if p.kind == OpeningKind::Window { f } else { 0. };
    let bays = p.family.bays(p.width)?;
    let mut primary = primary_bay(p)?;
    if bays.is_none() {
        primary.offset += f;
        primary.width -= 2. * f;
        primary.sill += bottom;
        primary.height -= bottom + f;
        primary.family.frame_width = 0.;
    }
    let make_panel = |component: &ResolvedOpening| -> Result<Mesh> {
        let mut mesh = if component.kind == OpeningKind::Door {
            // A leaf is a Euclidean extrusion in one tangent-at-hinge frame.
            // In particular, thickness must not be interpreted as arc travel:
            // doing so bends the leaf and changes its physical dimensions.
            let station = component.offset
                + if component.hinge == DoorHinge::End {
                    component.width
                } else {
                    0.
                };
            let hinge = wall.path.point(station);
            let tangent = wall.path.tangent(station);
            let side = if component.swing == DoorSwing::Left {
                1.
            } else {
                -1.
            };
            let mut mesh = extrude_floor(&component.family.profile, thickness / 2., thickness)?;
            for v in &mut mesh.vertices {
                let normal = side * (v.x * component.width - wall.thickness / 2.);
                let along = side * v.z;
                *v = crate::Vec3::new(
                    hinge.x + tangent.x * along - tangent.y * normal,
                    hinge.y + tangent.y * along + tangent.x * normal,
                    elevation + component.sill + v.y * component.height,
                );
            }
            mesh
        } else {
            // All fixed panes, including door side lites, share the parent
            // pane depth/alignment. No bay-specific depth truncation occurs.
            let y = pane_offset(p, wall);
            radial_profile_mesh(
                wall,
                elevation,
                &component.family.profile,
                component.offset,
                component.width,
                component.sill,
                component.height,
                y - thickness / 2.,
                y + thickness / 2.,
            )?
        };
        assign_material(&mut mesh, component.family.panel_material);
        mesh.validate()?;
        Ok(mesh)
    };
    let mut mesh = make_panel(&primary)?;
    let mut bars = Vec::new();
    if let Some(bays) = bays {
        let mut lite = primary.clone();
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
        bars.push(rect_profile(
            bays.mullion.0 / p.width,
            bottom / p.height,
            bays.mullion.1 / p.width,
            1. - f / p.height,
        ));
    }
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
    let y = frame_offset(p, wall);
    let d = frame_thickness(p, wall);
    for bar in bars {
        let mut frame = radial_profile_mesh(
            wall,
            elevation,
            &bar,
            p.offset,
            p.width,
            p.sill,
            p.height,
            y - d / 2.,
            y + d / 2.,
        )?;
        assign_material(&mut frame, p.family.frame_material);
        append_mesh(&mut mesh, frame)?;
    }
    os_core::ensure(
        mesh.vertices.len() <= crate::section::MAX_SECTION_INPUT_VERTICES
            && mesh.triangles.len() <= crate::section::MAX_SECTION_INPUT_TRIANGLES,
        "arc opening display budget exceeded",
    )?;
    mesh.validate()?;
    Ok(mesh)
}

#[allow(clippy::too_many_arguments)]
fn radial_profile_mesh(
    wall: &WallParams,
    elevation: f64,
    profile: &[Point2],
    offset: f64,
    width: f64,
    sill: f64,
    height: f64,
    min: f64,
    max: f64,
) -> Result<Mesh> {
    // Triangulate in normalized coordinates, then sweep the resulting convex
    // regions together so authored concavities survive without internal faces.
    let regions = triangulate_floor(profile)?
        .into_iter()
        .map(|t| {
            t.into_iter()
                .map(|i| {
                    let p = profile[i as usize];
                    Point2::new(offset + p.x * width, sill + p.y * height)
                })
                .collect()
        })
        .collect::<Vec<_>>();
    radial_regions_mesh(wall, elevation, &regions, min, max)
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
    let mut mesh = if matches!(p.family.door_leaves, os_model::DoorLeaves::Paired { .. }) {
        paired_panels(p, wall, elevation)?
    } else if has_closed_sash(p) {
        closed_sash_mesh(p, &primary, wall, elevation)?
    } else {
        make_panel(&primary)?
    };
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

    fn sash_fixture() -> (ResolvedOpening, WallParams) {
        let id = os_core::Id::new();
        (
            ResolvedOpening {
                open_state: Default::default(),
                window_operation: WindowOperation::Fixed,
                name: "Sash".into(),
                host: id,
                offset: 1.,
                kind: OpeningKind::Window,
                width: 1.2,
                height: 1.4,
                sill: 0.8,
                pane_position: WindowPanePosition::Center,
                type_id: None,
                type_name: None,
                hinge: DoorHinge::Start,
                swing: DoorSwing::Left,
                family: os_model::OpeningFamily {
                    panel_material: Some(os_core::Id::new()),
                    frame_material: Some(os_core::Id::new()),
                    ..Default::default()
                },
            },
            WallParams {
                name: "Host".into(),
                path: os_model::WallPath::Straight {
                    start: Point2::new(2., 3.),
                    end: Point2::new(8., 11.),
                },
                thickness: 0.2,
                height: 4.,
                level: id,
                material: None,
            },
        )
    }

    #[test]
    fn opening_state_door_default_exact_prism_and_rigid_intermediate_poses() {
        for hinge in [DoorHinge::Start, DoorHinge::End] {
            for swing in [DoorSwing::Left, DoorSwing::Right] {
                let (mut p, wall) = sash_fixture();
                p.kind = OpeningKind::Door;
                p.sill = 0.;
                p.hinge = hinge;
                p.swing = swing;
                let baseline = component_mesh(&p, &wall, 0.).unwrap();
                // Independent reconstruction of the historical 90° prism.
                let side = if swing == DoorSwing::Left { 1. } else { -1. };
                let x = p.offset + if hinge == DoorHinge::End { p.width } else { 0. };
                let mut leaf = wall.clone();
                leaf.path = os_model::WallPath::Straight {
                    start: world(&wall, x, -side * wall.thickness / 2.),
                    end: world(&wall, x, side * (p.width - wall.thickness / 2.)),
                };
                leaf.height = p.height;
                leaf.thickness = depth(&p, &wall);
                let mut historical = crate::PrismKernel
                    .tessellate(&crate::walls::wall_solid(&leaf, 0.).unwrap())
                    .unwrap();
                assign_material(&mut historical, p.family.panel_material);
                assert_eq!(baseline, historical);
                for angle in [0., 30., 45., 90.] {
                    p.open_state = os_model::OpeningState::DoorAngle(angle);
                    let mesh = component_mesh(&p, &wall, 0.).unwrap();
                    mesh.validate().unwrap();
                    assert!(mesh.signed_volume() > 0.);
                    assert!((mesh.signed_volume() - baseline.signed_volume()).abs() < 1e-10);
                    let a = component_point(&p, &wall, 0.);
                    let b = component_point(&p, &wall, 1.);
                    assert!((a.distance(b) - p.width).abs() < 1e-12);
                    if angle == 90. {
                        assert_eq!(mesh, baseline);
                    }
                }
            }
        }
    }

    #[test]
    fn paired_doors_clear_meeting_independent_angles_flips_materials_and_lites() {
        for fraction in [0.5, 0.65] {
            for hinge in [DoorHinge::Start, DoorHinge::End] {
                for swing in [DoorSwing::Left, DoorSwing::Right] {
                    for lite in [
                        None,
                        Some(os_model::LiteSide::Start),
                        Some(os_model::LiteSide::End),
                    ] {
                        let (mut p, mut wall) = sash_fixture();
                        p.kind = OpeningKind::Door;
                        p.sill = 0.;
                        p.width = 2.;
                        p.hinge = hinge;
                        p.swing = swing;
                        p.family.frame_width = 0.05;
                        p.family.door_leaves = os_model::DoorLeaves::Paired {
                            active_fraction: fraction,
                        };
                        p.family.side_lite = lite.map(|side| os_model::SideLite {
                            side,
                            width_fraction: 0.2,
                            mullion_width: 0.04,
                            material: Some(os_core::Id::new()),
                        });
                        p.open_state = os_model::OpeningState::DoorAngle(0.);
                        let leaves = door_pair(&p).unwrap().unwrap();
                        assert!(
                            component_point(&leaves[0], &wall, 1.)
                                .distance(component_point(&leaves[1], &wall, 1.))
                                < 1e-12
                        );
                        assert!(
                            (leaves[0].width / (leaves[0].width + leaves[1].width) - fraction)
                                .abs()
                                < 1e-12
                        );
                        assert_eq!(leaves[0].hinge, hinge);
                        for reversed in [false, true] {
                            if reversed {
                                wall.path = os_model::WallPath::Straight {
                                    start: wall.end(),
                                    end: wall.start(),
                                };
                            }
                            let closed = component_mesh(&p, &wall, 0.).unwrap();
                            p.open_state = os_model::OpeningState::DoorPairAngles {
                                active_degrees: 30.,
                                inactive_degrees: 75.,
                            };
                            let mesh = component_mesh(&p, &wall, 0.).unwrap();
                            mesh.validate().unwrap();
                            assert!((mesh.signed_volume() - closed.signed_volume()).abs() < 1e-9);
                            assert_eq!(mesh.surfaces, closed.surfaces);
                            let leaves = door_pair(&p).unwrap().unwrap();
                            for (leaf, angle) in leaves.iter().zip([30_f64, 75.]) {
                                assert_eq!(leaf.door_angle(&wall), angle);
                                let a = component_point(leaf, &wall, 0.);
                                let b = component_point(leaf, &wall, 1.);
                                assert!((a.distance(b) - leaf.width).abs() < 1e-12);
                                let sign = if swing == DoorSwing::Left { 1. } else { -1. };
                                assert!(
                                    (b.y - a.y - sign * leaf.width * angle.to_radians().sin())
                                        .abs()
                                        < 1e-12
                                );
                            }
                            p.open_state = os_model::OpeningState::DoorAngle(0.);
                        }
                        p.open_state = os_model::OpeningState::Default;
                        let default = component_mesh(&p, &wall, 0.).unwrap();
                        p.open_state = os_model::OpeningState::DoorAngle(90.);
                        assert_eq!(component_mesh(&p, &wall, 0.).unwrap(), default);
                        wall.path = os_model::WallPath::CircularArc {
                            center: Point2::new(0., 0.),
                            radius: 10.,
                            start_angle_rad: 0.,
                            signed_sweep_rad: 1.,
                        };
                        assert!(component_mesh(&p, &wall, 0.).is_err());
                    }
                }
            }
        }
    }

    #[test]
    fn opening_state_sashes_endpoints_intermediate_lites_and_bounds() {
        for operation in [WindowOperation::Sliding, WindowOperation::Casement] {
            for side in [
                None,
                Some(os_model::LiteSide::Start),
                Some(os_model::LiteSide::End),
            ] {
                for position in [
                    WindowPanePosition::Center,
                    WindowPanePosition::LeftFace,
                    WindowPanePosition::RightFace,
                ] {
                    let (mut p, wall) = sash_fixture();
                    p.window_operation = operation;
                    p.pane_position = position;
                    p.family.frame_width = 0.05;
                    p.family.side_lite = side.map(|side| os_model::SideLite {
                        side,
                        width_fraction: 0.2,
                        mullion_width: 0.04,
                        material: None,
                    });
                    let baseline = component_mesh(&p, &wall, 0.).unwrap();
                    let bay = sash_bay(&p).unwrap();
                    for fraction in [0., 0.25, 0.5, 1.] {
                        p.open_state = if operation == WindowOperation::Sliding {
                            os_model::OpeningState::SlidingFraction(fraction)
                        } else {
                            os_model::OpeningState::CasementAngle(fraction * 90.)
                        };
                        let mesh = component_mesh(&p, &wall, 0.).unwrap();
                        mesh.validate().unwrap();
                        assert!(mesh.signed_volume() > 0.);
                        assert!((mesh.signed_volume() - baseline.signed_volume()).abs() < 1e-10);
                        assert_eq!(mesh.triangles, baseline.triangles);
                        assert_eq!(mesh.surfaces, baseline.surfaces);
                        if fraction == 0. {
                            assert_eq!(mesh, baseline);
                        }
                        let primary_vertices = if operation == WindowOperation::Sliding {
                            80
                        } else {
                            40
                        };
                        assert_eq!(
                            &mesh.vertices[primary_vertices..],
                            &baseline.vertices[primary_vertices..]
                        );
                        let d = depth(&p, &wall);
                        let center = pane_offset(&p, &wall);
                        for v in &mesh.vertices[..primary_vertices] {
                            let station = wall.path.project(Point2::new(v.x, v.y));
                            let at = world(&wall, station, 0.);
                            let normal = wall.path.tangent(station);
                            let y = -(v.x - at.x) * normal.y + (v.y - at.y) * normal.x;
                            assert!(
                                station >= bay.offset - d - 1e-10
                                    && station <= bay.offset + bay.width + d + 1e-10
                            );
                            assert!(y >= center - d - 1e-10 && y <= center + bay.width + d + 1e-10);
                        }
                        let point = sash_point(&p, &bay, &wall, 0, bay.offset, center);
                        if operation == WindowOperation::Sliding {
                            let rail = 0.05_f64.min(bay.width * 0.1).min(bay.height * 0.1);
                            assert!(
                                (point.x - bay.offset - fraction * (bay.width - rail) / 2.).abs()
                                    < 1e-12
                            );
                        } else {
                            assert_eq!(point, Point2::new(bay.offset, center));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn opening_state_unsupported_profiles_and_arcs_preserve_geometry() {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            for arc in [false, true] {
                let (mut p, mut wall) = sash_fixture();
                p.kind = kind;
                if kind == OpeningKind::Door {
                    p.sill = 0.;
                }
                p.window_operation = if kind == OpeningKind::Window {
                    WindowOperation::Casement
                } else {
                    WindowOperation::Fixed
                };
                if arc {
                    wall.path = os_model::WallPath::CircularArc {
                        center: Point2::default(),
                        radius: 5.,
                        start_angle_rad: 0.,
                        signed_sweep_rad: 1.5,
                    };
                } else {
                    p.family.profile[2].y = 0.7;
                }
                let baseline = component_mesh(&p, &wall, 0.).unwrap();
                p.open_state = if kind == OpeningKind::Door {
                    os_model::OpeningState::DoorAngle(30.)
                } else {
                    os_model::OpeningState::CasementAngle(30.)
                };
                assert!(!p.supports_open_state(&wall));
                assert_eq!(component_mesh(&p, &wall, 0.).unwrap(), baseline);
            }
        }
    }

    #[test]
    fn closed_sash_tracks_bounds_materials_and_positive_solids() {
        for reversed in [false, true] {
            for position in [
                WindowPanePosition::Center,
                WindowPanePosition::LeftFace,
                WindowPanePosition::RightFace,
            ] {
                for frame in [0., 0.05] {
                    let (mut p, mut wall) = sash_fixture();
                    p.pane_position = position;
                    p.family.frame_width = frame;
                    if reversed {
                        wall.path = os_model::WallPath::Straight {
                            start: wall.end(),
                            end: wall.start(),
                        };
                    }
                    let fixed = component_mesh(&p, &wall, 7.).unwrap();
                    let mut previous = fixed.clone();
                    for operation in [WindowOperation::Casement, WindowOperation::Sliding] {
                        p.window_operation = operation;
                        let mesh = component_mesh(&p, &wall, 7.).unwrap();
                        mesh.validate().unwrap();
                        assert_ne!(mesh, fixed);
                        assert_ne!(mesh, previous);
                        previous = mesh.clone();
                        let parts = if operation == WindowOperation::Sliding {
                            10
                        } else {
                            5
                        };
                        assert_eq!(
                            mesh.triangles.len(),
                            12 * (parts + if frame > 0. { 4 } else { 0 })
                        );
                        let tangent = wall.path.tangent(0.);
                        let local = |v: &crate::Vec3| {
                            let x = v.x - wall.start().x;
                            let y = v.y - wall.start().y;
                            (
                                x * tangent.x + y * tangent.y,
                                -x * tangent.y + y * tangent.x,
                            )
                        };
                        let d = depth(&p, &wall);
                        let center = pane_offset(&p, &wall);
                        for (i, vertices) in mesh.vertices[..parts * 8].chunks_exact(8).enumerate()
                        {
                            // Each independent rail/glazing prism is closed and outward.
                            let part = Mesh {
                                vertices: vertices.to_vec(),
                                triangles: mesh.triangles[i * 12..(i + 1) * 12]
                                    .iter()
                                    .map(|t| t.map(|v| v - (i * 8) as u32))
                                    .collect(),
                                ..Default::default()
                            };
                            part.validate().unwrap();
                            assert!(part.signed_volume() > 0.);
                            let material = if i % 5 == 4 {
                                p.family.panel_material
                            } else {
                                p.family.frame_material
                            };
                            assert!(
                                mesh.surfaces[i * 12..(i + 1) * 12]
                                    .iter()
                                    .all(|s| s.material == material)
                            );
                            for v in vertices {
                                let (x, y) = local(v);
                                assert!(
                                    x >= p.offset + frame - 1e-9
                                        && x <= p.offset + p.width - frame + 1e-9
                                );
                                assert!(
                                    v.z >= 7. + p.sill + frame - 1e-9
                                        && v.z <= 7. + p.sill + p.height - frame + 1e-9
                                );
                                assert!((y - center).abs() <= d / 2. + 1e-9);
                                if operation == WindowOperation::Sliding {
                                    assert!(if i < 5 { y < center } else { y > center });
                                }
                            }
                        }
                        let ys: Vec<_> = mesh.vertices[..parts * 8]
                            .iter()
                            .map(|v| local(v).1)
                            .collect();
                        assert!(
                            (ys.iter().copied().fold(f64::INFINITY, f64::min) - (center - d / 2.))
                                .abs()
                                < 1e-9
                        );
                        assert!(
                            (ys.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                                - (center + d / 2.))
                                .abs()
                                < 1e-9
                        );
                        // Outer rails are byte-for-byte the existing Fixed rails.
                        assert_eq!(&mesh.vertices[parts * 8..], &fixed.vertices[8..]);
                        assert_eq!(&mesh.surfaces[parts * 12..], &fixed.surfaces[12..]);
                        if operation == WindowOperation::Sliding {
                            let first_end = mesh.vertices[..40]
                                .iter()
                                .map(|v| local(v).0)
                                .fold(f64::NEG_INFINITY, f64::max);
                            let second_start = mesh.vertices[40..80]
                                .iter()
                                .map(|v| local(v).0)
                                .fold(f64::INFINITY, f64::min);
                            assert!(
                                first_end > second_start,
                                "closed sashes overlap along the host"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn closed_sash_changes_only_primary_bay_and_handles_small_dimensions() {
        for side in [os_model::LiteSide::Start, os_model::LiteSide::End] {
            let (mut p, wall) = sash_fixture();
            p.family.frame_width = 0.05;
            p.family.side_lite = Some(os_model::SideLite {
                side,
                width_fraction: 0.25,
                mullion_width: 0.04,
                material: Some(os_core::Id::new()),
            });
            let fixed = component_mesh(&p, &wall, 0.).unwrap();
            for operation in [WindowOperation::Sliding, WindowOperation::Casement] {
                p.window_operation = operation;
                let mesh = component_mesh(&p, &wall, 0.).unwrap();
                let parts = if operation == WindowOperation::Sliding {
                    10
                } else {
                    5
                };
                assert_eq!(&mesh.vertices[parts * 8..], &fixed.vertices[8..]);
                assert_eq!(&mesh.surfaces[parts * 12..], &fixed.surfaces[12..]);
                let shift = (parts - 1) as u32 * 8;
                assert_eq!(
                    mesh.triangles[parts * 12..]
                        .iter()
                        .map(|t| t.map(|v| v - shift))
                        .collect::<Vec<_>>(),
                    fixed.triangles[12..]
                );
            }
        }
        for (width, height, frame) in [(0.001, 0.001, 0.), (0.103, 0.103, 0.051), (0.001, 2., 0.)] {
            let (mut p, wall) = sash_fixture();
            p.width = width;
            p.height = height;
            p.family.frame_width = frame;
            for operation in [WindowOperation::Sliding, WindowOperation::Casement] {
                p.window_operation = operation;
                let mesh = component_mesh(&p, &wall, 0.).unwrap();
                mesh.validate().unwrap();
                assert!(mesh.signed_volume() > 0.);
                assert!(mesh.vertices.len() <= 112);
            }
        }
    }

    #[test]
    fn closed_sash_fixed_doors_arc_and_custom_profile_regressions() {
        let (mut p, wall) = sash_fixture();
        // Legacy/default Fixed still uses the exact historical rectangular prism.
        let mut panel = wall.clone();
        panel.path = os_model::WallPath::Straight {
            start: world(&wall, p.offset, 0.),
            end: world(&wall, p.offset + p.width, 0.),
        };
        panel.height = p.height;
        panel.thickness = depth(&p, &wall);
        let mut expected = crate::PrismKernel
            .tessellate(&crate::walls::wall_solid(&panel, p.sill).unwrap())
            .unwrap();
        assign_material(&mut expected, p.family.panel_material);
        assert_eq!(component_mesh(&p, &wall, 0.).unwrap(), expected);
        for case in 0..4 {
            let mut host = wall.clone();
            let mut q = p.clone();
            match case {
                0 => {
                    q.kind = OpeningKind::Door;
                    q.sill = 0.;
                }
                1 => {
                    host.path = os_model::WallPath::CircularArc {
                        center: Point2::new(0., 0.),
                        radius: 4.,
                        start_angle_rad: 0.3,
                        signed_sweep_rad: 2.,
                    };
                }
                2 => {
                    q.family.profile[2].y = 0.7;
                }
                _ => {
                    q.family.profile[2].y = 0.7;
                    q.family.cut_profile = q.family.profile.clone();
                    q.family.host_cut = os_model::OpeningHostCut::Profile;
                }
            }
            let fixed = component_mesh(&q, &host, 0.).unwrap();
            for operation in [WindowOperation::Sliding, WindowOperation::Casement] {
                q.window_operation = operation;
                assert_eq!(component_mesh(&q, &host, 0.).unwrap(), fixed);
            }
        }
        // Equivalent rectangular profiles remain eligible.
        p.family.profile.reverse();
        p.window_operation = WindowOperation::Sliding;
        assert!(has_closed_sash(&p));
        component_mesh(&p, &wall, 0.).unwrap().validate().unwrap();
    }

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
                                open_state: Default::default(),
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
                    open_state: Default::default(),
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
                        open_state: Default::default(),
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
