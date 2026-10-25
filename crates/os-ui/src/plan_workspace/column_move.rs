//! Explicit single-column translation; all candidate state is disposable.
use super::*;
use os_core::ensure;
use os_model::ColumnParams;

pub(super) struct Draft {
    context: PlanContext,
    session: Id,
    revision: u64,
    drawing: Id,
    providers: Vec<(String, Id)>,
    id: Id,
    selection: BTreeSet<Id>,
    original: Model,
    base: Option<Point2>,
    pressed: bool,
}

impl Draft {
    pub fn available(app: &DesktopApp) -> bool {
        Self::source(app).is_ok()
    }

    fn source(app: &DesktopApp) -> Result<(Id, PlanContext, Id)> {
        ensure(app.plans.active_sheet.is_none(), "Open a floor plan")?;
        ensure(
            !app.editor.plugin_work_pending(),
            "Wait for the pending command",
        )?;
        let view = app
            .plans
            .active
            .ok_or_else(|| Error::Invalid("Open a floor plan".into()))?;
        let context = app.editor.native_plan_context(view)?;
        let model = app.editor.document.model();
        ensure(
            model.views[&view]
                .parameters
                .plan
                .is_some_and(|p| p.view_type == os_model::PlanViewType::FloorPlan),
            "Move column requires a FloorPlan",
        )?;
        let id = app
            .selected
            .ok_or_else(|| Error::Invalid("Select one native column".into()))?;
        ensure(
            app.selected_ids.len() == 1 && app.selected_ids.contains(&id),
            "Select exactly one native column",
        )?;
        let column = model
            .columns
            .get(&id)
            .ok_or_else(|| Error::Invalid("Select one native column".into()))?;
        ensure(
            column.header.type_id == "core.column",
            "Select a native rectangular column",
        )?;
        let drawing = app
            .plans
            .drawing
            .as_ref()
            .ok_or_else(|| Error::Invalid("Wait for the plan drawing".into()))?;
        ensure(
            drawing
                .columns(context)?
                .iter()
                .any(|item| item.entity == id),
            "Selected column is not visible in this plan",
        )?;
        Ok((id, context, drawing.identity()))
    }

    fn begin(app: &DesktopApp) -> Result<Self> {
        let (id, context, drawing) = Self::source(app)?;
        Ok(Self {
            context,
            drawing,
            id,
            session: app.editor.document.session_id(),
            revision: app.editor.document.revision(),
            providers: plan_provider_signature(&app.editor),
            selection: app.selected_ids.clone(),
            original: app.editor.document.model().clone(),
            base: None,
            pressed: false,
        })
    }

    fn current(&self, app: &DesktopApp) -> bool {
        app.editor.document.session_id() == self.session
            && app.editor.document.revision() == self.revision
            && app.selected_ids == self.selection
            && plan_provider_signature(&app.editor) == self.providers
            && Self::source(app).ok() == Some((self.id, self.context, self.drawing))
    }

    fn target(
        &self,
        drawing: &PlanDrawing,
        camera: PlanCamera,
        size: [f64; 2],
        pointer: Point2,
        snaps: SnapOptions,
    ) -> Result<Point2> {
        let raw = camera.unproject(pointer, size)?;
        let point = if snaps.enabled {
            let query = os_render::snapping::SnapQuery {
                camera,
                viewport: size,
                pointer,
                radius_pixels: 12.0,
                endpoints: snaps.endpoints,
                midpoints: snaps.midpoints,
                intersections: snaps.intersections,
                perpendicular_from: self.base.filter(|_| snaps.perpendicular),
                nearest: snaps.nearest,
                axis_extensions: snaps.axis_extensions,
                exclude_entity: Some(self.id),
            };
            let excluded = BTreeSet::from([self.id]);
            drawing
                .snap_excluding(self.context, query, &excluded)?
                .candidate_excluding(self.context, query, &excluded)?
                .map_or(raw, |candidate| candidate.point)
        } else {
            raw
        };
        ensure(
            point.is_finite() && point_in_plan_crop(self.context, point),
            "Choose a finite point inside the plan crop",
        )?;
        Ok(point)
    }

    fn candidate(
        &self,
        destination: Point2,
    ) -> Result<(ColumnParams, os_geometry::plan::PlanFootprint)> {
        let base = self
            .base
            .ok_or_else(|| Error::Invalid("Choose a base point".into()))?;
        ensure(
            destination.is_finite() && point_in_plan_crop(self.context, destination),
            "Destination is outside the plan crop or invalid",
        )?;
        // Rotate the displacement, not two large absolute world positions.
        let dx = destination.x - base.x;
        let dy = destination.y - base.y;
        let (s, c) = self.context.basis.rotation.sin_cos();
        let mut parameters = self.original.columns[&self.id].parameters.clone();
        parameters.center.x += c * dx - s * dy;
        parameters.center.y += s * dx + c * dy;
        parameters.validate_in(&self.original)?;
        let center = self.context.basis.world_to_plane(parameters.center)?;
        ensure(
            point_in_plan_crop(self.context, center),
            "Moved column center is outside the plan crop",
        )?;
        let elevation = self.original.levels[&parameters.level].parameters.elevation;
        os_geometry::columns::column_mesh(&parameters, elevation)?;
        let solid = os_geometry::columns::column_solid(&parameters, elevation)?;
        let footprint = os_geometry::plan::rectangular_plan(
            &solid,
            self.context.range,
            self.context.basis,
            self.context.crop,
        )?
        .ok_or_else(|| Error::Invalid("Moved column is outside the visible plan".into()))?;
        Ok((parameters, footprint))
    }

    fn commit(&self, app: &mut DesktopApp, destination: Point2) -> Result<()> {
        ensure(self.current(app), "Move column canceled: context changed")?;
        let (parameters, _) = self.candidate(destination)?;
        if parameters == self.original.columns[&self.id].parameters {
            return Ok(());
        }
        let command = Command::UpdateColumn {
            id: self.id,
            parameters,
        };
        // Validate document, drawing budgets and scene regeneration before publishing.
        let mut staged = Editor::new()?;
        staged.document = Document::from_model(self.original.clone())?;
        staged.scene = app.editor.scene.clone();
        staged.pending_geometry = app.editor.pending_geometry.clone();
        staged
            .document
            .execute("Move column", vec![command.clone()])?;
        staged.regenerate()?;
        staged.native_drawing(self.context.view_id)?;
        ensure(
            self.current(app),
            "Move column context changed during preflight",
        )?;
        app.editor.document.execute("Move column", vec![command])?;
        app.editor.scene = staged.scene;
        app.editor.pending_geometry = staged.pending_geometry;
        Ok(())
    }
}

impl DesktopApp {
    pub(super) fn begin_column_move(&mut self) {
        match Draft::begin(self) {
            Ok(draft) => {
                self.cancel_wall_set_interactions();
                self.plans.column_move = Some(draft);
                self.plans.column_move_claimed = true;
                self.report(
                    Ok(()),
                    "Move column · choose base point, then destination · Escape cancels",
                );
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn validate_column_move(&mut self, ctx: &egui::Context) {
        if self.plans.column_move.is_some() {
            self.plans.column_move_claimed = true;
        }
        let cancel = self.plans.column_move.as_ref().is_some_and(|draft| {
            !draft.current(self)
                || ctx.input(|i| {
                    i.key_pressed(egui::Key::Escape)
                        || i.events
                            .iter()
                            .any(|e| matches!(e, egui::Event::PointerGone))
                        || (draft.pressed
                            && !i.pointer.primary_down()
                            && !i.pointer.primary_released())
                })
        });
        if cancel {
            self.plans.column_move = None;
            self.report(Ok(()), "Move column canceled: input or context changed.");
        }
    }

    pub(super) fn column_move_frame(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        painter: &egui::Painter,
        camera: PlanCamera,
    ) {
        let Some(mut draft) = self.plans.column_move.take() else {
            return;
        };
        let rect = response.rect;
        let point = ctx
            .input(|i| i.pointer.interact_pos())
            .filter(|p| response.contains_pointer() && rect.contains(*p))
            .map(|p| {
                self.plans
                    .drawing
                    .as_ref()
                    .ok_or_else(|| Error::Invalid("Plan drawing is unavailable".into()))
                    .and_then(|drawing| {
                        draft.target(
                            drawing,
                            camera,
                            [f64::from(rect.width()), f64::from(rect.height())],
                            Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())),
                            self.plans.snaps,
                        )
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
            && draft.base.is_some()
        {
            match draft.candidate(*p) {
                Ok((_, footprint)) => columns::paint(painter, &footprint, camera, rect),
                Err(e) => error = Some(e.to_string()),
            }
        }
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            error.as_deref().unwrap_or(if draft.base.is_none() {
                "Move column · choose base point"
            } else {
                "Move column · choose destination · Escape cancels"
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
            if let Some(Ok(p)) = point {
                if draft.base.is_none() {
                    draft.base = Some(p);
                } else if error.is_none() {
                    let result = draft.commit(self, p);
                    self.report(result, "Move column applied.");
                    ctx.request_repaint();
                    return;
                }
            }
        }
        self.plans.column_move = Some(draft);
    }
}
