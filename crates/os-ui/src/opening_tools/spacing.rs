//! Measured opening spacing, with explicit locks only for actual host endpoints.
use super::*;
use os_model::WallPath;
use os_render::plan::{PlanCamera, PlanDrawing};

#[derive(Clone, Debug, PartialEq)]
enum Graphic {
    Straight,
    Arc(Vec<egui::Pos2>),
}

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
    graphic: Graphic,
    pub rect: egui::Rect,
}

pub(crate) struct Draft {
    pub lock_endpoint: bool,
    was_locked: bool,
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
        if !wall.path.is_straight() && line.start.distance(line.end) <= 1e-8 {
            return false;
        }
        [line.start, line.end].into_iter().all(|p| {
            context.basis.plane_to_world(p).is_ok_and(|p| {
                if !wall.path.is_straight() {
                    let anchor = wall.path.point(station);
                    let tangent = wall.path.tangent(station);
                    let dx = p.x - anchor.x;
                    let dy = p.y - anchor.y;
                    // Projection clamps outside the sweep. Also require the
                    // point to lie on this radial segment, within the wall.
                    return (wall.path.project(p) - station).abs() < 1e-8
                        && (dx * tangent.x + dy * tangent.y).abs() < 1e-8
                        && (-dx * tangent.y + dy * tangent.x).abs() <= wall.thickness / 2. + 1e-8;
                }
                let along = ((p.x - wall.start().x) * (wall.end().x - wall.start().x)
                    + (p.y - wall.start().y) * (wall.end().y - wall.start().y))
                    / wall.length();
                (along - station).abs() < 1e-8
            })
        })
    })
}

/// Endpoints alone cannot qualify a major arc. Test the exact extrema in
/// the rotated plan basis; the camera preserves these axes.
fn arc_fits(path: WallPath, context: PlanContext, camera: PlanCamera, canvas: egui::Rect) -> bool {
    let WallPath::CircularArc {
        radius,
        start_angle_rad,
        signed_sweep_rad,
        ..
    } = path
    else {
        return false;
    };
    if project(path.start(), context, camera, canvas).is_none()
        || project(path.end(), context, camera, canvas).is_none()
    {
        return false;
    }
    (0..4).all(|i| {
        let angle = context.basis.rotation + f64::from(i) * std::f64::consts::FRAC_PI_2;
        let travel = ((angle - start_angle_rad) * signed_sweep_rad.signum())
            .rem_euclid(std::f64::consts::TAU);
        travel > signed_sweep_rad.abs()
            || project(path.point(travel * radius), context, camera, canvas).is_some()
    })
}

fn arc_row(
    path: WallPath,
    stations: [f64; 2],
    offset: f64,
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
) -> Option<(Vec<egui::Pos2>, egui::Pos2)> {
    let WallPath::CircularArc {
        center,
        radius,
        start_angle_rad,
        signed_sweep_rad,
    } = path
    else {
        return None;
    };
    let start_angle_rad = start_angle_rad + stations[0] / radius * signed_sweep_rad.signum();
    let signed_sweep_rad = (stations[1] - stations[0]) / radius * signed_sweep_rad.signum();
    let interval = WallPath::CircularArc {
        center,
        radius,
        start_angle_rad,
        signed_sweep_rad,
    };
    if !arc_fits(interval, context, camera, canvas) {
        return None;
    }
    let radius = radius + offset / camera.pixels_per_metre;
    if !radius.is_finite() || radius <= 0. {
        return None;
    }
    let row = WallPath::CircularArc {
        center,
        radius,
        start_angle_rad,
        signed_sweep_rad,
    };
    if !arc_fits(row, context, camera, canvas) {
        return None;
    }
    let count = row.display_segments(0.).ok()?;
    let points = (0..=count)
        .map(|i| {
            project(
                row.point(row.length() * i as f64 / count as f64),
                context,
                camera,
                canvas,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some((
        points,
        project(row.point(row.length() / 2.), context, camera, canvas)?,
    ))
}

// Rectangles and tick ends also belong to the full graphic. Unprojecting their
// corners applies the same crop test as the arc and radial witnesses.
fn screen_fits(
    p: egui::Pos2,
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
) -> bool {
    let delta = p - canvas.min;
    camera
        .unproject(
            Point2::new(delta.x as f64, delta.y as f64),
            [canvas.width() as f64, canvas.height() as f64],
        )
        .and_then(|p| context.basis.plane_to_world(p))
        .is_ok_and(|p| project(p, context, camera, canvas).is_some())
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
        let center = if wall.path.is_straight() {
            start.lerp(end, 0.5)
        } else {
            project(
                wall.path.point((stations[0] + stations[1]) / 2.),
                context,
                camera,
                canvas,
            )?
        };
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
            let mut layout = None;
            for offset in [64., 112., -64., -112.] {
                let (points, midpoint, graphic) = if wall.path.is_straight() {
                    let points = [source + normal * offset, target + normal * offset];
                    (points, points[0].lerp(points[1], 0.5), Graphic::Straight)
                } else {
                    let Some((arc, midpoint)) = arc_row(
                        wall.path,
                        [selected_station, station],
                        offset as f64,
                        context,
                        camera,
                        canvas,
                    ) else {
                        continue;
                    };
                    let Some(last) = arc.last().copied() else {
                        continue;
                    };
                    ([arc[0], last], midpoint, Graphic::Arc(arc))
                };
                let rect = egui::Rect::from_center_size(
                    midpoint,
                    egui::vec2(if wall.path.is_straight() { 88. } else { 112. }, 24.),
                );
                if !wall.path.is_straight()
                    && ![
                        rect.left_top(),
                        rect.right_top(),
                        rect.left_bottom(),
                        rect.right_bottom(),
                    ]
                    .into_iter()
                    .chain(
                        points
                            .into_iter()
                            .flat_map(|p| [p - egui::vec2(3., 4.), p + egui::vec2(3., 4.)]),
                    )
                    .all(|p| screen_fits(p, context, camera, canvas))
                {
                    continue;
                }
                if canvas.contains_rect(rect.expand(4.))
                    && flips.iter().all(|(_, r)| !r.expand(4.).intersects(rect))
                    && [start, center, end].iter().all(|p| {
                        !rect
                            .expand(4.)
                            .intersects(egui::Rect::from_center_size(*p, egui::vec2(20., 20.)))
                    })
                    && result
                        .iter()
                        .all(|d: &Dimension| !d.rect.expand(4.).intersects(rect))
                {
                    layout = Some((points, rect, graphic));
                    break;
                }
            }
            let Some((points, rect, graphic)) = layout else {
                continue;
            };
            result.push(Dimension {
                end: side,
                reference,
                distance: (selected_station - station).abs(),
                station,
                witnesses: [source, target],
                points,
                graphic,
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
        let distance = if self.was_locked && !self.lock_endpoint {
            self.dimension.distance
        } else {
            self.value
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::Invalid("Enter a distance in metres".into()))?
        };
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
        if let Reference::Endpoint(end) = dimension.reference {
            let end = if end {
                os_model::ClearanceEnd::End
            } else {
                os_model::ClearanceEnd::Start
            };
            let existing = model.opening_clearances.get(&self.snapshot.id);
            let command = if self.lock_endpoint {
                Some(Command::SetOpeningClearance {
                    id: self.snapshot.id,
                    clearance: Some(os_model::OpeningClearance { end, distance }),
                })
            } else if existing.is_some_and(|lock| lock.end == end) {
                Some(Command::SetOpeningClearance {
                    id: self.snapshot.id,
                    clearance: None,
                })
            } else {
                None
            };
            if let Some(command) = command {
                let mut candidate = os_document::Document::from_model(model.clone())?;
                candidate.execute("Edit host-end clearance", vec![command.clone()])?;
                host_mesh(
                    candidate.model(),
                    model.openings[&self.snapshot.id].parameters.host,
                )?;
                panel_mesh(candidate.model(), self.snapshot.id)?;
                return editor.command("Edit host-end clearance", command);
            }
        }
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
            was_locked: match dimension.reference {
                Reference::Endpoint(end) => editor
                    .document
                    .model()
                    .opening_clearances
                    .get(&snapshot.id)
                    .is_some_and(|c| (c.end == os_model::ClearanceEnd::End) == end),
                Reference::Jamb(..) => false,
            },
            lock_endpoint: match dimension.reference {
                Reference::Endpoint(end) => editor
                    .document
                    .model()
                    .opening_clearances
                    .get(&snapshot.id)
                    .is_some_and(|c| (c.end == os_model::ClearanceEnd::End) == end),
                Reference::Jamb(..) => false,
            },
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
        match &d.graphic {
            Graphic::Straight => {
                painter.line_segment(d.points, egui::Stroke::new(1., theme::ACCENT));
            }
            Graphic::Arc(points) => {
                painter.add(egui::Shape::line(
                    points.clone(),
                    egui::Stroke::new(1., theme::ACCENT),
                ));
            }
        }
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
            match d.graphic {
                Graphic::Straight => format!("{:.3} m", d.distance),
                Graphic::Arc(_) => format!("Arc {:.3} m", d.distance),
            },
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
                    ui.label(match (&d.dimension.graphic, d.dimension.end) {
                        (Graphic::Arc(_), true) => "Centerline clear spacing to End (m)",
                        (Graphic::Arc(_), false) => "Centerline clear spacing to Start (m)",
                        (Graphic::Straight, true) => "Clear spacing to End (m)",
                        (Graphic::Straight, false) => "Clear spacing to Start (m)",
                    });
                    let unlocking = d.was_locked && !d.lock_endpoint;
                    let response = ui.add_enabled(!unlocking,
                        egui::TextEdit::singleline(&mut d.value)
                            .desired_width(150.)
                            .char_limit(64),
                    );
                    if unlocking { ui.small("Unlock keeps the current position."); }
                    match d.dimension.reference {
                        Reference::Endpoint(end) => {
                            ui.checkbox(&mut d.lock_endpoint, if end { "Lock to host End" } else { "Lock to host Start" });
                        }
                        Reference::Jamb(..) => { ui.small("Measured to a neighbor jamb; endpoint locking is available in Properties."); }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> PlanContext {
        PlanContext {
            session_id: Id::new(),
            model_revision: 0,
            view_id: Id::new(),
            settings_revision: 0,
            basis: os_geometry::plan::HorizontalBasis {
                origin: Point2::default(),
                rotation: 0.31,
            },
            range: Default::default(),
            crop: None,
            scale_denominator: 100.,
            show_walls: true,
            show_extensions: true,
        }
    }

    #[test]
    fn opening_spacing_arc_jamb_qualifies_geometry_not_feature_or_clamped_projection() {
        let context = context();
        let id = Id::new();
        for sweep in [5.6, -5.6] {
            let wall = WallParams {
                name: "Arc".into(),
                level: Id::new(),
                material: None,
                height: 3.,
                thickness: 0.2,
                path: WallPath::CircularArc {
                    center: Point2::default(),
                    radius: 4.,
                    start_angle_rad: 0.5,
                    signed_sweep_rad: sweep,
                },
            };
            let line = |a, b| PlanLine {
                entity: id,
                feature: 73,
                start: context.basis.world_to_plane(a).unwrap(),
                end: context.basis.world_to_plane(b).unwrap(),
                role: os_geometry::plan::PlanRole::Cut,
            };
            let jamb = line(
                wall.path.offset_point(2., -0.08),
                wall.path.offset_point(2., 0.04),
            );
            assert!(
                visible_jamb(id, 2., &wall, context, &[&jamb]),
                "surviving clipped jamb"
            );
            let outside = line(
                wall.path.offset_point(2., 0.12),
                wall.path.offset_point(2., 0.2),
            );
            assert!(!visible_jamb(id, 2., &wall, context, &[&outside]));
            let pane = line(
                wall.path.offset_point(2., 0.01),
                wall.path.offset_point(2.2, 0.01),
            );
            assert!(!visible_jamb(id, 2., &wall, context, &[&pane]));
            let clamped = line(wall.path.point(-0.1), wall.path.offset_point(-0.1, 0.04));
            assert_eq!(
                wall.path
                    .project(context.basis.plane_to_world(clamped.start).unwrap()),
                0.
            );
            assert!(!visible_jamb(id, 0., &wall, context, &[&clamped]));
            let zero = line(wall.path.point(2.), wall.path.point(2.));
            assert!(!visible_jamb(id, 2., &wall, context, &[&zero]));
        }
    }

    #[test]
    fn opening_spacing_arc_major_row_tolerance_and_transformed_crop_extrema() {
        let mut context = context();
        let camera = PlanCamera {
            center: Point2::default(),
            pixels_per_metre: 35.,
        };
        let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 800.));
        for sign in [-1., 1.] {
            let path = WallPath::CircularArc {
                center: Point2::default(),
                radius: 4.,
                start_angle_rad: context.basis.rotation + sign * 0.2,
                signed_sweep_rad: sign * 5.6,
            };
            let (points, midpoint) =
                arc_row(path, [0., path.length()], 64., context, camera, canvas).unwrap();
            let display_radius = 4. + 64. / camera.pixels_per_metre;
            assert!(points.len() > 3 && points.len() <= 4097);
            let expected = path.point(path.length() / 2.);
            let expected = Point2::new(
                expected.x * display_radius / 4.,
                expected.y * display_radius / 4.,
            );
            assert!(midpoint.distance(project(expected, context, camera, canvas).unwrap()) < 1e-4);
            let center = project(Point2::default(), context, camera, canvas).unwrap();
            for pair in points.windows(2) {
                let radius_at_chord = pair[0].lerp(pair[1], 0.5).distance(center) as f64 / 35.;
                assert!((display_radius - radius_at_chord).abs() <= 0.00101);
            }
            let (reverse, _) =
                arc_row(path, [path.length(), 0.], 64., context, camera, canvas).unwrap();
            assert!(reverse[0].distance(*points.last().unwrap()) < 1e-4);
            assert!(arc_row(path, [0., path.length()], -200., context, camera, canvas).is_none());
            context.crop = Some(os_geometry::plan::PlanCrop {
                min: Point2::new(0., -8.),
                max: Point2::new(8., 8.),
            });
            assert!(project(path.start(), context, camera, canvas).is_some());
            assert!(project(path.end(), context, camera, canvas).is_some());
            assert!(
                arc_row(path, [0., path.length()], 64., context, camera, canvas).is_none(),
                "visible endpoints cannot hide a cropped major-arc bulge"
            );
            context.crop = None;
        }
    }
}
