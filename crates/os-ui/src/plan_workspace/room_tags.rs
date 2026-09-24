//! Room-tag interaction owns a press through release, including after cancellation.
use super::*;
use os_model::{RoomTag, RoomTagParams};
use os_render::plan::PlanRoomTag;

#[derive(Clone)]
pub(super) struct Draft {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    session: Id,
    revision: u64,
    drawing: Option<Id>,
    room: Option<Id>,
    moving: Option<Id>,
    origin: Option<egui::Pos2>,
    original: Option<Point2>,
    moved: bool,
    preview: Option<Point2>,
}

pub(super) enum Action {
    Select(Id),
    Commit(Box<Draft>, Point2),
}

impl Draft {
    fn new(editor: &Editor, context: PlanContext, room: Option<Id>) -> Self {
        Self {
            context,
            providers: plan_provider_signature(editor),
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            drawing: None,
            room,
            moving: None,
            origin: None,
            original: None,
            moved: false,
            preview: None,
        }
    }
    pub(super) fn stale(&self, editor: &Editor, active: Option<Id>) -> bool {
        active != Some(self.context.view_id)
            || editor.native_plan_context(self.context.view_id).ok() != Some(self.context)
            || plan_provider_signature(editor) != self.providers
            || editor.document.session_id() != self.session
            || editor.document.revision() != self.revision
    }
}

impl DesktopApp {
    pub(crate) fn begin_room_tag(&mut self) {
        let Some(view) = self.plans.active else {
            return;
        };
        let Ok(context) = self.editor.native_plan_context(view) else {
            return;
        };
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.floor_sketch = None;
        self.plans.room_placement_active = false;
        self.grid_draft = None;
        self.opening_draft = None;
        self.plan_draft = None;
        let room = self.selected.filter(|id| {
            self.editor
                .document
                .model()
                .rooms
                .get(id)
                .is_some_and(|room| {
                    Some(room.parameters.level)
                        == self.editor.document.model().views[&view].parameters.level
                })
        });
        self.plans.room_tag_draft = Some(Draft::new(&self.editor, context, room));
        self.status = if room.is_some() {
            "Room Tag · click a position in the plan"
        } else {
            "Room Tag · click a room face, then its label position"
        }
        .into();
    }

    pub(crate) fn apply_room_tag_properties(&mut self) {
        let Some(tag) = self
            .selected
            .and_then(|id| self.editor.document.model().room_tags.get(&id))
            .cloned()
        else {
            return;
        };
        let mut parameters = tag.parameters;
        parameters.position = self.room_tag_position;
        let crop_check = self
            .editor
            .native_plan_context(parameters.view)
            .and_then(|context| {
                os_core::ensure(
                    point_in_plan_crop(context, context.basis.world_to_plane(parameters.position)?),
                    "Room tag position is outside its view crop",
                )
            });
        if let Err(error) = crop_check {
            self.report(Err(error), "");
            return;
        }
        let result = self.editor.command(
            "Move room tag",
            Command::UpdateRoomTag {
                id: tag.header.id,
                parameters,
            },
        );
        self.report(result, "Room tag position updated.");
    }

    pub(super) fn finish_room_tag_action(&mut self, action: Action) {
        match action {
            Action::Select(id) => self.select(Some(id)),
            Action::Commit(draft, position) => {
                let result = (|| {
                    os_core::ensure(
                        !draft.stale(&self.editor, self.plans.active),
                        "room tag draft is stale",
                    )?;
                    let context = draft.context;
                    os_core::ensure(
                        point_in_plan_crop(context, context.basis.world_to_plane(position)?),
                        "room tag position outside crop",
                    )?;
                    let parameters = RoomTagParams {
                        view: context.view_id,
                        room: draft
                            .room
                            .ok_or_else(|| Error::Invalid("choose a room".into()))?,
                        position,
                    };
                    if draft.moving.is_some() {
                        // Existing orphan tags remain movable; only new tags need a live room.
                        parameters.validate(self.editor.document.model())?;
                    } else {
                        parameters.validate_creation(self.editor.document.model())?;
                    }
                    os_core::ensure(
                        !self.editor.document.model().room_tags.values().any(|tag| {
                            Some(tag.id()) != draft.moving
                                && tag.parameters.view == parameters.view
                                && tag.parameters.room == parameters.room
                        }),
                        "room already tagged in this view",
                    )?;
                    let id = if let Some(id) = draft.moving {
                        self.editor
                            .command("Move room tag", Command::UpdateRoomTag { id, parameters })?;
                        id
                    } else {
                        let tag = RoomTag::new("core.room_tag", parameters);
                        let id = tag.id();
                        self.editor
                            .command("Place room tag", Command::AddRoomTag(tag))?;
                        id
                    };
                    Ok(id)
                })();
                match result {
                    Ok(id) => {
                        self.plans.room_tag_draft = None;
                        self.select(Some(id));
                        self.report(Ok(()), "Room tag saved.");
                    }
                    Err(error) => self.report(Err(error), ""),
                }
            }
        }
    }
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
    allow_press: bool,
) -> Option<Action> {
    let rect = response.rect;
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let plane_at = |pos: egui::Pos2| {
        camera.unproject(
            Point2::new(
                f64::from(pos.x - rect.left()),
                f64::from(pos.y - rect.top()),
            ),
            size,
        )
    };
    if draft
        .as_ref()
        .is_some_and(|d| d.drawing.is_some_and(|id| id != drawing.identity()))
    {
        *draft = None;
    }
    if let Some(d) = draft {
        d.drawing = Some(drawing.identity());
    }
    if allow_press
        && draft.is_none()
        && !*claimed
        && response.contains_pointer()
        && ctx.input(|i| i.pointer.primary_pressed())
        && let Some(pos) = pointer.filter(|p| rect.contains(*p))
        && let Ok(Some(id)) = drawing.pick_room_tag_screen(
            context,
            camera,
            size,
            Point2::new(
                f64::from(pos.x - rect.left()),
                f64::from(pos.y - rect.top()),
            ),
        )
        && let Some(tag) = editor.document.model().room_tags.get(&id)
    {
        let mut d = Draft::new(editor, context, Some(tag.parameters.room));
        d.drawing = Some(drawing.identity());
        d.moving = Some(id);
        d.origin = Some(pos);
        d.original = Some(tag.parameters.position);
        d.preview = d.original;
        *draft = Some(d);
        *claimed = true;
    }
    let d = draft.as_mut()?;
    if let Some(origin) = d.origin {
        d.moved |= pointer
            .is_some_and(|p| p.distance(origin) > ctx.options(|o| o.input_options.max_click_dist));
    }
    d.preview = pointer.filter(|p| rect.contains(*p)).and_then(|pos| {
        let plane = plane_at(pos).ok()?;
        let world = context.basis.plane_to_world(plane).ok()?;
        let target = if let (Some(origin), Some(original)) = (d.origin, d.original) {
            let start = context.basis.plane_to_world(plane_at(origin).ok()?).ok()?;
            Point2::new(
                original.x + world.x - start.x,
                original.y + world.y - start.y,
            )
        } else {
            world
        };
        point_in_plan_crop(context, context.basis.world_to_plane(target).ok()?).then_some(target)
    });
    if let Some(id) = d.moving {
        if ctx.input(|i| i.pointer.primary_released()) {
            let d = draft.take().unwrap();
            return if !d.moved {
                Some(Action::Select(id))
            } else {
                d.preview.map(|p| Action::Commit(Box::new(d), p))
            };
        }
    } else if response.clicked()
        && let Some(pos) = pointer
    {
        if d.room.is_none() {
            d.room = plane_at(pos)
                .ok()
                .filter(|p| point_in_plan_crop(context, *p))
                .and_then(|p| drawing.pick_room(context, p).ok().flatten());
        } else if let Some(p) = d.preview {
            return Some(Action::Commit(Box::new(d.clone()), p));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    painter: &egui::Painter,
    tags: &[PlanRoomTag],
    draft: Option<&Draft>,
    editor: &Editor,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    selected: Option<Id>,
    pointer: Option<egui::Pos2>,
) {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let preview = draft.and_then(|d| {
        let position = context.basis.world_to_plane(d.preview?).ok()?;
        if let Some(id) = d.moving {
            let mut tag = tags.iter().find(|tag| tag.entity == id)?.clone();
            tag.anchor = position;
            return Some(tag);
        }
        let room = editor.document.model().rooms.get(&d.room?)?;
        Some(PlanRoomTag {
            entity: room.id(),
            view: context.view_id,
            room: room.id(),
            anchor: position,
            label: format!("{} · {}", room.parameters.number, room.parameters.name),
            diagnostic: None,
        })
    });
    for tag in tags
        .iter()
        .filter(|tag| preview.as_ref().is_none_or(|p| p.entity != tag.entity))
        .chain(preview.iter())
    {
        let Ok(Some([a, b])) = tag.bounds(context, camera, size) else {
            continue;
        };
        let bounds = egui::Rect::from_min_max(
            rect.min + egui::vec2(a.x as f32, a.y as f32),
            rect.min + egui::vec2(b.x as f32, b.y as f32),
        );
        let active = selected == Some(tag.entity)
            || preview.as_ref().is_some_and(|p| p.entity == tag.entity)
            || pointer.is_some_and(|p| bounds.contains(p));
        let color = if tag.diagnostic.is_some() {
            theme::ERROR
        } else if active {
            theme::ACCENT
        } else {
            theme::TEXT
        };
        painter.rect_filled(bounds, 3.0, theme::SURFACE);
        painter.rect_stroke(
            bounds,
            3.0,
            egui::Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
        painter.with_clip_rect(bounds.intersect(rect)).text(
            bounds.center(),
            egui::Align2::CENTER_CENTER,
            &tag.label,
            egui::FontId::proportional(12.0),
            color,
        );
    }
}
