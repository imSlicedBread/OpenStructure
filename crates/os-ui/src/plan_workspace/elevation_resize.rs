//! Opening extent edits in the linked elevation's station/elevation frame.
use super::*;
use os_core::ensure;
use os_model::{DimensionReference, ElevationDimensionFeature as F, OpeningDefinition};

fn jamb(feature: F) -> bool {
    matches!(feature, F::Start | F::End)
}

fn along(feature: F, p: Point2) -> f64 {
    if jamb(feature) { p.x } else { p.y }
}

pub(super) struct Draft {
    guard: OpeningDraft,
    id: Id,
    original: OpeningParams,
    context: PlanContext,
    drawing: Id,
    camera: PlanCamera,
    canvas: egui::Rect,
    feature: F,
    anchor: Point2,
    grab: f64,
}

#[derive(Default)]
pub(super) struct Frame {
    pub drawing: Option<PlanDrawing>,
    grips: Vec<(egui::Pos2, bool)>,
    label: Option<String>,
}

impl Frame {
    pub fn paint(&self, painter: &egui::Painter, rect: egui::Rect) {
        for (p, active) in &self.grips {
            painter.circle_filled(*p, 6., theme::ACCENT);
            painter.circle_stroke(
                *p,
                if *active { 9. } else { 6. },
                egui::Stroke::new(1., theme::TEXT),
            );
        }
        if let Some(label) = &self.label {
            painter.text(
                rect.left_top() + egui::vec2(12., 12.),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(13.),
                theme::ACCENT,
            );
        }
    }
}

pub(super) fn project(
    camera: PlanCamera,
    canvas: egui::Rect,
    context: PlanContext,
    p: Point2,
) -> Option<egui::Pos2> {
    if !point_in_plan_crop(context, p) {
        return None;
    }
    let p = camera
        .project(p, [canvas.width() as f64, canvas.height() as f64])
        .ok()?;
    let p = canvas.min + egui::vec2(p.x as f32, p.y as f32);
    (p.is_finite() && canvas.contains(p)).then_some(p)
}

/// Only actual visible pieces of nominal edges qualify. Posed component edges
/// and custom profiles are never used as nominal extent references.
pub(super) fn visible_edge(
    model: &os_model::Model,
    drawing: &PlanDrawing,
    context: PlanContext,
    entity: Id,
    host: Id,
    opening: bool,
    feature: F,
) -> Option<(Point2, Point2)> {
    let reference = |feature| {
        if opening {
            DimensionReference::ElevationOpening {
                opening: entity,
                feature,
            }
        } else {
            DimensionReference::ElevationWall {
                wall: entity,
                feature,
            }
        }
    };
    let anchor = reference(feature).resolve_elevation(model, host).ok()?;
    let vertical = jamb(feature);
    let axis_tolerance = if opening && vertical {
        let resolved = model
            .resolve_opening(&model.openings.get(&entity)?.parameters)
            .ok()?;
        let family = &resolved.family;
        let frame_depth = if family.frame_width > 0. {
            family.frame_depth * 0.5
        } else {
            0.
        };
        family
            .extrusions
            .iter()
            .map(|solid| solid.depth * 0.5 + solid.normal_offset.abs())
            .fold((family.depth * 0.5).max(frame_depth), f64::max)
            + 1e-7
    } else {
        1e-7
    };
    let limits = if vertical {
        [F::Bottom, F::Top]
    } else {
        [F::Start, F::End]
    };
    let across = |p: Point2| if vertical { p.y } else { p.x };
    let lo = across(reference(limits[0]).resolve_elevation(model, host).ok()?);
    let hi = across(reference(limits[1]).resolve_elevation(model, host).ok()?);
    drawing
        .provider_lines(context)
        .ok()?
        .iter()
        .filter_map(|line| {
            if (line.entity != entity && line.entity != host)
                || (along(feature, line.start) - along(feature, anchor)).abs() > axis_tolerance
                || (along(feature, line.end) - along(feature, anchor)).abs() > axis_tolerance
            {
                return None;
            }
            let a = across(line.start).min(across(line.end)).max(lo);
            let b = across(line.start).max(across(line.end)).min(hi);
            if b <= a {
                return None;
            }
            os_render::plan::crop_dimension_segment(
                context,
                if vertical {
                    Point2::new(anchor.x, a)
                } else {
                    Point2::new(a, anchor.y)
                },
                if vertical {
                    Point2::new(anchor.x, b)
                } else {
                    Point2::new(b, anchor.y)
                },
            )
            .ok()
            .flatten()
        })
        .max_by(|(a, b), (c, d)| (across(*b) - across(*a)).total_cmp(&(across(*d) - across(*c))))
}

impl Draft {
    fn current(&self, app: &DesktopApp) -> bool {
        self.guard
            .current(&app.editor, app.plans.active, app.selected)
            && app.selected_ids.len() == 1
            && app.selected_ids.contains(&self.id)
            && app.plans.active_sheet.is_none()
            && !app.editor.plugin_work_pending()
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
    ) -> Result<(OpeningParams, PlanDrawing, Point2)> {
        ensure(self.current(app), "Elevation resize is stale")?;
        ensure(
            self.canvas.contains(pointer),
            "Pointer left the elevation canvas",
        )?;
        let world = self.camera.unproject(
            Point2::new(
                (pointer.x - self.canvas.left()) as f64,
                (pointer.y - self.canvas.top()) as f64,
            ),
            [self.canvas.width() as f64, self.canvas.height() as f64],
        )?;
        let model = app.editor.document.model();
        let resolved = model.resolve_opening(&self.original)?;
        let wall = model.resolve_wall(resolved.host)?.parameters;
        let base = model.levels[&wall.level].parameters.elevation;
        let raw = along(self.feature, world) - self.grab;
        let horizontal = jamb(self.feature);
        let mut target = if horizontal {
            Point2::new(raw, self.anchor.y)
        } else {
            Point2::new(self.anchor.x, raw)
        };
        let snaps = app.plans.snaps;
        if (raw - along(self.feature, self.anchor)).abs() * self.camera.pixels_per_metre > 0.001
            && snaps.enabled
            && (snaps.endpoints || snaps.midpoints || snaps.nearest || snaps.axis_extensions)
        {
            let drawing = app.plans.drawing.as_ref().unwrap();
            let mut best = 12.0;
            for (entity, opening) in std::iter::once((resolved.host, false)).chain(
                model
                    .openings
                    .values()
                    .filter(|o| o.id() != self.id && o.parameters.host == resolved.host)
                    .filter(|o| {
                        model
                            .resolve_opening(&o.parameters)
                            .is_ok_and(|r| r.family.rectangular_cut())
                    })
                    .map(|o| (o.id(), true)),
            ) {
                for feature in if horizontal {
                    [F::Start, F::End]
                } else {
                    [F::Bottom, F::Top]
                } {
                    if let Some((a, b)) = visible_edge(
                        model,
                        drawing,
                        self.context,
                        entity,
                        resolved.host,
                        opening,
                        feature,
                    ) {
                        let p = if horizontal {
                            Point2::new(a.x, target.y.clamp(a.y, b.y))
                        } else {
                            Point2::new(target.x.clamp(a.x, b.x), a.y)
                        };
                        let distance =
                            (along(self.feature, p) - raw).abs() * self.camera.pixels_per_metre;
                        if project(self.camera, self.canvas, self.context, p).is_some()
                            && distance <= best
                        {
                            best = distance;
                            if horizontal {
                                target.x = a.x;
                            } else {
                                target.y = a.y;
                            }
                        }
                    }
                }
            }
        }
        ensure(
            point_in_plan_crop(self.context, target),
            "Resize is outside the elevation crop",
        )?;
        let delta = along(self.feature, target) - along(self.feature, self.anchor);
        let mut parameters = self.original.clone();
        // Preserve the exact wire representation on a round trip through f32 UI coordinates.
        if delta.abs() * self.camera.pixels_per_metre > 0.001 && horizontal {
            let width = if self.feature == F::Start {
                parameters.offset = target.x;
                resolved.offset + resolved.width - target.x
            } else {
                target.x - resolved.offset
            };
            ensure(width > 0., "Opening must have positive width")?;
            match &mut parameters.definition {
                OpeningDefinition::Typed { .. } => parameters.width_override = Some(width),
                OpeningDefinition::Legacy { width: w, .. } => *w = width,
            }
        } else if delta.abs() * self.camera.pixels_per_metre > 0.001 {
            let (height, sill) = if self.feature == F::Top {
                (target.y - base - resolved.sill, resolved.sill)
            } else {
                (
                    resolved.sill + resolved.height - (target.y - base),
                    target.y - base,
                )
            };
            ensure(
                height > 0. && sill >= 0. && sill + height <= wall.height,
                "Opening must have positive height and fit within its host",
            )?;
            match &mut parameters.definition {
                OpeningDefinition::Typed { .. } => {
                    parameters.height_override = Some(height);
                    if self.feature == F::Bottom {
                        parameters.sill_override = Some(sill);
                    }
                }
                OpeningDefinition::Legacy {
                    height: h, sill: s, ..
                } => {
                    *h = height;
                    *s = sill;
                }
            }
        }
        let mut candidate = model.clone();
        candidate.openings.get_mut(&self.id).unwrap().parameters = parameters.clone();
        candidate.validate()?;
        crate::opening_tools::host_mesh(&candidate, resolved.host)?;
        for opening in candidate
            .openings
            .values()
            .filter(|o| o.parameters.host == resolved.host)
        {
            crate::opening_tools::panel_mesh(&candidate, opening.id())?;
        }
        Ok((
            parameters,
            crate::plan::elevations::preview(&candidate, self.context)?,
            target,
        ))
    }
}

impl DesktopApp {
    fn elevation_resize_competing(&self) -> bool {
        self.plans.elevation.placement.is_some()
            || self.plans.elevation.dimension.is_some()
            || self.plans.elevation.crop.is_some()
            || self.opening_draft.is_some()
            || self.wall_gesture.is_some()
            || self.plans.facade.is_some()
            || self.plans.active_sheet.is_some()
    }

    pub(super) fn validate_elevation_resize(&mut self, ctx: &egui::Context) {
        let holding = ctx.input(|i| i.pointer.primary_down() || i.pointer.primary_released());
        self.plans.elevation.resize_claimed = holding
            && (self.plans.elevation.resize_claimed || self.plans.elevation.resize.is_some());
        if self
            .plans
            .elevation
            .resize
            .as_ref()
            .is_some_and(|d| !d.current(self))
            || self.elevation_resize_competing()
            || !holding
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
            self.plans.elevation.resize = None;
        }
    }

    pub(super) fn elevation_resize_input(
        &mut self,
        ctx: &egui::Context,
        camera: PlanCamera,
        canvas: egui::Rect,
        context: PlanContext,
    ) -> Frame {
        let mut frame = Frame::default();
        if self.elevation_resize_competing() {
            self.plans.elevation.resize = None;
            return frame;
        }
        if self
            .plans
            .elevation
            .resize
            .as_ref()
            .is_some_and(|d| d.camera != camera || d.canvas != canvas)
        {
            self.plans.elevation.resize = None;
        }
        let Some(drawing) = &self.plans.drawing else {
            return frame;
        };
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        if self.plans.elevation.resize.is_none()
            && !self.plans.elevation.resize_claimed
            && let Some((view, settings)) = self.active_elevation()
            && let Some(id) = self
                .selected
                .filter(|id| self.selected_ids.len() == 1 && self.selected_ids.contains(id))
            && let Some(opening) = self.editor.document.model().openings.get(&id)
            && opening.parameters.host == settings.host
            && let Ok(resolved) = self
                .editor
                .document
                .model()
                .resolve_opening(&opening.parameters)
            && resolved.family.rectangular_cut()
            && !self.editor.plugin_work_pending()
            && self
                .editor
                .document
                .model()
                .resolve_wall(settings.host)
                .is_ok_and(|w| w.parameters.path.is_straight())
        {
            let mut grips = Vec::new();
            for feature in [F::Top, F::Bottom, F::Start, F::End] {
                if feature == F::Bottom && resolved.kind == OpeningKind::Door {
                    continue;
                }
                let Some((a, b)) = visible_edge(
                    self.editor.document.model(),
                    drawing,
                    context,
                    id,
                    settings.host,
                    true,
                    feature,
                ) else {
                    continue;
                };
                let anchor = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                let Some(pos) = project(camera, canvas, context, anchor) else {
                    continue;
                };
                grips.push((feature, anchor, pos));
            }
            for &(feature, anchor, pos) in &grips {
                // Keep established head/sill acquisition unchanged. New jamb grips
                // disappear when their hit disks overlap any other visible grip.
                if jamb(feature)
                    && grips
                        .iter()
                        .any(|(other, _, p)| *other != feature && p.distance(pos) <= 20.)
                {
                    continue;
                }
                let hover = pointer.is_some_and(|p| p.distance(pos) <= 10.);
                frame.grips.push((pos, hover));
                if hover
                    && ctx.input(|i| i.pointer.primary_pressed())
                    && let Ok(guard) =
                        OpeningDraft::begin(&self.editor, Some(view), Some(id), None, None)
                {
                    let p = pointer.unwrap();
                    let world = camera.unproject(
                        Point2::new((p.x - canvas.left()) as f64, (p.y - canvas.top()) as f64),
                        [canvas.width() as f64, canvas.height() as f64],
                    );
                    if let Ok(world) = world {
                        self.plans.elevation.resize = Some(Draft {
                            guard,
                            id,
                            original: opening.parameters.clone(),
                            context,
                            drawing: drawing.identity(),
                            camera,
                            canvas,
                            feature,
                            anchor,
                            grab: along(feature, world) - along(feature, anchor),
                        });
                        self.plans.elevation.resize_claimed = true;
                        break;
                    }
                }
            }
        }
        if let Some(draft) = self.plans.elevation.resize.take() {
            frame.grips.clear();
            let result = pointer
                .ok_or_else(|| Error::Invalid("Pointer unavailable".into()))
                .and_then(|p| draft.candidate(self, p));
            match result {
                Ok((parameters, drawing, target)) => {
                    frame.drawing = Some(drawing);
                    frame.label = Some(format!(
                        "{}: {:.6} m",
                        match draft.feature {
                            F::Start => "Start jamb station",
                            F::End => "End jamb station",
                            F::Top => "Head elevation",
                            _ => "Sill elevation",
                        },
                        along(draft.feature, target)
                    ));
                    if let Some(p) = project(camera, canvas, context, target) {
                        frame.grips.push((p, true));
                    }
                    if ctx.input(|i| i.pointer.primary_released()) && parameters != draft.original {
                        let result = self.editor.command(
                            "Resize opening elevation",
                            Command::UpdateOpening {
                                id: draft.id,
                                parameters,
                            },
                        );
                        self.report(result, "Opening elevation resized.");
                    }
                }
                Err(e) => {
                    frame.label = Some(e.to_string());
                }
            }
            if !ctx.input(|i| i.pointer.primary_released()) {
                self.plans.elevation.resize = Some(draft);
            }
        }
        frame
    }
}
