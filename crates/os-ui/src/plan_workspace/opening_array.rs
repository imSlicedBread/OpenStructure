//! Disposable same-host opening arrays; the batch is rebuilt and checked on Apply.
use super::*;
use os_core::ensure;
use os_model::OpeningTag;

pub(super) struct Draft {
    guard: crate::opening_tools::OpeningRehost,
    context: PlanContext,
    pub count: usize,
    pub spacing: f64,
    pub toward_end: bool,
    pub copy_tag: bool,
}

pub(super) struct Preview {
    pub openings: Vec<Opening>,
    tags: Vec<OpeningTag>,
    pub error: Option<String>,
    pub gap: f64,
}

impl Draft {
    fn begin(app: &DesktopApp) -> Result<Self> {
        ensure(
            app.selected_ids.len() == 1,
            "Select one visible door or window",
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
        let model = app.editor.document.model();
        let source = &model.openings[&guard.id].parameters;
        let width = model.resolve_opening(source)?.width;
        // Only native model walls can host opening arrays; plugin elements are
        // not reinterpreted as architectural hosts.
        ensure(
            model.walls.contains_key(&source.host),
            "A native wall host is required",
        )?;
        Ok(Self {
            guard,
            context: app.editor.native_plan_context(app.plans.active.unwrap())?,
            count: 2,
            spacing: width + 0.5,
            toward_end: true,
            copy_tag: false,
        })
    }

    fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active_sheet.is_none()
            && app.selected_ids.len() == 1
            && app.selected_ids.contains(&self.guard.id)
            && self.guard.current(
                &app.editor,
                app.plans.active,
                app.selected,
                app.plans.drawing.as_ref(),
            )
    }

    fn source_tag<'a>(&self, model: &'a Model) -> Option<&'a OpeningTag> {
        model.opening_tags.values().find(|tag| {
            tag.parameters.opening == self.guard.id && tag.parameters.view == self.context.view_id
        })
    }

    pub fn preview(&self, model: &Model) -> Result<Preview> {
        ensure(
            (2..=256).contains(&self.count),
            "Count must be 2–256, including the source",
        )?;
        ensure(
            self.spacing.is_finite() && self.spacing > 0.0,
            "Spacing must be finite and positive",
        )?;
        let source = &model
            .openings
            .get(&self.guard.id)
            .ok_or_else(|| Error::Invalid("Source opening is missing".into()))?
            .parameters;
        let host = model.resolve_wall(source.host)?;
        let wall = &host.parameters;
        let resolved = model.resolve_opening(source)?;
        let step = if self.toward_end {
            self.spacing
        } else {
            -self.spacing
        };
        ensure(
            (step * (self.count - 1) as f64).is_finite(),
            "Array distance overflow",
        )?;
        let tag = self.source_tag(model).filter(|_| self.copy_tag);
        ensure(
            !self.copy_tag || tag.is_some(),
            "Source has no tag in this plan",
        )?;
        let mut candidate = model.clone();
        let mut preview = Preview {
            openings: Vec::with_capacity(self.count - 1),
            tags: Vec::new(),
            error: None,
            gap: self.spacing - resolved.width,
        };
        for i in 1..self.count {
            let delta = step * i as f64;
            let mut parameters = source.clone();
            parameters.offset += delta;
            ensure(parameters.offset.is_finite(), "Array offset overflow")?;
            let opening = Opening::new("core.opening", parameters);
            let check = (|| -> Result<()> {
                opening.parameters.validate(&candidate)?;
                for other in candidate
                    .openings
                    .values()
                    .filter(|o| o.parameters.host == source.host)
                {
                    let other = candidate.resolve_opening(&other.parameters)?;
                    ensure(
                        opening.parameters.offset + resolved.width + 0.001 <= other.offset
                            || other.offset + other.width + 0.001 <= opening.parameters.offset,
                        "Openings require at least 1 mm separation",
                    )?;
                }
                Ok(())
            })();
            if preview.error.is_none() {
                preview.error = check.err().map(|e| format!("Copy {i}: {e}"));
            }
            candidate.openings.insert(opening.id(), opening.clone());
            if let Some(tag) = tag {
                let mut tag_parameters = tag.parameters.clone();
                tag_parameters.opening = opening.id();
                let source_station = source.offset + resolved.width * 0.5;
                let target_station = opening.parameters.offset + resolved.width * 0.5;
                let source_center = wall.path.point(source_station);
                let target_center = wall.path.point(target_station);
                let source_tangent = wall.path.tangent(source_station);
                let target_tangent = wall.path.tangent(target_station);
                let source_normal = Point2::new(-source_tangent.y, source_tangent.x);
                let target_normal = Point2::new(-target_tangent.y, target_tangent.x);
                let relative = Point2::new(
                    tag_parameters.position.x - source_center.x,
                    tag_parameters.position.y - source_center.y,
                );
                let along = relative.x * source_tangent.x + relative.y * source_tangent.y;
                let across = relative.x * source_normal.x + relative.y * source_normal.y;
                tag_parameters.position = Point2::new(
                    target_center.x + target_tangent.x * along + target_normal.x * across,
                    target_center.y + target_tangent.y * along + target_normal.y * across,
                );
                if let Err(e) = tag_parameters.validate_creation(&candidate) {
                    preview
                        .error
                        .get_or_insert_with(|| format!("Copy {i} tag: {e}"));
                }
                let tag = OpeningTag::new("core.opening_tag", tag_parameters);
                candidate.opening_tags.insert(tag.id(), tag.clone());
                preview.tags.push(tag);
            }
            preview.openings.push(opening);
        }
        if let Err(e) = candidate.validate() {
            preview.error.get_or_insert_with(|| e.to_string());
        }
        Ok(preview)
    }
}

impl DesktopApp {
    pub(crate) fn begin_opening_array(&mut self) {
        match Draft::begin(self) {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_opening_placement();
                self.cancel_aligned_dimension();
                self.plans.crop.mode = None;
                self.plans.floor_sketch = None;
                self.plans.floor_hole_sketch = None;
                self.plans.ceiling_draft = None;
                self.plans.section_placement = None;
                self.plans.room_placement_active = false;
                self.plans.opening_move = None;
                self.plans.opening_flip = None;
                self.plans.area_selection.cancel();
                self.opening_draft = None;
                self.plans.opening_array = Some(draft);
            }
            Err(e) => self.report(Err(e), ""),
        }
    }

    pub(super) fn validate_opening_array(&mut self, ctx: &egui::Context) {
        if self.plans.opening_array.is_some() && ctx.input(|i| i.pointer.primary_down()) {
            self.plans.opening_array_claimed = true;
        }
        if self
            .plans
            .opening_array
            .as_ref()
            .is_some_and(|d| !d.current(self))
            || ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::PointerGone))
            })
            || (self.plans.opening_array_claimed
                && ctx.input(|i| !i.pointer.primary_down() && !i.pointer.primary_released()))
        {
            self.plans.opening_array = None;
        }
    }

    pub(super) fn apply_opening_array(&mut self) -> Result<()> {
        let draft = self
            .plans
            .opening_array
            .as_ref()
            .ok_or_else(|| Error::Invalid("No opening array draft".into()))?;
        ensure(draft.current(self), "Opening array context is stale")?;
        let preview = draft.preview(self.editor.document.model())?;
        if let Some(error) = preview.error {
            return Err(Error::Invalid(error));
        }
        // Check geometry before committing, so invalid native geometry cannot partially apply.
        let mut candidate = self.editor.document.model().clone();
        for opening in &preview.openings {
            candidate.openings.insert(opening.id(), opening.clone());
        }
        crate::opening_tools::host_mesh(&candidate, draft.guard.host())?;
        for opening in &preview.openings {
            crate::opening_tools::panel_mesh(&candidate, opening.id())?;
        }
        let commands = preview
            .openings
            .into_iter()
            .map(Command::AddOpening)
            .chain(preview.tags.into_iter().map(Command::AddOpeningTag))
            .collect();
        self.editor.document.execute("Array along wall", commands)?;
        self.plans.opening_array = None;
        self.editor.regenerate()
    }

    pub(super) fn opening_array_dialog(&mut self, ctx: &egui::Context) {
        self.validate_opening_array(ctx);
        let Some(mut draft) = self.plans.opening_array.take() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Array along wall")
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Independent copies on the selected opening’s wall");
                ui.horizontal(|ui| {
                    ui.label("Count (including source)");
                    ui.add(egui::DragValue::new(&mut draft.count).range(2..=256));
                });
                ui.horizontal(|ui| {
                    ui.label("Centre-to-centre spacing (m)");
                    ui.add(
                        egui::DragValue::new(&mut draft.spacing)
                            .speed(0.05)
                            .max_decimals(3),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Toward host");
                    ui.selectable_value(&mut draft.toward_end, false, "Start");
                    ui.selectable_value(&mut draft.toward_end, true, "End");
                });
                if draft.source_tag(self.editor.document.model()).is_some() {
                    ui.checkbox(&mut draft.copy_tag, "Copy tag in this plan");
                }
                let valid = match draft.preview(self.editor.document.model()) {
                    Ok(preview) => {
                        ui.label(format!("Clear gap: {:.3} m", preview.gap));
                        if let Some(error) = preview.error {
                            ui.colored_label(theme::ERROR, error);
                            false
                        } else {
                            true
                        }
                    }
                    Err(e) => {
                        ui.colored_label(theme::ERROR, e.to_string());
                        false
                    }
                };
                ui.horizontal(|ui| {
                    apply = ui
                        .add_enabled(valid, egui::Button::new("Apply array"))
                        .clicked();
                    cancel = ui.button("Cancel array").clicked();
                });
            });
        if !cancel {
            self.plans.opening_array = Some(draft);
        }
        if apply {
            let result = self.apply_opening_array();
            self.report(result, "Opening array created.");
        }
    }
}

pub(super) fn paint(
    draft: &Draft,
    model: &Model,
    painter: &egui::Painter,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    let Ok(preview) = draft.preview(model) else {
        return;
    };
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let color = if preview.error.is_some() {
        theme::ERROR
    } else {
        theme::ACCENT
    };
    for opening in &preview.openings {
        let Ok(resolved) = model.resolve_opening(&opening.parameters) else {
            continue;
        };
        let Ok(host) = model.resolve_wall(resolved.host) else {
            continue;
        };
        let wall = &host.parameters;
        let elevation = model.levels[&wall.level].parameters.elevation;
        if let Ok(lines) = crate::opening_tools::plan_symbol(
            opening.id(),
            &resolved,
            wall,
            elevation,
            draft.context,
        ) {
            for line in lines {
                if let Some((a, b)) = clip_plan_segment(line.start, line.end, draft.context.crop)
                    && let (Ok(a), Ok(b)) = (camera.project(a, size), camera.project(b, size))
                    && [a.x, a.y, b.x, b.y]
                        .iter()
                        .all(|v| v.is_finite() && v.abs() < 1e8)
                {
                    painter.line_segment(
                        [
                            rect.min + egui::vec2(a.x as f32, a.y as f32),
                            rect.min + egui::vec2(b.x as f32, b.y as f32),
                        ],
                        egui::Stroke::new(2.0, color),
                    );
                }
            }
        }
    }
    if let Some(source_tag) = draft.source_tag(model).filter(|_| draft.copy_tag) {
        let label = source_tag.parameters.label(model).0;
        for tag in &preview.tags {
            if let Ok(point) = draft.context.basis.world_to_plane(tag.parameters.position)
                && point_in_plan_crop(draft.context, point)
                && let Ok(point) = camera.project(point, size)
                && point.x.is_finite()
                && point.y.is_finite()
                && point.x.abs() < 1e8
                && point.y.abs() < 1e8
            {
                painter.text(
                    rect.min + egui::vec2(point.x as f32, point.y as f32),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    egui::FontId::proportional(12.0),
                    color,
                );
            }
        }
    }
}
