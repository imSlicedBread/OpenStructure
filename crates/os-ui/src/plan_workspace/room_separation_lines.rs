//! Level-owned room boundary gestures. A claimed press stays owned after cancellation.
use super::*;
use os_model::{RoomSeparationLine, RoomSeparationLineParams};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Create,
    Select,
    Move,
    Start,
    End,
}

pub(super) struct Draft {
    context: PlanContext,
    session_id: Id,
    revision: u64,
    providers: Vec<(String, Id)>,
    drawing: Option<Id>,
    entity: Option<Id>,
    mode: Mode,
    original: Option<RoomSeparationLineParams>,
    anchor: Option<Point2>,
    origin: Option<egui::Pos2>,
    moved: bool,
    preview: Option<RoomSeparationLineParams>,
    snap_point: Option<Point2>,
}

pub(super) enum Action {
    Select(Id),
    Commit(Box<Draft>),
}

impl Draft {
    fn new(editor: &Editor, context: PlanContext) -> Self {
        Self {
            context,
            session_id: editor.document.session_id(),
            revision: editor.document.revision(),
            providers: plan_provider_signature(editor),
            drawing: None,
            entity: None,
            mode: Mode::Create,
            original: None,
            anchor: None,
            origin: None,
            moved: false,
            preview: None,
            snap_point: None,
        }
    }
    pub(super) fn stale(&self, editor: &Editor, active: Option<Id>) -> bool {
        active != Some(self.context.view_id)
            || editor.document.session_id() != self.session_id
            || editor.document.revision() != self.revision
            || editor.native_plan_context(self.context.view_id).ok() != Some(self.context)
            || plan_provider_signature(editor) != self.providers
    }
}

impl DesktopApp {
    pub(crate) fn begin_room_separation_line(&mut self) {
        let Some(context) = self
            .plans
            .active
            .and_then(|id| self.editor.native_plan_context(id).ok())
        else {
            return;
        };
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.floor_sketch = None;
        self.plans.section_placement = None;
        self.plans.room_placement_active = false;
        self.grid_draft = None;
        self.opening_draft = None;
        self.plan_draft = None;
        self.plans.room_separation_line_draft = Some(Draft::new(&self.editor, context));
        self.status = "Room Separator · click start and end; Escape cancels".into();
    }

    pub(super) fn finish_room_separation_line_action(&mut self, action: Action) {
        match action {
            Action::Select(id) => self.select(Some(id)),
            Action::Commit(draft) => {
                let repeat = draft.mode == Mode::Create;
                let result = (|| {
                    os_core::ensure(
                        !draft.stale(&self.editor, self.plans.active),
                        "room separator draft is stale",
                    )?;
                    let parameters = draft
                        .preview
                        .ok_or_else(|| Error::Invalid("invalid room separator release".into()))?;
                    parameters.validate(self.editor.document.model())?;
                    if let Some(id) = draft.entity {
                        self.editor.command(
                            "Edit room separator",
                            Command::UpdateRoomSeparationLine { id, parameters },
                        )?;
                        Ok(id)
                    } else {
                        let line = RoomSeparationLine::new("core.room_separation_line", parameters);
                        let id = line.id();
                        self.editor
                            .command("Draw room separator", Command::AddRoomSeparationLine(line))?;
                        Ok(id)
                    }
                })();
                match result {
                    Ok(id) => {
                        self.select(Some(id));
                        self.report(Ok(()), "Room separator saved.");
                        if repeat {
                            self.begin_room_separation_line();
                        }
                    }
                    Err(error) => self.report(Err(error), ""),
                }
            }
        }
    }
}

fn local(pos: egui::Pos2, rect: egui::Rect) -> Point2 {
    Point2::new(
        f64::from(pos.x - rect.left()),
        f64::from(pos.y - rect.top()),
    )
}

/// Only real endpoints inside the crop/canvas get handles; clipped ends do not.
fn handles(
    parameters: &RoomSeparationLineParams,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
) -> Vec<(Mode, egui::Pos2)> {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    [(Mode::Start, parameters.start), (Mode::End, parameters.end)]
        .into_iter()
        .filter_map(|(mode, world)| {
            let plane = context.basis.world_to_plane(world).ok()?;
            if !point_in_plan_crop(context, plane) {
                return None;
            }
            let p = camera.project(plane, size).ok()?;
            let p = rect.min + egui::vec2(p.x as f32, p.y as f32);
            rect.contains(p).then_some((mode, p))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    editor: &Editor,
    draft: &mut Option<Draft>,
    claimed: &mut bool,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    response: &egui::Response,
    ctx: &egui::Context,
    selected: Option<Id>,
    snaps: SnapOptions,
    allow_press: bool,
) -> Option<Action> {
    let rect = response.rect;
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    if draft.as_ref().is_some_and(|d| {
        d.stale(editor, Some(context.view_id))
            || d.drawing.is_some_and(|id| id != drawing.identity())
            || d.entity.is_some_and(|id| {
                !drawing
                    .room_separation_lines(context)
                    .is_ok_and(|lines| lines.iter().any(|l| l.entity == id))
            })
    }) {
        *draft = None;
    }
    if let Some(d) = draft {
        d.drawing = Some(drawing.identity());
    }
    if allow_press
        && !*claimed
        && draft.is_none()
        && response.contains_pointer()
        && ctx.input(|i| i.pointer.primary_pressed())
        && let Some(pos) = pointer.filter(|p| rect.contains(*p))
    {
        let endpoint = selected.and_then(|id| {
            let line = editor.document.model().room_separation_lines.get(&id)?;
            if Some(line.parameters.level)
                != editor.document.model().views[&context.view_id]
                    .parameters
                    .level
            {
                return None;
            }
            handles(&line.parameters, context, camera, rect)
                .into_iter()
                .find(|(_, p)| p.distance(pos) <= 10.0)
                .map(|(mode, _)| (id, mode))
        });
        let hit = endpoint.or_else(|| {
            drawing
                .pick_room_separation_line_screen(context, camera, size, local(pos, rect), 6.0)
                .ok()
                .flatten()
                .map(|id| {
                    (
                        id,
                        if selected == Some(id) {
                            Mode::Move
                        } else {
                            Mode::Select
                        },
                    )
                })
        });
        if let Some((id, mode)) = hit
            && let Some(line) = editor.document.model().room_separation_lines.get(&id)
        {
            let mut d = Draft::new(editor, context);
            d.entity = Some(id);
            d.mode = mode;
            d.original = Some(line.parameters.clone());
            d.origin = Some(pos);
            d.drawing = Some(drawing.identity());
            d.anchor = camera
                .unproject(local(pos, rect), size)
                .ok()
                .and_then(|p| context.basis.plane_to_world(p).ok());
            *draft = Some(d);
            *claimed = true;
        }
    }
    let d = draft.as_mut()?;
    if response.contains_pointer() && ctx.input(|i| i.pointer.primary_pressed()) {
        *claimed = true;
    }
    d.moved |= d
        .origin
        .zip(pointer)
        .is_some_and(|(a, b)| a.distance(b) > ctx.options(|o| o.input_options.max_click_dist));
    d.snap_point = None;
    let target = pointer.filter(|p| rect.contains(*p)).and_then(|pos| {
        let mut plane = camera.unproject(local(pos, rect), size).ok()?;
        if d.mode == Mode::Move {
            let original = d.original.as_ref()?;
            let world = context.basis.plane_to_world(plane).ok()?;
            let anchor = d.anchor?;
            plane = context
                .basis
                .world_to_plane(Point2::new(
                    original.start.x + world.x - anchor.x,
                    original.start.y + world.y - anchor.y,
                ))
                .ok()?;
        }
        let query = os_render::snapping::SnapQuery {
            camera,
            viewport: size,
            pointer: camera.project(plane, size).ok()?,
            radius_pixels: 12.0,
            endpoints: snaps.enabled && snaps.endpoints,
            midpoints: snaps.enabled && snaps.midpoints,
            intersections: snaps.enabled && snaps.intersections,
            perpendicular_from: if snaps.enabled && snaps.perpendicular {
                d.original
                    .as_ref()
                    .map(|p| {
                        if d.mode == Mode::Start {
                            p.end
                        } else {
                            p.start
                        }
                    })
                    .or(d.anchor)
                    .and_then(|p| context.basis.world_to_plane(p).ok())
            } else {
                None
            },
            nearest: snaps.enabled && snaps.nearest,
            axis_extensions: snaps.enabled && snaps.axis_extensions,
            exclude_entity: d.entity,
        };
        let candidate = drawing
            .snap(context, query)
            .ok()?
            .candidate(context, query)
            .ok()?;
        d.snap_point = candidate.as_ref().map(|c| c.point);
        let plane = candidate.map_or(plane, |c| c.point);
        if !point_in_plan_crop(context, plane) {
            return None;
        }
        context.basis.plane_to_world(plane).ok()
    });
    d.preview = target.and_then(|target| {
        let (start, end) = match d.mode {
            Mode::Create => (d.anchor?, target),
            Mode::Start => (target, d.original.as_ref()?.end),
            Mode::End => (d.original.as_ref()?.start, target),
            Mode::Move => {
                let p = d.original.as_ref()?;
                (
                    target,
                    Point2::new(
                        p.end.x + target.x - p.start.x,
                        p.end.y + target.y - p.start.y,
                    ),
                )
            }
            Mode::Select => return None,
        };
        let p = RoomSeparationLineParams {
            level: editor.document.model().views[&context.view_id]
                .parameters
                .level?,
            start,
            end,
        };
        p.validate(editor.document.model()).ok()?;
        Some(p)
    });
    if d.mode == Mode::Create && response.clicked() {
        if d.anchor.is_none() {
            d.anchor = target;
        } else {
            let d = draft.take().unwrap();
            return d.preview.is_some().then(|| Action::Commit(Box::new(d)));
        }
    } else if d.mode != Mode::Create && ctx.input(|i| i.pointer.primary_released()) {
        let d = draft.take().unwrap();
        if !d.moved || d.mode == Mode::Select {
            return d.entity.map(Action::Select);
        }
        return d.preview.is_some().then(|| Action::Commit(Box::new(d)));
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    painter: &egui::Painter,
    drawing: &PlanDrawing,
    draft: Option<&Draft>,
    editor: &Editor,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    selected: Option<Id>,
) {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let screen = |p| {
        camera
            .project(p, size)
            .ok()
            .map(|p| rect.min + egui::vec2(p.x as f32, p.y as f32))
    };
    let width = (os_render::plan::ROOM_SEPARATOR_WEIGHT_MM
        * camera.pixels_per_metre
        * context.scale_denominator
        / 1000.0)
        .clamp(1.0, 8.0) as f32;
    for line in drawing.room_separation_lines(context).unwrap_or_default() {
        if let (Some(a), Some(b)) = (screen(line.start), screen(line.end)) {
            painter.line_segment(
                [a, b],
                egui::Stroke::new(
                    width,
                    if selected == Some(line.entity) {
                        theme::ACCENT
                    } else {
                        egui::Color32::from_rgb(140, 75, 160)
                    },
                ),
            );
        }
    }
    if let Some(p) = draft.and_then(|d| d.snap_point).and_then(screen) {
        painter.circle_stroke(p, 5.0, egui::Stroke::new(1.5, theme::ACCENT));
    }
    if let Some(p) = draft.and_then(|d| d.preview.as_ref())
        && let (Ok(a), Ok(b)) = (
            context.basis.world_to_plane(p.start),
            context.basis.world_to_plane(p.end),
        )
        && let Ok(Some((a, b))) = os_render::plan::clip_room_separation_line(context, a, b)
        && let (Some(a), Some(b)) = (screen(a), screen(b))
    {
        painter.line_segment([a, b], egui::Stroke::new(2.0, theme::ACCENT));
    }
    if let Some(line) =
        selected.and_then(|id| editor.document.model().room_separation_lines.get(&id))
        && drawing
            .room_separation_lines(context)
            .is_ok_and(|lines| lines.iter().any(|l| l.entity == line.id()))
    {
        for (_, p) in handles(&line.parameters, context, camera, rect) {
            painter.circle_filled(p, 6.0, theme::ACCENT);
        }
    }
}
