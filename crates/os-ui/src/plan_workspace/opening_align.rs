//! Transient cross-wall alignment. Only a validated click changes the document.
use super::*;
use os_core::ensure;
use std::f64::consts::TAU;

/// Project onto the selected centerline without endpoint or sweep clamping.
fn project_station(path: os_model::WallPath, point: Point2) -> Result<f64> {
    let station = match path {
        os_model::WallPath::Straight { start, .. } => {
            let tangent = path.tangent(0.);
            (point.x - start.x) * tangent.x + (point.y - start.y) * tangent.y
        }
        os_model::WallPath::CircularArc {
            center,
            radius,
            start_angle_rad,
            signed_sweep_rad,
        } => {
            let dx = point.x - center.x;
            let dy = point.y - center.y;
            let radial_length = dx.hypot(dy);
            ensure(
                radial_length.is_finite() && radial_length > 1e-9,
                "Reference center has no defined radial projection onto the selected arc",
            )?;
            let angle = dy.atan2(dx);
            let travel = ((angle - start_angle_rad) * signed_sweep_rad.signum()).rem_euclid(TAU);
            let sweep = signed_sweep_rad.abs();
            ensure(
                travel <= sweep + 1e-10,
                "Reference center projects outside the selected arc sweep",
            )?;
            radius * travel.min(sweep)
        }
    };
    ensure(station.is_finite(), "Projected station is not finite")?;
    Ok(station)
}

pub(super) struct Draft {
    guard: crate::opening_tools::OpeningRehost,
    context: PlanContext,
}

pub(super) struct Preview {
    pub parameters: OpeningParams,
    pub drawing: PlanDrawing,
    /// View-plane centers, projected source first, reference second.
    pub centers: [Point2; 2],
}

impl Draft {
    fn begin(app: &DesktopApp) -> Result<Self> {
        ensure(
            app.plans.active_sheet.is_none()
                && app.selected_ids.len() == 1
                && app
                    .selected
                    .is_some_and(|id| app.selected_ids.contains(&id)),
            "Select one visible door or window in a floor plan",
        )?;
        let drawing = app
            .plans
            .drawing
            .as_ref()
            .ok_or_else(|| Error::Invalid("Wait for the plan drawing".into()))?;
        let guard = crate::opening_tools::OpeningRehost::begin(
            &app.editor,
            app.plans.active,
            app.selected,
            drawing,
        )?;
        Ok(Self {
            guard,
            context: app.editor.native_plan_context(app.plans.active.unwrap())?,
        })
    }

    pub fn id(&self) -> Id {
        self.guard.id
    }
    pub fn host(&self) -> Id {
        self.guard.host()
    }

    fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active_sheet.is_none()
            && app.selected_ids.len() == 1
            && app.selected_ids.contains(&self.id())
            && self.guard.current(
                &app.editor,
                app.plans.active,
                app.selected,
                app.plans.drawing.as_ref(),
            )
    }

    pub fn candidate(
        &self,
        editor: &Editor,
        drawing: &PlanDrawing,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
    ) -> Result<Preview> {
        let model = editor.document.model();
        let reference = drawing
            .pick_screen(self.context, camera, viewport, pointer, 7.)?
            .filter(|id| model.openings.contains_key(id))
            .ok_or_else(|| Error::Invalid("Choose a visible reference door or window".into()))?;
        ensure(
            reference != self.id(),
            "Choose a distinct reference opening",
        )?;
        let target = model.resolve_opening(&model.openings[&reference].parameters)?;
        ensure(
            target.host != self.host(),
            "Choose an opening on a different host wall",
        )?;
        let source = model.resolve_opening(&model.openings[&self.id()].parameters)?;
        let host = &model.walls[&source.host].parameters;
        let other = &model.walls[&target.host].parameters;
        ensure(
            host.level == other.level,
            "Reference must be on the active plan level",
        )?;
        if host.path.is_straight() && other.path.is_straight() {
            let axis = host.path.tangent(0.);
            let other_axis = other.path.tangent(0.);
            ensure(
                (axis.x * other_axis.y - axis.y * other_axis.x).abs() <= 1e-8,
                "Reference wall must be parallel to the selected host",
            )?;
        }
        let center = other.path.point(target.offset + target.width * 0.5);
        let reference_center = self.context.basis.world_to_plane(center)?;
        ensure(
            self.context.crop.is_none_or(|c| {
                reference_center.x >= c.min.x
                    && reference_center.x <= c.max.x
                    && reference_center.y >= c.min.y
                    && reference_center.y <= c.max.y
            }),
            "Reference center is outside the plan crop",
        )?;
        let station = project_station(host.path, center)?;
        let mut parameters = model.openings[&self.id()].parameters.clone();
        let offset = station - source.width * 0.5;
        if (offset - parameters.offset).abs() > 1e-9 {
            parameters.offset = offset;
        }
        let mut candidate = model.clone();
        candidate.openings.get_mut(&self.id()).unwrap().parameters = parameters.clone();
        candidate.validate()?;
        crate::opening_tools::host_mesh(&candidate, source.host)?;
        crate::opening_tools::panel_mesh(&candidate, self.id())?;
        let drawing = crate::opening_tools::opening_edit_preview(
            &candidate,
            self.id(),
            &[source.host],
            self.context,
        )?;
        Ok(Preview {
            parameters,
            drawing,
            centers: [
                self.context
                    .basis
                    .world_to_plane(host.path.point(station))?,
                reference_center,
            ],
        })
    }
}

impl DesktopApp {
    pub(crate) fn begin_opening_align(&mut self) {
        match Draft::begin(self) {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_opening_placement();
                self.cancel_opening_controls();
                self.cancel_aligned_dimension();
                self.plans.crop.mode = None;
                self.plans.column_placement = None;
                self.plans.casework_placement = None;
                self.plans.floor_sketch = None;
                self.plans.floor_hole_sketch = None;
                self.plans.ceiling_draft = None;
                self.plans.section_placement = None;
                self.plans.room_placement_active = false;
                self.plans.area_selection.cancel();
                self.opening_draft = None;
                self.plans.opening_align = Some(draft);
                self.report(
                    Ok(()),
                    "Align opening center: pick an opening on a parallel wall. Escape cancels.",
                );
            }
            Err(e) => self.report(Err(e), ""),
        }
    }

    pub(super) fn validate_opening_align(&mut self, ctx: &egui::Context) {
        if self.plans.opening_align.is_some() && ctx.input(|i| i.pointer.primary_down()) {
            self.plans.opening_align_claimed = true;
        }
        if self
            .plans
            .opening_align
            .as_ref()
            .is_some_and(|d| !d.current(self))
            || ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::PointerGone))
            })
            || (self.plans.opening_align_claimed
                && ctx.input(|i| !i.pointer.primary_down() && !i.pointer.primary_released()))
        {
            self.plans.opening_align = None;
        }
    }
}

pub(super) fn paint(
    preview: &Preview,
    painter: &egui::Painter,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    let project = |p| {
        camera
            .project(p, [f64::from(rect.width()), f64::from(rect.height())])
            .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
    };
    if let (Ok(a), Ok(b)) = (project(preview.centers[0]), project(preview.centers[1])) {
        painter.line_segment([a, b], egui::Stroke::new(1., theme::ACCENT));
        for p in [a, b] {
            painter.circle_stroke(p, 5., egui::Stroke::new(2., theme::ACCENT));
            painter.line_segment(
                [p - egui::vec2(8., 0.), p + egui::vec2(8., 0.)],
                egui::Stroke::new(1., theme::ACCENT),
            );
            painter.line_segment(
                [p - egui::vec2(0., 8.), p + egui::vec2(0., 8.)],
                egui::Stroke::new(1., theme::ACCENT),
            );
        }
    }
}
