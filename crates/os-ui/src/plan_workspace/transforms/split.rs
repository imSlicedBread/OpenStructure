//! Atomic native wall subdivision, including references that model validation
//! deliberately permits to remain unresolved during ordinary editing.
use super::*;
use os_geometry::rooms::{FaceKey, ROOM_TOLERANCE, RoomFace, derive_faces};
use os_model::{DimensionEndpoint, DimensionLayout, DimensionParams};

impl Draft {
    pub(super) fn split_candidate(&self, point: Point2) -> Result<Candidate> {
        ensure(
            self.context.show_walls
                && point_in_plan_crop(self.context, point)
                && self
                    .align_targets
                    .iter()
                    .any(|item| item.entity == self.id && item.footprint.contains(point)),
            "Pick inside the selected visible wall",
        )?;
        let source = &self.original.walls[&self.id];
        let original = &source.parameters;
        let world = self.context.basis.plane_to_world(point)?;
        let length = original.length();
        let ux = (original.end.x - original.start.x) / length;
        let uy = (original.end.y - original.start.y) / length;
        // Project only onto this wall's axis. Generic snaps must not displace it.
        let station = (world.x - original.start.x) * ux + (world.y - original.start.y) * uy;
        ensure(
            station.is_finite() && station > 0.001 && length - station > 0.001,
            "Split must leave more than 1 mm at both wall ends",
        )?;
        let cut = Point2::new(
            original.start.x + ux * station,
            original.start.y + uy * station,
        );
        ensure(
            point_in_plan_crop(self.context, self.context.basis.world_to_plane(cut)?),
            "Split station is outside the plan crop",
        )?;
        let mut start = original.clone();
        start.end = cut;
        let mut end = source.clone();
        end.header.id = self.split_id;
        end.parameters.start = cut;
        start.validate()?;
        end.parameters.validate()?;
        let lifecycle = self
            .original
            .element_lifecycle(self.id)
            .ok_or_else(|| Error::Invalid("wall phase is missing".into()))?;
        let mut commands = vec![
            Command::UpdateWall {
                id: self.id,
                parameters: start,
            },
            Command::AddWall(end),
            Command::SetElementLifecycle {
                element: self.split_id,
                lifecycle,
            },
        ];
        if let Some(assignment) = self.original.wall_type_assignments.get(&self.id) {
            commands.push(Command::AssignWallType {
                wall: self.split_id,
                assignment: Some(*assignment),
            });
        }
        for opening in self
            .original
            .openings
            .values()
            .filter(|o| o.parameters.host == self.id)
        {
            let resolved = self.original.resolve_opening(&opening.parameters)?;
            if resolved.offset + resolved.width <= station - 0.001 {
                continue;
            }
            ensure(
                resolved.offset >= station + 0.001,
                "Split crosses an opening or its 1 mm end clearance",
            )?;
            let mut parameters = opening.parameters.clone();
            parameters.host = self.split_id;
            parameters.offset -= station;
            commands.push(Command::UpdateOpening {
                id: opening.id(),
                parameters,
            });
        }
        for dimension in self.original.dimensions.values().filter(|d| {
            d.parameters
                .references()
                .any(|r| r.wall_endpoint().is_some_and(|(wall, _)| wall == self.id))
        }) {
            dimension.parameters.validate_creation(&self.original)?;
            let mut parameters = dimension.parameters.clone();
            for reference in [&mut parameters.first, &mut parameters.second]
                .into_iter()
                .chain(parameters.additional.iter_mut())
            {
                if let os_model::DimensionReference::WallEndpoint {
                    wall,
                    endpoint: DimensionEndpoint::End,
                } = reference
                    && *wall == self.id
                {
                    *wall = self.split_id;
                }
            }
            commands.push(Command::UpdateDimension {
                id: dimension.id(),
                parameters,
            });
        }
        let mut candidate = Document::from_model(self.original.clone())?;
        candidate.execute("Validate split dependencies", commands.clone())?;
        for dimension in self.original.dimensions.values().filter(|d| {
            d.parameters
                .references()
                .any(|r| r.wall_endpoint().is_some_and(|(wall, _)| wall == self.id))
        }) {
            verify_dimension(
                &dimension.parameters,
                &candidate.model().dimensions[&dimension.id()].parameters,
                &self.original,
                candidate.model(),
            )?;
        }
        // FaceKey directions refer to sorted quantized endpoints, not wall direction.
        let quantized = |p: Point2| {
            (
                (p.x / ROOM_TOLERANCE).round() as i64,
                (p.y / ROOM_TOLERANCE).round() as i64,
            )
        };
        let forward = quantized(original.start) < quantized(original.end);
        let affected: Vec<_> = self
            .original
            .rooms
            .values()
            .filter(|room| {
                room.parameters
                    .boundary_signature
                    .iter()
                    .any(|(id, _)| *id == self.id)
            })
            .collect();
        let mut faces = BTreeMap::new();
        for room in affected {
            let p = &room.parameters;
            ensure(
                p.level == original.level,
                "Split room boundary is on another level",
            )?;
            if let std::collections::btree_map::Entry::Vacant(entry) = faces.entry(p.level) {
                let before = derive_faces(&self.original.room_boundary_segments(p.level)?)
                    .map_err(|e| {
                        Error::Invalid(format!("Cannot resolve room before split: {e:?}"))
                    })?;
                let after = derive_faces(&candidate.model().room_boundary_segments(p.level)?)
                    .map_err(|e| {
                        Error::Invalid(format!("Cannot resolve room after split: {e:?}"))
                    })?;
                ensure(
                    before.faces.len() == after.faces.len(),
                    "Split changed room topology",
                )?;
                entry.insert((before, after));
            }
            let (before, after) = &faces[&p.level];
            let old_key = FaceKey::from_signature(&p.boundary_signature)
                .ok_or_else(|| Error::Invalid("Invalid room boundary signature".into()))?;
            let old_face = before
                .resolve_seed(p.seed, &old_key)
                .map_err(|e| Error::Invalid(format!("Cannot resolve room before split: {e:?}")))?;
            ensure(
                p.boundary_signature
                    .iter()
                    .filter(|(id, _)| *id == self.id)
                    .count()
                    == 1,
                "Split requires one directed source edge in each room",
            )?;
            let mut sides = Vec::new();
            for &(id, direction) in &p.boundary_signature {
                if id != self.id {
                    sides.push((id, direction));
                } else if direction == forward {
                    sides.extend([(self.id, direction), (self.split_id, direction)]);
                } else {
                    sides.extend([(self.split_id, direction), (self.id, direction)]);
                }
            }
            let key = FaceKey::from_signature(&sides)
                .ok_or_else(|| Error::Invalid("Unsupported split room signature".into()))?;
            let new_face = after
                .resolve_seed(p.seed, &key)
                .map_err(|e| Error::Invalid(format!("Split changed room enclosure: {e:?}")))?;
            ensure(
                same_face(old_face, new_face),
                "Split changed room geometry or area",
            )?;
            let mut parameters = p.clone();
            parameters.boundary_signature = key.as_signature().to_vec();
            commands.push(Command::UpdateRoom {
                id: room.id(),
                parameters,
            });
        }
        // Validate the complete, final batch using exactly the commit path.
        let mut candidate = Document::from_model(self.original.clone())?;
        candidate.execute("Validate wall split", commands.clone())?;
        Ok(Candidate {
            model: candidate.model().clone(),
            commands,
        })
    }
}

fn verify_dimension(
    before: &DimensionParams,
    after: &DimensionParams,
    old: &Model,
    new: &Model,
) -> Result<()> {
    before.validate_creation(old)?;
    after.validate_creation(new)?;
    // Check physical endpoints even for angular layouts, whose resolver uses axes.
    for (a, b) in before.references().zip(after.references()) {
        let endpoint = |model: &Model, r: os_model::DimensionReference| -> Result<Point2> {
            let level = model.views[&before.view]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Unresolved split dimension level".into()))?;
            r.resolve(model, level).map_err(|reason| {
                Error::Invalid(format!("Unresolved split dimension anchor: {reason:?}"))
            })
        };
        ensure(
            endpoint(old, a)?.distance(endpoint(new, b)?) <= 1e-9,
            "Split moved a dimension anchor",
        )?;
    }
    if before.layout == DimensionLayout::Angular {
        let a = before
            .resolve_angular(old)
            .map_err(|e| Error::Invalid(format!("Invalid angle: {e:?}")))?;
        let b = after
            .resolve_angular(new)
            .map_err(|e| Error::Invalid(format!("Invalid angle: {e:?}")))?;
        ensure(
            a.center.distance(b.center) <= 1e-6
                && (a.start_radians - b.start_radians).abs() <= 1e-9
                && (a.sweep_radians - b.sweep_radians).abs() <= 1e-9
                && a.radius == b.radius,
            "Split changed angular dimension layout",
        )?;
    }
    Ok(())
}

fn same_face(a: &RoomFace, b: &RoomFace) -> bool {
    let perimeter: f64 = a
        .boundary
        .iter()
        .zip(a.boundary.iter().cycle().skip(1))
        .map(|(a, b)| a.distance(*b))
        .sum();
    if (a.area_m2 - b.area_m2).abs() > ROOM_TOLERANCE * perimeter.max(1.0) {
        return false;
    }
    // The expected directed key checks topology. Bidirectional boundary distance
    // also checks geometry, allowing the inserted collinear station and lattice rounding.
    let on_boundary = |points: &[Point2], ring: &[Point2]| {
        points.iter().all(|p| {
            ring.iter().zip(ring.iter().cycle().skip(1)).any(|(a, b)| {
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let t =
                    (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                p.distance(Point2::new(a.x + t * dx, a.y + t * dy)) <= ROOM_TOLERANCE
            })
        })
    };
    on_boundary(&a.boundary, &b.boundary) && on_boundary(&b.boundary, &a.boundary)
}
