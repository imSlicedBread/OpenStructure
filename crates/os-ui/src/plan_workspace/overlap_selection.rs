//! Transient keyboard cycling for native plan hits.
use super::*;

struct Preview {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    session: Id,
    revision: u64,
    drawing: Id,
    camera: PlanCamera,
    canvas: egui::Rect,
    pointer: egui::Pos2,
    hits: Vec<Id>,
    index: usize,
    filters: selection_filters::SelectionFilters,
}

pub(super) struct Frame<'a> {
    pub(super) editor: &'a Editor,
    pub(super) drawing: &'a PlanDrawing,
    pub(super) context: PlanContext,
    pub(super) camera: PlanCamera,
    pub(super) response: &'a egui::Response,
    pub(super) ctx: &'a egui::Context,
    pub(super) filters: &'a selection_filters::SelectionFilters,
    pub(super) eligible: bool,
}

pub(super) struct Click<'a> {
    pub(super) editor: &'a Editor,
    pub(super) drawing: &'a PlanDrawing,
    pub(super) context: PlanContext,
    pub(super) camera: PlanCamera,
    pub(super) canvas: egui::Rect,
    pub(super) pointer: egui::Pos2,
    pub(super) filters: &'a selection_filters::SelectionFilters,
}

#[derive(Default)]
pub(super) struct State {
    preview: Option<Preview>,
    focus_armed: bool,
}

impl State {
    pub fn clear_preview(&mut self) {
        self.preview = None;
    }

    pub fn cancel(&mut self) {
        self.preview = None;
        self.focus_armed = false;
    }

    pub fn update(&mut self, frame: Frame<'_>) -> bool {
        let Frame {
            editor,
            drawing,
            context,
            camera,
            response,
            ctx,
            filters,
            eligible,
        } = frame;
        let cycle = cycle_key(ctx);
        self.focus_armed |= response.clicked() || response.has_focus();
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.cancel();
            return false;
        }
        let clicked_elsewhere = ctx.input(|input| input.pointer.any_click()) && !response.hovered();
        if clicked_elsewhere || !eligible {
            self.cancel();
            return false;
        }
        if !response.hovered() {
            self.preview = None;
            return false;
        }
        // egui handles Tab traversal before this widget sees the key event. Keep
        // click/focus intent from the preceding frame so the first Tab cycles
        // hits instead of moving focus to an unrelated toolbar control.
        if !self.focus_armed {
            self.cancel();
            return false;
        }
        let Some(pointer) = response.hover_pos() else {
            self.cancel();
            return false;
        };
        let stale = self.preview.as_ref().is_some_and(|preview| {
            preview.context != context
                || preview.filters != *filters
                || preview.providers != plan_provider_signature(editor)
                || preview.session != editor.document.session_id()
                || preview.revision != editor.document.revision()
                || preview.drawing != drawing.identity()
                || preview.camera != camera
                || preview.canvas != response.rect
                || preview.pointer.distance(pointer) > 2.0
        });
        if stale {
            self.cancel();
            return false;
        }
        let Some(modifiers) = cycle else {
            return false;
        };
        let reverse = modifiers.shift;
        let local = Point2::new(
            f64::from(pointer.x - response.rect.left()),
            f64::from(pointer.y - response.rect.top()),
        );
        let Ok(hits) = drawing.hits_screen(
            context,
            camera,
            [
                f64::from(response.rect.width()),
                f64::from(response.rect.height()),
            ],
            local,
            6.0,
        ) else {
            self.cancel();
            return false;
        };
        let hits: Vec<_> = hits
            .into_iter()
            .filter(|id| filters.allows(editor.document.model(), *id))
            .collect();
        if hits.len() < 2 {
            self.cancel();
            return false;
        }
        response.request_focus();
        ctx.input_mut(|input| {
            input.consume_key(modifiers, egui::Key::Tab);
        });
        let old_index = self
            .preview
            .as_ref()
            .filter(|preview| preview.hits == hits)
            .map(|preview| preview.index);
        let index = match (old_index, reverse) {
            (None, false) => 1 % hits.len(),
            (None, true) => hits.len() - 1,
            (Some(index), false) => (index + 1) % hits.len(),
            (Some(index), true) => (index + hits.len() - 1) % hits.len(),
        };
        self.preview = Some(Preview {
            context,
            providers: plan_provider_signature(editor),
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            drawing: drawing.identity(),
            camera,
            canvas: response.rect,
            pointer,
            hits,
            index,
            filters: filters.clone(),
        });
        true
    }

    pub fn take_for_click(&mut self, click: Click<'_>) -> Option<Id> {
        let Click {
            editor,
            drawing,
            context,
            camera,
            canvas,
            pointer,
            filters,
        } = click;
        let preview = self.preview.take()?;
        if preview.context != context
            || preview.filters != *filters
            || preview.providers != plan_provider_signature(editor)
            || preview.session != editor.document.session_id()
            || preview.revision != editor.document.revision()
            || preview.drawing != drawing.identity()
            || preview.camera != camera
            || preview.canvas != canvas
            || preview.pointer.distance(pointer) > 2.0
        {
            return None;
        }
        let local = Point2::new(
            f64::from(pointer.x - canvas.left()),
            f64::from(pointer.y - canvas.top()),
        );
        let fresh = drawing
            .hits_screen(
                context,
                camera,
                [f64::from(canvas.width()), f64::from(canvas.height())],
                local,
                6.0,
            )
            .ok()?
            .into_iter()
            .filter(|id| filters.allows(editor.document.model(), *id))
            .collect::<Vec<_>>();
        let candidate = *preview.hits.get(preview.index)?;
        fresh.contains(&candidate).then_some(candidate)
    }

    pub fn paint(
        &self,
        painter: &egui::Painter,
        drawing: &PlanDrawing,
        context: PlanContext,
        camera: PlanCamera,
        model: &os_model::Model,
    ) {
        let Some(preview) = &self.preview else {
            return;
        };
        let Some(id) = preview.hits.get(preview.index).copied() else {
            return;
        };
        let canvas = preview.canvas;
        let viewport = [f64::from(canvas.width()), f64::from(canvas.height())];
        let project = |point: Point2| {
            camera
                .project(point, viewport)
                .ok()
                .map(|point| canvas.min + egui::vec2(point.x as f32, point.y as f32))
        };
        let stroke = egui::Stroke::new(2.5, theme::ACCENT);
        let line = |a: Point2, b: Point2| {
            if let (Some(a), Some(b)) = (project(a), project(b)) {
                painter.line_segment([a, b], stroke);
            }
        };
        if let Ok(items) = drawing.items(context) {
            for item in items.iter().filter(|item| item.entity == id) {
                let points: Vec<_> = item
                    .footprint
                    .vertices()
                    .iter()
                    .copied()
                    .filter_map(project)
                    .collect();
                for edge in points.windows(2) {
                    painter.line_segment([edge[0], edge[1]], stroke);
                }
                if points.len() > 2 {
                    painter.line_segment([*points.last().unwrap(), points[0]], stroke);
                }
            }
        }
        if let Ok(items) = drawing.columns(context) {
            for item in items.iter().filter(|item| item.entity == id) {
                let points: Vec<_> = item
                    .footprint
                    .vertices()
                    .iter()
                    .copied()
                    .filter_map(project)
                    .collect();
                for edge in points.windows(2) {
                    painter.line_segment([edge[0], edge[1]], stroke);
                }
                if points.len() > 2 {
                    painter.line_segment([*points.last().unwrap(), points[0]], stroke);
                }
            }
        }
        if let Ok(lines) = drawing.provider_lines(context) {
            for item in lines.iter().filter(|item| item.entity == id) {
                line(item.start, item.end);
            }
        }
        if let Ok(lines) = drawing.detail_lines(context) {
            for item in lines.iter().filter(|item| item.entity == id) {
                line(item.start, item.end);
            }
        }
        if let Ok(dimensions) = drawing.dimensions(context) {
            for dimension in dimensions.iter().filter(|dimension| dimension.entity == id) {
                for graphic in dimension.graphics() {
                    for (start, end) in graphic.lines() {
                        line(start, end);
                    }
                }
            }
        }
        if let Ok(grids) = drawing.grids(context) {
            for grid in grids.iter().filter(|grid| grid.entity == id) {
                line(grid.start, grid.end);
            }
        }
        if let Ok(rooms) = drawing.rooms(context) {
            for room in rooms.iter().filter(|room| room.entity == id) {
                for (a, b) in room
                    .boundary
                    .iter()
                    .copied()
                    .zip(room.boundary.iter().copied().cycle().skip(1))
                {
                    line(a, b);
                }
            }
        }
        for floor in drawing
            .floors(context)
            .into_iter()
            .flatten()
            .chain(drawing.ceilings(context).into_iter().flatten())
            .filter(|floor| floor.entity == id)
        {
            for (a, b) in floor
                .boundary
                .iter()
                .copied()
                .zip(floor.boundary.iter().copied().cycle().skip(1))
            {
                line(a, b);
            }
        }
        if let Ok(tags) = drawing.opening_tags(context)
            && let Some(bounds) = tags
                .iter()
                .find(|tag| tag.entity == id)
                .and_then(|tag| tag.bounds(context, camera, viewport).ok().flatten())
        {
            let to_canvas = |point: Point2| canvas.min + egui::vec2(point.x as f32, point.y as f32);
            painter.rect_stroke(
                egui::Rect::from_two_pos(to_canvas(bounds[0]), to_canvas(bounds[1])),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        if let Ok(tags) = drawing.room_tags(context)
            && let Some(bounds) = tags
                .iter()
                .find(|tag| tag.entity == id)
                .and_then(|tag| tag.bounds(context, camera, viewport).ok().flatten())
        {
            let to_canvas = |point: Point2| canvas.min + egui::vec2(point.x as f32, point.y as f32);
            painter.rect_stroke(
                egui::Rect::from_two_pos(to_canvas(bounds[0]), to_canvas(bounds[1])),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        let label = format!(
            "{} of {} · {}",
            preview.index + 1,
            preview.hits.len(),
            label_for(model, id)
        );
        let width = (label.chars().count() as f32 * 7.0 + 12.0).min(canvas.width());
        let pos = egui::pos2(
            (preview.pointer.x + 12.0).min(canvas.right() - width),
            (preview.pointer.y + 12.0).min(canvas.bottom() - 24.0),
        );
        painter.circle_stroke(preview.pointer, 9.0, stroke);
        painter.rect_filled(
            egui::Rect::from_min_size(pos, egui::vec2(width, 22.0)),
            3.0,
            theme::SURFACE,
        );
        painter.text(
            pos + egui::vec2(6.0, 3.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(12.0),
            theme::ACCENT,
        );
    }
}

fn cycle_key(ctx: &egui::Context) -> Option<egui::Modifiers> {
    ctx.input(|input| {
        input.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.ctrl && !modifiers.alt && !modifiers.command => Some(*modifiers),
            _ => None,
        })
    })
}

fn label_for(model: &os_model::Model, id: Id) -> &'static str {
    if model.walls.contains_key(&id) {
        "Wall"
    } else if let Some(opening) = model.openings.get(&id) {
        match model
            .resolve_opening(&opening.parameters)
            .map(|opening| opening.kind)
        {
            Ok(OpeningKind::Door) => "Door",
            Ok(OpeningKind::Window) => "Window",
            Err(_) => "Opening",
        }
    } else if model.columns.contains_key(&id) {
        "Column"
    } else if model.floors.contains_key(&id) {
        "Floor"
    } else if model.ceilings.contains_key(&id) {
        "Ceiling"
    } else if model.rooms.contains_key(&id) {
        "Room"
    } else if model.opening_tags.contains_key(&id) {
        "Opening tag"
    } else if model.room_tags.contains_key(&id) {
        "Room tag"
    } else if model.dimensions.contains_key(&id) {
        "Dimension"
    } else if model.detail_lines.contains_key(&id) {
        "Detail line"
    } else {
        "Plan object"
    }
}

impl DesktopApp {
    pub(crate) fn cancel_overlap_selection(&mut self) {
        self.plans.overlap_selection.clear_preview();
    }
}
