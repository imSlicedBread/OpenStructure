//! Single-wall edits. All preview geometry belongs to a disposable candidate.
use super::*;
use crate::plan_gesture::WallGesture;
use os_core::ensure;
use os_model::{DoorSwing, Model, WindowPanePosition};
mod split;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Rotate,
    Mirror,
    Align,
    Split,
}

pub(super) struct Draft {
    context: PlanContext,
    id: Id,
    drawing: Id,
    providers: Vec<(String, Id)>,
    adapter: WallGesture,
    original: Model,
    align_targets: Vec<os_render::plan::PlanItem>,
    pub mode: Mode,
    pub degrees: String,
    pub axis: Option<Point2>,
    pressed: bool,
    split_id: Id,
}

pub(super) struct Candidate {
    pub model: Model,
    pub commands: Vec<Command>,
}

impl Draft {
    pub fn begin(app: &DesktopApp, mode: Mode) -> Result<Self> {
        ensure(
            app.selected_ids.len() == 1,
            "Select exactly one native wall",
        )?;
        let id = app
            .selected
            .ok_or_else(|| Error::Invalid("Select a wall".into()))?;
        let view = app
            .plans
            .active
            .ok_or_else(|| Error::Invalid("Open a floor plan".into()))?;
        let context = app.editor.native_plan_context(view)?;
        let drawing = app
            .plans
            .drawing
            .as_ref()
            .ok_or_else(|| Error::Invalid("Wait for the plan drawing".into()))?;
        let model = app.editor.document.model();
        let wall = model
            .walls
            .get(&id)
            .ok_or_else(|| Error::Invalid("Select a native wall".into()))?;
        ensure(
            wall.header.type_id == os_walls::WALL_TYPE,
            "Select a native straight wall",
        )?;
        ensure(
            context.show_walls && drawing.items(context)?.iter().any(|i| i.entity == id),
            "Selected wall is hidden or outside the crop",
        )?;
        ensure(
            !model
                .wall_joins
                .values()
                .any(|j| j.parameters.members().contains(&id)),
            "Wall transforms are unavailable for joined walls",
        )?;
        // Split preserves world geometry and explicitly remaps references. Rigid
        // transforms still require an annotation relocation policy.
        ensure(
            mode == Mode::Split
                || (!model.dimensions.values().any(|d| {
                    d.parameters.references().any(|r| match r {
                        os_model::DimensionReference::WallEndpoint { wall, .. } => wall == id,
                        os_model::DimensionReference::OpeningJamb { opening, .. } => model
                            .openings
                            .get(&opening)
                            .is_some_and(|o| o.parameters.host == id),
                    })
                }) && !model.opening_tags.values().any(|t| {
                    model
                        .openings
                        .get(&t.parameters.opening)
                        .is_some_and(|o| o.parameters.host == id)
                })),
            "Wall transforms are unavailable for walls with attached dimensions or opening tags",
        )?;
        if mode == Mode::Mirror {
            for opening in model.openings.values().filter(|o| o.parameters.host == id) {
                let resolved = model.resolve_opening(&opening.parameters)?;
                ensure(
                    resolved.kind != OpeningKind::Window
                        || resolved.pane_position == WindowPanePosition::Center,
                    "Mirror requires centered window panes",
                )?;
            }
        }
        #[cfg(feature = "external-plugins")]
        let installed = app.editor.host.worker_supported(os_walls::PLUGIN_ID);
        #[cfg(not(feature = "external-plugins"))]
        let installed = false;
        ensure(
            !app.editor.plugin_work_pending(),
            "Finish or cancel the pending plugin command",
        )?;
        Self::provider_compatible(model, id, mode, installed)?;
        #[cfg(feature = "external-plugins")]
        let adapter = if installed {
            WallGesture::begin_edit_installed(&app.editor, view, id, WallEdit::Move)?
        } else {
            WallGesture::begin_edit(&app.editor, view, id, WallEdit::Move)?
        };
        #[cfg(not(feature = "external-plugins"))]
        let adapter = WallGesture::begin_edit(&app.editor, view, id, WallEdit::Move)?;
        Ok(Self {
            context,
            id,
            drawing: drawing.identity(),
            providers: plan_provider_signature(&app.editor),
            adapter,
            original: model.clone(),
            // Snapshot only visible footprints; current() invalidates this snapshot
            // whenever the drawing, document, view, or provider changes.
            align_targets: if matches!(mode, Mode::Align | Mode::Split) {
                drawing.items(context)?.to_vec()
            } else {
                Vec::new()
            },
            mode,
            degrees: String::new(),
            axis: None,
            pressed: false,
            split_id: Id::new(),
        })
    }

    fn provider_compatible(model: &Model, id: Id, mode: Mode, installed: bool) -> Result<()> {
        ensure(
            !(installed && mode == Mode::Split),
            "Split requires the native Wall provider for an atomic transaction",
        )?;
        ensure(
            !installed
                || mode != Mode::Mirror
                || (!model.wall_type_assignments.contains_key(&id)
                    && !model
                        .openings
                        .values()
                        .filter(|o| o.parameters.host == id)
                        .any(|o| {
                            model
                                .resolve_opening(&o.parameters)
                                .is_ok_and(|r| r.kind == OpeningKind::Door)
                        })),
            "This mirror requires an atomic native batch; the installed Wall provider cannot change door swing or wall layer sides",
        )
    }

    pub fn current(&self, app: &DesktopApp) -> bool {
        self.adapter.current(&app.editor, app.plans.active)
            && app.plans.active_sheet.is_none()
            && app.selected == Some(self.id)
            && app.selected_ids.len() == 1
            && app.selected_ids.contains(&self.id)
            && app.editor.document.session_id() == self.context.session_id
            && app.editor.document.revision() == self.context.model_revision
            && plan_provider_signature(&app.editor) == self.providers
            && !app.editor.plugin_work_pending()
            && app
                .plans
                .drawing
                .as_ref()
                .is_some_and(|d| d.identity() == self.drawing && d.items(self.context).is_ok())
    }

    pub fn candidate(&self, point: Point2) -> Result<Candidate> {
        ensure(point.is_finite(), "Invalid transform point")?;
        if self.mode == Mode::Split {
            return self.split_candidate(point);
        }
        let original = &self.original.walls[&self.id].parameters;
        let mut wall = original.clone();
        let world = self.context.basis.plane_to_world(point)?;
        match self.mode {
            Mode::Split => unreachable!("split uses a compound candidate"),
            Mode::Align => {
                ensure(
                    self.context.show_walls && point_in_plan_crop(self.context, point),
                    "Choose a reference wall inside the plan crop",
                )?;
                let reference = self
                    .align_targets
                    .iter()
                    .rev()
                    .filter(|item| item.entity != self.id && item.footprint.contains(point))
                    .find_map(|item| {
                        self.original.walls.get(&item.entity).filter(|target| {
                            target.header.type_id == os_walls::WALL_TYPE
                                && target.parameters.level == original.level
                        })
                    })
                    .ok_or_else(|| {
                        Error::Invalid(
                            "Choose a visible straight reference wall on the same level".into(),
                        )
                    })?;
                let shift = alignment_shift(
                    original.start,
                    original.end,
                    reference.parameters.start,
                    reference.parameters.end,
                )?;
                wall.start = Point2::new(original.start.x + shift.x, original.start.y + shift.y);
                wall.end = Point2::new(original.end.x + shift.x, original.end.y + shift.y);
                ensure(wall != *original, "Wall is already aligned")?;
            }
            Mode::Rotate => {
                let center = Point2::new(
                    original.start.x + (original.end.x - original.start.x) * 0.5,
                    original.start.y + (original.end.y - original.start.y) * 0.5,
                );
                let angle = if self.degrees.trim().is_empty() {
                    ensure(
                        world.distance(center) > 1e-6,
                        "Choose an angle away from the midpoint",
                    )?;
                    (world.y - center.y).atan2(world.x - center.x)
                        - (original.end.y - original.start.y)
                            .atan2(original.end.x - original.start.x)
                } else {
                    let degrees = self
                        .degrees
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| Error::Invalid("Angle needs a number".into()))?;
                    ensure(degrees.is_finite(), "Angle must be finite")?;
                    (degrees % 360.0).to_radians()
                };
                let (s, c) = angle.sin_cos();
                let rotate = |p: Point2| {
                    Point2::new(
                        center.x + c * (p.x - center.x) - s * (p.y - center.y),
                        center.y + s * (p.x - center.x) + c * (p.y - center.y),
                    )
                };
                wall.start = rotate(original.start);
                wall.end = rotate(original.end);
            }
            Mode::Mirror => {
                let a =
                    self.context.basis.plane_to_world(self.axis.ok_or_else(|| {
                        Error::Invalid("Click the first mirror axis point".into())
                    })?)?;
                let length = a.distance(world);
                ensure(
                    length.is_finite() && length > 1e-6,
                    "Mirror axis must have two distinct points",
                )?;
                let (x, y) = ((world.x - a.x) / length, (world.y - a.y) / length);
                let reflect = |p: Point2| {
                    let (dx, dy) = (p.x - a.x, p.y - a.y);
                    let projection = dx * x + dy * y;
                    Point2::new(
                        a.x + 2.0 * projection * x - dx,
                        a.y + 2.0 * projection * y - dy,
                    )
                };
                wall.start = reflect(original.start);
                wall.end = reflect(original.end);
            }
        }
        wall.validate()?;
        let mut commands = vec![Command::UpdateWall {
            id: self.id,
            parameters: wall,
        }];
        if self.mode == Mode::Mirror {
            if let Some(mut assignment) = self.original.wall_type_assignments.get(&self.id).copied()
            {
                assignment.flipped = !assignment.flipped;
                commands.push(Command::AssignWallType {
                    wall: self.id,
                    assignment: Some(assignment),
                });
            }
            for opening in self
                .original
                .openings
                .values()
                .filter(|o| o.parameters.host == self.id)
            {
                if self.original.resolve_opening(&opening.parameters)?.kind == OpeningKind::Door {
                    let mut parameters = opening.parameters.clone();
                    parameters.swing = match parameters.swing {
                        DoorSwing::Left => DoorSwing::Right,
                        DoorSwing::Right => DoorSwing::Left,
                    };
                    commands.push(Command::UpdateOpening {
                        id: opening.id(),
                        parameters,
                    });
                }
            }
        }
        // Use precisely the document transaction validator on an isolated model.
        let mut candidate = Document::from_model(self.original.clone())?;
        candidate.execute("Validate wall transform", commands.clone())?;
        Ok(Candidate {
            model: candidate.model().clone(),
            commands,
        })
    }

    fn label(&self) -> &'static str {
        match self.mode {
            Mode::Rotate => "Rotate wall",
            Mode::Mirror => "Mirror wall",
            Mode::Align => "Align wall",
            Mode::Split => "Split wall",
        }
    }

    pub fn commit(&self, app: &mut DesktopApp, point: Point2) -> Result<()> {
        ensure(self.current(app), "Wall transform is stale")?;
        let candidate = self.candidate(point)?;
        if candidate.model == self.original {
            return Ok(());
        }
        let parameters = candidate.model.walls[&self.id].parameters.clone();
        #[cfg(feature = "external-plugins")]
        if self.adapter.is_installed() {
            Self::provider_compatible(&self.original, self.id, self.mode, true)?;
            let (draft, context, view) =
                self.adapter
                    .installed_parameters(&app.editor, app.plans.active, parameters)?;
            return app.submit_installed_wall(draft, context, view);
        }
        if candidate.commands.len() == 1 {
            app.editor.wall_command(
                self.label(),
                os_plugin_api::Request::EditWall {
                    id: self.id,
                    parameters,
                },
            )
        } else {
            app.editor
                .document
                .execute(self.label(), candidate.commands)?;
            app.editor.regenerate()
        }
    }

    fn paint(
        &self,
        candidate: &Candidate,
        painter: &egui::Painter,
        camera: PlanCamera,
        rect: egui::Rect,
    ) -> Result<()> {
        self.paint_wall(candidate, painter, camera, rect, self.id)?;
        if self.mode == Mode::Split {
            self.paint_wall(candidate, painter, camera, rect, self.split_id)?;
        }
        Ok(())
    }

    fn paint_wall(
        &self,
        candidate: &Candidate,
        painter: &egui::Painter,
        camera: PlanCamera,
        rect: egui::Rect,
        id: Id,
    ) -> Result<()> {
        let native = os_geometry::walls::NativeWall::from_model(&candidate.model, id)?;
        let drawing = PlanDrawing::from_segmented_prisms(
            self.context,
            &BTreeMap::from([(id, native.cells()?)]),
            vec![],
        )?;
        let mut lines: Vec<_> = drawing
            .items(self.context)?
            .iter()
            .flat_map(|i| i.outline())
            .collect();
        for opening in candidate
            .model
            .openings
            .values()
            .filter(|o| o.parameters.host == id)
        {
            let resolved = candidate.model.resolve_opening(&opening.parameters)?;
            lines.extend(
                crate::opening_tools::plan_symbol(
                    opening.id(),
                    &resolved,
                    &native.parameters,
                    native.elevation,
                    self.context,
                )?
                .into_iter()
                .map(|l| (l.start, l.end)),
            );
        }
        for (a, b) in lines {
            paint_line(painter, camera, rect, self.context, a, b)?;
        }
        Ok(())
    }
}

#[test]
fn wall_split_installed_provider_requires_native_atomic_batch() {
    let model = Model::new("Provider guard");
    assert!(Draft::provider_compatible(&model, Id::new(), Mode::Split, true).is_err());
    assert!(Draft::provider_compatible(&model, Id::new(), Mode::Split, false).is_ok());
}

/// Treat axes within 1e-8 radians as parallel (including antiparallel).
/// Keep the source direction and axial position; ignore the reference length.
fn alignment_shift(a: Point2, b: Point2, c: Point2, d: Point2) -> Result<Point2> {
    ensure(
        [a, b, c, d].iter().all(|p| p.is_finite()),
        "Invalid wall centerline",
    )?;
    let source_length = a.distance(b);
    let target_length = c.distance(d);
    ensure(
        source_length.is_finite()
            && target_length.is_finite()
            && source_length > 1e-6
            && target_length > 1e-6,
        "Invalid wall centerline",
    )?;
    let (ux, uy) = ((b.x - a.x) / source_length, (b.y - a.y) / source_length);
    let (vx, vy) = ((d.x - c.x) / target_length, (d.y - c.y) / target_length);
    ensure(
        (ux * vy - uy * vx).abs() <= 1e-8,
        "Align requires parallel walls",
    )?;
    // Subtract nearby coordinates before forming the midpoint difference.
    let dx = (c.x - a.x) - (b.x - a.x) * 0.5;
    let dy = (c.y - a.y) - (b.y - a.y) * 0.5;
    let distance = (-vy * dx + vx * dy) / (ux * vx + uy * vy);
    ensure(
        distance.is_finite() && distance.abs() > 1e-6,
        "Wall is already aligned",
    )?;
    Ok(Point2::new(-uy * distance, ux * distance))
}

#[test]
fn wall_align_geometry_parallel_tolerance_and_perpendicular_translation() {
    let a = Point2::new(10.0, 20.0);
    let b = Point2::new(14.0, 23.0);
    let c = Point2::new(7.0, 24.0);
    let d = Point2::new(15.0, 30.0);
    for (c, d) in [(c, d), (d, c)] {
        let shift = alignment_shift(a, b, c, d).unwrap();
        assert!(shift.distance(Point2::new(-3.0, 4.0)) < 1e-12);
        assert!((shift.x * 4.0 + shift.y * 3.0).abs() < 1e-12);
    }
    let a = Point2::new(0.0, 0.0);
    let b = Point2::new(1.0, 0.0);
    for (c, d) in [
        (a, b),
        (Point2::new(0.0, 0.0000005), Point2::new(1.0, 0.0000005)),
        (Point2::new(0.0, 1.0), Point2::new(1.0, 1.00000002)),
        (Point2::new(0.0, 1.0), Point2::new(0.0, 1.0)),
        (Point2::new(f64::NAN, 1.0), b),
        (Point2::new(f64::INFINITY, 1.0), b),
    ] {
        assert!(alignment_shift(a, b, c, d).is_err());
    }
    assert!(alignment_shift(a, b, Point2::new(0.0, 1.0), Point2::new(1.0, 1.000000005)).is_ok());
}

fn paint_line(
    painter: &egui::Painter,
    camera: PlanCamera,
    rect: egui::Rect,
    context: PlanContext,
    a: Point2,
    b: Point2,
) -> Result<()> {
    if let Some((a, b)) = clip_plan_segment(a, b, context.crop) {
        let project = |p| {
            camera
                .project(p, [f64::from(rect.width()), f64::from(rect.height())])
                .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
        };
        painter.line_segment(
            [project(a)?, project(b)?],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
    }
    Ok(())
}

impl DesktopApp {
    pub(super) fn begin_wall_transform(&mut self, mode: Mode) {
        match Draft::begin(self, mode) {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_opening_placement();
                self.cancel_aligned_dimension();
                self.plans.area_selection.cancel();
                self.plans.overlap_selection.cancel();
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
                self.plans.column_edit = None;
                self.plans.stair_edit = None;
                self.plans.ceiling_draft = None;
                self.plans.transform = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn validate_wall_transform(&mut self, ctx: &egui::Context) {
        if self.plans.transform.is_some() {
            self.plans.transform_claimed = true;
        }
        let cancel = self.plans.transform.as_ref().is_some_and(|d| {
            !d.current(self)
                || ctx.input(|i| {
                    i.key_pressed(egui::Key::Escape)
                        || !i.focused
                        || i.events
                            .iter()
                            .any(|e| matches!(e, egui::Event::PointerGone))
                        || (d.pressed && !i.pointer.primary_down() && !i.pointer.primary_released())
                })
        });
        if cancel {
            self.plans.transform = None;
        }
    }

    pub(super) fn wall_transform_frame(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        painter: &egui::Painter,
        camera: PlanCamera,
    ) {
        let Some(mut draft) = self.plans.transform.take() else {
            return;
        };
        let rect = response.rect;
        let pointer = ctx
            .input(|i| i.pointer.interact_pos())
            .filter(|p| response.contains_pointer() && rect.contains(*p));
        let point = pointer.and_then(|p| {
            camera
                .unproject(
                    Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())),
                    [f64::from(rect.width()), f64::from(rect.height())],
                )
                .ok()
        });
        if point.is_some() && ctx.input(|i| i.pointer.primary_pressed()) {
            draft.pressed = true;
        }
        let candidate = point.map(|p| draft.candidate(p));
        let message = match &candidate {
            Some(Ok(candidate)) => draft
                .paint(candidate, painter, camera, rect)
                .err()
                .map(|e| e.to_string()),
            Some(Err(error)) if draft.mode != Mode::Mirror || draft.axis.is_some() => {
                Some(error.to_string())
            }
            _ => None,
        };
        if let (Some(a), Some(b)) = (draft.axis, point) {
            let _ = paint_line(painter, camera, rect, draft.context, a, b);
        }
        let prompt = match draft.mode {
            Mode::Split => "Split: click an interior station on the selected wall. Escape cancels.",
            Mode::Align => "Align centerlines: click a parallel reference wall. Escape cancels.",
            Mode::Rotate => {
                "Rotate about midpoint: point toward the new end and release. Exact degrees: Snaps → Wall transforms."
            }
            Mode::Mirror if draft.axis.is_none() => {
                "Mirror: click the first axis point. Escape cancels."
            }
            Mode::Mirror => "Mirror: click the second axis point. Escape cancels.",
        };
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            message.as_deref().unwrap_or(prompt),
            egui::FontId::proportional(12.0),
            if message.is_some() {
                theme::ERROR
            } else {
                theme::ACCENT
            },
        );
        if draft.pressed && ctx.input(|i| i.pointer.primary_released()) {
            draft.pressed = false;
            if let Some(point) = point {
                if draft.mode == Mode::Mirror && draft.axis.is_none() {
                    draft.axis = Some(point);
                } else {
                    let result = draft.commit(self, point);
                    self.report(result, "Wall transform applied.");
                    ctx.request_repaint();
                    return;
                }
            } else {
                return;
            }
        }
        self.plans.transform = Some(draft);
    }
}
