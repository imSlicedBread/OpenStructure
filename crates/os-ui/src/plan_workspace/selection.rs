//! Transient native footprint selection. No document commands are submitted.
use super::*;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct State {
    pub claimed: bool,
    draft: Option<Draft>,
}

struct Draft {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    session: Id,
    revision: u64,
    native_identity: Id,
    displayed_identity: Id,
    camera: PlanCamera,
    canvas: egui::Rect,
    origin: egui::Pos2,
    pointer: egui::Pos2,
    moved: bool,
    filters: selection_filters::SelectionFilters,
}

impl State {
    pub fn cancel(&mut self) {
        self.draft = None; // Keep ownership until button-up, including canceled clicks.
    }

    #[allow(clippy::too_many_arguments)]
    pub fn input(
        &mut self,
        editor: &Editor,
        drawing: &PlanDrawing,
        context: PlanContext,
        camera: PlanCamera,
        response: &egui::Response,
        ctx: &egui::Context,
        eligible: bool,
        native_identity: Id,
        filters: &selection_filters::SelectionFilters,
    ) -> Option<BTreeSet<Id>> {
        if self.draft.as_ref().is_some_and(|d| {
            !eligible
                || d.filters != *filters
                || d.displayed_identity != drawing.identity()
                || d.context != context
                || d.camera != camera
                || d.canvas != response.rect
                || d.session != editor.document.session_id()
                || d.revision != editor.document.revision()
        }) {
            self.cancel();
        }
        if eligible
            && !self.claimed
            && response.contains_pointer()
            && ctx.input(|i| i.pointer.primary_pressed())
            && ctx.input(|i| {
                i.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::PointerButton {
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers,
                            ..
                        } if modifiers.shift
                    )
                })
            })
            && let Some(origin) = ctx.input(|i| i.pointer.press_origin())
            && response.rect.contains(origin)
        {
            self.claimed = true;
            self.draft = Some(Draft {
                context,
                providers: plan_provider_signature(editor),
                session: editor.document.session_id(),
                revision: editor.document.revision(),
                native_identity,
                displayed_identity: drawing.identity(),
                camera,
                canvas: response.rect,
                origin,
                pointer: origin,
                moved: false,
                filters: filters.clone(),
            });
        }
        let d = self.draft.as_mut()?;
        let Some(pointer) = ctx.input(|i| i.pointer.interact_pos()) else {
            self.cancel();
            return None;
        };
        d.pointer = pointer;
        d.moved |= pointer.distance(d.origin) > ctx.options(|o| o.input_options.max_click_dist);
        if !ctx.input(|i| i.pointer.primary_released()) {
            return None;
        }
        let d = self.draft.take().expect("active area draft");
        if !response.rect.contains(pointer) || !response.contains_pointer() {
            return None;
        }
        if !d.moved {
            self.claimed = false; // Let the existing single-pick path handle this release.
            return None;
        }
        Some(
            candidates(editor, drawing, context, camera, response.rect)
                .into_iter()
                .filter_map(|(id, polygons)| {
                    (d.filters.allows(editor.document.model(), id)
                        && matches_area(&polygons, d.origin, pointer))
                    .then_some(id)
                })
                .collect(),
        )
    }

    pub fn paint(&self, painter: &egui::Painter) {
        if let Some(d) = &self.draft
            && d.moved
        {
            let rect = egui::Rect::from_two_pos(d.origin, d.pointer).intersect(d.canvas);
            let color = if d.pointer.x >= d.origin.x {
                theme::ACCENT
            } else {
                egui::Color32::from_rgb(65, 190, 135)
            };
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.12));
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.5, color),
                egui::StrokeKind::Inside,
            );
        }
    }
}

impl DesktopApp {
    pub(super) fn select_area(&mut self, ids: BTreeSet<Id>) {
        // Property editors have a target only for a single-element selection.
        self.select((ids.len() == 1).then(|| *ids.first().expect("one selected item")));
        self.selected_ids = ids;
    }

    pub(super) fn validate_area_selection(&mut self, ctx: &egui::Context) {
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let stale = self.plans.area_selection.draft.as_ref().is_some_and(|d| {
            self.plans.active != Some(d.context.view_id)
                || self.plans.active_sheet.is_some()
                || self.editor.native_plan_context(d.context.view_id).ok() != Some(d.context)
                || plan_provider_signature(&self.editor) != d.providers
                || self.editor.document.session_id() != d.session
                || self.editor.document.revision() != d.revision
                || self.plans.drawing.as_ref().map(PlanDrawing::identity) != Some(d.native_identity)
                || self.plans.cameras.get(&d.context.view_id) != Some(&d.camera)
        });
        if escape
            || stale
            || ctx.input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::PointerGone))
                    || (!i.pointer.primary_down() && !i.pointer.primary_released())
            })
        {
            self.plans.area_selection.cancel();
        }
    }

    pub(crate) fn finish_area_selection_input(&mut self, ctx: &egui::Context) {
        // Also runs when the canvas cannot render or the user has left the plan.
        self.validate_area_selection(ctx);
        if self.plans.active_sheet.is_some() || self.plans.active.is_none() {
            self.plans.overlap_selection.cancel();
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.plans.area_selection.cancel();
            self.plans.area_selection.claimed = false;
        }
    }
}

type Polygons = BTreeMap<Id, Vec<Vec<egui::Pos2>>>;

fn candidates(
    editor: &Editor,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
) -> Polygons {
    let size = [f64::from(canvas.width()), f64::from(canvas.height())];
    let project = |p| {
        camera.project(p, size).ok().and_then(|p| {
            (p.x.is_finite() && p.y.is_finite() && p.x.abs() < 1e8 && p.y.abs() < 1e8)
                .then(|| canvas.min + egui::vec2(p.x as f32, p.y as f32))
        })
    };
    let mut clip = canvas;
    if let Some(crop) = context.crop {
        let (Some(a), Some(b)) = (project(crop.min), project(crop.max)) else {
            return BTreeMap::new();
        };
        clip = clip.intersect(egui::Rect::from_two_pos(a, b));
    }
    let mut out = BTreeMap::<Id, Vec<Vec<egui::Pos2>>>::new();
    if !clip.is_positive() {
        return out;
    }
    let model = editor.document.model();
    let unavailable = drawing.unavailable(context).unwrap_or_default();
    let mut add = |id, points: &[Point2]| {
        if unavailable.contains(&id) {
            return;
        }
        let Some(screen) = points
            .iter()
            .copied()
            .map(project)
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        if screen.len() == 2 {
            if let Some((start, end)) = clip_segment_to_rect(screen[0], screen[1], clip) {
                out.entry(id).or_default().push(vec![start, end]);
            }
            return;
        }
        let polygon = clip_polygon(screen, clip);
        if polygon.len() >= 3 {
            out.entry(id).or_default().push(polygon);
        }
    };
    for item in drawing
        .items(context)
        .unwrap_or_default()
        .iter()
        .chain(drawing.columns(context).unwrap_or_default())
        .chain(drawing.casework(context).unwrap_or_default())
    {
        if model.walls.contains_key(&item.entity)
            || model.openings.contains_key(&item.entity)
            || model.columns.contains_key(&item.entity)
            || model.casework.contains_key(&item.entity)
        {
            add(item.entity, item.footprint.vertices());
        }
    }
    // Native doors and windows are semantic line groups, not filled PlanItems.
    // Keep only their checked, already crop-clipped symbol segments.
    for line in drawing.provider_lines(context).unwrap_or_default() {
        if drawing.is_native_line(line.entity) && model.openings.contains_key(&line.entity) {
            add(line.entity, &[line.start, line.end]);
        }
    }
    for floor in drawing
        .floors(context)
        .unwrap_or_default()
        .iter()
        .chain(drawing.ceilings(context).unwrap_or_default())
    {
        if !model.floors.contains_key(&floor.entity) && !model.ceilings.contains_key(&floor.entity)
        {
            continue;
        }
        for triangle in &floor.triangles {
            add(floor.entity, &triangle.map(|i| floor.vertices[i as usize]));
        }
    }
    for room in drawing.rooms(context).unwrap_or_default() {
        if !model.rooms.contains_key(&room.entity) || room.diagnostic.is_some() {
            continue;
        }
        if let Ok(triangles) = os_geometry::floors::triangulate_floor(&room.boundary) {
            for triangle in triangles {
                add(room.entity, &triangle.map(|i| room.boundary[i as usize]));
            }
        }
    }
    out
}

fn matches_area(polygons: &[Vec<egui::Pos2>], origin: egui::Pos2, end: egui::Pos2) -> bool {
    let area = egui::Rect::from_two_pos(origin, end);
    if end.x >= origin.x {
        !polygons.is_empty() && polygons.iter().flatten().all(|p| area.contains(*p))
    } else {
        polygons.iter().any(|shape| match shape.len() {
            2 => clip_segment_to_rect(shape[0], shape[1], area).is_some(),
            _ => !clip_polygon(shape.clone(), area).is_empty(),
        })
    }
}

fn clip_segment_to_rect(
    start: egui::Pos2,
    end: egui::Pos2,
    rect: egui::Rect,
) -> Option<(egui::Pos2, egui::Pos2)> {
    let delta = end - start;
    let mut lower = 0.0_f32;
    let mut upper = 1.0_f32;
    for (p, q) in [
        (-delta.x, start.x - rect.left()),
        (delta.x, rect.right() - start.x),
        (-delta.y, start.y - rect.top()),
        (delta.y, rect.bottom() - start.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let ratio = q / p;
        if p < 0.0 {
            if ratio > upper {
                return None;
            }
            lower = lower.max(ratio);
        } else {
            if ratio < lower {
                return None;
            }
            upper = upper.min(ratio);
        }
    }
    (lower <= upper).then_some((start + delta * lower, start + delta * upper))
}

// Sutherland–Hodgman rectangle clipping. Concave regions use checked triangles,
// so disconnected pieces and slab holes do not become a bounding-box hit.
fn clip_polygon(mut points: Vec<egui::Pos2>, rect: egui::Rect) -> Vec<egui::Pos2> {
    for (axis, bound, lower) in [
        (0, rect.left(), true),
        (0, rect.right(), false),
        (1, rect.top(), true),
        (1, rect.bottom(), false),
    ] {
        let input = std::mem::take(&mut points);
        let Some(mut previous) = input.last().copied() else {
            break;
        };
        let inside = |p: egui::Pos2| {
            if lower {
                p[axis] >= bound
            } else {
                p[axis] <= bound
            }
        };
        for current in input {
            if inside(previous) != inside(current) {
                let t = (bound - previous[axis]) / (current[axis] - previous[axis]);
                points.push(previous + (current - previous) * t);
            }
            if inside(current) {
                points.push(current);
            }
            previous = current;
        }
    }
    points
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
