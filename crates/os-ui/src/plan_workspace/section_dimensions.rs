//! Revision-bound, three-click section dimension placement and explicit repair.
use super::*;
use os_model::{DimensionLayout as L, ElevationDimensionFeature as F};

pub(super) struct Draft {
    context: PlanContext,
    drawing: Id,
    providers: Vec<(String, Id)>,
    activation: Option<Id>,
    layout: L,
    first: Option<DimensionReference>,
    second: Option<DimensionReference>,
    repair: Option<(Id, usize, DimensionParams)>,
    pointer_down: bool,
}
impl Draft {
    fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active == Some(self.context.view_id)
            && app.plans.active_sheet.is_none()
            && app.editor.native_section_context(self.context.view_id).ok() == Some(self.context)
            && self.providers == plan_provider_signature(&app.editor)
            && self.activation == app.editor.host.activation_id(os_walls::PLUGIN_ID)
            && app
                .plans
                .drawing
                .as_ref()
                .is_some_and(|d| d.identity() == self.drawing && d.dimensions(self.context).is_ok())
            && self.repair.as_ref().is_none_or(|(id, _, p)| {
                app.selected == Some(*id)
                    && app
                        .editor
                        .document
                        .model()
                        .dimensions
                        .get(id)
                        .is_some_and(|d| d.parameters == *p)
            })
    }
}
impl DesktopApp {
    pub(super) fn validate_section_dimension(&mut self, ctx: &egui::Context) {
        let canceled = self
            .plans
            .section_dimension
            .as_ref()
            .is_some_and(|d| !d.current(self))
            || ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if canceled && self.plans.section_dimension.take().is_some() {
            self.status = "Section dimension canceled.".into();
        }
    }
    pub(super) fn section_dimension_toolbar(&mut self, ui: &mut egui::Ui) {
        if self.plans.active_sheet.is_some()
            || self
                .plans
                .active
                .and_then(|v| self.editor.native_section_context(v).ok())
                .is_none()
        {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for (label, layout) in [
                ("Horizontal dimension", L::SectionHorizontal),
                ("Vertical dimension", L::SectionVertical),
            ] {
                if ui.button(label).clicked() {
                    self.begin_section_dimension(layout);
                }
            }
            if self.plans.section_dimension.is_some() && ui.button("Cancel dimension").clicked() {
                self.plans.section_dimension = None;
            }
        });
    }
    pub(super) fn begin_section_dimension(&mut self, layout: L) {
        if !layout.is_section() {
            return;
        }
        let Some(context) = self
            .plans
            .active
            .and_then(|v| self.editor.native_section_context(v).ok())
        else {
            return;
        };
        let Some(drawing) = self
            .plans
            .drawing
            .as_ref()
            .filter(|d| d.dimensions(context).is_ok())
        else {
            return;
        };
        self.plans.section_dimension = Some(Draft {
            context,
            drawing: drawing.identity(),
            providers: plan_provider_signature(&self.editor),
            activation: self.editor.host.activation_id(os_walls::PLUGIN_ID),
            layout,
            first: None,
            second: None,
            repair: None,
            pointer_down: false,
        });
        self.plans.section_placement = None;
        self.status =
            "Pick two visible section extents, then click dimension offset. Escape cancels.".into();
    }
    pub(super) fn begin_section_dimension_repair(&mut self, index: usize) {
        let Some(d) = self
            .selected
            .and_then(|id| self.editor.document.model().dimensions.get(&id))
            .cloned()
        else {
            return;
        };
        if index > 1 || self.plans.active != Some(d.parameters.view) {
            return;
        }
        self.begin_section_dimension(d.parameters.layout);
        if let Some(draft) = &mut self.plans.section_dimension {
            draft.repair = Some((d.id(), index, d.parameters));
        }
    }
    pub(super) fn section_dimension_input(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        camera: PlanCamera,
    ) {
        let Some(mut draft) = self.plans.section_dimension.take() else {
            return;
        };
        if !draft.current(self) {
            self.status = "Section dimension canceled: drawing changed.".into();
            return;
        }
        let rect = response.rect;
        if response.contains_pointer() && response.ctx.input(|i| i.pointer.primary_pressed()) {
            draft.pointer_down = true;
        }
        let released = draft.pointer_down && response.ctx.input(|i| i.pointer.primary_released());
        if released {
            draft.pointer_down = false;
        }
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let Some(pointer) = response.hover_pos().filter(|p| rect.contains(*p)) else {
            if !released {
                self.plans.section_dimension = Some(draft);
            }
            return;
        };
        let pointer = Point2::new(
            f64::from(pointer.x - rect.left()),
            f64::from(pointer.y - rect.top()),
        );
        let model = self.editor.document.model();
        if draft.second.is_none() || draft.repair.is_some() {
            let candidate = pick(
                model,
                self.plans.drawing.as_ref().unwrap(),
                draft.context,
                camera,
                size,
                pointer,
                draft.layout,
            );
            if let Ok((reference, point)) = candidate {
                if let Ok(p) = camera.project(point, size) {
                    painter.circle_filled(
                        rect.min + egui::vec2(p.x as f32, p.y as f32),
                        5.,
                        theme::ACCENT,
                    );
                }
                if response.clicked() {
                    if let Some((id, index, mut p)) = draft.repair.clone() {
                        if index == 0 {
                            p.first = reference;
                        } else {
                            p.second = reference;
                        }
                        let result = self.editor.command(
                            "Repair section dimension",
                            Command::UpdateDimension { id, parameters: p },
                        );
                        self.report(result, "Section reference repaired.");
                        return;
                    }
                    if draft.first.is_none() {
                        draft.first = Some(reference);
                    } else if draft.first != Some(reference) {
                        draft.second = Some(reference);
                    } else {
                        self.status = "Section dimension canceled: repeated reference.".into();
                        return;
                    }
                } else if released {
                    return;
                }
            } else if released {
                self.status = "Section dimension canceled: no visible nominal extent.".into();
                return;
            }
        } else if let (Some(first), Some(second), Ok(point)) =
            (draft.first, draft.second, camera.unproject(pointer, size))
        {
            let mut p = DimensionParams {
                layout: draft.layout,
                view: draft.context.view_id,
                first,
                second,
                additional: vec![],
                baseline_spacing_m: 0.25,
                offset_m: 0.,
                orphan_hint: point,
            };
            let result = (|| {
                os_core::ensure(
                    point_in_plan_crop(draft.context, point),
                    "Offset outside section crop",
                )?;
                let r = p
                    .resolve_section(model)
                    .map_err(|e| Error::Invalid(format!("Section references: {e:?}")))?;
                p.offset_m = if draft.layout == L::SectionHorizontal {
                    point.y - (r.first.y + r.second.y) * 0.5
                } else {
                    point.x - (r.first.x + r.second.x) * 0.5
                };
                p.validate_creation(model)
            })();
            if result.is_ok() {
                let graphic = crate::plan::elevations::dimension_graphic(model, Id::new(), &p);
                let _ = paint_dimension_graphic(
                    painter,
                    draft.context,
                    camera,
                    rect,
                    size,
                    &graphic,
                    true,
                );
                if response.clicked() {
                    let d = os_model::Dimension::new("core.dimension", p);
                    let id = d.id();
                    let result = self
                        .editor
                        .command("Create section dimension", Command::AddDimension(d));
                    let done = result.is_ok();
                    self.report(result, "Section dimension created.");
                    if done {
                        self.select(Some(id));
                    }
                    return;
                }
            }
            if released {
                self.status = "Section dimension canceled: invalid release.".into();
                return;
            }
        }
        self.plans.section_dimension = Some(draft);
    }
}

fn pick(
    model: &os_model::Model,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    size: [f64; 2],
    pointer: Point2,
    layout: L,
) -> Result<(DimensionReference, Point2)> {
    let point = camera.unproject(pointer, size)?;
    os_core::ensure(point_in_plan_crop(context, point), "Outside section crop")?;
    let horizontal = layout == L::SectionHorizontal;
    let features = if horizontal {
        [F::Start, F::End]
    } else {
        [F::Bottom, F::Top]
    };
    let mut refs = Vec::new();
    // Aperture edges win exact-distance ties against the host cut outline.
    for opening in model.openings.keys() {
        for feature in features {
            refs.push(DimensionReference::SectionOpening {
                opening: *opening,
                feature,
            });
        }
    }
    for wall in model.walls.keys() {
        for feature in features {
            refs.push(DimensionReference::SectionWall {
                wall: *wall,
                feature,
            });
        }
    }
    let lines = drawing.provider_lines(context)?;
    let mut best: Option<(f64, DimensionReference, Point2)> = None;
    for reference in refs {
        let Ok(anchor) = reference.resolve_section(model, context.view_id) else {
            continue;
        };
        let Ok([left, right, bottom, top]) = reference.section_bounds(model, context.view_id)
        else {
            continue;
        };
        let host = match reference {
            DimensionReference::SectionOpening { opening, .. } => {
                model.openings[&opening].parameters.host
            }
            _ => reference.entity(),
        };
        for line in lines {
            if line.entity != reference.entity() && line.entity != host {
                continue;
            }
            let (mut a, mut b) = (line.start, line.end);
            if horizontal {
                if (a.x - anchor.x).abs() > 1e-7 || (b.x - anchor.x).abs() > 1e-7 {
                    continue;
                }
                let lo = a.y.min(b.y).max(bottom);
                let hi = a.y.max(b.y).min(top);
                if hi - lo <= 1e-7 {
                    continue;
                }
                a.y = lo;
                b.y = hi;
            } else {
                if (a.y - anchor.y).abs() > 1e-7 || (b.y - anchor.y).abs() > 1e-7 {
                    continue;
                }
                let lo = a.x.min(b.x).max(left);
                let hi = a.x.max(b.x).min(right);
                if hi - lo <= 1e-7 {
                    continue;
                }
                a.x = lo;
                b.x = hi;
            }
            let Some((a, b)) = os_render::plan::crop_dimension_segment(context, a, b)? else {
                continue;
            };
            let a = camera.project(a, size)?;
            let b = camera.project(b, size)?;
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            if dx * dx + dy * dy <= 1e-12 {
                continue;
            }
            let t = (((pointer.x - a.x) * dx + (pointer.y - a.y) * dy) / (dx * dx + dy * dy))
                .clamp(0., 1.);
            let closest = Point2::new(a.x + t * dx, a.y + t * dy);
            let distance = pointer.distance(closest);
            if distance <= 12. && best.as_ref().is_none_or(|(d, _, _)| distance < *d) {
                best = Some((distance, reference, camera.unproject(closest, size)?));
            }
        }
    }
    best.map(|(_, r, p)| (r, p))
        .ok_or_else(|| Error::Invalid("No visible section extent".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (DesktopApp, Id, Id) {
        let mut app = DesktopApp::new().unwrap();
        let mut p = default_wall(app.active_level);
        p.path = os_model::WallPath::Straight {
            start: Point2::new(0., 0.),
            end: Point2::new(10., 0.),
        };
        p.height = 4.;
        let w = os_model::Wall::new(os_walls::WALL_TYPE, p);
        let wall = w.id();
        app.editor.command("Host", Command::AddWall(w)).unwrap();
        let s =
            os_model::SectionViewSettings::new(Point2::new(0., 0.), Point2::new(10., 0.), -1., 5.);
        let view = app
            .editor
            .create_section_view("Section", app.active_level, s)
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        app.plans.drawing = Some(app.editor.native_drawing(view).unwrap());
        (app, wall, view)
    }
    #[test]
    fn section_dimension_context_rejects_drawing_provider_activation_revision_view_sheet() {
        let (mut app, wall, view) = fixture();
        app.begin_section_dimension(L::SectionHorizontal);
        let mut d = app.plans.section_dimension.take().unwrap();
        assert!(d.current(&app));
        d.providers.push(("changed".into(), Id::new()));
        assert!(!d.current(&app));
        d.providers.pop();
        let activation = d.activation;
        d.activation = Some(Id::new());
        assert!(!d.current(&app));
        d.activation = activation;
        app.plans.active_sheet = Some(Id::new());
        assert!(!d.current(&app));
        app.plans.active_sheet = None;
        app.plans.active = None;
        assert!(!d.current(&app));
        app.plans.active = Some(view);
        app.plans.drawing = Some(app.editor.native_drawing(view).unwrap());
        assert!(!d.current(&app));
        d.drawing = app.plans.drawing.as_ref().unwrap().identity();
        assert!(d.current(&app));
        let mut p = app.editor.document.model().walls[&wall].parameters.clone();
        p.height += 1.;
        app.editor
            .command(
                "Edit",
                Command::UpdateWall {
                    id: wall,
                    parameters: p,
                },
            )
            .unwrap();
        assert!(!d.current(&app));
        app.plans.section_dimension = Some(d);
        app.validate_section_dimension(&egui::Context::default());
        assert!(app.plans.section_dimension.is_none());
    }
    #[test]
    fn section_dimension_acquisition_radius_crop_depth_axis_and_snapshot() {
        let (mut app, _, view) = fixture();
        for size in [[1280., 800.], [1000., 650.]] {
            let c = app.editor.native_section_context(view).unwrap();
            let d = app.editor.native_drawing(view).unwrap();
            let camera = PlanCamera {
                center: Point2::new(5., 2.),
                pixels_per_metre: 65.,
            };
            let screen = camera.project(Point2::new(0., 2.), size).unwrap();
            let at = |dx| Point2::new(screen.x + dx, screen.y);
            assert!(
                pick(
                    app.editor.document.model(),
                    &d,
                    c,
                    camera,
                    size,
                    at(11.9),
                    L::SectionHorizontal
                )
                .is_ok()
            );
            assert!(
                pick(
                    app.editor.document.model(),
                    &d,
                    c,
                    camera,
                    size,
                    at(12.1),
                    L::SectionHorizontal
                )
                .is_err()
            );
            assert!(
                pick(
                    app.editor.document.model(),
                    &d,
                    c,
                    camera,
                    size,
                    at(0.),
                    L::SectionVertical
                )
                .is_err()
            );
            let empty = PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![]).unwrap();
            assert!(
                pick(
                    app.editor.document.model(),
                    &empty,
                    c,
                    camera,
                    size,
                    at(0.),
                    L::SectionHorizontal
                )
                .is_err()
            );
            let mut cropped = c;
            cropped.crop.as_mut().unwrap().min.x = 0.5;
            assert!(
                pick(
                    app.editor.document.model(),
                    &d,
                    cropped,
                    camera,
                    size,
                    at(0.),
                    L::SectionHorizontal
                )
                .is_err()
            );
        }
        let mut p = app.editor.document.model().views[&view].parameters.clone();
        let s = p.section.as_mut().unwrap();
        s.start.y = 2.;
        s.end.y = 2.;
        s.mode = os_model::SectionDisplayMode::CutAndProjection;
        s.far_depth = 1.;
        app.editor
            .command(
                "Depth",
                Command::UpdateView {
                    id: view,
                    parameters: p,
                },
            )
            .unwrap();
        let c = app.editor.native_section_context(view).unwrap();
        let d = app.editor.native_drawing(view).unwrap();
        let camera = PlanCamera {
            center: Point2::new(5., 2.),
            pixels_per_metre: 65.,
        };
        let size = [1000., 650.];
        let screen = camera.project(Point2::new(0., 2.), size).unwrap();
        assert!(
            pick(
                app.editor.document.model(),
                &d,
                c,
                camera,
                size,
                screen,
                L::SectionHorizontal
            )
            .is_err()
        );
    }
}
