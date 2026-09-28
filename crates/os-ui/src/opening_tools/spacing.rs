//! Temporary, exact opening clearances. Nothing here is stored in the model.
use super::*;
use os_render::plan::{PlanCamera, PlanDrawing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reference {
    Endpoint(bool),
    Jamb(Id, bool),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Dimension {
    pub end: bool,
    pub reference: Reference,
    pub distance: f64,
    station: f64,
    witnesses: [egui::Pos2; 2],
    points: [egui::Pos2; 2],
    pub rect: egui::Rect,
}

pub(crate) struct Draft {
    snapshot: OpeningRehost,
    dimension: Dimension,
    camera: PlanCamera,
    canvas: egui::Rect,
    origin: egui::Pos2,
    pub editing: bool,
    pub value: String,
    pub error: Option<String>,
    focus: bool,
}

fn project(
    point: Point2,
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
) -> Option<egui::Pos2> {
    let point = context.basis.world_to_plane(point).ok()?;
    if context.crop.is_some_and(|c| {
        point.x < c.min.x || point.x > c.max.x || point.y < c.min.y || point.y > c.max.y
    }) {
        return None;
    }
    let p = camera
        .project(point, [canvas.width() as f64, canvas.height() as f64])
        .ok()?;
    let p = canvas.min + egui::vec2(p.x as f32, p.y as f32);
    (p.is_finite() && canvas.contains(p)).then_some(p)
}

/// Qualify the actual jamb, not merely a visible fragment of its symbol.
fn visible_jamb(
    id: Id,
    station: f64,
    wall: &WallParams,
    context: PlanContext,
    lines: &[&PlanLine],
) -> bool {
    lines.iter().filter(|line| line.entity == id).any(|line| {
        [line.start, line.end].into_iter().all(|p| {
            context.basis.plane_to_world(p).is_ok_and(|p| {
                let along = ((p.x - wall.start().x) * (wall.end().x - wall.start().x)
                    + (p.y - wall.start().y) * (wall.end().y - wall.start().y))
                    / wall.length();
                (along - station).abs() < 1e-8
            })
        })
    })
}

pub(crate) fn dimensions(
    editor: &Editor,
    id: Id,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
) -> Vec<Dimension> {
    let derive = || -> Option<Vec<Dimension>> {
        let model = editor.document.model();
        let p = model
            .resolve_opening(&model.openings.get(&id)?.parameters)
            .ok()?;
        let wall = &model.resolve_wall(p.host).ok()?.parameters;
        if !context.show_walls || model.views[&context.view_id].parameters.level != Some(wall.level)
        {
            return None;
        }
        let lines = drawing.provider_lines(context).ok()?;
        let mut by_entity = std::collections::BTreeMap::<Id, Vec<&PlanLine>>::new();
        for line in lines {
            by_entity.entry(line.entity).or_default().push(line);
        }
        let jamb_lines = |id| by_entity.get(&id).map(Vec::as_slice).unwrap_or_default();
        let stations = [p.offset, p.offset + p.width];
        if !stations
            .iter()
            .all(|s| visible_jamb(id, *s, wall, context, jamb_lines(id)))
        {
            return None;
        }
        let start = project(world(wall, stations[0], 0.), context, camera, canvas)?;
        let end = project(world(wall, stations[1], 0.), context, camera, canvas)?;
        let center = start.lerp(end, 0.5);
        let flips = flip_rects(editor, id, drawing, context, camera, canvas).unwrap_or_default();
        let mut result = Vec::new();
        for side in [false, true] {
            let selected_station = stations[usize::from(side)];
            let mut reference = Reference::Endpoint(side);
            let mut station = if side { wall.length() } else { 0. };
            for (&other_id, other) in &model.openings {
                if other_id == id || other.parameters.host != p.host {
                    continue;
                }
                let other = model.resolve_opening(&other.parameters).ok()?;
                for jamb_end in [false, true] {
                    let s = other.offset + if jamb_end { other.width } else { 0. };
                    let closer = if side {
                        s >= selected_station && s < station
                    } else {
                        s <= selected_station && s > station
                    };
                    if closer
                        && visible_jamb(other_id, s, wall, context, jamb_lines(other_id))
                        && project(world(wall, s, 0.), context, camera, canvas).is_some()
                    {
                        station = s;
                        reference = Reference::Jamb(other_id, jamb_end);
                    }
                }
            }
            let Some(target) = project(world(wall, station, 0.), context, camera, canvas) else {
                continue;
            };
            let source = if side { end } else { start };
            // Fixed screen-space displacement keeps labels separate at either DPI.
            let axis = (end - start).normalized();
            let mut normal = egui::vec2(-axis.y, axis.x);
            if normal.y < 0. || (normal.y == 0. && normal.x < 0.) {
                normal = -normal;
            }
            // Try another row for short clearances or near-canvas openings.
            let layout = [64., 112., -64., -112.].into_iter().find_map(|offset| {
                let points = [source + normal * offset, target + normal * offset];
                let rect = egui::Rect::from_center_size(
                    points[0].lerp(points[1], 0.5),
                    egui::vec2(88., 24.),
                );
                (canvas.contains_rect(rect.expand(4.))
                    && flips.iter().all(|(_, r)| !r.expand(4.).intersects(rect))
                    && [start, center, end].iter().all(|p| {
                        !rect
                            .expand(4.)
                            .intersects(egui::Rect::from_center_size(*p, egui::vec2(20., 20.)))
                    })
                    && result
                        .iter()
                        .all(|d: &Dimension| !d.rect.expand(4.).intersects(rect)))
                .then_some((points, rect))
            });
            let Some((points, rect)) = layout else {
                continue;
            };
            result.push(Dimension {
                end: side,
                reference,
                distance: (selected_station - station).abs(),
                station,
                witnesses: [source, target],
                points,
                rect,
            });
        }
        Some(result)
    };
    derive().unwrap_or_default()
}

impl Draft {
    pub(crate) fn current(
        &self,
        editor: &Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&PlanDrawing>,
    ) -> bool {
        self.snapshot.current(editor, view, selected, drawing)
    }

    pub(crate) fn commit(
        &self,
        editor: &mut Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&PlanDrawing>,
    ) -> Result<()> {
        ensure(
            self.current(editor, view, selected, drawing),
            "Opening spacing is stale",
        )?;
        let distance = self
            .value
            .trim()
            .parse::<f64>()
            .map_err(|_| Error::Invalid("Enter a distance in metres".into()))?;
        ensure(
            distance.is_finite() && distance >= 0.,
            "Enter a finite positive clearance",
        )?;
        let dimension = dimensions(
            editor,
            self.snapshot.id,
            drawing.unwrap(),
            self.snapshot.context,
            self.camera,
            self.canvas,
        )
        .into_iter()
        .find(|d| d.end == self.dimension.end && d.reference == self.dimension.reference)
        .ok_or_else(|| Error::Invalid("Spacing reference changed".into()))?;
        let model = editor.document.model();
        let mut parameters = model.openings[&self.snapshot.id].parameters.clone();
        let width = model.resolve_opening(&parameters)?.width;
        // Preserve exact no-ops, including subtraction roundoff on the End side.
        if distance == dimension.distance {
            return Ok(());
        }
        parameters.offset = if dimension.end {
            dimension.station - distance - width
        } else {
            dimension.station + distance
        };
        if parameters == model.openings[&self.snapshot.id].parameters {
            return Ok(());
        }
        let mut candidate = model.clone();
        candidate
            .openings
            .get_mut(&self.snapshot.id)
            .unwrap()
            .parameters = parameters.clone();
        candidate.validate()?;
        host_mesh(&candidate, parameters.host)?;
        panel_mesh(&candidate, self.snapshot.id)?;
        editor.command(
            "Edit opening spacing",
            Command::UpdateOpening {
                id: self.snapshot.id,
                parameters,
            },
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn input(
    editor: &Editor,
    draft: &mut Option<Draft>,
    claimed: &mut bool,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
    ui: &mut egui::Ui,
    selected: Option<Id>,
    allow: bool,
) -> (Vec<Dimension>, bool) {
    let controls = if allow {
        selected
            .map(|id| dimensions(editor, id, drawing, context, camera, canvas))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let owned = *claimed || draft.is_some();
    let max_click_dist = ui.ctx().options(|o| o.input_options.max_click_dist);
    let cancel = ui.input(|i| {
        i.key_pressed(egui::Key::Escape)
            || i.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerGone))
    }) || draft.as_ref().is_some_and(|d| {
        !allow
            || !d.current(editor, Some(context.view_id), selected, Some(drawing))
            || d.camera != camera
            || d.canvas != canvas
            || !controls.contains(&d.dimension)
            || (!d.editing
                && ui.input(|i| {
                    i.pointer
                        .interact_pos()
                        .is_some_and(|p| p.distance(d.origin) > max_click_dist)
                }))
    });
    if cancel {
        *draft = None;
    }
    if !cancel
        && !*claimed
        && draft.is_none()
        && ui.input(|i| i.pointer.primary_pressed())
        && let Some(origin) = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            })
        })
        && let Some(dimension) = controls.iter().find(|d| d.rect.contains(origin))
        && ui.rect_contains_pointer(dimension.rect)
        && let Ok(snapshot) = OpeningRehost::begin(editor, Some(context.view_id), selected, drawing)
    {
        *draft = Some(Draft {
            snapshot,
            dimension: dimension.clone(),
            camera,
            canvas,
            origin,
            editing: false,
            value: dimension.distance.to_string(),
            error: None,
            focus: true,
        });
        *claimed = true;
    }
    if ui.input(|i| i.pointer.primary_released())
        && let Some(d) = draft.as_mut().filter(|d| !d.editing)
    {
        if ui.input(|i| {
            i.pointer
                .interact_pos()
                .is_some_and(|p| d.dimension.rect.contains(p))
        }) {
            d.editing = true;
        } else {
            *draft = None;
        }
    }
    (controls, owned || *claimed || draft.is_some())
}

pub(crate) fn paint(
    ui: &egui::Ui,
    painter: &egui::Painter,
    controls: &[Dimension],
    draft: &mut Option<Draft>,
) -> bool {
    for d in controls {
        for (source, target) in d.witnesses.into_iter().zip(d.points) {
            painter.line_segment([source, target], egui::Stroke::new(1., theme::ACCENT));
        }
        painter.line_segment(d.points, egui::Stroke::new(1., theme::ACCENT));
        for p in d.points {
            painter.line_segment(
                [p - egui::vec2(3., 4.), p + egui::vec2(3., 4.)],
                egui::Stroke::new(1., theme::ACCENT),
            );
        }
        painter.rect_filled(d.rect, 3., theme::CANVAS);
        painter.text(
            d.rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{:.3} m", d.distance),
            egui::FontId::proportional(12.),
            theme::ACCENT,
        );
        if ui.rect_contains_pointer(d.rect) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    let mut commit = false;
    if let Some(d) = draft.as_mut().filter(|d| d.editing) {
        egui::Area::new(egui::Id::new("opening_spacing_editor"))
            .order(egui::Order::Foreground)
            .fixed_pos(d.dimension.rect.left_top())
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(if d.dimension.end {
                        "Clear spacing to End (m)"
                    } else {
                        "Clear spacing to Start (m)"
                    });
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut d.value)
                            .desired_width(150.)
                            .char_limit(64),
                    );
                    if d.focus {
                        response.request_focus();
                        d.focus = false;
                    }
                    commit =
                        ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    ui.label("Enter applies · Escape cancels");
                    if let Some(error) = &d.error {
                        ui.colored_label(theme::ERROR, error);
                    }
                });
            });
    }
    commit
}
