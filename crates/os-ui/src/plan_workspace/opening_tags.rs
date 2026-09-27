//! Opening-tag interaction owns a press through release, including after cancellation.
use super::*;
use os_model::{OpeningTag, OpeningTagParams};
use os_render::plan::PlanOpeningTag;

#[derive(Clone)]
pub(super) struct Draft {
    context: PlanContext,
    providers: Vec<(String, Id)>,
    session: Id,
    revision: u64,
    drawing: Option<Id>,
    opening: Option<Id>,
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
    fn new(editor: &Editor, context: PlanContext, opening: Option<Id>) -> Self {
        Self {
            context,
            providers: plan_provider_signature(editor),
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            drawing: None,
            opening,
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
    pub(crate) fn begin_opening_tag(&mut self) {
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
        let opening = self.selected.filter(|id| {
            self.editor
                .document
                .model()
                .openings
                .get(id)
                .is_some_and(|opening| {
                    self.editor
                        .document
                        .model()
                        .walls
                        .get(&opening.parameters.host)
                        .is_some_and(|host| {
                            Some(host.parameters.level)
                                == self.editor.document.model().views[&view].parameters.level
                        })
                })
        });
        if opening.is_none() {
            self.status = "Select a door or window on this plan level first.".into();
            return;
        }
        if let Some(tag) = self
            .editor
            .document
            .model()
            .opening_tags
            .values()
            .find(|tag| tag.parameters.view == view && Some(tag.parameters.opening) == opening)
        {
            let id = tag.id();
            self.select(Some(id));
            self.status = "Opening already tagged in this view; drag its tag to move it.".into();
            return;
        }
        self.plans.opening_tag_draft = Some(Draft::new(&self.editor, context, opening));
        self.status = "Opening Tag · click a position in the plan".into();
    }

    pub(crate) fn apply_opening_tag_properties(&mut self) {
        let Some(tag) = self
            .selected
            .and_then(|id| self.editor.document.model().opening_tags.get(&id))
            .cloned()
        else {
            return;
        };
        let mut parameters = tag.parameters;
        parameters.position = self.opening_tag_position;
        let crop_check = self
            .editor
            .native_plan_context(parameters.view)
            .and_then(|context| {
                os_core::ensure(
                    point_in_plan_crop(context, context.basis.world_to_plane(parameters.position)?),
                    "Opening tag position is outside its view crop",
                )
            });
        if let Err(error) = crop_check {
            self.report(Err(error), "");
            return;
        }
        let result = self.editor.command(
            "Move opening tag",
            Command::UpdateOpeningTag {
                id: tag.header.id,
                parameters,
            },
        );
        self.report(result, "Opening tag position updated.");
    }

    pub(super) fn finish_opening_tag_action(&mut self, action: Action) {
        match action {
            Action::Select(id) => self.select(Some(id)),
            Action::Commit(draft, position) => {
                let result = (|| {
                    os_core::ensure(
                        !draft.stale(&self.editor, self.plans.active),
                        "opening tag draft is stale",
                    )?;
                    let context = draft.context;
                    os_core::ensure(
                        point_in_plan_crop(context, context.basis.world_to_plane(position)?),
                        "opening tag position outside crop",
                    )?;
                    let mut parameters = if let Some(id) = draft.moving {
                        self.editor
                            .document
                            .model()
                            .opening_tags
                            .get(&id)
                            .ok_or_else(|| Error::Invalid("missing opening tag".into()))?
                            .parameters
                            .clone()
                    } else {
                        OpeningTagParams {
                            view: context.view_id,
                            opening: draft
                                .opening
                                .ok_or_else(|| Error::Invalid("choose a opening".into()))?,
                            position,
                            label_preset: Default::default(),
                        }
                    };
                    parameters.position = position;
                    if draft.moving.is_some() {
                        // Existing orphan tags remain movable; only new tags need a live opening.
                        parameters.validate(self.editor.document.model())?;
                    } else {
                        parameters.validate_creation(self.editor.document.model())?;
                    }
                    os_core::ensure(
                        !self
                            .editor
                            .document
                            .model()
                            .opening_tags
                            .values()
                            .any(|tag| {
                                Some(tag.id()) != draft.moving
                                    && tag.parameters.view == parameters.view
                                    && tag.parameters.opening == parameters.opening
                            }),
                        "opening already tagged in this view",
                    )?;
                    let id = if let Some(id) = draft.moving {
                        self.editor.command(
                            "Move opening tag",
                            Command::UpdateOpeningTag { id, parameters },
                        )?;
                        id
                    } else {
                        let tag = OpeningTag::new("core.opening_tag", parameters);
                        let id = tag.id();
                        self.editor
                            .command("Place opening tag", Command::AddOpeningTag(tag))?;
                        id
                    };
                    Ok(id)
                })();
                match result {
                    Ok(id) => {
                        self.plans.opening_tag_draft = None;
                        self.select(Some(id));
                        self.report(Ok(()), "Opening tag saved.");
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
        && let Ok(Some(id)) = drawing.pick_opening_tag_screen(
            context,
            camera,
            size,
            Point2::new(
                f64::from(pos.x - rect.left()),
                f64::from(pos.y - rect.top()),
            ),
        )
        && let Some(tag) = editor.document.model().opening_tags.get(&id)
    {
        let mut d = Draft::new(editor, context, Some(tag.parameters.opening));
        d.drawing = Some(drawing.identity());
        d.moving = Some(id);
        d.origin = Some(pos);
        d.original = Some(tag.parameters.position);
        d.preview = d.original;
        *draft = Some(d);
        *claimed = true;
    }
    let d = draft.as_mut()?;
    if response.contains_pointer() && ctx.input(|i| i.pointer.primary_pressed()) {
        *claimed = true;
    }
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
            return if !d.moved || d.preview == d.original {
                Some(Action::Select(id))
            } else {
                d.preview.map(|p| Action::Commit(Box::new(d), p))
            };
        }
    } else if response.clicked()
        && let Some(p) = d.preview
    {
        return Some(Action::Commit(Box::new(d.clone()), p));
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    painter: &egui::Painter,
    tags: &[PlanOpeningTag],
    draft: Option<&Draft>,
    editor: &Editor,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    selected: Option<Id>,
    pointer: Option<egui::Pos2>,
) {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let leader_clip = context
        .crop
        .and_then(|crop| {
            let a = camera
                .project(Point2::new(crop.min.x, crop.max.y), size)
                .ok()?;
            let b = camera
                .project(Point2::new(crop.max.x, crop.min.y), size)
                .ok()?;
            Some(
                egui::Rect::from_min_max(
                    rect.min + egui::vec2(a.x as f32, a.y as f32),
                    rect.min + egui::vec2(b.x as f32, b.y as f32),
                )
                .intersect(rect),
            )
        })
        .unwrap_or(rect);
    let preview = draft.and_then(|d| {
        let position = context.basis.world_to_plane(d.preview?).ok()?;
        if let Some(id) = d.moving {
            let mut tag = tags.iter().find(|tag| tag.entity == id)?.clone();
            tag.anchor = position;
            return Some(tag);
        }
        let opening = editor.document.model().openings.get(&d.opening?)?;
        let parameters = OpeningTagParams {
            view: context.view_id,
            opening: opening.id(),
            position: d.preview?,
            label_preset: Default::default(),
        };
        crate::plan::opening_tag_graphic(
            editor.document.model(),
            opening.id(),
            &parameters,
            context,
        )
        .ok()
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
        if let Ok(Some((a, b))) = tag.leader(context, camera, size) {
            painter.with_clip_rect(leader_clip).line_segment(
                [
                    rect.min + egui::vec2(a.x as f32, a.y as f32),
                    rect.min + egui::vec2(b.x as f32, b.y as f32),
                ],
                egui::Stroke::new(1.0, color),
            );
        }
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
