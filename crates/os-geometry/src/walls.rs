//! Authoritative native straight-wall cells. Butt joins preserve authored axes,
//! footprints and per-wall quantities; only explicit end-face seams are internal.
use crate::{GeometryKernel, Mesh, Profile, Solid, Transform, Vec3};
use os_core::{Id, Point2, Result, ensure};
use os_model::{Model, ResolvedOpening, WallJoinParams, WallParams};
use std::collections::BTreeMap;

mod profiles;

#[derive(Clone, Debug)]
pub struct ButtInterface {
    pub members: [Id; 2],
    pub trace: (Point2, Point2),
    pub bottom: f64,
    pub top: f64,
    pub end_member: Id,
}

pub fn butt_interfaces(model: &Model) -> Result<Vec<ButtInterface>> {
    os_model::validate_wall_joins(model)?;
    model
        .wall_joins
        .values()
        .map(|join| {
            let p = &join.parameters;
            let members = p.members();
            let w = &model.walls[&members[0]].parameters;
            let elevation = model.levels[&w.level].parameters.elevation;
            let end_member = match *p {
                WallJoinParams::Butt { a, b } => a.wall.max(b.wall),
                WallJoinParams::Corner { a, b, owner } => {
                    if a.wall == owner {
                        b.wall
                    } else {
                        a.wall
                    }
                }
                WallJoinParams::Tee { branch, .. } => branch.wall,
            };
            Ok(ButtInterface {
                members,
                trace: os_model::wall_join_trace(model, p)?,
                bottom: elevation,
                top: elevation + w.height,
                end_member,
            })
        })
        .collect()
}

/// Suppress interfaces only for declared join members. In a cut coincident
/// with the end face, retain one full face (stable UUID owner) rather than
/// canceling both. Other cuts omit the internal vertical edge on both walls.
pub fn remove_section_interfaces(
    segments: &mut BTreeMap<Id, Vec<(Point2, Point2)>>,
    interfaces: &[ButtInterface],
    plane: crate::section::VerticalSectionPlane,
) -> Result<()> {
    use crate::section::SECTION_TOLERANCE as T;
    let len = plane.direction.x.hypot(plane.direction.y);
    ensure(len.is_finite() && len > 0.0, "invalid section direction")?;
    let (dx, dy) = (plane.direction.x / len, plane.direction.y / len);
    let project = |p: Point2| {
        Point2::new(
            (p.x - plane.origin.x) * dx + (p.y - plane.origin.y) * dy,
            -(p.x - plane.origin.x) * dy + (p.y - plane.origin.y) * dx,
        )
    };
    for face in interfaces {
        if !face.members.iter().all(|id| segments.contains_key(id)) {
            continue;
        }
        let (a, b) = (project(face.trace.0), project(face.trace.1));
        if a.y.abs() <= T && b.y.abs() <= T {
            segments.get_mut(&face.end_member).unwrap().clear();
            continue;
        }
        if (a.y > T && b.y > T) || (a.y < -T && b.y < -T) {
            continue;
        }
        let denominator = b.y - a.y;
        if denominator.abs() <= T {
            continue;
        }
        let x = a.x - a.y * (b.x - a.x) / denominator;
        for id in face.members {
            let edges = segments.get_mut(&id).unwrap();
            *edges = edges
                .iter()
                .flat_map(|&edge| {
                    subtract_segment(
                        edge,
                        (Point2::new(x, face.bottom), Point2::new(x, face.top)),
                    )
                })
                .collect();
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct NativeWall {
    pub layers: Vec<os_model::ResolvedWallLayer>,
    pub entity: Id,
    pub parameters: WallParams,
    pub elevation: f64,
    pub openings: Vec<ResolvedOpening>,
    pub stations: (f64, f64),
    pub interfaces: Vec<(Point2, Point2)>,
}
impl NativeWall {
    /// Capture bounded semantic inputs from a validated model for worker use.
    pub fn from_model(model: &Model, entity: Id) -> Result<Self> {
        let wall = model
            .walls
            .get(&entity)
            .ok_or_else(|| os_core::Error::Invalid("native wall missing".into()))?;
        let mut interfaces = Vec::new();
        for join in model.wall_joins.values() {
            if join.parameters.members().contains(&entity) {
                join.parameters.validate(model)?;
                interfaces.push(os_model::wall_join_trace(model, &join.parameters)?);
            }
        }
        let resolved = model.resolve_wall(wall.id())?;
        let mut parameters = resolved.parameters;
        parameters.name = "Native wall geometry".into();
        let elevation = model
            .levels
            .get(&parameters.level)
            .ok_or_else(|| os_core::Error::Invalid("wall level missing".into()))?
            .parameters
            .elevation;
        let openings = model
            .openings
            .values()
            .filter(|o| o.parameters.host == entity)
            .map(|o| model.resolve_opening(&o.parameters))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            layers: resolved.layers,
            entity,
            parameters,
            elevation,
            openings,
            stations: os_model::wall_station_limits(model, entity)?,
            interfaces,
        })
    }
    pub fn cells(&self) -> Result<Vec<Solid>> {
        Ok(self
            .layer_cells()?
            .into_iter()
            .flat_map(|(_, cells)| cells)
            .collect())
    }
    pub fn layer_cells(&self) -> Result<Vec<(os_model::ResolvedWallLayer, Vec<Solid>)>> {
        let base = wall_prisms_at(
            &self.parameters,
            self.elevation,
            &self.openings,
            self.stations,
        )?;
        Ok(self
            .layers
            .iter()
            .map(|layer| {
                let cells = base
                    .iter()
                    .cloned()
                    .map(|mut cell| {
                        cell.profile.vertices[0].y = layer.min;
                        cell.profile.vertices[1].y = layer.min;
                        cell.profile.vertices[2].y = layer.max;
                        cell.profile.vertices[3].y = layer.max;
                        cell
                    })
                    .collect();
                (layer.clone(), cells)
            })
            .collect())
    }
    pub fn net_volume(&self) -> Result<f64> {
        if self.has_profile_cuts() {
            let volume: f64 = self.layer_quantities()?.iter().map(|q| q.volume_m3).sum();
            ensure(volume.is_finite(), "wall volume overflow")?;
            return Ok(volume);
        }
        let volume: f64 = self
            .cells()?
            .iter()
            .map(|s| {
                (s.profile.vertices[1].x - s.profile.vertices[0].x)
                    * (s.profile.vertices[3].y - s.profile.vertices[0].y)
                    * s.height
            })
            .sum();
        ensure(volume.is_finite(), "wall volume overflow")?;
        Ok(volume)
    }
    pub fn mesh(&self) -> Result<Mesh> {
        let mut result = Mesh::default();
        for layer in &self.layers {
            let mut part = self.clone();
            part.layers = vec![layer.clone()];
            let mesh = if self.has_profile_cuts() {
                part.profile_mesh()?
            } else {
                part.mesh_cells()?
            };
            let offset = result.vertices.len() as u32;
            result
                .triangles
                .extend(mesh.triangles.iter().map(|t| t.map(|i| i + offset)));
            result.vertices.extend(mesh.vertices);
            result.surfaces.extend(std::iter::repeat_n(
                crate::SurfaceIdentity {
                    layer: layer.id,
                    material: layer.material,
                },
                mesh.triangles.len(),
            ));
        }
        result.validate()?;
        Ok(result)
    }
    pub fn layer_quantities(&self) -> Result<Vec<LayerQuantity>> {
        if self.has_profile_cuts() {
            return self.profile_quantities();
        }
        self.layer_cells()?
            .into_iter()
            .map(|(layer, cells)| {
                let volume_m3: f64 = cells
                    .iter()
                    .map(|s| {
                        (s.profile.vertices[1].x - s.profile.vertices[0].x)
                            * (layer.max - layer.min)
                            * s.height
                    })
                    .sum();
                let mass_kg = layer.density_kg_m3.map(|density| density * volume_m3);
                ensure(
                    volume_m3.is_finite() && mass_kg.is_none_or(f64::is_finite),
                    "layer quantity overflow",
                )?;
                Ok(LayerQuantity {
                    layer: layer.id,
                    material: layer.material,
                    name: layer.name,
                    volume_m3,
                    mass_kg,
                })
            })
            .collect()
    }
    fn mesh_cells(&self) -> Result<Mesh> {
        self.mesh_solids(self.cells()?)
    }
    fn mesh_solids(&self, cells: Vec<Solid>) -> Result<Mesh> {
        if self.interfaces.is_empty() {
            return Mesh::from_prisms(&cells);
        }
        let mut result = Mesh::default();
        for cell in cells {
            let mesh = crate::PrismKernel.tessellate(&cell)?;
            let offset = result.vertices.len() as u32;
            result
                .triangles
                .extend(mesh.triangles[..4].iter().map(|t| t.map(|i| i + offset)));
            result.vertices.extend_from_slice(&mesh.vertices);
            for i in 0..4 {
                let a = mesh.vertices[i];
                let b = mesh.vertices[(i + 1) % 4];
                let mut edges = vec![(Point2::new(a.x, a.y), Point2::new(b.x, b.y))];
                for &seam in &self.interfaces {
                    edges = edges
                        .into_iter()
                        .flat_map(|e| subtract_segment(e, seam))
                        .collect();
                }
                for (a, b) in edges {
                    let n = result.vertices.len() as u32;
                    let z = cell.transform.translation.z;
                    result.vertices.extend([
                        Vec3::new(a.x, a.y, z),
                        Vec3::new(b.x, b.y, z),
                        Vec3::new(b.x, b.y, z + cell.height),
                        Vec3::new(a.x, a.y, z + cell.height),
                    ]);
                    result
                        .triangles
                        .extend([[n, n + 1, n + 2], [n, n + 2, n + 3]]);
                }
            }
        }
        result.validate()?;
        Ok(result)
    }
    /// World-space shared face trace, used to suppress only persisted joins.
    pub fn seams(&self) -> Vec<(Point2, Point2)> {
        self.interfaces.clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerQuantity {
    pub layer: Option<Id>,
    pub material: Option<Id>,
    pub name: String,
    pub volume_m3: f64,
    pub mass_kg: Option<f64>,
}

pub fn wall_solid(wall: &WallParams, elevation: f64) -> Result<Solid> {
    wall.validate()?;
    ensure(elevation.is_finite(), "invalid level elevation")?;
    let length = wall.length();
    let half = wall.thickness / 2.0;
    Ok(Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(0., -half),
                Point2::new(length, -half),
                Point2::new(length, half),
                Point2::new(0., half),
            ],
        },
        height: wall.height,
        transform: Transform {
            translation: Vec3::new(wall.start.x, wall.start.y, elevation),
            rotation_z: (wall.end.y - wall.start.y).atan2(wall.end.x - wall.start.x),
        },
    })
}

pub fn wall_prisms(
    wall: &WallParams,
    elevation: f64,
    openings: &[ResolvedOpening],
) -> Result<Vec<Solid>> {
    wall_prisms_at(wall, elevation, openings, (0.0, wall.length()))
}

fn wall_prisms_at(
    wall: &WallParams,
    elevation: f64,
    openings: &[ResolvedOpening],
    stations: (f64, f64),
) -> Result<Vec<Solid>> {
    validate_station_limits(wall, stations)?;
    if openings.iter().any(|o| !o.family.rectangular_cut()) {
        return Err(os_core::Error::Unsupported("elevation-profile wall cuts cannot be represented by vertical Solid prisms; use NativeWall mesh or plan footprints".into()));
    }
    let base = wall_solid(wall, elevation)?;
    let mut openings: Vec<_> = openings.iter().collect();
    openings.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    let mut result = Vec::new();
    let mut cell = |start: f64, end: f64, bottom: f64, top: f64| {
        let mut solid = base.clone();
        solid.profile.vertices[0].x = start;
        solid.profile.vertices[3].x = start;
        solid.profile.vertices[1].x = end;
        solid.profile.vertices[2].x = end;
        solid.transform.translation.z += bottom;
        solid.height = top - bottom;
        result.push(solid);
    };
    let mut cursor = stations.0;
    for opening in openings {
        opening.validate_host(wall)?;
        ensure(
            opening.offset >= cursor + 0.001,
            "opening overlap or insufficient separation",
        )?;
        cell(cursor, opening.offset, 0., wall.height);
        let end = opening.offset + opening.width;
        if opening.sill > 0. {
            cell(opening.offset, end, 0., opening.sill);
        }
        cell(
            opening.offset,
            end,
            opening.sill + opening.height,
            wall.height,
        );
        cursor = end;
    }
    ensure(
        stations.1 >= cursor + 0.001,
        "opening intersects trimmed wall end",
    )?;
    cell(cursor, stations.1, 0., wall.height);
    Ok(result)
}

fn validate_station_limits(wall: &WallParams, stations: (f64, f64)) -> Result<()> {
    wall.validate()?;
    let length = wall.length();
    // Corner owners intentionally extend half a wall thickness beyond their
    // authored endpoint; the mating member is trimmed by the same amount.
    let join_allowance = wall.thickness / 2.0 + 1e-9;
    ensure(
        stations.0.is_finite()
            && stations.1.is_finite()
            && stations.0 >= -join_allowance
            && stations.0 + 0.001 <= stations.1
            && stations.1 <= length + join_allowance,
        "invalid wall station limits",
    )
}

/// Subtract a collinear interval, preserving direction and exposed subsegments.
/// Used by mesh faces, clipped plan strokes and section interfaces.
pub fn subtract_segment(edge: (Point2, Point2), cut: (Point2, Point2)) -> Vec<(Point2, Point2)> {
    let (a, b) = edge;
    let length = a.distance(b);
    if length <= 1e-9 {
        return Vec::new();
    }
    let d = Point2::new((b.x - a.x) / length, (b.y - a.y) / length);
    let project = |p: Point2| {
        (
            (p.x - a.x) * d.x + (p.y - a.y) * d.y,
            (p.x - a.x) * d.y - (p.y - a.y) * d.x,
        )
    };
    let (p, q) = (project(cut.0), project(cut.1));
    if p.1.abs() > 1e-8 || q.1.abs() > 1e-8 {
        return vec![edge];
    }
    let lo = p.0.min(q.0).max(0.0);
    let hi = p.0.max(q.0).min(length);
    if hi - lo <= 1e-9 {
        return vec![edge];
    }
    let point = |s| Point2::new(a.x + d.x * s, a.y + d.y * s);
    let mut result = Vec::new();
    if lo > 1e-9 {
        result.push((a, point(lo)));
    }
    if length - hi > 1e-9 {
        result.push((point(hi), b));
    }
    result
}
