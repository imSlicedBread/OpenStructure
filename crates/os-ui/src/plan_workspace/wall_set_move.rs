//! Bounded native wall-set translation. A draft owns no live model or drawing data.
use super::*;
use crate::plan_gesture::WallGesture;
use os_core::ensure;

pub(super) struct Draft {
    context: PlanContext,
    session: Id,
    revision: u64,
    drawing: Id,
    providers: Vec<(String, Id)>,
    primary: Option<Id>,
    ids: BTreeSet<Id>,
    selection: BTreeSet<Id>,
    excluded: BTreeSet<Id>,
    original: Model,
    pub gesture: WallGesture,
    pressed: bool,
    copy: Option<super::wall_set_copy::Identities>,
    group: Option<Command>,
    group_screen: Option<GroupScreenContext>,
    array: Option<super::wall_assembly_array::Array>,
    pub(super) radial: bool,
    pub(super) radial_degrees: String,
}

struct GroupScreenContext {
    camera: PlanCamera,
    canvas: egui::Rect,
    filters: selection_filters::SelectionFilters,
}

pub(super) struct Candidate {
    pub model: Model,
    pub(super) commands: Vec<Command>,
}

impl Draft {
    pub fn begin(app: &DesktopApp) -> Result<Self> {
        Self::begin_mode(app, false, true)
    }

    pub fn begin_copy(app: &DesktopApp) -> Result<Self> {
        Self::begin_mode(app, true, true)
    }

    pub(super) fn begin_group_duplicate(app: &DesktopApp, id: Id) -> Result<Self> {
        let view = super::groups::check_context(app)?;
        let model = app.editor.document.model();
        let instance = &model.group_instances[&id];
        let definition = &model.group_definitions[&instance.parameters.definition];
        ensure(
            definition
                .parameters
                .template
                .opening_tags
                .values()
                .all(|tag| Some(tag.parameters.view) == app.plans.active),
            "Group tags belong to another plan; duplicate in their owning plan",
        )?;
        let walls = definition
            .parameters
            .template
            .walls
            .keys()
            .map(|slot| instance.parameters.members[slot])
            .collect();
        let mut draft = Self::begin_with_walls(app, false, true, walls, true)?;
        draft.group_screen = Some(GroupScreenContext {
            camera: *app
                .plans
                .cameras
                .get(&view)
                .ok_or_else(|| Error::Invalid("Wait for the plan camera".into()))?,
            canvas: app
                .plans
                .canvas_rect
                .ok_or_else(|| Error::Invalid("Wait for the plan canvas".into()))?,
            filters: app.plans.selection_filters.clone(),
        });
        draft.group = Some(Command::DuplicateGroupInstancePrepared {
            id,
            translation: Point2::new(0.0, 0.0),
            instance: Id::new(),
            members: instance
                .parameters
                .members
                .keys()
                .map(|slot| (*slot, Id::new()))
                .collect(),
        });
        Ok(draft)
    }

    pub(super) fn copied_group(&self) -> Option<Id> {
        match &self.group {
            Some(Command::DuplicateGroupInstancePrepared { instance, .. }) => Some(*instance),
            _ => None,
        }
    }

    pub(super) fn begin_array(app: &DesktopApp) -> Result<Self> {
        let mut draft = Self::begin_copy(app)?;
        draft.set_array_count(2)?;
        Ok(draft)
    }

    pub(super) fn array_count(&self) -> Option<usize> {
        self.array
            .as_ref()
            .map(super::wall_assembly_array::Array::count)
    }

    pub(super) fn max_array_count(&self) -> usize {
        let dependents = self.copy.as_ref().map_or(0, |copy| copy.dependent_count());
        32.min(1 + 64 / self.ids.len()).min(if dependents == 0 {
            32
        } else {
            1 + 4096 / dependents
        })
    }

    pub(super) fn set_array_count(&mut self, count: usize) -> Result<()> {
        let array = super::wall_assembly_array::Array::new(
            &self.original,
            &self.ids,
            self.context.view_id,
            count,
        )?;
        self.array = Some(array);
        Ok(())
    }

    pub(super) fn begin_level_copy(app: &DesktopApp) -> Result<Self> {
        Self::begin_mode(app, true, false)
    }

    pub(super) fn candidate_on_level(&self, level: Id) -> Result<Candidate> {
        self.copy
            .as_ref()
            .ok_or_else(|| Error::Invalid("No copy identities".into()))?
            .candidate_on_level(&self.original, level)
    }

    pub(super) fn copied_ids(&self) -> BTreeSet<Id> {
        if let Some(Command::DuplicateGroupInstancePrepared { id, members, .. }) = &self.group {
            let definition = self.original.group_instances[id].parameters.definition;
            return self.original.group_definitions[&definition]
                .parameters
                .template
                .walls
                .keys()
                .map(|slot| members[slot])
                .collect();
        }
        if let Some(array) = &self.array {
            return array.walls();
        }
        self.copy
            .as_ref()
            .map(|ids| ids.walls.values().copied().collect())
            .unwrap_or_default()
    }

    pub fn is_copy(&self) -> bool {
        self.copy.is_some() || self.group.is_some()
    }

    pub fn label(&self) -> &'static str {
        if self.group.is_some() {
            "Duplicate group"
        } else if self.radial {
            "Radial array walls"
        } else if self.array.is_some() {
            "Array walls"
        } else if self.is_copy() {
            "Copy walls"
        } else {
            "Move walls"
        }
    }

    pub fn dependency_summary(&self) -> Option<String> {
        self.array
            .as_ref()
            .map(super::wall_assembly_array::Array::summary)
            .or_else(|| self.copy.as_ref().map(|ids| ids.summary()))
    }

    fn begin_mode(app: &DesktopApp, copy: bool, copy_tags: bool) -> Result<Self> {
        Self::begin_with_walls(app, copy, copy_tags, app.selected_ids.clone(), false)
    }

    fn begin_with_walls(
        app: &DesktopApp,
        copy: bool,
        copy_tags: bool,
        ids: BTreeSet<Id>,
        group: bool,
    ) -> Result<Self> {
        ensure(
            !copy || !app.editor.host.worker_supported(os_walls::PLUGIN_ID),
            "Copy wall assemblies requires the bundled native Wall provider",
        )?;
        ensure(
            (if copy || group { 1 } else { 2 }..=64).contains(&ids.len()),
            if copy {
                "Copy walls requires 1–64 selected native straight walls"
            } else {
                "Move walls requires 2–64 selected native straight walls"
            },
        )?;
        ensure(
            app.plans.active_sheet.is_none(),
            "Open an active floor plan",
        )?;
        ensure(
            !app.editor.plugin_work_pending(),
            "Finish or cancel the pending plugin command",
        )?;
        let view = app
            .plans
            .active
            .ok_or_else(|| Error::Invalid("Open a floor plan".into()))?;
        let context = app.editor.native_plan_context(view)?;
        let model = app.editor.document.model();
        let settings = model.views[&view].parameters.plan.unwrap();
        ensure(
            settings.view_type == os_model::PlanViewType::FloorPlan,
            if copy {
                "Copy walls requires a floor plan"
            } else {
                "Move walls requires a floor plan"
            },
        )?;
        let drawing = app
            .plans
            .drawing
            .as_ref()
            .ok_or_else(|| Error::Invalid("Wait for the plan drawing".into()))?;
        let items = drawing.items(context)?;
        let level = model.views[&view].parameters.level.unwrap();
        for id in &ids {
            ensure(
                group || model.group_instance_for_member(*id).is_none(),
                "Grouped members are locked; use Duplicate group or Ungroup",
            )?;
            let wall = model.walls.get(id).ok_or_else(|| Error::Invalid("Every selected entity must be a native straight wall; selection is mixed or stale".into()))?;
            ensure(
                wall.header.type_id == os_walls::WALL_TYPE && wall.parameters.path.is_straight(),
                if copy {
                    "Copy walls supports native STRAIGHT walls only"
                } else {
                    "Move walls supports native STRAIGHT walls only"
                },
            )?;
            ensure(
                wall.parameters.level == level,
                "All selected walls must be on the active plan's level",
            )?;
            ensure(
                context.show_walls && items.iter().any(|i| i.entity == *id),
                "Selected wall is hidden or outside the plan crop",
            )?;
        }
        for join in model.wall_joins.values() {
            let members = join.parameters.members();
            ensure(
                !members.iter().any(|id| ids.contains(id))
                    || members.iter().all(|id| ids.contains(id)),
                "Partial wall join: select every member before moving or copying walls",
            )?;
        }
        // A synthetic origin wall lets the existing Move adapter compute the exact
        // world displacement without subtracting large source coordinates.
        let mut parameters = model.walls[ids.first().unwrap()].parameters.clone();
        *parameters.path.straight_start_mut()? = Point2::new(0.0, 0.0);
        *parameters.path.straight_end_mut()? = Point2::new(1.0, 0.0);
        let gesture = WallGesture::begin_move_vector(&app.editor, view, parameters)?;
        let mut excluded = ids.clone();
        excluded.extend(
            model
                .openings
                .values()
                .filter(|o| ids.contains(&o.parameters.host))
                .map(|o| o.id()),
        );
        Ok(Self {
            group: None,
            group_screen: None,
            array: None,
            radial: false,
            radial_degrees: "30".into(),
            copy: if copy {
                Some(if copy_tags {
                    super::wall_set_copy::Identities::new(model, &ids, view)?
                } else {
                    super::wall_set_copy::Identities::with_tags(model, &ids, view, false)?
                })
            } else {
                None
            },
            context,
            session: app.editor.document.session_id(),
            revision: app.editor.document.revision(),
            drawing: drawing.identity(),
            providers: plan_provider_signature(&app.editor),
            primary: app.selected,
            ids,
            selection: app.selected_ids.clone(),
            excluded,
            original: model.clone(),
            gesture,
            pressed: false,
        })
    }

    pub fn current(&self, app: &DesktopApp) -> bool {
        self.gesture.current(&app.editor, app.plans.active)
            && self.session == app.editor.document.session_id()
            && self.revision == app.editor.document.revision()
            && app.plans.active_sheet.is_none()
            && app.selected == self.primary
            && app.selected_ids == self.selection
            && !app.editor.plugin_work_pending()
            && plan_provider_signature(&app.editor) == self.providers
            && self.group_screen.as_ref().is_none_or(|screen| {
                app.plans.cameras.get(&self.context.view_id) == Some(&screen.camera)
                    && app.plans.canvas_rect == Some(screen.canvas)
                    && app.plans.selection_filters == screen.filters
            })
            && app
                .plans
                .drawing
                .as_ref()
                .is_some_and(|d| d.identity() == self.drawing && d.items(self.context).is_ok())
    }

    pub fn candidate(&self, point: Point2) -> Result<Candidate> {
        if self.radial {
            let pivot = self
                .gesture
                .start
                .ok_or_else(|| Error::Invalid("Choose the radial array pivot".into()))?;
            let degrees = self
                .radial_degrees
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::Invalid("Angular step needs a number".into()))?;
            return self
                .array
                .as_ref()
                .ok_or_else(|| Error::Invalid("No array identities".into()))?
                .radial_candidate(
                    &self.original,
                    self.context.basis.plane_to_world(pivot)?,
                    degrees,
                );
        }
        let delta = self.gesture.parameters(point)?.start();
        if let Some(mut command) = self.group.clone() {
            ensure(
                delta.is_finite() && delta != Point2::new(0.0, 0.0),
                "Duplicate group requires a nonzero displacement",
            )?;
            if let Command::DuplicateGroupInstancePrepared { translation, .. } = &mut command {
                *translation = delta;
            }
            let model = Document::from_model(self.original.clone())?
                .preview_commands(vec![command.clone()])?;
            let copied = self.copied_ids();
            for id in &copied {
                let native = os_geometry::walls::NativeWall::from_model(&model, *id)?;
                native.mesh()?;
                for opening in model.openings.values().filter(|o| o.parameters.host == *id) {
                    crate::opening_tools::panel_mesh(&model, opening.id())?;
                }
            }
            super::wall_set_copy::validate_collisions(&model, &copied)?;
            super::wall_set_copy::validate_rooms(&self.original, &model)?;
            return Ok(Candidate {
                model,
                commands: vec![command],
            });
        }
        if let Some(array) = &self.array {
            return array.candidate(&self.original, delta);
        }
        if let Some(ids) = &self.copy {
            return ids.candidate(&self.original, delta);
        }
        let shift = |p: Point2| Point2::new(p.x + delta.x, p.y + delta.y);
        let mut commands = Vec::new();
        for id in &self.ids {
            let mut parameters = self.original.walls[id].parameters.clone();
            *parameters.path.straight_start_mut()? = shift(parameters.start());
            *parameters.path.straight_end_mut()? = shift(parameters.end());
            commands.push(Command::UpdateWall {
                id: *id,
                parameters,
            });
        }
        for tag in self.original.opening_tags.values().filter(|t| {
            self.original
                .openings
                .get(&t.parameters.opening)
                .is_some_and(|o| self.ids.contains(&o.parameters.host))
        }) {
            let mut parameters = tag.parameters.clone();
            parameters.position = shift(parameters.position);
            commands.push(Command::UpdateOpeningTag {
                id: tag.id(),
                parameters,
            });
        }
        let mut rooms = BTreeSet::new();
        for room in self.original.rooms.values().filter(|r| {
            r.parameters
                .boundary_signature
                .iter()
                .any(|(id, _)| self.ids.contains(id))
        }) {
            ensure(
                room.parameters
                    .boundary_signature
                    .iter()
                    .all(|(id, _)| self.ids.contains(id)),
                "Partial room boundary: select every boundary entity; only complete all-wall loops can move",
            )?;
            rooms.insert(room.id());
            let mut parameters = room.parameters.clone();
            parameters.seed = shift(parameters.seed);
            commands.push(Command::UpdateRoom {
                id: room.id(),
                parameters,
            });
        }
        for tag in self
            .original
            .room_tags
            .values()
            .filter(|t| rooms.contains(&t.parameters.room))
        {
            let mut parameters = tag.parameters.clone();
            parameters.position = shift(parameters.position);
            commands.push(Command::UpdateRoomTag {
                id: tag.id(),
                parameters,
            });
        }
        let mut document = Document::from_model(self.original.clone())?;
        document.execute("Validate move walls", commands.clone())?;
        let model = document.model();

        // Pointer previews are frequent: validate only geometry affected by this
        // translation instead of regenerating every model element and view on
        // each frame. Document::execute still performs whole-model invariants.
        for id in &self.ids {
            let native = os_geometry::walls::NativeWall::from_model(model, *id)?;
            native.mesh()?;
            for opening in model.openings.values().filter(|o| o.parameters.host == *id) {
                crate::opening_tools::panel_mesh(model, opening.id())?;
            }
        }
        for room in model.rooms.values().filter(|r| {
            r.parameters
                .boundary_signature
                .iter()
                .any(|(id, _)| self.ids.contains(id))
        }) {
            let segments = model.room_boundary_segments(room.parameters.level)?;
            let faces = os_geometry::rooms::derive_faces(&segments).map_err(|error| {
                Error::Invalid(format!("Moved room boundary is invalid: {error:?}"))
            })?;
            let face = faces.assign_seed(room.parameters.seed).map_err(|error| {
                Error::Invalid(format!("Moved room seed is invalid: {error:?}"))
            })?;
            ensure(
                face.key.as_signature() == room.parameters.boundary_signature,
                "Moved room boundary no longer resolves to its original loop",
            )?;
        }
        Ok(Candidate {
            model: model.clone(),
            commands,
        })
    }

    pub fn commit(&self, app: &mut DesktopApp, point: Point2) -> Result<()> {
        ensure(
            self.current(app),
            format!(
                "{} canceled: selection, drawing, view, document or provider changed",
                self.label()
            ),
        )?;
        let candidate = self.candidate(point)?;
        if self.is_copy() {
            // Preflight regeneration and drawing budgets in an isolated editor.
            // Publish exactly the validated commands and already generated scene.
            let mut staged = Editor::new()?;
            staged.document = Document::from_model(self.original.clone())?;
            staged.scene = app.editor.scene.clone();
            staged.pending_geometry = app.editor.pending_geometry.clone();
            staged
                .document
                .execute(self.label(), candidate.commands.clone())?;
            staged.regenerate()?;
            staged.native_drawing(self.context.view_id)?;
            ensure(self.current(app), "Copy context changed during preflight")?;
            app.editor
                .document
                .execute(self.label(), candidate.commands)?;
            app.editor.scene = staged.scene;
            app.editor.pending_geometry = staged.pending_geometry;
            return Ok(());
        }
        if candidate.model != self.original {
            app.editor
                .document
                .execute("Move walls", candidate.commands)?;
            app.editor.regenerate()?;
        }
        Ok(())
    }

    pub fn target(
        &self,
        drawing: &PlanDrawing,
        camera: PlanCamera,
        size: [f64; 2],
        pointer: Point2,
        snaps: SnapOptions,
    ) -> Result<Point2> {
        let raw = camera.unproject(pointer, size)?;
        if !snaps.enabled || (!self.radial && self.gesture.has_exact_destination()) {
            return Ok(raw);
        }
        let query = os_render::snapping::SnapQuery {
            camera,
            viewport: size,
            pointer,
            radius_pixels: 12.0,
            endpoints: snaps.endpoints,
            midpoints: snaps.midpoints,
            intersections: snaps.intersections,
            perpendicular_from: self.gesture.start.filter(|_| snaps.perpendicular),
            nearest: snaps.nearest,
            axis_extensions: snaps.axis_extensions,
            exclude_entity: None,
        };
        let empty = BTreeSet::new();
        let excluded = if self.is_copy() {
            &empty
        } else {
            &self.excluded
        };
        Ok(drawing
            .snap_excluding(self.context, query, excluded)?
            .candidate_excluding(self.context, query, excluded)?
            .map_or(raw, |c| c.point))
    }

    fn paint(
        &self,
        candidate: &Candidate,
        painter: &egui::Painter,
        camera: PlanCamera,
        rect: egui::Rect,
    ) -> Result<()> {
        let copied;
        let ids = if self.is_copy() {
            copied = self.copied_ids();
            &copied
        } else {
            &self.ids
        };
        for id in ids {
            let native = os_geometry::walls::NativeWall::from_model(&candidate.model, *id)?;
            let drawing = PlanDrawing::from_segmented_prisms(
                self.context,
                &BTreeMap::from([(*id, native.cells()?)]),
                vec![],
            )?;
            for (a, b) in drawing
                .items(self.context)?
                .iter()
                .flat_map(|i| i.outline())
            {
                transforms::paint_line(painter, camera, rect, self.context, a, b)?;
            }
            for opening in candidate
                .model
                .openings
                .values()
                .filter(|o| o.parameters.host == *id)
            {
                let resolved = candidate.model.resolve_opening(&opening.parameters)?;
                for line in crate::opening_tools::plan_symbol(
                    opening.id(),
                    &resolved,
                    &native.parameters,
                    native.elevation,
                    self.context,
                )? {
                    transforms::paint_line(
                        painter,
                        camera,
                        rect,
                        self.context,
                        line.start,
                        line.end,
                    )?;
                }
            }
        }
        let label = |position, text: String, annotation: bool| -> Result<()> {
            let p = self.context.basis.world_to_plane(position)?;
            let in_crop = if annotation {
                point_in_annotation_crop(self.context, p)
            } else {
                point_in_plan_crop(self.context, p)
            };
            if in_crop {
                let p = camera.project(p, [rect.width() as f64, rect.height() as f64])?;
                painter.text(
                    rect.min + egui::vec2(p.x as f32, p.y as f32),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(12.0),
                    theme::ACCENT,
                );
            }
            Ok(())
        };
        for tag in candidate.model.opening_tags.values().filter(|t| {
            t.parameters.view == self.context.view_id
                && if let Some(array) = &self.array {
                    array.contains_tag(t.id())
                } else if let Some(copy) = &self.copy {
                    copy.tags.values().any(|id| *id == t.id())
                } else if let Some(Command::DuplicateGroupInstancePrepared { members, .. }) =
                    &self.group
                {
                    members.values().any(|id| *id == t.id())
                } else {
                    self.excluded.contains(&t.parameters.opening)
                }
        }) {
            label(
                tag.parameters.position,
                tag.parameters.label(&candidate.model).0,
                true,
            )?;
        }
        for room in candidate.model.rooms.values().filter(|r| {
            !self.is_copy()
                && r.parameters
                    .boundary_signature
                    .iter()
                    .any(|(id, _)| ids.contains(id))
        }) {
            label(
                room.parameters.seed,
                format!("{} {}", room.parameters.number, room.parameters.name),
                false,
            )?;
            for tag in candidate.model.room_tags.values().filter(|t| {
                t.parameters.room == room.id() && t.parameters.view == self.context.view_id
            }) {
                label(
                    tag.parameters.position,
                    format!("{} {}", room.parameters.number, room.parameters.name),
                    false,
                )?;
            }
        }
        Ok(())
    }
}

impl DesktopApp {
    pub(super) fn begin_wall_set_move(&mut self) {
        self.begin_wall_set_translation(false);
    }

    pub(super) fn begin_wall_set_copy(&mut self) {
        self.begin_wall_set_translation(true);
    }

    pub(super) fn begin_wall_assembly_array(&mut self) {
        match Draft::begin_array(self) {
            Ok(draft) => {
                self.cancel_wall_set_interactions();
                self.plans.wall_set_move = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn begin_wall_set_translation(&mut self, copy: bool) {
        match if copy {
            Draft::begin_copy(self)
        } else {
            Draft::begin(self)
        } {
            Ok(draft) => {
                self.cancel_wall_set_interactions();
                self.plans.wall_set_move = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn cancel_wall_set_interactions(&mut self) {
        self.cancel_plan_wall();
        self.cancel_opening_placement();
        self.cancel_aligned_dimension();
        self.plans.area_selection.cancel();
        self.plans.overlap_selection.cancel();
        self.plans.crop.mode = None;
        self.plans.floor_sketch = None;
        self.plans.floor_hole_sketch = None;
        self.plans.floor_vertex_drag = None;
        self.plans.section_placement = None;
        self.plans.room_placement_active = false;
        self.plans.detail_line_draft = None;
        self.plans.room_separation_line_draft = None;
        self.plans.room_tag_draft = None;
        self.plans.opening_tag_draft = None;
        self.plans.opening_move = None;
        self.plans.opening_flip = None;
        self.plans.opening_rehost = None;
        self.plans.opening_align = None;
        self.plans.opening_spacing = None;
        self.plans.opening_type_assignment = None;
        self.plans.opening_batch_edit = None;
        self.plans.column_edit = None;
        self.plans.stair_edit = None;
        self.plans.ceiling_draft = None;
        self.opening_draft = None;
        self.grid_draft = None;
        self.plan_draft = None;
    }

    pub(super) fn validate_wall_set_move(&mut self, ctx: &egui::Context) {
        if self.plans.wall_set_move.is_some() {
            self.plans.wall_set_move_claimed = true;
        }
        let stale = self
            .plans
            .wall_set_move
            .as_ref()
            .is_some_and(|d| !d.current(self));
        let cancel = self.plans.wall_set_move.as_ref().is_some_and(|d| {
            ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::PointerGone))
                    || (d.pressed && !i.pointer.primary_down() && !i.pointer.primary_released())
            })
        });
        if stale || cancel {
            let label = self
                .plans
                .wall_set_move
                .as_ref()
                .map_or("Wall operation", Draft::label);
            self.plans.wall_set_move = None;
            self.report(
                Ok(()),
                &format!(
                    "{label} canceled{}.",
                    if stale { ": its context changed" } else { "" }
                ),
            );
        }
    }

    pub(super) fn wall_set_move_frame(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        painter: &egui::Painter,
        camera: PlanCamera,
    ) {
        let Some(mut draft) = self.plans.wall_set_move.take() else {
            return;
        };
        let rect = response.rect;
        let point = ctx
            .input(|i| i.pointer.interact_pos())
            .filter(|p| response.contains_pointer() && rect.contains(*p))
            .map(|p| {
                draft
                    .target(
                        self.plans.drawing.as_ref().unwrap(),
                        camera,
                        [rect.width() as f64, rect.height() as f64],
                        Point2::new((p.x - rect.left()) as f64, (p.y - rect.top()) as f64),
                        self.plans.snaps,
                    )
                    .and_then(|p| {
                        ensure(
                            point_in_plan_crop(draft.context, p),
                            "Choose a point inside the active plan crop",
                        )?;
                        Ok(p)
                    })
            });
        if point.is_some() && ctx.input(|i| i.pointer.primary_pressed()) {
            draft.pressed = true;
        }
        let mut error = point
            .as_ref()
            .and_then(|p| p.as_ref().err())
            .map(ToString::to_string);
        if let Some(Ok(p)) = &point
            && draft.gesture.start.is_some()
        {
            error = draft
                .candidate(*p)
                .and_then(|c| draft.paint(&c, painter, camera, rect))
                .err()
                .map(|e| e.to_string());
        }
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            error.as_deref().unwrap_or(if draft.group.is_some() {
                if draft.gesture.start.is_none() {
                    "Duplicate group · choose base point"
                } else {
                    "Duplicate group · choose destination · Escape cancels"
                }
            } else if draft.radial {
                if draft.gesture.start.is_none() {
                    "Radial array walls · choose a snapped pivot"
                } else {
                    "Radial array walls · enter signed angular step; click to apply"
                }
            } else if draft.array.is_some() {
                if draft.gesture.start.is_none() {
                    "Array walls · choose base point"
                } else {
                    "Array walls · set per-instance spacing (or enter distance and angle)"
                }
            } else if draft.is_copy() {
                if draft.gesture.start.is_none() {
                    "Copy walls · choose base point"
                } else {
                    "Copy walls · place copy (or enter distance and angle)"
                }
            } else {
                draft.gesture.prompt()
            }),
            egui::FontId::proportional(12.0),
            if error.is_some() {
                theme::ERROR
            } else {
                theme::ACCENT
            },
        );
        if ctx.input(|i| i.pointer.primary_released()) && draft.pressed {
            draft.pressed = false;
            match point {
                Some(Ok(p)) if draft.gesture.start.is_none() => draft.gesture.start = Some(p),
                Some(Ok(p)) => {
                    let copied_selection = draft.is_copy().then(|| draft.copied_ids());
                    match draft.commit(self, p) {
                        Ok(()) => {
                            if let Some(group) = draft.copied_group() {
                                self.select(Some(group));
                            } else if let Some(ids) = copied_selection {
                                self.select(ids.first().copied());
                                self.selected_ids = ids;
                            }
                            self.report(Ok(()), &format!("{} applied.", draft.label()));
                            ctx.request_repaint();
                            return;
                        }
                        Err(error) => {
                            self.plans.wall_set_move = None;
                            self.report(
                                Err(error),
                                "Wall operation canceled; original geometry kept.",
                            );
                            return;
                        }
                    }
                }
                _ => {
                    self.report(
                        Ok(()),
                        if draft.is_copy() {
                            "Copy walls canceled outside the plan."
                        } else {
                            "Move walls canceled outside the plan."
                        },
                    );
                    return;
                }
            }
        }
        self.plans.wall_set_move = Some(draft);
    }
}
