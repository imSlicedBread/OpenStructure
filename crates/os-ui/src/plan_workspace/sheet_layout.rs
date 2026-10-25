//! Transactional paper layout editor; geometry is never changed by a paper drag.
use super::*;
use os_model::{SheetPaperRect, SheetSchedulePlacement};

pub(super) struct Draft {
    sheet: Id,
    session: Id,
    revision: u64,
    providers: Vec<(String, Id)>,
    original: SheetParams,
    pub(super) parameters: SheetParams,
    pub(super) selected: Option<Id>,
    pub(super) source: Option<Id>,
    schedule: Option<Id>,
    drag: Option<(Id, egui::Pos2, SheetPaperRect, bool, f32)>,
    #[cfg(test)]
    canvas_rect: Option<egui::Rect>,
}

impl Draft {
    pub(super) fn new(editor: &Editor, sheet: Id) -> Self {
        let parameters = editor.document.model().sheets[&sheet].parameters.clone();
        Self {
            sheet,
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            providers: plan_provider_signature(editor),
            selected: parameters.viewports.first().map(|v| v.id),
            original: parameters.clone(),
            parameters,
            source: None,
            schedule: None,
            drag: None,
            #[cfg(test)]
            canvas_rect: None,
        }
    }
    pub(super) fn current(&self, editor: &Editor, sheet: Option<Id>) -> bool {
        sheet == Some(self.sheet)
            && self.session == editor.document.session_id()
            && self.revision == editor.document.revision()
            && self.providers == plan_provider_signature(editor)
            && editor
                .document
                .model()
                .sheets
                .get(&self.sheet)
                .is_some_and(|s| s.parameters == self.original)
    }
    pub(super) fn dirty(&self) -> bool {
        self.original != self.parameters
    }

    fn add_view(&mut self, editor: &Editor, view: Id) -> Result<()> {
        os_core::ensure(
            self.parameters.viewports.len() < os_model::MAX_SHEET_VIEWPORTS,
            "sheet exceeds 64 viewports",
        )?;
        let context = editor.native_view_context(view)?;
        let id = Id::new();
        self.parameters.viewports.push(SheetViewport {
            id,
            view,
            model_center_m: context.crop.map_or(Point2::default(), |crop| {
                Point2::new(
                    (crop.min.x + crop.max.x) / 2.0,
                    (crop.min.y + crop.max.y) / 2.0,
                )
            }),
            paper_center_mm: Point2::new(210.0, 120.0),
            width_mm: 100.0,
            height_mm: 65.0,
            scale_denominator: context.scale_denominator,
            detail_number: (1..)
                .map(|n| n.to_string())
                .find(|n| {
                    !self
                        .parameters
                        .viewports
                        .iter()
                        .any(|v| &v.detail_number == n)
                })
                .unwrap(),
            title_override: None,
        });
        self.selected = Some(id);
        Ok(())
    }

    fn add_schedule(&mut self, schedule: Id) -> Result<()> {
        os_core::ensure(
            self.parameters.schedule_placements.is_empty(),
            "one schedule per sheet",
        )?;
        let id = Id::new();
        self.parameters
            .schedule_placements
            .push(SheetSchedulePlacement {
                id,
                schedule,
                paper_rect_mm: SheetPaperRect {
                    min_mm: Point2::new(18.0, 178.0),
                    max_mm: Point2::new(402.0, 238.0),
                },
            });
        self.selected = Some(id);
        Ok(())
    }

    fn frames(&self) -> Vec<(Id, SheetPaperRect)> {
        self.parameters
            .viewports
            .iter()
            .map(|v| {
                (
                    v.id,
                    SheetPaperRect {
                        min_mm: Point2::new(
                            v.paper_center_mm.x - v.width_mm / 2.0,
                            v.paper_center_mm.y - v.height_mm / 2.0,
                        ),
                        max_mm: Point2::new(
                            v.paper_center_mm.x + v.width_mm / 2.0,
                            v.paper_center_mm.y + v.height_mm / 2.0,
                        ),
                    },
                )
            })
            .chain(
                self.parameters
                    .schedule_placements
                    .iter()
                    .map(|p| (p.id, p.paper_rect_mm)),
            )
            .collect()
    }

    fn set_frame(&mut self, id: Id, rect: SheetPaperRect) {
        if let Some(v) = self.parameters.viewports.iter_mut().find(|v| v.id == id) {
            v.paper_center_mm = Point2::new(
                (rect.min_mm.x + rect.max_mm.x) / 2.0,
                (rect.min_mm.y + rect.max_mm.y) / 2.0,
            );
            v.width_mm = rect.max_mm.x - rect.min_mm.x;
            v.height_mm = rect.max_mm.y - rect.min_mm.y;
        } else if let Some(p) = self
            .parameters
            .schedule_placements
            .iter_mut()
            .find(|p| p.id == id)
        {
            p.paper_rect_mm = rect;
        }
    }

    fn validate(&self, editor: &Editor) -> Result<()> {
        self.parameters.validate(editor.document.model())?;
        // Reading/exporting an existing single-viewport sheet retains its
        // historic full-page contract. New edits use reserved-area preflight.
        if !self.dirty() {
            return Ok(());
        }
        let frames = self.frames();
        for (i, (_, r)) in frames.iter().enumerate() {
            os_core::ensure(
                r.min_mm.x >= 8.0
                    && r.min_mm.y >= 8.0
                    && r.max_mm.x <= 412.0
                    && r.max_mm.y <= 249.0,
                "layout crosses the 8 mm paper frame or reserved title block",
            )?;
            os_core::ensure(
                !frames[..i].iter().any(|(_, b)| {
                    r.min_mm.x < b.max_mm.x
                        && r.max_mm.x > b.min_mm.x
                        && r.min_mm.y < b.max_mm.y
                        && r.max_mm.y > b.min_mm.y
                }),
                "sheet placements overlap; move or resize the selected frame",
            )?;
        }
        Ok(())
    }

    fn canvas(&mut self, response: &egui::Response, painter: &egui::Painter) {
        #[cfg(test)]
        {
            self.canvas_rect = Some(response.rect);
        }
        let Some((paper, scale)) = sheet_fit_rect(response.rect, 420.0, 297.0) else {
            return;
        };
        let screen_rect = |r: SheetPaperRect| {
            egui::Rect::from_min_max(
                paper_to_screen(r.min_mm, paper, scale),
                paper_to_screen(r.max_mm, paper, scale),
            )
        };
        let frames = self.frames();
        if response.contains_pointer() && response.ctx.input(|i| i.pointer.primary_pressed()) {
            let at = response
                .ctx
                .input(|i| i.pointer.press_origin())
                .or(response.interact_pointer_pos());
            if let Some(at) = at {
                let hit = frames
                    .iter()
                    .rev()
                    .find(|(_, r)| screen_rect(*r).expand(7.0).contains(at));
                if let Some((id, rect)) = hit {
                    self.selected = Some(*id);
                    let resize = at.distance(screen_rect(*rect).right_bottom()) <= 10.0;
                    self.drag = Some((*id, at, *rect, resize, scale));
                }
            }
        }
        if let Some((id, start, original, resize, drag_scale)) = self.drag
            && let Some(at) = response.interact_pointer_pos()
        {
            let delta = (at - start) / drag_scale;
            let mut rect = original;
            rect.max_mm.x += f64::from(delta.x);
            rect.max_mm.y += f64::from(delta.y);
            if resize {
                rect.max_mm.x = rect.max_mm.x.max(rect.min_mm.x + 1.0);
                rect.max_mm.y = rect.max_mm.y.max(rect.min_mm.y + 1.0);
            } else {
                rect.min_mm.x += f64::from(delta.x);
                rect.min_mm.y += f64::from(delta.y);
            }
            self.set_frame(id, rect);
        }
        if response.drag_stopped() || !response.ctx.input(|i| i.pointer.primary_down()) {
            self.drag = None;
        }
        for (id, rect) in self.frames() {
            let selected = self.selected == Some(id);
            let color = if selected {
                theme::ACCENT
            } else {
                egui::Color32::GRAY
            };
            let rect = screen_rect(rect);
            if rect.is_finite() && rect.is_positive() {
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(if selected { 2.0 } else { 1.0 }, color),
                    egui::StrokeKind::Inside,
                );
                if selected {
                    painter.rect_filled(
                        egui::Rect::from_center_size(rect.right_bottom(), egui::vec2(8.0, 8.0)),
                        0.0,
                        color,
                    );
                }
            }
        }
    }
}

impl DesktopApp {
    fn move_sheet_viewport(&mut self, source: Id, target: Id, viewport: Id) -> Result<()> {
        let mut from = Draft::new(&self.editor, source);
        let mut to = Draft::new(&self.editor, target);
        let index = from
            .parameters
            .viewports
            .iter()
            .position(|v| v.id == viewport)
            .ok_or_else(|| Error::Invalid("placement missing".into()))?;
        let mut placement = from.parameters.viewports.remove(index);
        if to.parameters.viewports.iter().any(|v| {
            v.detail_number
                .trim()
                .eq_ignore_ascii_case(placement.detail_number.trim())
        }) {
            placement.detail_number = (1..)
                .map(|n| n.to_string())
                .find(|n| {
                    !to.parameters
                        .viewports
                        .iter()
                        .any(|v| v.detail_number.trim().eq_ignore_ascii_case(n))
                })
                .unwrap();
        }
        to.parameters.viewports.push(placement);
        // Keep the exact paper placement. User resolves overlaps on the destination
        // first; moving never silently rearranges existing viewports.
        to.validate(&self.editor)?;
        self.editor.commands(
            "Move sheet placement",
            vec![
                Command::UpdateSheet {
                    id: source,
                    parameters: from.parameters,
                },
                Command::UpdateSheet {
                    id: target,
                    parameters: to.parameters,
                },
            ],
        )
    }
    fn apply_sheet_draft(&mut self, draft: &Draft) -> Result<()> {
        os_core::ensure(
            draft.current(&self.editor, self.plans.active_sheet),
            "sheet draft is stale",
        )?;
        draft.validate(&self.editor)?;
        self.sheet_page_for(&draft.parameters)?;
        if draft.dirty() {
            self.editor.command(
                "Arrange sheet",
                Command::UpdateSheet {
                    id: draft.sheet,
                    parameters: draft.parameters.clone(),
                },
            )?;
        }
        Ok(())
    }

    pub(super) fn sheet_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(sheet) = self
            .plans
            .active_sheet
            .filter(|id| self.editor.document.model().sheets.contains_key(id))
        else {
            return;
        };
        let mut draft = self
            .plans
            .sheet_draft
            .take()
            .filter(|d| d.current(&self.editor, Some(sheet)))
            .unwrap_or_else(|| Draft::new(&self.editor, sheet));
        let mut cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let mut apply = false;
        let mut navigate = None;
        let mut move_to_sheet = None;
        let views: Vec<_> = self
            .editor
            .document
            .model()
            .views
            .values()
            .filter(|v| {
                v.parameters.kind != os_model::ViewKind::Perspective
                    && v.parameters.validate_edit().is_ok()
            })
            .map(|v| (v.id(), v.parameters.name.clone()))
            .collect();
        if draft.source.is_none() {
            draft.source = views.first().map(|v| v.0);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} — {}",
                draft.parameters.number, draft.parameters.name
            ));
            egui::ComboBox::from_id_salt("sheet_source_picker")
                .width(130.0)
                .selected_text(
                    views
                        .iter()
                        .find(|v| Some(v.0) == draft.source)
                        .map_or("Source view", |v| v.1.as_str()),
                )
                .show_ui(ui, |ui| {
                    for (id, name) in &views {
                        ui.selectable_value(&mut draft.source, Some(*id), name);
                    }
                });
            if ui.button("Add linked view").clicked()
                && let Some(view) = draft.source
            {
                let result = draft.add_view(&self.editor, view);
                self.report(result, "Added draft viewport; existing frames unchanged.");
            }
            if ui.button("Apply layout").clicked() {
                apply = true;
            }
            cancel |= ui.button("Reset layout draft").clicked();
        });
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("sheet_placement_picker")
                .width(170.0)
                .selected_text(
                    draft
                        .selected
                        .and_then(|id| draft.parameters.viewports.iter().position(|v| v.id == id))
                        .map_or("Schedule / placement".into(), |i| {
                            format!("Viewport {}", i + 1)
                        }),
                )
                .show_ui(ui, |ui| {
                    for (i, v) in draft.parameters.viewports.iter().enumerate() {
                        let name = self
                            .editor
                            .document
                            .model()
                            .views
                            .get(&v.view)
                            .map_or("Missing view", |v| v.parameters.name.as_str());
                        ui.selectable_value(
                            &mut draft.selected,
                            Some(v.id),
                            format!("{}: {}", i + 1, name),
                        );
                    }
                    for p in &draft.parameters.schedule_placements {
                        ui.selectable_value(&mut draft.selected, Some(p.id), "Schedule table");
                    }
                });
            if ui.button("Remove placement").clicked() {
                draft
                    .parameters
                    .viewports
                    .retain(|v| Some(v.id) != draft.selected);
                draft
                    .parameters
                    .schedule_placements
                    .retain(|v| Some(v.id) != draft.selected);
                draft.selected = draft.parameters.viewports.first().map(|v| v.id);
            }
            ui.add_enabled_ui(
                !draft.dirty()
                    && draft
                        .parameters
                        .viewports
                        .iter()
                        .any(|v| Some(v.id) == draft.selected),
                |ui| {
                    ui.menu_button("Move placement to sheet", |ui| {
                        for target in self
                            .editor
                            .document
                            .model()
                            .sheets
                            .values()
                            .filter(|s| s.id() != sheet)
                        {
                            if ui.button(&target.parameters.number).clicked() {
                                move_to_sheet = Some(target.id());
                                ui.close();
                            }
                        }
                    });
                },
            );
            if let Some(v) = draft
                .parameters
                .viewports
                .iter_mut()
                .find(|v| Some(v.id) == draft.selected)
            {
                let kind = self
                    .editor
                    .document
                    .model()
                    .views
                    .get(&v.view)
                    .map_or(os_model::ViewKind::Plan, |v| v.parameters.kind);
                let label = match kind {
                    os_model::ViewKind::Section => "Edit source section",
                    os_model::ViewKind::Elevation => "Edit source elevation",
                    os_model::ViewKind::Facade => "Edit source facade",
                    os_model::ViewKind::Drafting => "Edit source drafting view",
                    _ => "Edit source plan",
                };
                if ui.button(label).clicked() {
                    navigate = Some(v.view);
                }
                ui.label("Scale 1:");
                ui.add(
                    egui::DragValue::new(&mut v.scale_denominator)
                        .speed(1.0)
                        .max_decimals(3),
                );
                ui.label("Title");
                let mut title = v.title_override.clone().unwrap_or_default();
                ui.label("Detail number");
                ui.text_edit_singleline(&mut v.detail_number);
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut title)
                            .hint_text("Source name")
                            .desired_width(120.0)
                            .char_limit(256),
                    )
                    .changed()
                {
                    v.title_override = if title.is_empty() { None } else { Some(title) };
                }
            }
        });
        if let Some(v) = draft
            .parameters
            .viewports
            .iter_mut()
            .find(|v| Some(v.id) == draft.selected)
        {
            ui.horizontal_wrapped(|ui| {
                ui.label("Paper center X/Y mm");
                for value in [&mut v.paper_center_mm.x, &mut v.paper_center_mm.y] {
                    ui.add(egui::DragValue::new(value).speed(0.5).max_decimals(2));
                }
                ui.label("Size W/H mm");
                for value in [&mut v.width_mm, &mut v.height_mm] {
                    ui.add(egui::DragValue::new(value).speed(0.5).max_decimals(2));
                }
                ui.label("Source center X/Y m");
                for value in [&mut v.model_center_m.x, &mut v.model_center_m.y] {
                    ui.add(egui::DragValue::new(value).speed(0.05).max_decimals(3));
                }
            });
        } else if let Some(p) = draft
            .parameters
            .schedule_placements
            .iter_mut()
            .find(|p| Some(p.id) == draft.selected)
        {
            ui.horizontal_wrapped(|ui| {
                ui.label("Table bounds left/top/right/bottom mm");
                for value in [
                    &mut p.paper_rect_mm.min_mm.x,
                    &mut p.paper_rect_mm.min_mm.y,
                    &mut p.paper_rect_mm.max_mm.x,
                    &mut p.paper_rect_mm.max_mm.y,
                ] {
                    ui.add(egui::DragValue::new(value).speed(0.5).max_decimals(2));
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            let schedules = &self.editor.document.model().schedules;
            if draft.schedule.is_none() {
                draft.schedule = schedules.keys().next().copied();
            }
            egui::ComboBox::from_id_salt("sheet_schedule_picker")
                .width(150.0)
                .selected_text(
                    draft
                        .schedule
                        .and_then(|id| schedules.get(&id))
                        .map_or("Saved schedule", |s| s.parameters.name.as_str()),
                )
                .show_ui(ui, |ui| {
                    for s in schedules.values() {
                        ui.selectable_value(&mut draft.schedule, Some(s.id()), &s.parameters.name);
                    }
                });
            if ui
                .add_enabled(
                    draft.schedule.is_some() && draft.parameters.schedule_placements.is_empty(),
                    egui::Button::new("Add saved schedule to sheet"),
                )
                .clicked()
                && let Some(id) = draft.schedule
            {
                let result = draft.add_schedule(id);
                self.report(result, "Added draft table; arrange to avoid overlaps.");
            }
            ui.label("PDF path");
            ui.add(
                egui::TextEdit::singleline(&mut self.plans.pdf_path)
                    .desired_width(160.0)
                    .char_limit(1024),
            );
        });
        ui.add_enabled_ui(!draft.dirty(), |ui| self.drawing_issues_ui(ui));
        let page = draft
            .validate(&self.editor)
            .and_then(|()| self.sheet_page_for(&draft.parameters));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    page.is_ok() && !draft.dirty(),
                    egui::Button::new("Export vector PDF"),
                )
                .clicked()
                && let Ok(page) = &page
            {
                self.prepare_sheet_pdf(page);
            }
            ui.label("Drag frame to move; bottom-right grip resizes. Apply before export.");
        });
        let diagnostic = page.as_ref().err().map_or(
            "Layout preview — model unchanged until Apply".into(),
            ToString::to_string,
        );
        ui.add_sized(
            [ui.available_width(), 30.0],
            egui::Label::new(egui::RichText::new(diagnostic).color(if page.is_err() {
                theme::ERROR
            } else {
                theme::MUTED
            }))
            .truncate(),
        );
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, theme::CANVAS);
        if let Ok(page) = &page {
            let _ = paint_sheet_page(&painter, response.rect, page);
        } else if let Some((rect, _)) = sheet_fit_rect(response.rect, 420.0, 297.0) {
            painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
        }
        draft.canvas(&response, &painter);
        if draft.dirty() {
            self.plans.pending_pdf = None;
        }
        if let Some(target) = move_to_sheet {
            let result = self.move_sheet_viewport(sheet, target, draft.selected.unwrap());
            self.report(result, "Placement moved; drawing references updated.");
            cancel = true;
        }
        if apply {
            let result = self.apply_sheet_draft(&draft);
            cancel |= result.is_ok();
            self.report(result, "Sheet layout applied in one transaction.");
        }
        if let Some(view) = navigate {
            self.focus_plan(Some(view));
            self.plans.active_sheet = None;
        } else if !cancel {
            self.plans.sheet_draft = Some(draft);
        }
    }
}

#[cfg(test)]
mod tests;
