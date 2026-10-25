//! Explicit nominal-aperture movement in station/project-elevation coordinates.
use super::*;
use os_core::ensure;
use os_model::{ElevationDimensionFeature as F, OpeningDefinition};

pub(super) struct Draft {
    guard: OpeningDraft,
    id: Id,
    original: OpeningParams,
    context: PlanContext,
    drawing: Id,
    drag: Option<(PlanCamera, egui::Rect, Point2)>,
}

impl Draft {
    fn current(&self, app: &DesktopApp) -> bool {
        self.guard
            .current(&app.editor, app.plans.active, app.selected)
            && app.selected_ids.len() == 1
            && app.selected_ids.contains(&self.id)
            && !app.editor.plugin_work_pending()
            && app.plans.active_sheet.is_none()
            && app
                .plans
                .drawing
                .as_ref()
                .is_some_and(|d| d.identity() == self.drawing)
    }

    fn candidate(
        &self,
        app: &DesktopApp,
        pointer: egui::Pos2,
    ) -> Result<(OpeningParams, PlanDrawing)> {
        ensure(self.current(app), "Elevation move is stale")?;
        let (camera, canvas, grab) = self
            .drag
            .ok_or_else(|| Error::Invalid("Move not started".into()))?;
        ensure(
            canvas.contains(pointer),
            "Pointer left the elevation canvas",
        )?;
        let world = camera.unproject(
            Point2::new(
                (pointer.x - canvas.left()) as f64,
                (pointer.y - canvas.top()) as f64,
            ),
            [canvas.width() as f64, canvas.height() as f64],
        )?;
        ensure(
            point_in_plan_crop(self.context, world),
            "Move outside elevation crop",
        )?;
        let model = app.editor.document.model();
        let r = model.resolve_opening(&self.original)?;
        let wall = model.resolve_wall(r.host)?.parameters;
        let base = model.levels[&wall.level].parameters.elevation;
        let mut delta = Point2::new(
            world.x - grab.x,
            if r.kind == OpeningKind::Door {
                0.
            } else {
                world.y - grab.y
            },
        );
        // A round trip through f32 screen coordinates must preserve serialized pins.
        if delta.x.abs() * camera.pixels_per_metre <= 0.001 {
            delta.x = 0.;
        }
        if delta.y.abs() * camera.pixels_per_metre <= 0.001 {
            delta.y = 0.;
        }
        let snaps = app.plans.snaps;
        if snaps.enabled
            && (snaps.endpoints || snaps.midpoints || snaps.nearest || snaps.axis_extensions)
        {
            let drawing = app.plans.drawing.as_ref().unwrap();
            for horizontal in [true, false] {
                if (!horizontal && r.kind == OpeningKind::Door)
                    || (if horizontal { delta.x } else { delta.y }) == 0.
                {
                    continue;
                }
                let raw = if horizontal { delta.x } else { delta.y };
                let mut best = 12.;
                let mut correction = 0.;
                for (entity, opening) in std::iter::once((r.host, false)).chain(
                    model
                        .openings
                        .values()
                        .filter(|o| o.id() != self.id && o.parameters.host == r.host)
                        .filter(|o| {
                            model
                                .resolve_opening(&o.parameters)
                                .is_ok_and(|v| v.family.rectangular_cut())
                        })
                        .map(|o| (o.id(), true)),
                ) {
                    for feature in if horizontal {
                        [F::Start, F::End]
                    } else {
                        [F::Bottom, F::Top]
                    } {
                        if let Some((a, b)) = resize::visible_edge(
                            model,
                            drawing,
                            self.context,
                            entity,
                            r.host,
                            opening,
                            feature,
                        ) {
                            let p = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                            if resize::project(camera, canvas, self.context, p).is_none() {
                                continue;
                            }
                            for edge in if horizontal {
                                [r.offset, r.offset + r.width]
                            } else {
                                [base + r.sill, base + r.sill + r.height]
                            } {
                                let shift = (if horizontal { p.x } else { p.y }) - (edge + raw);
                                let distance = shift.abs() * camera.pixels_per_metre;
                                if distance <= best {
                                    best = distance;
                                    correction = shift;
                                }
                            }
                        }
                    }
                }
                if horizontal {
                    delta.x += correction;
                } else {
                    delta.y += correction;
                }
            }
        }
        let mut parameters = self.original.clone();
        if delta.x.abs() * camera.pixels_per_metre > 0.001 {
            parameters.offset += delta.x;
        }
        if delta.y.abs() * camera.pixels_per_metre > 0.001 {
            let sill = r.sill + delta.y;
            ensure(sill >= 0., "Opening sill must be nonnegative")?;
            match &mut parameters.definition {
                OpeningDefinition::Typed { .. } => parameters.sill_override = Some(sill),
                OpeningDefinition::Legacy { sill: value, .. } => *value = sill,
            }
        }
        let mut candidate = model.clone();
        candidate.openings.get_mut(&self.id).unwrap().parameters = parameters.clone();
        candidate.validate()?;
        crate::opening_tools::host_mesh(&candidate, r.host)?;
        for opening in candidate
            .openings
            .values()
            .filter(|o| o.parameters.host == r.host)
        {
            crate::opening_tools::panel_mesh(&candidate, opening.id())?;
        }
        Ok((
            parameters,
            crate::plan::elevations::preview(&candidate, self.context)?,
        ))
    }
}

impl DesktopApp {
    pub(super) fn elevation_move_available(&self) -> bool {
        let Some((view, settings)) = self.active_elevation() else {
            return false;
        };
        let Some(id) = self
            .selected
            .filter(|id| self.selected_ids.len() == 1 && self.selected_ids.contains(id))
        else {
            return false;
        };
        let model = self.editor.document.model();
        let Some(o) = model.openings.get(&id) else {
            return false;
        };
        let (Some(drawing), Ok(context)) =
            (&self.plans.drawing, self.editor.native_view_context(view))
        else {
            return false;
        };
        o.parameters.host == settings.host
            && !self.editor.plugin_work_pending()
            && model
                .resolve_wall(settings.host)
                .is_ok_and(|w| w.parameters.path.is_straight())
            && model
                .resolve_opening(&o.parameters)
                .is_ok_and(|r| r.family.rectangular_cut())
            && [F::Start, F::End, F::Top, F::Bottom].into_iter().any(|f| {
                resize::visible_edge(model, drawing, context, id, settings.host, true, f).is_some()
            })
    }

    pub(super) fn begin_elevation_move(&mut self) {
        if !self.elevation_move_available() {
            return;
        }
        let id = self.selected.unwrap();
        let view = self.plans.active.unwrap();
        if let (Ok(guard), Ok(context)) = (
            OpeningDraft::begin(&self.editor, Some(view), Some(id), None, None),
            self.editor.native_view_context(view),
        ) {
            self.plans.elevation.cancel();
            self.opening_draft = None;
            self.plans.elevation.moving = Some(Draft {
                guard,
                id,
                original: self.editor.document.model().openings[&id]
                    .parameters
                    .clone(),
                context,
                drawing: self.plans.drawing.as_ref().unwrap().identity(),
                drag: None,
            });
            self.status = "Drag the selected opening body. Escape cancels.".into();
        }
    }

    pub(super) fn validate_elevation_move(&mut self, ctx: &egui::Context) {
        let holding = ctx.input(|i| i.pointer.primary_down() || i.pointer.primary_released());
        self.plans.elevation.move_claimed &= holding;
        if self
            .plans
            .elevation
            .moving
            .as_ref()
            .is_some_and(|d| !d.current(self) || (d.drag.is_some() && !holding))
            || self.plans.elevation.placement.is_some()
            || self.plans.elevation.dimension.is_some()
            || self.plans.elevation.crop.is_some()
            || self.opening_draft.is_some()
            || self.wall_gesture.is_some()
            || self.plans.facade.is_some()
            || ctx.input(|i| {
                !i.focused
                    || i.key_pressed(egui::Key::Escape)
                    || i.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::PointerGone | egui::Event::WindowFocused(false)
                        )
                    })
            })
        {
            self.plans.elevation.moving = None;
        }
    }

    pub(super) fn elevation_move_input(
        &mut self,
        ctx: &egui::Context,
        camera: PlanCamera,
        canvas: egui::Rect,
        context: PlanContext,
    ) -> Option<PlanDrawing> {
        let mut draft = self.plans.elevation.moving.take()?;
        if self.plans.elevation.resize_claimed
            || !draft.current(self)
            || draft.context != context
            || draft
                .drag
                .is_some_and(|(c, r, _)| c != camera || r != canvas)
        {
            return None;
        }
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        if draft.drag.is_none()
            && ctx.input(|i| i.pointer.primary_pressed())
            && let Some(p) = pointer.filter(|p| canvas.contains(*p))
        {
            let world = camera
                .unproject(
                    Point2::new((p.x - canvas.left()) as f64, (p.y - canvas.top()) as f64),
                    [canvas.width() as f64, canvas.height() as f64],
                )
                .ok()?;
            let model = self.editor.document.model();
            let r = model.resolve_opening(&draft.original).ok()?;
            let wall = model.resolve_wall(r.host).ok()?.parameters;
            let bottom = model.levels[&wall.level].parameters.elevation + r.sill;
            if self.elevation_move_available()
                && point_in_plan_crop(context, world)
                && world.x >= r.offset
                && world.x <= r.offset + r.width
                && world.y >= bottom
                && world.y <= bottom + r.height
            {
                draft.drag = Some((camera, canvas, world));
                self.plans.elevation.move_claimed = true;
            }
        }
        let mut preview = None;
        if draft.drag.is_some() {
            match pointer
                .ok_or_else(|| Error::Invalid("Pointer unavailable".into()))
                .and_then(|p| draft.candidate(self, p))
            {
                Ok((parameters, drawing)) => {
                    preview = Some(drawing);
                    if ctx.input(|i| i.pointer.primary_released()) && parameters != draft.original {
                        let result = self.editor.command(
                            "Move opening elevation",
                            Command::UpdateOpening {
                                id: draft.id,
                                parameters,
                            },
                        );
                        self.report(result, "Opening moved.");
                    }
                }
                Err(e) => {
                    self.status = e.to_string();
                    self.status_error = true;
                }
            }
            if ctx.input(|i| i.pointer.primary_released()) {
                return preview;
            }
        }
        self.plans.elevation.moving = Some(draft);
        preview
    }
}
