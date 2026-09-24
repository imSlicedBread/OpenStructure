//! Graphical editing of an existing crop. Drawing geometry always uses the saved context.
use super::*;
use os_model::PlanViewCrop;

#[derive(Default)]
pub(super) struct State {
    pub mode: Option<Id>,
    pub claimed: bool,
    drag: Option<Drag>,
}
struct Drag {
    context: PlanContext,
    session: Id,
    revision: u64,
    providers: Vec<(String, Id)>,
    drawing: Id,
    original: PlanViewCrop,
    origin: Point2,
    handle: usize,
    candidate: Option<PlanViewCrop>,
}

pub(super) fn handles(c: PlanViewCrop) -> [Point2; 8] {
    let x = c.min.x + (c.max.x - c.min.x) * 0.5;
    let y = c.min.y + (c.max.y - c.min.y) * 0.5;
    [
        c.min,
        Point2::new(x, c.min.y),
        Point2::new(c.max.x, c.min.y),
        Point2::new(c.max.x, y),
        c.max,
        Point2::new(x, c.max.y),
        Point2::new(c.min.x, c.max.y),
        Point2::new(c.min.x, y),
    ]
}
fn changed(mut c: PlanViewCrop, handle: usize, delta: Point2) -> PlanViewCrop {
    if [0, 6, 7].contains(&handle) {
        c.min.x += delta.x;
    }
    if [2, 3, 4].contains(&handle) {
        c.max.x += delta.x;
    }
    if [0, 1, 2].contains(&handle) {
        c.min.y += delta.y;
    }
    if [4, 5, 6].contains(&handle) {
        c.max.y += delta.y;
    }
    c
}
impl State {
    pub fn cancel(&mut self) {
        self.drag = None;
    }
    pub fn validate(
        &mut self,
        editor: &Editor,
        active: Option<Id>,
        drawing: Option<Id>,
        cancel: bool,
    ) {
        let crop_available = active
            .and_then(|id| editor.document.model().views.get(&id))
            .and_then(|view| view.parameters.plan)
            .is_some_and(|settings| settings.crop.is_some());
        if cancel
            || self.mode != active
            || !crop_available
            || self.drag.as_ref().is_some_and(|d| {
                editor.document.session_id() != d.session
                    || editor.document.revision() != d.revision
                    || editor.native_plan_context(d.context.view_id).ok() != Some(d.context)
                    || plan_provider_signature(editor) != d.providers
                    || drawing.is_some_and(|id| id != d.drawing)
            })
        {
            self.cancel();
        }
        if cancel || self.mode != active || !crop_available {
            self.mode = None;
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn input(
        &mut self,
        editor: &Editor,
        context: PlanContext,
        drawing: Id,
        camera: PlanCamera,
        response: &egui::Response,
        ctx: &egui::Context,
    ) -> Option<PlanViewCrop> {
        let saved = editor.document.model().views[&context.view_id]
            .parameters
            .plan?
            .crop?;
        if self.mode != Some(context.view_id) {
            self.cancel();
            return None;
        }
        let rect = response.rect;
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let project = |p| {
            camera
                .project(p, size)
                .ok()
                .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
        };
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let plane = pointer.filter(|p| rect.contains(*p)).and_then(|p| {
            camera
                .unproject(
                    Point2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top())),
                    size,
                )
                .ok()
        });
        let hit = pointer.and_then(|p| {
            handles(saved)
                .iter()
                .enumerate()
                .filter_map(|(i, h)| {
                    project(*h)
                        .filter(|h| rect.contains(*h) && h.distance(p) <= 10.0)
                        .map(|h| (i, h.distance(p)))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|v| v.0)
        });
        if !self.claimed
            && response.contains_pointer()
            && ctx.input(|i| i.pointer.primary_pressed())
            && let (Some(handle), Some(origin)) = (hit, plane)
        {
            self.claimed = true;
            self.drag = Some(Drag {
                context,
                session: editor.document.session_id(),
                revision: editor.document.revision(),
                providers: plan_provider_signature(editor),
                drawing,
                original: saved,
                origin,
                handle,
                candidate: Some(saved),
            });
        }
        if let Some(d) = &mut self.drag {
            d.candidate = plane.map(|p| {
                changed(
                    d.original,
                    d.handle,
                    Point2::new(p.x - d.origin.x, p.y - d.origin.y),
                )
            });
        }
        if ctx.input(|i| i.pointer.primary_released())
            && let Some(d) = self.drag.take()
        {
            let c = d.candidate?;
            let mut settings = editor.document.model().views[&context.view_id]
                .parameters
                .plan?;
            settings.crop = Some(c);
            return (settings.validate().is_ok() && c != d.original).then_some(c);
        }
        None
    }

    pub fn paint(
        &self,
        editor: &Editor,
        context: PlanContext,
        camera: PlanCamera,
        response: &egui::Response,
        ctx: &egui::Context,
        painter: &egui::Painter,
    ) {
        if self.mode != Some(context.view_id) {
            return;
        }
        let Some(saved) = editor
            .document
            .model()
            .views
            .get(&context.view_id)
            .and_then(|view| view.parameters.plan)
            .and_then(|settings| settings.crop)
        else {
            return;
        };
        let rect = response.rect;
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let project = |p| {
            camera
                .project(p, size)
                .ok()
                .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
        };
        let pointer = response
            .contains_pointer()
            .then(|| ctx.input(|i| i.pointer.interact_pos()))
            .flatten();
        let hit = pointer.and_then(|p| {
            handles(saved)
                .iter()
                .enumerate()
                .filter_map(|(i, h)| {
                    project(*h)
                        .filter(|h| rect.contains(*h) && h.distance(p) <= 10.0)
                        .map(|h| (i, h.distance(p)))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|v| v.0)
        });
        let candidate = self
            .drag
            .as_ref()
            .and_then(|drag| drag.candidate)
            .unwrap_or(saved);
        let points = handles(candidate);
        for (a, b) in [(0, 2), (2, 4), (4, 6), (6, 0)] {
            if let (Some(a), Some(b)) = (project(points[a]), project(points[b])) {
                painter.line_segment([a, b], egui::Stroke::new(1.5, theme::ACCENT));
            }
        }
        for (i, p) in points.iter().enumerate() {
            if let Some(p) = project(*p) {
                let active = self.drag.as_ref().is_some_and(|d| d.handle == i) || hit == Some(i);
                painter.circle_filled(
                    p,
                    6.0,
                    if active {
                        theme::ACCENT
                    } else {
                        theme::SURFACE
                    },
                );
                painter.circle_stroke(p, 6.0, egui::Stroke::new(1.0, theme::ACCENT));
            }
        }
    }
}

impl DesktopApp {
    pub(super) fn toggle_crop_edit(&mut self) {
        if self.plans.crop.mode.is_some() {
            self.plans.crop.mode = None;
            self.plans.crop.cancel();
            return;
        }
        let view = self.plans.active.filter(|id| {
            self.editor
                .document
                .model()
                .views
                .get(id)
                .and_then(|v| v.parameters.plan)
                .is_some_and(|s| s.crop.is_some())
        });
        if view.is_none() {
            return;
        }
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.room_tag_draft = None;
        self.plans.room_placement_active = false;
        self.plans.floor_sketch = None;
        self.grid_draft = None;
        self.opening_draft = None;
        self.plans.crop.mode = view;
    }
}
