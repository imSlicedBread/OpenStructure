//! Identity-preserving rigid transforms of complete native wall assemblies.
use super::*;
use os_geometry::rooms::{RoomFace, derive_faces};
use os_model::{WallEndpoint, WallJoinParams};

fn boundary_ids(face: &RoomFace) -> BTreeSet<Id> {
    face.key.as_signature().iter().map(|(id, _)| *id).collect()
}

fn bounded(point: Point2) -> Result<Point2> {
    ensure(
        point.is_finite() && point.x.abs() <= 1e6 && point.y.abs() <= 1e6,
        "Transform coordinates must be finite and within +/- 1000000 metres",
    )?;
    Ok(point)
}

impl Draft {
    pub(super) fn begin_assembly(app: &DesktopApp, mode: Mode) -> Result<Self> {
        Self::provider_compatible(
            app.editor.document.model(),
            app.selected.unwrap_or_default(),
            mode,
            app.editor.host.worker_supported(os_walls::PLUGIN_ID),
            app.selected_ids.len(),
        )?;
        // Reuse the complete selection, visibility, join and context contract.
        let guard = super::super::wall_set_move::Draft::begin(app)?;
        let id = app
            .selected
            .filter(|id| app.selected_ids.contains(id))
            .ok_or_else(|| Error::Invalid("Select a primary wall in the assembly".into()))?;
        let view = app.plans.active.unwrap();
        let context = app.editor.native_plan_context(view)?;
        Ok(Self {
            context,
            id,
            drawing: app.plans.drawing.as_ref().unwrap().identity(),
            providers: plan_provider_signature(&app.editor),
            adapter: WallGesture::begin_edit(&app.editor, view, id, WallEdit::Move)?,
            original: app.editor.document.model().clone(),
            align_targets: Vec::new(),
            mode,
            degrees: String::new(),
            axis: None,
            pressed: false,
            split_id: Id::new(),
            assembly: Some(guard),
            ids: app.selected_ids.clone(),
        })
    }

    pub(super) fn assembly_candidate(&self, point: Point2) -> Result<Candidate> {
        let origin =
            bounded(self.context.basis.plane_to_world(self.axis.ok_or_else(|| {
                Error::Invalid(
                    if self.mode == Mode::Rotate {
                        "Click the rotation pivot"
                    } else {
                        "Click the first mirror axis point"
                    }
                    .into(),
                )
            })?)?)?;
        let target = bounded(self.context.basis.plane_to_world(point)?)?;
        let (axis_x, axis_y) = if self.mode == Mode::Rotate {
            let angle = if self.degrees.trim().is_empty() {
                ensure(
                    origin.distance(target) > 1e-6,
                    "Choose a direction away from the pivot",
                )?;
                let wall = &self.original.walls[&self.id].parameters;
                (target.y - origin.y).atan2(target.x - origin.x)
                    - (wall.end().y - wall.start().y).atan2(wall.end().x - wall.start().x)
            } else {
                let degrees = self
                    .degrees
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| Error::Invalid("Angle needs a number".into()))?;
                ensure(degrees.is_finite(), "Angle must be finite")?;
                (degrees % 360.).to_radians()
            };
            let (sin, cos) = angle.sin_cos();
            (cos, sin)
        } else {
            let length = origin.distance(target);
            ensure(length > 1e-6, "Mirror axis must have two distinct points")?;
            (
                (target.x - origin.x) / length,
                (target.y - origin.y) / length,
            )
        };
        let transform = |p: Point2| -> Result<Point2> {
            let (x, y) = (p.x - origin.x, p.y - origin.y);
            bounded(if self.mode == Mode::Rotate {
                Point2::new(
                    origin.x + axis_x * x - axis_y * y,
                    origin.y + axis_y * x + axis_x * y,
                )
            } else {
                let projection = x * axis_x + y * axis_y;
                Point2::new(
                    origin.x + 2. * projection * axis_x - x,
                    origin.y + 2. * projection * axis_y - y,
                )
            })
        };
        let mut transformed_walls = BTreeMap::new();
        for &id in &self.ids {
            let mut parameters = self.original.walls[&id].parameters.clone();
            *parameters.path.straight_start_mut()? = transform(parameters.start())?;
            *parameters.path.straight_end_mut()? = transform(parameters.end())?;
            transformed_walls.insert(id, parameters);
        }
        // Tee validity intentionally requires exact point equality. Evaluating
        // the branch endpoint and host station independently after rotation can
        // differ by a final floating-point bit, despite a rigid transform.
        // Canonicalize the branch to the transformed host's exact station point.
        for join in self.original.wall_joins.values() {
            if let WallJoinParams::Tee {
                host,
                station,
                branch,
            } = &join.parameters
                && self.ids.contains(host)
                && self.ids.contains(&branch.wall)
            {
                let node = transformed_walls[host].path.point(*station);
                let wall = transformed_walls.get_mut(&branch.wall).unwrap();
                match branch.endpoint {
                    WallEndpoint::Start => *wall.path.straight_start_mut()? = node,
                    WallEndpoint::End => *wall.path.straight_end_mut()? = node,
                }
            }
        }
        let mut commands = Vec::new();
        for (id, parameters) in transformed_walls {
            commands.push(Command::UpdateWall { id, parameters });
            if self.mode == Mode::Mirror {
                mirror_dependents(&self.original, id, &mut commands)?;
            }
        }
        let affected = |id| {
            self.ids.contains(&id)
                || self
                    .original
                    .openings
                    .get(&id)
                    .is_some_and(|o| self.ids.contains(&o.parameters.host))
        };
        for tag in self
            .original
            .opening_tags
            .values()
            .filter(|t| affected(t.parameters.opening))
        {
            let mut parameters = tag.parameters.clone();
            parameters.position = transform(parameters.position)?;
            commands.push(Command::UpdateOpeningTag {
                id: tag.id(),
                parameters,
            });
        }
        let document = Document::from_model(self.original.clone())?;
        let geometry = document.preview_commands(commands.clone())?;
        let level = self.original.walls[&self.id].parameters.level;
        let faces = |model: &Model| {
            derive_faces(&model.room_boundary_segments(level)?).map_err(|e| {
                Error::Invalid(format!("Cannot resolve assembly room boundaries: {e:?}"))
            })
        };
        let before = faces(&self.original)?;
        let after = faces(&geometry)?;
        // Include unassigned closed faces: no partial wall/separation-line loop
        // may be silently broken, and no new loop may swallow another room.
        for face in &before.faces {
            let ids = boundary_ids(face);
            ensure(
                ids.is_disjoint(&self.ids) || ids.is_subset(&self.ids),
                "Partial room boundary: select every boundary wall; separation-line loops cannot transform",
            )?;
        }
        let topology = |faces: &[RoomFace]| {
            let mut keys: Vec<_> = faces.iter().map(boundary_ids).collect();
            keys.sort();
            keys
        };
        ensure(
            topology(&before.faces) == topology(&after.faces),
            "Transform changes room boundary loops",
        )?;
        let mut moved_rooms = BTreeSet::new();
        for room in self
            .original
            .rooms
            .values()
            .filter(|r| r.parameters.level == level)
        {
            let old = before
                .assign_seed(room.parameters.seed)
                .map_err(|e| Error::Invalid(format!("Source room is unresolved: {e:?}")))?;
            ensure(
                old.key.as_signature() == room.parameters.boundary_signature,
                "Source room does not resolve to its accepted boundary",
            )?;
            let moving = !boundary_ids(old).is_disjoint(&self.ids);
            let mut parameters = room.parameters.clone();
            if moving {
                parameters.seed = transform(parameters.seed)?;
            }
            let new = after
                .assign_seed(parameters.seed)
                .map_err(|e| Error::Invalid(format!("Transformed room is unresolved: {e:?}")))?;
            ensure(
                boundary_ids(old) == boundary_ids(new),
                "Transform changes the room's UUID boundary set",
            )?;
            if moving {
                parameters.boundary_signature = new.key.as_signature().to_vec();
                commands.push(Command::UpdateRoom {
                    id: room.id(),
                    parameters,
                });
                moved_rooms.insert(room.id());
            } else {
                ensure(old == new, "Transform changes an unselected room")?;
            }
        }
        for tag in self
            .original
            .room_tags
            .values()
            .filter(|t| moved_rooms.contains(&t.parameters.room))
        {
            let mut parameters = tag.parameters.clone();
            parameters.position = transform(parameters.position)?;
            commands.push(Command::UpdateRoomTag {
                id: tag.id(),
                parameters,
            });
        }
        ensure(
            commands.len() <= 4096,
            "Assembly transform exceeds 4096 changes",
        )?;
        let model = document.preview_commands(commands.clone())?;
        for dimension in model
            .dimensions
            .values()
            .filter(|d| d.parameters.references().any(|r| affected(r.entity())))
        {
            // validate_creation dispatches to Plan/Section/Elevation and checks
            // entire layouts, not merely whether referenced UUIDs still exist.
            dimension.parameters.validate_creation(&self.original)?;
            dimension.parameters.validate_creation(&model)?;
        }
        for &id in &self.ids {
            os_geometry::walls::NativeWall::from_model(&model, id)?.mesh()?;
        }
        for opening in model
            .openings
            .values()
            .filter(|o| self.ids.contains(&o.parameters.host))
        {
            crate::opening_tools::panel_mesh(&model, opening.id())?;
        }
        Ok(Candidate { model, commands })
    }

    pub(super) fn commit_assembly(&self, app: &mut DesktopApp, candidate: Candidate) -> Result<()> {
        let mut staged = Editor::new()?;
        staged.document = Document::from_model(self.original.clone())?;
        staged.scene = app.editor.scene.clone();
        staged.pending_geometry = app.editor.pending_geometry.clone();
        let label = if self.mode == Mode::Rotate {
            "Rotate walls"
        } else {
            "Mirror walls"
        };
        staged.document.execute(label, candidate.commands.clone())?;
        staged.regenerate()?;
        let mut views = BTreeSet::from([self.context.view_id]);
        views.extend(
            candidate
                .model
                .dimensions
                .values()
                .filter(|d| {
                    d.parameters.references().any(|r| {
                        self.ids.contains(&r.entity())
                            || candidate
                                .model
                                .openings
                                .get(&r.entity())
                                .is_some_and(|o| self.ids.contains(&o.parameters.host))
                    })
                })
                .map(|d| d.parameters.view),
        );
        views.extend(
            candidate
                .model
                .opening_tags
                .values()
                .filter(|t| {
                    candidate
                        .model
                        .openings
                        .get(&t.parameters.opening)
                        .is_some_and(|o| self.ids.contains(&o.parameters.host))
                })
                .map(|t| t.parameters.view),
        );
        views.extend(
            candidate
                .model
                .room_tags
                .values()
                .filter(|t| {
                    candidate
                        .model
                        .rooms
                        .get(&t.parameters.room)
                        .is_some_and(|r| {
                            r.parameters
                                .boundary_signature
                                .iter()
                                .any(|(id, _)| self.ids.contains(id))
                        })
                })
                .map(|t| t.parameters.view),
        );
        for view in views {
            staged.native_drawing(view)?;
        }
        ensure(
            self.current(app),
            "Assembly transform became stale during preflight",
        )?;
        app.editor.document.execute(label, candidate.commands)?;
        app.editor.scene = staged.scene;
        app.editor.pending_geometry = staged.pending_geometry;
        Ok(())
    }

    pub(in crate::plan_workspace) fn assembly_preview(&self, point: Point2) -> Result<PlanDrawing> {
        ensure(
            self.assembly.is_some(),
            "Single walls use the existing preview",
        )?;
        let candidate = self.candidate(point)?;
        let mut staged = Editor::new()?;
        staged.document = Document::from_model(candidate.model)?;
        let mut snapshot = staged.plan_snapshot(self.context.view_id)?;
        snapshot.context = self.context;
        snapshot.derive()
    }

    pub(in crate::plan_workspace) fn target(
        &self,
        drawing: &PlanDrawing,
        camera: PlanCamera,
        size: [f64; 2],
        pointer: Point2,
        snaps: SnapOptions,
    ) -> Result<Point2> {
        let raw = camera.unproject(pointer, size)?;
        if self.assembly.is_none()
            || !snaps.enabled
            || (self.mode == Mode::Rotate && self.axis.is_some() && !self.degrees.trim().is_empty())
        {
            return Ok(raw);
        }
        let mut excluded = self.ids.clone();
        excluded.extend(
            self.original
                .openings
                .values()
                .filter(|o| self.ids.contains(&o.parameters.host))
                .map(|o| o.id()),
        );
        let query = os_render::snapping::SnapQuery {
            camera,
            viewport: size,
            pointer,
            radius_pixels: 12.,
            endpoints: snaps.endpoints,
            midpoints: snaps.midpoints,
            intersections: snaps.intersections,
            perpendicular_from: self.axis.filter(|_| snaps.perpendicular),
            nearest: snaps.nearest,
            axis_extensions: snaps.axis_extensions,
            exclude_entity: None,
        };
        Ok(drawing
            .snap_excluding(self.context, query, &excluded)?
            .candidate_excluding(self.context, query, &excluded)?
            .map_or(raw, |c| c.point))
    }
}

/// Reflection preserves endpoint identity and axial stations, and changes only
/// instance handedness. Shared definitions, pins and lite-side inheritance stay.
pub(super) fn mirror_dependents(model: &Model, id: Id, commands: &mut Vec<Command>) -> Result<()> {
    if let Some(mut assignment) = model.wall_type_assignments.get(&id).copied() {
        assignment.flipped = !assignment.flipped;
        commands.push(Command::AssignWallType {
            wall: id,
            assignment: Some(assignment),
        });
    }
    for opening in model.openings.values().filter(|o| o.parameters.host == id) {
        let resolved = model.resolve_opening(&opening.parameters)?;
        let mut parameters = opening.parameters.clone();
        if resolved.kind == OpeningKind::Door {
            parameters.swing = match parameters.swing {
                DoorSwing::Left => DoorSwing::Right,
                DoorSwing::Right => DoorSwing::Left,
            };
        } else if resolved.type_id.is_some() {
            match resolved.pane_position {
                WindowPanePosition::LeftFace => {
                    parameters.pane_position_override = Some(WindowPanePosition::RightFace)
                }
                WindowPanePosition::RightFace => {
                    parameters.pane_position_override = Some(WindowPanePosition::LeftFace)
                }
                WindowPanePosition::Center => continue,
            }
        } else {
            continue;
        }
        commands.push(Command::UpdateOpening {
            id: opening.id(),
            parameters,
        });
    }
    Ok(())
}
