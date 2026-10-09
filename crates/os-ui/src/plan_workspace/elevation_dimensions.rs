//! Two-reference elevation dimensions. All draft state is private and transient.
use super::*;
use os_model::{DimensionLayout as L, ElevationDimensionFeature as F};

pub(super) struct Draft {
    context: PlanContext,
    drawing_identity: Id,
    providers: Vec<(String, Id)>,
    activation: Option<Id>,
    layout: L,
    first: Option<DimensionReference>,
    second: Option<DimensionReference>,
    repair: Option<(Id, usize, DimensionParams)>,
}

impl Draft {
    pub(super) fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active == Some(self.context.view_id)
            && app.plans.active_sheet.is_none()
            && app
                .editor
                .native_elevation_context(self.context.view_id)
                .ok()
                == Some(self.context)
            && self.providers == plan_provider_signature(&app.editor)
            && self.activation == app.editor.host.activation_id(os_walls::PLUGIN_ID)
            && app.plans.drawing.as_ref().is_some_and(|d| {
                d.identity() == self.drawing_identity && d.dimensions(self.context).is_ok()
            })
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
    pub(super) fn begin_elevation_dimension(&mut self, layout: L) {
        let Some((view, _)) = self.active_elevation() else {
            return;
        };
        let Ok(context) = self.editor.native_elevation_context(view) else {
            return;
        };
        let Some(drawing_identity) = self
            .plans
            .drawing
            .as_ref()
            .filter(|d| d.dimensions(context).is_ok())
            .map(PlanDrawing::identity)
        else {
            self.status = "Wait for the current elevation drawing.".into();
            return;
        };
        self.plans.elevation.placement = None;
        self.opening_draft = None;
        self.plans.elevation.dimension = Some(Draft {
            context,
            drawing_identity,
            layout,
            providers: plan_provider_signature(&self.editor),
            activation: self.editor.host.activation_id(os_walls::PLUGIN_ID),
            first: None,
            second: None,
            repair: None,
        });
        self.status =
            "Pick two nominal wall/opening extents, then click dimension offset. Escape cancels."
                .into();
    }

    pub(super) fn begin_elevation_dimension_repair(&mut self, index: usize) {
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
        self.begin_elevation_dimension(d.parameters.layout);
        if let Some(draft) = &mut self.plans.elevation.dimension {
            draft.repair = Some((d.id(), index, d.parameters));
            self.status =
                "Pick replacement nominal extent. One click repairs the reference; Escape cancels."
                    .into();
        }
    }

    pub(super) fn elevation_dimension_input(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        context: PlanContext,
        camera: PlanCamera,
    ) {
        let Some(mut draft) = self.plans.elevation.dimension.take() else {
            return;
        };
        if !draft.current(self) {
            self.status = "Elevation dimension canceled: drawing changed.".into();
            return;
        }
        let rect = response.rect;
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let model = self.editor.document.model();
        let pointer = response
            .hover_pos()
            .map(|p| Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())));
        let Some(pointer) = pointer else {
            self.plans.elevation.dimension = Some(draft);
            return;
        };
        let Some(drawing) = &self.plans.drawing else {
            return;
        };
        let candidate = pick(model, drawing, context, camera, size, pointer, draft.layout);
        if draft.repair.is_some() || draft.second.is_none() {
            if let Ok((reference, p)) = candidate {
                if let Ok(p) = camera.project(p, size) {
                    painter.circle_filled(
                        rect.min + egui::vec2(p.x as f32, p.y as f32),
                        5.,
                        theme::ACCENT,
                    );
                }
                if response.clicked() {
                    if let Some((id, index, mut parameters)) = draft.repair.clone() {
                        if index == 0 {
                            parameters.first = reference;
                        } else {
                            parameters.second = reference;
                        }
                        let result = parameters.validate().and_then(|_| {
                            self.editor.command(
                                "Repair elevation dimension",
                                Command::UpdateDimension { id, parameters },
                            )
                        });
                        let done = result.is_ok();
                        self.report(result, "Elevation reference repaired.");
                        if done {
                            return;
                        }
                    } else if draft.first.is_none() {
                        draft.first = Some(reference);
                        self.status = "Pick second extent on the same host.".into();
                    } else if draft.first != Some(reference) {
                        draft.second = Some(reference);
                        self.status = "Click dimension offset. Escape cancels.".into();
                    }
                }
            } else if response.clicked() {
                self.status = "Choose a visible nominal extent within 12 logical pixels.".into();
            }
        } else if let (Some(first), Some(second), Ok(point)) =
            (draft.first, draft.second, camera.unproject(pointer, size))
        {
            let mut parameters = DimensionParams {
                layout: draft.layout,
                view: context.view_id,
                first,
                second,
                additional: Vec::new(),
                baseline_spacing_m: 0.25,
                offset_m: 0.,
                orphan_hint: point,
            };
            let result = (|| {
                os_core::ensure(
                    point_in_plan_crop(context, point),
                    "Dimension offset outside elevation crop",
                )?;
                let r = parameters
                    .resolve_elevation(model)
                    .map_err(|e| Error::Invalid(format!("Invalid references: {e:?}")))?;
                parameters.offset_m = if draft.layout == L::ElevationHorizontal {
                    point.y - (r.first.y + r.second.y) * 0.5
                } else {
                    point.x - (r.first.x + r.second.x) * 0.5
                };
                parameters.validate_creation(model)
            })();
            if result.is_ok() {
                let graphic =
                    crate::plan::elevations::dimension_graphic(model, Id::new(), &parameters);
                let _ =
                    paint_dimension_graphic(painter, context, camera, rect, size, &graphic, true);
                if response.clicked() {
                    let dimension = os_model::Dimension::new("core.dimension", parameters);
                    let id = dimension.id();
                    let result = self.editor.command(
                        "Create elevation dimension",
                        Command::AddDimension(dimension),
                    );
                    let done = result.is_ok();
                    self.report(result, "Elevation dimension created.");
                    if done {
                        self.select(Some(id));
                        return;
                    }
                }
            } else if response.clicked() {
                self.report(result, "");
            }
        }
        self.plans.elevation.dimension = Some(draft);
    }
}

/// Pick only visible collinear portions of a nominal edge, clipped to both its
/// semantic extent and the view crop. Custom-profile contours are not references.
fn pick(
    model: &os_model::Model,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    size: [f64; 2],
    pointer: Point2,
    layout: L,
) -> Result<(DimensionReference, Point2)> {
    let settings = model.views[&context.view_id].parameters.elevation.unwrap();
    let point = camera.unproject(pointer, size)?;
    os_core::ensure(point_in_plan_crop(context, point), "Outside elevation crop")?;
    let horizontal = layout == L::ElevationHorizontal;
    let features = if horizontal {
        [F::Start, F::End]
    } else {
        [F::Bottom, F::Top]
    };
    let mut refs = Vec::new();
    for feature in features {
        refs.push(DimensionReference::ElevationWall {
            wall: settings.host,
            feature,
        });
    }
    for o in model
        .openings
        .values()
        .filter(|o| o.parameters.host == settings.host)
    {
        for feature in features {
            refs.push(DimensionReference::ElevationOpening {
                opening: o.id(),
                feature,
            });
        }
    }
    let lines = drawing.provider_lines(context)?;
    let mut best: Option<(f64, DimensionReference, Point2)> = None;
    for reference in refs {
        let Ok(anchor) = reference.resolve_elevation(model, settings.host) else {
            continue;
        };
        let opposite = if horizontal {
            [F::Bottom, F::Top]
        } else {
            [F::Start, F::End]
        };
        let edge_point = |feature| {
            match reference {
                DimensionReference::ElevationWall { wall, .. } => {
                    DimensionReference::ElevationWall { wall, feature }
                }
                _ => DimensionReference::ElevationOpening {
                    opening: reference.entity(),
                    feature,
                },
            }
            .resolve_elevation(model, settings.host)
        };
        let (Ok(lo), Ok(hi)) = (edge_point(opposite[0]), edge_point(opposite[1])) else {
            continue;
        };
        for line in lines {
            if line.entity != reference.entity() && line.entity != settings.host {
                continue;
            }
            let (mut a, mut b) = (line.start, line.end);
            if horizontal {
                if (a.x - anchor.x).abs() > 1e-7 || (b.x - anchor.x).abs() > 1e-7 {
                    continue;
                }
                let min = a.y.min(b.y).max(lo.y);
                let max = a.y.max(b.y).min(hi.y);
                if max <= min {
                    continue;
                }
                a.y = min;
                b.y = max;
            } else {
                if (a.y - anchor.y).abs() > 1e-7 || (b.y - anchor.y).abs() > 1e-7 {
                    continue;
                }
                let min = a.x.min(b.x).max(lo.x);
                let max = a.x.max(b.x).min(hi.x);
                if max <= min {
                    continue;
                }
                a.x = min;
                b.x = max;
            }
            let Some((a, b)) = os_render::plan::crop_dimension_segment(context, a, b)? else {
                continue;
            };
            let a = camera.project(a, size)?;
            let b = camera.project(b, size)?;
            let (dx, dy) = (b.x - a.x, b.y - a.y);
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
        .ok_or_else(|| Error::Invalid("No visible nominal extent".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevation_dimension_draft_binds_drawing_provider_view_and_revision() {
        let mut app = DesktopApp::new().unwrap();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(app.active_level));
        let host = wall.id();
        app.editor.command("Wall", Command::AddWall(wall)).unwrap();
        let view = app
            .editor
            .create_wall_elevation("Elevation", host, os_model::ElevationSide::Left)
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        app.plans.drawing = Some(app.editor.native_drawing(view).unwrap());
        app.begin_elevation_dimension(L::ElevationHorizontal);
        let mut draft = app.plans.elevation.dimension.take().unwrap();
        assert!(draft.current(&app));
        draft.providers.push(("changed".into(), Id::new()));
        assert!(!draft.current(&app));
        draft.providers.pop();
        let saved = draft.activation;
        draft.activation = Some(Id::new());
        assert!(!draft.current(&app));
        draft.activation = saved;
        app.plans.drawing = Some(app.editor.native_drawing(view).unwrap());
        assert!(!draft.current(&app));
        draft.drawing_identity = app.plans.drawing.as_ref().unwrap().identity();
        assert!(draft.current(&app));
        app.plans.active = None;
        assert!(!draft.current(&app));
        app.plans.active = Some(view);
        let mut p = app.editor.document.model().walls[&host].parameters.clone();
        p.name = "Changed".into();
        app.editor
            .command(
                "Revision",
                Command::UpdateWall {
                    id: host,
                    parameters: p,
                },
            )
            .unwrap();
        assert!(!draft.current(&app));
    }

    #[test]
    fn elevation_dimension_pick_radius_crop_axis_visibility_and_shared_paper_strokes() {
        let mut app = DesktopApp::new().unwrap();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(app.active_level));
        let host = wall.id();
        let length = wall.parameters.length();
        let height = wall.parameters.height;
        app.editor.command("Wall", Command::AddWall(wall)).unwrap();
        let view = app
            .editor
            .create_wall_elevation("Elevation", host, os_model::ElevationSide::Left)
            .unwrap();
        let c = app.editor.native_elevation_context(view).unwrap();
        let drawing = app.editor.native_drawing(view).unwrap();
        let camera = PlanCamera {
            center: Point2::new(length * 0.5, height * 0.5),
            pixels_per_metre: 100.,
        };
        let size = [1000., 650.];
        let start = camera.project(Point2::new(0., height * 0.5), size).unwrap();
        assert!(
            pick(
                app.editor.document.model(),
                &drawing,
                c,
                camera,
                size,
                Point2::new(start.x + 11.9, start.y),
                L::ElevationHorizontal
            )
            .is_ok()
        );
        assert!(
            pick(
                app.editor.document.model(),
                &drawing,
                c,
                camera,
                size,
                Point2::new(start.x + 12.1, start.y),
                L::ElevationHorizontal
            )
            .is_err()
        );
        assert!(
            pick(
                app.editor.document.model(),
                &drawing,
                c,
                camera,
                size,
                start,
                L::ElevationVertical
            )
            .is_err()
        );
        let empty =
            PlanDrawing::from_prisms(c, &std::collections::BTreeMap::new(), Vec::new()).unwrap();
        assert!(
            pick(
                app.editor.document.model(),
                &empty,
                c,
                camera,
                size,
                start,
                L::ElevationHorizontal
            )
            .is_err()
        );
        let p = DimensionParams {
            layout: L::ElevationHorizontal,
            additional: vec![],
            baseline_spacing_m: 0.25,
            view,
            first: DimensionReference::ElevationWall {
                wall: host,
                feature: F::Start,
            },
            second: DimensionReference::ElevationWall {
                wall: host,
                feature: F::End,
            },
            offset_m: 0.2,
            orphan_hint: Point2::new(length * 0.5, height * 0.5 + 0.2),
        };
        let d = os_model::Dimension::new("core.dimension", p);
        let id = d.id();
        app.editor
            .command("Dimension", Command::AddDimension(d))
            .unwrap();
        let c = app.editor.native_elevation_context(view).unwrap();
        let drawing = app.editor.native_drawing(view).unwrap();
        use os_render::sheet::*;
        let viewport = PaperViewport {
            center_mm: Point2::new(210., 148.5),
            width_mm: 200.,
            height_mm: 100.,
            model_center_m: camera.center,
            scale_denominator: 100.,
        };
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 420.,
                height_mm: 297.,
                number: "A1",
                name: "Dimensions",
            },
            "Elevation",
            viewport,
            c,
            &drawing,
        )
        .unwrap();
        let graphic = drawing
            .dimensions(c)
            .unwrap()
            .iter()
            .find(|d| d.entity == id)
            .unwrap();
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        for (a, b) in graphic.lines().filter(|(a, b)| a.distance(*b) > 1e-6) {
            let points = [
                viewport.model_to_paper(a).unwrap(),
                viewport.model_to_paper(b).unwrap(),
            ];
            assert!(page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Path { points_mm,closed:false,.. } if points_mm.as_slice()==points)));
            let pt = 72. / 25.4;
            assert!(pdf.contains(&format!(
                "{:.6} {:.6} m\n{:.6} {:.6} l",
                points[0].x * pt,
                (297. - points[0].y) * pt,
                points[1].x * pt,
                (297. - points[1].y) * pt
            )));
        }
        let mut vp = app.editor.document.model().views[&view].parameters.clone();
        vp.elevation.as_mut().unwrap().min_station = 0.3;
        app.editor
            .command(
                "Crop",
                Command::UpdateView {
                    id: view,
                    parameters: vp,
                },
            )
            .unwrap();
        let c = app.editor.native_elevation_context(view).unwrap();
        let drawing = app.editor.native_drawing(view).unwrap();
        assert!(
            pick(
                app.editor.document.model(),
                &drawing,
                c,
                camera,
                size,
                start,
                L::ElevationHorizontal
            )
            .is_err()
        );
    }
}
