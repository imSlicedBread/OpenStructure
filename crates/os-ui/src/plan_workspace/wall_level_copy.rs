//! Aligned copies into an existing floor plan; preview owns an isolated editor.
use super::*;
use os_core::ensure;

const EXCLUSIONS: &str =
    "Opening tags, rooms, room tags and dimensions stay in the source plan; they are not copied.";

pub(super) struct Draft {
    guard: wall_set_move::Draft,
    source: Id,
    destination: Option<Id>,
    pub(super) preview: Result<Preview>,
}

pub(super) struct Preview {
    staged: Editor,
    commands: Vec<Command>,
    drawing: PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
}

fn destination_level(model: &Model, source: Id, destination: Id) -> Result<Id> {
    let floor_level = |id| -> Result<Id> {
        let view = model
            .views
            .get(&id)
            .ok_or_else(|| Error::Invalid("Copy plan is missing".into()))?;
        view.parameters.validate_edit()?;
        ensure(
            view.parameters.kind == os_model::ViewKind::Plan
                && view
                    .parameters
                    .plan
                    .is_some_and(|p| p.view_type == os_model::PlanViewType::FloorPlan),
            "Choose an existing floor plan",
        )?;
        let level = view
            .parameters
            .level
            .ok_or_else(|| Error::Invalid("Plan level is missing".into()))?;
        ensure(model.levels.contains_key(&level), "Plan level is missing")?;
        Ok(level)
    };
    let source = floor_level(source)?;
    let destination = floor_level(destination)?;
    ensure(
        source != destination,
        "Choose a floor plan on a different level",
    )?;
    ensure(
        model.levels[&source].parameters.building == model.levels[&destination].parameters.building,
        "Destination must be in the same building",
    )?;
    Ok(destination)
}

impl Draft {
    pub(super) fn begin(app: &DesktopApp) -> Result<Self> {
        Ok(Self {
            guard: wall_set_move::Draft::begin_level_copy(app)?,
            source: app.plans.active.unwrap(),
            destination: None,
            preview: Err(Error::Invalid("Choose a destination floor plan".into())),
        })
    }

    fn current(&self, app: &DesktopApp) -> bool {
        self.guard.current(app) && !app.editor.host.worker_supported(os_walls::PLUGIN_ID)
    }

    pub(super) fn choose(&mut self, app: &DesktopApp, destination: Id) {
        self.destination = Some(destination);
        self.preview = self.prepare(app, destination);
    }

    fn prepare(&self, app: &DesktopApp, destination: Id) -> Result<Preview> {
        ensure(self.current(app), "Copy context changed")?;
        let level = destination_level(app.editor.document.model(), self.source, destination)?;
        let candidate = self.guard.candidate_on_level(level)?;
        let mut staged = Editor::new()?;
        staged.document = Document::from_model(app.editor.document.model().clone())?;
        staged.scene = app.editor.scene.clone();
        staged.pending_geometry = app.editor.pending_geometry.clone();
        staged
            .document
            .execute("Copy walls to level", candidate.commands.clone())?;
        staged.regenerate()?;
        let context = staged.native_plan_context(destination)?;
        let drawing = staged.native_drawing(destination)?;
        // Frame the assembly, not unrelated destination geometry. Basis and crop
        // are those of the destination; no source-view coordinates are reused.
        let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
        let mut max = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for id in self.guard.copied_ids() {
            let wall = &candidate.model.walls[&id].parameters;
            for point in [wall.start(), wall.end()] {
                let point = context.basis.world_to_plane(point)?;
                min.x = min.x.min(point.x);
                min.y = min.y.min(point.y);
                max.x = max.x.max(point.x);
                max.y = max.y.max(point.y);
            }
        }
        let camera = PlanCamera {
            center: Point2::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0),
            pixels_per_metre: (150.0 / (max.x - min.x).max(max.y - min.y).max(1.0))
                .clamp(0.001, 65.0),
        };
        Ok(Preview {
            staged,
            commands: candidate.commands,
            drawing,
            context,
            camera,
        })
    }

    pub(super) fn commit(self, app: &mut DesktopApp) -> Result<()> {
        ensure(
            self.current(app),
            "Copy canceled: selection, view, document or provider changed",
        )?;
        let destination = self
            .destination
            .ok_or_else(|| Error::Invalid("Choose a destination".into()))?;
        destination_level(app.editor.document.model(), self.source, destination)?;
        let ids = self.guard.copied_ids();
        let preview = self.preview?;
        // Nothing fallible follows publication. Commands, scene and drawing were
        // validated together, against the source revision guarded above.
        app.editor
            .document
            .execute("Copy walls to level", preview.commands)?;
        app.editor.scene = preview.staged.scene;
        app.editor.pending_geometry = preview.staged.pending_geometry;
        app.focus_plan(Some(destination));
        app.plans.cameras.insert(destination, preview.camera);
        app.select(ids.first().copied());
        app.selected_ids = ids;
        Ok(())
    }
}

impl Preview {
    fn paint(&self, ui: &mut egui::Ui, ids: &BTreeSet<Id>) -> Result<()> {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 180.0),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, theme::CANVAS);
        let project = |p| -> Result<egui::Pos2> {
            let p = self
                .camera
                .project(p, [rect.width() as f64, rect.height() as f64])?;
            Ok(rect.min + egui::vec2(p.x as f32, p.y as f32))
        };
        let mut visible = BTreeSet::new();
        for item in self.drawing.items(self.context)? {
            let copied = ids.contains(&item.entity);
            if copied {
                visible.insert(item.entity);
            }
            let color = if copied {
                theme::ACCENT
            } else {
                egui::Color32::GRAY
            };
            for (a, b) in item.outline() {
                painter.line_segment([project(a)?, project(b)?], egui::Stroke::new(1.5, color));
            }
        }
        for line in self.drawing.provider_lines(self.context)? {
            let copied = self
                .staged
                .document
                .model()
                .openings
                .get(&line.entity)
                .is_some_and(|o| ids.contains(&o.parameters.host));
            painter.line_segment(
                [project(line.start)?, project(line.end)?],
                egui::Stroke::new(
                    1.5,
                    if copied {
                        theme::ACCENT
                    } else {
                        egui::Color32::GRAY
                    },
                ),
            );
        }
        ui.label(format!(
            "Destination preview · {} / {} copied walls visible with this plan's settings",
            visible.len(),
            ids.len()
        ));
        Ok(())
    }
}

impl DesktopApp {
    pub(crate) fn wall_level_copy_active(&self) -> bool {
        self.plans.wall_level_copy.is_some()
    }

    pub(super) fn begin_wall_level_copy(&mut self) {
        match Draft::begin(self) {
            Ok(draft) => {
                // Use the established assembly-tool cancellation path.
                self.cancel_wall_set_interactions();
                self.cancel_opening_controls();
                self.plans.wall_level_copy = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(crate) fn wall_level_copy_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.plans.wall_level_copy.take() else {
            return;
        };
        self.plans.wall_set_move_claimed = true;
        if !draft.current(self) {
            self.report(Ok(()), "Copy to level canceled: its context changed.");
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.report(Ok(()), "Copy to level canceled.");
            return;
        }
        let model = self.editor.document.model();
        let mut choices: Vec<_> = model
            .views
            .values()
            .filter(|v| destination_level(model, draft.source, v.id()).is_ok())
            .map(|v| {
                (
                    v.id(),
                    format!(
                        "{} · {}",
                        v.parameters.name,
                        model.levels[&v.parameters.level.unwrap()].parameters.name
                    ),
                )
            })
            .collect();
        choices.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        let mut apply = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("wall_level_copy")).show(ctx, |ui| {
            ui.set_width(420.0_f32.min(ctx.content_rect().width() - 60.0));
            ui.heading("Copy walls to level");
            ui.label(draft.guard.dependency_summary().unwrap_or_default());
            ui.label("Aligned at the same XY; wall heights and hosted door/window settings are preserved.");
            ui.label(EXCLUSIONS);
            let previous = draft.destination;
            egui::ComboBox::from_id_salt("copy_destination")
                .selected_text(choices.iter().find(|(id, _)| Some(*id) == draft.destination).map_or("Choose floor plan", |(_, label)| label.as_str()))
                .show_ui(ui, |ui| {
                    for (id, label) in &choices { ui.selectable_value(&mut draft.destination, Some(*id), label); }
                });
            if choices.is_empty() { ui.label("Create a floor plan on another level in this building first."); }
            if previous != draft.destination && let Some(destination) = draft.destination { draft.choose(self, destination); }
            match &draft.preview {
                Ok(preview) => {
                    if let Err(error) = preview.paint(ui, &draft.guard.copied_ids()) { ui.colored_label(theme::ERROR, error.to_string()); }
                }
                Err(error) => { ui.colored_label(theme::ERROR, error.to_string()); }
            }
            ui.horizontal(|ui| {
                apply = ui.add_enabled(draft.preview.is_ok(), egui::Button::new("Apply copy to level")).clicked();
                cancel = ui.button("Cancel copy to level").clicked();
            });
        });
        if apply {
            let result = draft.commit(self);
            self.report(
                result,
                &format!("Wall assembly copied to destination level. {EXCLUSIONS}"),
            );
        } else if cancel {
            self.report(Ok(()), "Copy to level canceled.");
        } else {
            self.plans.wall_level_copy = Some(draft);
        }
    }
}
