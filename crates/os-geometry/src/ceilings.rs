//! The same bounded perforated shell supplies scene, section and RCP graphics.
use crate::Mesh;
use crate::plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, PlanRole};
use os_core::{Id, Point2, Result, ensure};
use os_model::{CeilingParams, Model};
use std::collections::BTreeMap;

/// Resolves room-backed ceilings once per level per derived scene/drawing. A
/// broken room remains an intentional unresolved association and is omitted
/// from derived geometry without blocking edits to its source walls.
#[derive(Default)]
pub struct CeilingResolver {
    faces_by_level: BTreeMap<Id, Option<crate::rooms::RoomFaces>>,
}

impl CeilingResolver {
    /// Resolve the current accepted room boundary without testing the ceiling's
    /// openings against it. This is also useful when detaching a ceiling whose
    /// source still resolves but whose retained openings no longer fit.
    pub fn resolved_room_boundary(
        &mut self,
        model: &Model,
        parameters: &CeilingParams,
    ) -> Result<Option<Vec<Point2>>> {
        let Some(room_id) = parameters.boundary_room else {
            return Ok(None);
        };
        let Some(room) = model.rooms.get(&room_id) else {
            return Ok(None);
        };
        if room.parameters.level != parameters.level {
            return Ok(None);
        }
        let faces = self
            .faces_by_level
            .entry(parameters.level)
            .or_insert_with(|| {
                let segments = model.room_boundary_segments(parameters.level).ok()?;
                crate::rooms::derive_faces(&segments).ok()
            });
        let Some(faces) = faces else {
            return Ok(None);
        };
        let Some(key) = crate::rooms::FaceKey::from_signature(&room.parameters.boundary_signature)
        else {
            return Ok(None);
        };
        let Ok(face) = faces.resolve_seed(room.parameters.seed, &key) else {
            return Ok(None);
        };
        Ok(Some(face.boundary.clone()))
    }

    pub fn effective_parameters(
        &mut self,
        model: &Model,
        parameters: &CeilingParams,
    ) -> Result<Option<CeilingParams>> {
        parameters.validate_in(model)?;
        if parameters.boundary_room.is_none() {
            return Ok(Some(parameters.clone()));
        }
        let Some(boundary) = self.resolved_room_boundary(model, parameters)? else {
            return Ok(None);
        };
        let mut effective = parameters.clone();
        effective.boundary = boundary;
        // A changed room can make a preserved ceiling opening invalid. Report
        // that as an unresolved association; the cached outline remains intact
        // for repair or explicit detachment.
        if effective.validate_in(model).is_err() {
            return Ok(None);
        }
        Ok(Some(effective))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CeilingPlan {
    /// Boundary and holes in the saved view plane; holes remain empty in fill/pick.
    pub boundary: Vec<Point2>,
    pub holes: Vec<Vec<Point2>>,
    pub vertices: Vec<Point2>,
    pub triangles: Vec<[u32; 3]>,
    pub area_m2: f64,
}

/// Derive a closed solid with its underside at `level_z + elevation_offset`.
pub fn ceiling_mesh(p: &CeilingParams, level_z: f64) -> Result<Mesh> {
    p.validate_at(level_z)?;
    let mut mesh = crate::floor_holes::extrude_floor_rings(
        &p.boundary,
        &p.holes,
        level_z + p.elevation_offset + p.thickness,
        p.thickness,
    )?;
    mesh.surfaces.resize(
        mesh.triangles.len(),
        crate::SurfaceIdentity {
            layer: None,
            material: p.material,
        },
    );
    mesh.validate()?;
    Ok(mesh)
}

/// Project the underside footprint only when the ceiling shell intersects the
/// saved view range. RCP ranges are converted to the geometry kernel's ordered
/// vertical interval by the view controller before reaching this function.
pub fn ceiling_plan(
    p: &CeilingParams,
    level_z: f64,
    range: PlanRange,
    basis: HorizontalBasis,
    crop: Option<PlanCrop>,
) -> Result<Option<CeilingPlan>> {
    p.validate_at(level_z)?;
    range.validate()?;
    basis.validate()?;
    if let Some(crop) = crop {
        crop.validate()?;
    }
    let underside = level_z + p.elevation_offset;
    let top = underside + p.thickness;
    ensure(
        underside.is_finite() && top.is_finite(),
        "ceiling projection elevation overflow",
    )?;
    if top <= range.depth || underside > range.top {
        return Ok(None);
    }
    let boundary = p
        .boundary
        .iter()
        .map(|point| basis.world_to_plane(*point))
        .collect::<Result<Vec<_>>>()?;
    let holes = p
        .holes
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|point| basis.world_to_plane(*point))
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    let triangulation = crate::floor_holes::triangulate_floor_rings(&boundary, &holes)?;
    // Constructing footprints here is a checked postcondition: every output
    // triangle has positive area and the same crop clipping policy as plans.
    for triangle in triangulation.triangles.iter().copied() {
        let points = triangle.map(|index| triangulation.vertices[index as usize]);
        let _ = PlanFootprint::from_convex(PlanRole::Projected, points.to_vec(), crop)?;
    }
    Ok(Some(CeilingPlan {
        boundary,
        holes,
        vertices: triangulation.vertices,
        triangles: triangulation.triangles,
        area_m2: triangulation.net_area,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{Room, RoomParams, Wall, WallParams};

    fn params() -> CeilingParams {
        CeilingParams {
            name: "RCP".into(),
            level: os_core::Id::new(),
            material: None,
            boundary_room: None,
            boundary: vec![
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 0.0),
                Point2::new(8.0, 6.0),
                Point2::new(0.0, 6.0),
            ],
            holes: vec![vec![
                Point2::new(2.0, 2.0),
                Point2::new(4.0, 2.0),
                Point2::new(4.0, 4.0),
                Point2::new(2.0, 4.0),
            ]],
            thickness: 0.12,
            elevation_offset: 2.55,
        }
    }

    fn room_backed_ceiling_fixture() -> (Model, CeilingParams, Id, [Id; 3]) {
        let mut model = Model::new("Room-backed ceiling");
        let level = *model.levels.keys().next().unwrap();
        let boundary = [
            (Point2::new(0.0, 0.0), Point2::new(8.0, 0.0)),
            (Point2::new(8.0, 0.0), Point2::new(8.0, 6.0)),
            (Point2::new(8.0, 6.0), Point2::new(0.0, 6.0)),
            (Point2::new(0.0, 6.0), Point2::new(0.0, 0.0)),
        ];
        let walls = boundary
            .into_iter()
            .map(|(start, end)| {
                let wall = Wall::new(
                    "core.wall",
                    WallParams {
                        name: "Boundary".into(),
                        path: os_model::WallPath::Straight { start, end },
                        thickness: 0.2,
                        height: 3.0,
                        level,
                        material: None,
                    },
                );
                let id = wall.id();
                model.walls.insert(id, wall);
                id
            })
            .collect::<Vec<_>>();
        let seed = Point2::new(1.0, 1.0);
        let face = crate::rooms::derive_faces(&model.room_boundary_segments(level).unwrap())
            .unwrap()
            .assign_seed(seed)
            .unwrap()
            .clone();
        let room = Room::new(
            "core.room",
            RoomParams {
                number: "101".into(),
                name: "Office".into(),
                floor_finish: None,
                wall_finish: None,
                ceiling_finish: None,
                floor_material: None,
                wall_material: None,
                ceiling_material: None,
                level,
                seed,
                boundary_signature: face.key.as_signature().to_vec(),
            },
        );
        let room_id = room.id();
        model.rooms.insert(room_id, room);
        (
            model,
            CeilingParams {
                level,
                boundary_room: Some(room_id),
                boundary: face.boundary,
                ..params()
            },
            room_id,
            [walls[0], walls[1], walls[2]],
        )
    }

    #[test]
    fn ceiling_mesh_is_closed_and_respects_underside_offset_and_hole() {
        let mesh = ceiling_mesh(&params(), 0.0).unwrap();
        mesh.validate().unwrap();
        assert!(
            (mesh
                .vertices
                .iter()
                .map(|v| v.z)
                .fold(f64::INFINITY, f64::min)
                - 2.55)
                .abs()
                < 1e-9
        );
        assert!(
            (mesh
                .vertices
                .iter()
                .map(|v| v.z)
                .fold(f64::NEG_INFINITY, f64::max)
                - 2.67)
                .abs()
                < 1e-9
        );
        assert!(
            mesh.surfaces
                .iter()
                .all(|surface| surface.material.is_none())
        );
    }

    #[test]
    fn room_backed_ceiling_tracks_room_and_unresolves_without_stale_geometry() {
        let (mut model, parameters, _room, [bottom_wall, right_wall, top_wall]) =
            room_backed_ceiling_fixture();
        let initial = CeilingResolver::default()
            .effective_parameters(&model, &parameters)
            .unwrap()
            .unwrap();
        assert_eq!(initial.boundary, parameters.boundary);

        model
            .walls
            .get_mut(&bottom_wall)
            .unwrap()
            .parameters
            .path
            .straight_end_mut()
            .unwrap()
            .x = 10.0;
        model
            .walls
            .get_mut(&right_wall)
            .unwrap()
            .parameters
            .path
            .straight_start_mut()
            .unwrap()
            .x = 10.0;
        model
            .walls
            .get_mut(&right_wall)
            .unwrap()
            .parameters
            .path
            .straight_end_mut()
            .unwrap()
            .x = 10.0;
        model
            .walls
            .get_mut(&top_wall)
            .unwrap()
            .parameters
            .path
            .straight_start_mut()
            .unwrap()
            .x = 10.0;
        let updated = CeilingResolver::default()
            .effective_parameters(&model, &parameters)
            .unwrap()
            .unwrap();
        assert_eq!(updated.boundary[1].x, 10.0);
        assert_eq!(updated.boundary[2].x, 10.0);

        model.walls.remove(&right_wall);
        assert!(
            CeilingResolver::default()
                .effective_parameters(&model, &parameters)
                .unwrap()
                .is_none(),
            "a broken source room must not reuse the cached ceiling outline"
        );
    }

    #[test]
    fn rcp_projection_obeys_range_basis_crop_and_opening_area() {
        let basis = HorizontalBasis {
            origin: Point2::new(1.0, 2.0),
            rotation: 0.3,
        };
        let range = PlanRange {
            top: 4.0,
            cut: 3.0,
            bottom: 1.2,
            depth: 0.0,
        };
        let plan = ceiling_plan(&params(), 0.0, range, basis, None)
            .unwrap()
            .unwrap();
        assert!((plan.area_m2 - 44.0).abs() < 1e-9);
        assert_eq!(plan.holes.len(), 1);
        assert!(
            ceiling_plan(
                &params(),
                0.0,
                PlanRange {
                    top: 2.4,
                    cut: 1.5,
                    bottom: 0.5,
                    depth: -1.0
                },
                HorizontalBasis::default(),
                None,
            )
            .unwrap()
            .is_none()
        );
        let cropped = ceiling_plan(
            &params(),
            0.0,
            range,
            HorizontalBasis::default(),
            Some(PlanCrop {
                min: Point2::new(0.0, 0.0),
                max: Point2::new(3.0, 3.0),
            }),
        )
        .unwrap()
        .unwrap();
        assert_eq!(cropped.boundary.len(), 4);
    }
}
