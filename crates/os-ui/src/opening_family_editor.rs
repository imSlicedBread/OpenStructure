//! Draft-only visual profile editing and previews through the production evaluator.
use super::*;
use os_geometry::{Mesh, Vec3};
use os_render::plan::{PlanContext, PlanLine};

impl OpeningTypeDraft {
    pub(super) fn preview_model(&self, editor: &Editor) -> Result<Model> {
        ensure(self.current(editor), "Opening type draft is stale")?;
        let parameters = self.parameters()?;
        parameters.validate()?;
        let mut model = editor.document.model().clone();
        if self.editing && !self.duplicate {
            model.opening_types.get_mut(&self.id).unwrap().parameters = parameters;
        } else {
            let mut ty = OpeningType::new("core.opening_type", parameters);
            ty.header.id = self.id;
            model.opening_types.insert(self.id, ty);
            if let Some(id) = self.source_opening {
                model.openings.get_mut(&id).unwrap().parameters.definition =
                    OpeningDefinition::Typed { type_id: self.id };
            }
        }
        model.validate()?;
        for opening in model
            .openings
            .values()
            .filter(|o| o.parameters.type_id() == Some(self.id))
        {
            // Preflight all placements, not only the instance displayed in preview.
            super::super::opening_tools::panel_mesh(&model, opening.id())?;
            super::super::opening_tools::host_mesh(&model, opening.parameters.host)?;
        }
        Ok(model)
    }

    #[cfg(test)]
    pub(super) fn preview_geometry(
        &self,
        editor: &Editor,
        view: Option<Id>,
    ) -> Result<(Mesh, Vec<PlanLine>)> {
        self.evaluate_preview(editor, view, false)
    }

    fn evaluate_preview(
        &self,
        editor: &Editor,
        view: Option<Id>,
        include_host: bool,
    ) -> Result<(Mesh, Vec<PlanLine>)> {
        let mut model = self.preview_model(editor)?;
        let instance = model
            .openings
            .values()
            .find(|o| o.parameters.type_id() == Some(self.id))
            .map(|o| o.id());
        let id = if let Some(id) = instance {
            id
        } else {
            let p = &model.opening_types[&self.id].parameters;
            let wall = os_model::Wall::new(
                os_walls::WALL_TYPE,
                WallParams {
                    name: "Preview host".into(),
                    start: Point2::new(0., 0.),
                    end: Point2::new(p.width + 2., 0.),
                    height: p.sill + p.height + 1.,
                    thickness: 0.2,
                    level: *model.levels.keys().next().unwrap(),
                    material: None,
                },
            );
            let opening = os_model::Opening::new(
                "core.opening",
                os_model::OpeningParams {
                    name: "Preview".into(),
                    host: wall.id(),
                    offset: 1.,
                    definition: OpeningDefinition::Typed { type_id: self.id },
                    hinge: Default::default(),
                    swing: Default::default(),
                },
            );
            let id = opening.id();
            model.walls.insert(wall.id(), wall);
            model.openings.insert(id, opening);
            id
        };
        let p = model.resolve_opening(&model.openings[&id].parameters)?;
        let wall = model.resolve_wall(p.host)?;
        let elevation = model.levels[&wall.parameters.level].parameters.elevation;
        let context = view
            .and_then(|id| editor.native_plan_context(id).ok())
            .unwrap_or(PlanContext {
                session_id: self.session,
                model_revision: self.revision,
                view_id: self.id,
                settings_revision: 0,
                basis: Default::default(),
                range: os_geometry::plan::PlanRange {
                    cut: elevation + p.sill + p.height * 0.5,
                    top: elevation + p.sill + p.height + 1.,
                    bottom: elevation,
                    depth: elevation - 1.,
                },
                crop: None,
                scale_denominator: 100.,
                show_walls: true,
                show_extensions: true,
            });
        let mut mesh = super::super::opening_tools::panel_mesh(&model, id)?;
        let mut lines =
            super::super::opening_tools::plan_symbol(id, &p, &wall.parameters, elevation, context)?;
        if include_host {
            let host = os_geometry::walls::NativeWall::from_model(&model, p.host)?;
            let part = host.mesh()?;
            let offset = mesh.vertices.len() as u32;
            mesh.triangles
                .extend(part.triangles.iter().map(|t| t.map(|i| i + offset)));
            mesh.vertices.extend(part.vertices);
            for (_, footprints) in
                host.layer_plan_footprints(context.range, context.basis, context.crop)?
            {
                for footprint in footprints {
                    for (a, b) in footprint
                        .vertices()
                        .iter()
                        .zip(footprint.vertices().iter().cycle().skip(1))
                        .take(footprint.vertices().len())
                    {
                        lines.push(PlanLine {
                            entity: p.host,
                            feature: lines.len() as u32,
                            start: *a,
                            end: *b,
                            role: footprint.role,
                        });
                    }
                }
            }
        }
        Ok((mesh, lines))
    }

    pub(super) fn profile_editor(&mut self, ui: &mut egui::Ui, editor: &Editor, view: Option<Id>) {
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .selectable_label(!self.editing_cut, "Component profile")
                .clicked()
            {
                self.editing_cut = false;
                self.selected_vertex = 0;
            }
            if ui
                .selectable_label(self.editing_cut, "Host cut profile")
                .clicked()
            {
                self.editing_cut = true;
                self.selected_vertex = 0;
            }
        });
        let mut profile = if self.editing_cut {
            self.family.cut_profile.clone()
        } else {
            self.family.profile.clone()
        };
        ui.small("Drag vertices; select a vertex to edit, insert or remove. Coordinates scale with type width/height.");
        ui.small("Cut and component are independent. The component must fit inside the cut; generated frames require a rectangular cut.");
        if self.editing_cut && ui.button("Copy component to cut").clicked() {
            profile = self.family.profile.clone();
            self.error = None;
        }
        self.selected_vertex = self.selected_vertex.min(profile.len() - 1);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(240., 160.), egui::Sense::hover());
            let plot = rect.shrink(12.);
            let width = self.values[0].parse::<f64>().unwrap_or(1.);
            let height = self.values[1].parse::<f64>().unwrap_or(1.);
            let ratio = if width.is_finite() && height.is_finite() && width > 0. && height > 0. {
                (width / height).clamp(0.1, 10.) as f32
            } else {
                1.
            };
            let size = if plot.width() / plot.height() > ratio {
                egui::vec2(plot.height() * ratio, plot.height())
            } else {
                egui::vec2(plot.width(), plot.width() / ratio)
            };
            let plot = egui::Rect::from_center_size(plot.center(), size);
            let screen = |p: Point2| {
                egui::pos2(
                    plot.left() + p.x as f32 * plot.width(),
                    plot.bottom() - p.y as f32 * plot.height(),
                )
            };
            ui.painter().rect_stroke(
                plot,
                0.,
                egui::Stroke::new(1., theme::MUTED),
                egui::StrokeKind::Inside,
            );
            for (i, point) in profile.iter_mut().enumerate() {
                let pos = screen(*point);
                let response = ui.interact(
                    egui::Rect::from_center_size(pos, egui::vec2(14., 14.)),
                    ui.id().with(("profile_vertex", i)),
                    egui::Sense::click_and_drag(),
                );
                if response.clicked() || response.drag_started() {
                    self.selected_vertex = i;
                }
                if response.dragged()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    self.error = None;
                    *point = Point2::new(
                        ((pos.x - plot.left()) / plot.width()).clamp(0., 1.) as f64,
                        ((plot.bottom() - pos.y) / plot.height()).clamp(0., 1.) as f64,
                    );
                }
            }
            let mut candidate = self.family.clone();
            if self.editing_cut {
                candidate.cut_profile = profile.clone();
                candidate.host_cut = os_model::OpeningHostCut::Profile;
            } else {
                candidate.profile = profile.clone();
            }
            let valid = candidate.validate_for(width, height, self.kind).is_ok();
            let color = if valid { theme::ACCENT } else { theme::ERROR };
            for i in 0..profile.len() {
                let p = screen(profile[i]);
                ui.painter().line_segment(
                    [p, screen(profile[(i + 1) % profile.len()])],
                    egui::Stroke::new(2., color),
                );
                ui.painter().circle_filled(
                    p,
                    if i == self.selected_vertex { 5. } else { 3. },
                    color,
                );
            }
            ui.vertical(|ui| {
                ui.label(format!(
                    "Vertex {} / {}",
                    self.selected_vertex + 1,
                    profile.len()
                ));
                let p = &mut profile[self.selected_vertex];
                ui.horizontal(|ui| {
                    ui.label("Width fraction");
                    if ui
                        .add(egui::DragValue::new(&mut p.x).speed(0.01).range(0.0..=1.0))
                        .changed()
                    {
                        self.error = None;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Height fraction");
                    if ui
                        .add(egui::DragValue::new(&mut p.y).speed(0.01).range(0.0..=1.0))
                        .changed()
                    {
                        self.error = None;
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            profile.len() < os_model::OpeningFamily::MAX_VERTICES,
                            egui::Button::new("Insert vertex"),
                        )
                        .clicked()
                    {
                        self.error = None;
                        let a = profile[self.selected_vertex];
                        let b = profile[(self.selected_vertex + 1) % profile.len()];
                        self.selected_vertex += 1;
                        profile.insert(
                            self.selected_vertex,
                            Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5),
                        );
                    }
                    if ui
                        .add_enabled(profile.len() > 3, egui::Button::new("Remove vertex"))
                        .clicked()
                    {
                        self.error = None;
                        profile.remove(self.selected_vertex);
                        self.selected_vertex = self.selected_vertex.min(profile.len() - 1);
                    }
                });
                if ui.button("Reset rectangle").clicked() {
                    self.error = None;
                    profile = os_model::OpeningFamily::default().profile;
                    self.selected_vertex = 0;
                }
                if self.family.frame_width == 0.0 {
                    if ui.button("Add frame").clicked() {
                        self.error = None;
                        self.family.frame_width = 0.05;
                    }
                } else {
                    if ui.button("Remove frame").clicked() {
                        self.error = None;
                        self.family.frame_width = 0.0;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Frame width (m)");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.family.frame_width)
                                    .speed(0.001)
                                    .range(0.001..=0.3),
                            )
                            .changed()
                        {
                            self.error = None;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Frame depth (m)");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.family.frame_depth)
                                    .speed(0.001)
                                    .range(0.001..=0.5),
                            )
                            .changed()
                        {
                            self.error = None;
                        }
                    });
                    ui.small(if self.kind == OpeningKind::Door {
                        "Door frame: two jambs and a head; no threshold."
                    } else {
                        "Window frame: jambs, head and sill around the inset pane."
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Depth cap (m)");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.family.depth)
                                .speed(0.001)
                                .range(0.001..=0.2),
                        )
                        .changed()
                    {
                        self.error = None;
                    }
                });
                ui.small("Depth is also capped at 20% of host thickness and opening width.");
            });
        });
        if self.editing_cut {
            self.family.cut_profile = profile;
            self.family.host_cut = if self.family.rectangular_cut() {
                os_model::OpeningHostCut::Rectangular
            } else {
                os_model::OpeningHostCut::Profile
            };
        } else {
            self.family.profile = profile;
        }
        match self.evaluate_preview(editor, view, self.editing_cut) {
            Ok((mesh, lines)) => {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(if self.editing_cut {
                            "3D host cut preview"
                        } else {
                            "3D component preview"
                        });
                        let project =
                            |p: Vec3| Point2::new(p.x - p.y * 0.65, p.z + (p.x + p.y) * 0.25);
                        let edges: Vec<_> = mesh
                            .triangles
                            .iter()
                            .flat_map(|t| (0..3).map(move |i| (t[i], t[(i + 1) % 3])))
                            .map(|(a, b)| {
                                (
                                    project(mesh.vertices[a as usize]),
                                    project(mesh.vertices[b as usize]),
                                )
                            })
                            .collect();
                        paint_lines(ui, &edges);
                    });
                    ui.vertical(|ui| {
                        ui.label("Plan preview · current cut/range");
                        paint_lines(
                            ui,
                            &lines.iter().map(|l| (l.start, l.end)).collect::<Vec<_>>(),
                        );
                    });
                });
            }
            Err(error) => {
                ui.colored_label(theme::ERROR, error.to_string());
            }
        }
    }
}

fn paint_lines(ui: &mut egui::Ui, lines: &[(Point2, Point2)]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(280., 100.), egui::Sense::hover());
    if lines.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Outside plan range/crop",
            egui::FontId::proportional(12.),
            theme::MUTED,
        );
        return;
    }
    let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in lines.iter().flat_map(|(a, b)| [a, b]) {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    let scale = (260. / (max.x - min.x).max(0.001)).min(80. / (max.y - min.y).max(0.001));
    let screen = |p: Point2| {
        rect.center()
            + egui::vec2(
                ((p.x - (min.x + max.x) * 0.5) * scale) as f32,
                (-(p.y - (min.y + max.y) * 0.5) * scale) as f32,
            )
    };
    for &(a, b) in lines {
        ui.painter()
            .line_segment([screen(a), screen(b)], egui::Stroke::new(1., theme::ACCENT));
    }
}
