//! Disposable plan pose gestures. All coordinates come from production components.
use super::*;
use os_core::ensure;
use os_geometry::openings::{
    OpeningPoseHandle, PoseLeaf, PoseMotion, door_pair, pose_handle_for_leaf,
};
use os_model::{OpeningState, WindowOperation};

pub(super) struct Draft {
    guard: crate::opening_tools::OpeningRehost,
    context: PlanContext,
    original: OpeningParams,
    geometry: OpeningPoseHandle,
    leaf: PoseLeaf,
    paired: bool,
    paired_sashes: bool,
    drag: Option<Drag>,
}

struct Drag {
    leaf: PoseLeaf,
    camera: PlanCamera,
    canvas: egui::Rect,
    drawing: Id,
    offset: Point2,
    previous_angle: f64,
    angular_delta: f64,
}

#[derive(Default)]
pub(super) struct Frame {
    pub lines: Option<(Id, Vec<os_render::plan::PlanLine>)>,
    point: Option<egui::Pos2>,
    label: String,
    commit: Option<OpeningParams>,
    released: bool,
    selector: Vec<(egui::Rect, egui::style::WidgetVisuals, &'static str)>,
}

impl Draft {
    fn begin(app: &DesktopApp) -> Result<Self> {
        let view = app
            .plans
            .active
            .ok_or_else(|| Error::Invalid("Open a floor plan".into()))?;
        ensure(
            app.plans.active_sheet.is_none()
                && app.editor.document.model().is_native_floor_plan(view)
                && app.selected_ids.len() == 1
                && app
                    .selected
                    .is_some_and(|id| app.selected_ids.contains(&id))
                && !app.editor.plugin_work_pending(),
            "Select one visible opening in an authored FloorPlan",
        )?;
        let drawing = app
            .plans
            .drawing
            .as_ref()
            .ok_or_else(|| Error::Invalid("Wait for the drawing".into()))?;
        let guard = crate::opening_tools::OpeningRehost::begin(
            &app.editor,
            Some(view),
            app.selected,
            drawing,
        )?;
        let original = app.editor.document.model().openings[&guard.id]
            .parameters
            .clone();
        let resolved = app.editor.document.model().resolve_opening(&original)?;
        let wall = app.editor.document.model().resolve_wall(resolved.host)?;
        let paired_sashes = matches!(
            resolved.window_operation,
            WindowOperation::PairedCasement { .. }
        );
        let leaf = if paired_sashes {
            PoseLeaf::StartSash
        } else {
            PoseLeaf::Active
        };
        let paired = matches!(
            resolved.family.door_leaves,
            os_model::DoorLeaves::Paired { .. }
        );
        let geometry = pose_handle_for_leaf(&resolved, &wall.parameters, leaf)?;
        Ok(Self {
            guard,
            context: app.editor.native_plan_context(view)?,
            original,
            geometry,
            leaf,
            paired,
            paired_sashes,
            drag: None,
        })
    }

    fn label(&self, geometry: OpeningPoseHandle) -> String {
        if self.paired || self.paired_sashes {
            format!("{} · {}", leaf_label(self.leaf), format_value(geometry))
        } else {
            format_value(geometry)
        }
    }

    fn select_leaf(&mut self, editor: &Editor, leaf: PoseLeaf) -> Result<()> {
        if leaf != self.leaf {
            self.drag = None;
            let resolved = editor.document.model().resolve_opening(&self.original)?;
            let wall = editor.document.model().resolve_wall(resolved.host)?;
            self.geometry = pose_handle_for_leaf(&resolved, &wall.parameters, leaf)?;
            self.leaf = leaf;
        }
        Ok(())
    }

    fn leaf_selector(
        &mut self,
        editor: &Editor,
        ui: &egui::Ui,
        canvas: egui::Rect,
        frame: &mut Frame,
    ) -> bool {
        if !self.paired && !self.paired_sashes {
            return false;
        }
        let mut over = false;
        for (index, leaf) in (if self.paired_sashes {
            [PoseLeaf::StartSash, PoseLeaf::EndSash]
        } else {
            [PoseLeaf::Active, PoseLeaf::Inactive]
        })
        .into_iter()
        .enumerate()
        {
            let rect = egui::Rect::from_min_size(
                canvas.right_top() + egui::vec2(-220. + index as f32 * 108., 8.),
                egui::vec2(104., 24.),
            );
            let response = ui.interact(
                rect,
                ui.id().with(("pose_leaf", index)),
                egui::Sense::click(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    self.leaf == leaf,
                    leaf_label(leaf),
                )
            });
            over |= ui.input(|i| i.pointer.interact_pos().is_some_and(|p| rect.contains(p)));
            if response.clicked() && self.select_leaf(editor, leaf).is_err() {
                self.drag = None;
            }
            let visuals = ui.style().interact_selectable(&response, self.leaf == leaf);
            frame.selector.push((rect, visuals, leaf_label(leaf)));
        }
        // A release onto the selector is never a pose commit, including a drag
        // that began on the handle. The mode remains ready for a fresh press.
        if over && ui.input(|i| i.pointer.primary_pressed() || i.pointer.primary_released()) {
            self.drag = None;
        }
        over
    }

    fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active_sheet.is_none()
            && app.selected_ids.len() == 1
            && app.selected_ids.contains(&self.guard.id)
            && !app.editor.plugin_work_pending()
            && self.guard.current(
                &app.editor,
                app.plans.active,
                app.selected,
                app.plans.drawing.as_ref(),
            )
    }

    fn candidate(
        &mut self,
        editor: &Editor,
        world: Point2,
        shift: bool,
    ) -> Result<(
        OpeningParams,
        Vec<os_render::plan::PlanLine>,
        OpeningPoseHandle,
    )> {
        let drag = self
            .drag
            .as_mut()
            .ok_or_else(|| Error::Invalid("No pose drag".into()))?;
        ensure(drag.leaf == self.leaf, "Pose leaf changed")?;
        let target = Point2::new(world.x - drag.offset.x, world.y - drag.offset.y);
        let (value, step, limit) = match self.geometry.motion {
            PoseMotion::Angular { pivot, sense } => {
                let a = Point2::new(
                    self.geometry.point.x - pivot.x,
                    self.geometry.point.y - pivot.y,
                );
                let b = Point2::new(target.x - pivot.x, target.y - pivot.y);
                ensure(
                    a.x.hypot(a.y) > 1e-8 && b.x.hypot(b.y) * drag.camera.pixels_per_metre > 2.,
                    "Pointer is at the pose pivot",
                )?;
                let angle = b.y.atan2(b.x);
                let delta = angle - drag.previous_angle;
                // Unwrap consecutive pointer samples before clamping the pose.
                // Crossing the opposite ray must not switch to the other endpoint.
                drag.angular_delta += delta.sin().atan2(delta.cos());
                drag.previous_angle = angle;
                (
                    self.geometry.value + sense * drag.angular_delta.to_degrees(),
                    15.,
                    90.,
                )
            }
            PoseMotion::Sliding { axis, travel } => {
                ensure(
                    travel.is_finite() && travel > 1e-8,
                    "Sash has no defined travel",
                )?;
                let delta = (target.x - self.geometry.point.x) * axis.x
                    + (target.y - self.geometry.point.y) * axis.y;
                (self.geometry.value + delta / travel, 0.1, 1.)
            }
        };
        ensure(value.is_finite(), "Invalid pose")?;
        let value = if shift {
            (value / step).round() * step
        } else {
            value
        }
        .clamp(0., limit);
        let mut parameters = self.original.clone();
        // Screen coordinates have f32 precision; retain the exact original wire
        // representation when returning to its effective pose.
        let delta = (value - self.geometry.value).abs();
        let movement_pixels = match self.geometry.motion {
            PoseMotion::Angular { pivot, .. } => {
                delta.to_radians()
                    * self.geometry.point.distance(pivot)
                    * drag.camera.pixels_per_metre
            }
            PoseMotion::Sliding { travel, .. } => delta * travel * drag.camera.pixels_per_metre,
        };
        if movement_pixels > 0.001 {
            let resolved = editor.document.model().resolve_opening(&parameters)?;
            parameters.open_state = match resolved.kind {
                OpeningKind::Door if self.paired => {
                    let leaves = door_pair(&resolved)?
                        .ok_or_else(|| Error::Invalid("Door pair changed".into()))?;
                    let wall = editor.document.model().resolve_wall(resolved.host)?;
                    OpeningState::DoorPairAngles {
                        active_degrees: if drag.leaf == PoseLeaf::Active {
                            value
                        } else {
                            leaves[0].door_angle(&wall.parameters)
                        },
                        inactive_degrees: if drag.leaf == PoseLeaf::Inactive {
                            value
                        } else {
                            leaves[1].door_angle(&wall.parameters)
                        },
                    }
                }
                OpeningKind::Door => OpeningState::DoorAngle(value),
                OpeningKind::Window if self.paired_sashes => {
                    let [start, end] = resolved.casement_pair_angles();
                    OpeningState::CasementPairAngles {
                        start_degrees: if drag.leaf == PoseLeaf::StartSash {
                            value
                        } else {
                            start
                        },
                        end_degrees: if drag.leaf == PoseLeaf::EndSash {
                            value
                        } else {
                            end
                        },
                    }
                }
                OpeningKind::Window if resolved.window_operation == WindowOperation::Sliding => {
                    OpeningState::SlidingFraction(value)
                }
                OpeningKind::Window => OpeningState::CasementAngle(value),
            };
        }
        let mut model = editor.document.model().clone();
        model.openings.get_mut(&self.guard.id).unwrap().parameters = parameters.clone();
        model.validate()?;
        crate::opening_tools::panel_mesh(&model, self.guard.id)?;
        let drawing =
            crate::opening_tools::opening_edit_preview(&model, self.guard.id, &[], self.context)?;
        let resolved = model.resolve_opening(&parameters)?;
        let wall = model.resolve_wall(resolved.host)?;
        Ok((
            parameters,
            drawing.provider_lines(self.context)?.to_vec(),
            pose_handle_for_leaf(&resolved, &wall.parameters, drag.leaf)?,
        ))
    }
}

fn project(
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
    world: Point2,
) -> Option<egui::Pos2> {
    let p = context.basis.world_to_plane(world).ok()?;
    if !point_in_plan_crop(context, p) {
        return None;
    }
    let p = camera
        .project(p, [canvas.width() as f64, canvas.height() as f64])
        .ok()?;
    let p = canvas.min + egui::vec2(p.x as f32, p.y as f32);
    (p.is_finite() && canvas.contains(p)).then_some(p)
}

fn unproject(
    context: PlanContext,
    camera: PlanCamera,
    canvas: egui::Rect,
    p: egui::Pos2,
) -> Result<Point2> {
    context.basis.plane_to_world(camera.unproject(
        Point2::new((p.x - canvas.left()) as f64, (p.y - canvas.top()) as f64),
        [canvas.width() as f64, canvas.height() as f64],
    )?)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    draft: &mut Option<Draft>,
    claimed: &mut bool,
    editor: &Editor,
    drawing: &PlanDrawing,
    camera: PlanCamera,
    canvas: egui::Rect,
    ctx: &egui::Context,
    ui: &egui::Ui,
) -> Frame {
    let mut frame = Frame::default();
    let Some(d) = draft.as_mut() else {
        return frame;
    };
    if ctx.input(|i| i.pointer.primary_down() || i.pointer.primary_released()) {
        *claimed = true;
    }
    if d.drag.as_ref().is_some_and(|drag| {
        drag.camera != camera || drag.canvas != canvas || drag.drawing != drawing.identity()
    }) {
        *draft = None;
        return frame;
    }
    // Controls stay inside the existing canvas and run before acquisition/release.
    // Switching leaf discards the draft and requires a fresh handle press.
    let over_selector = d.leaf_selector(editor, ui, canvas, &mut frame);
    let Some(handle) = project(d.context, camera, canvas, d.geometry.point) else {
        *draft = None;
        return frame;
    };
    frame.point = Some(handle);
    frame.label = d.label(d.geometry);
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    if ctx.input(|i| i.pointer.primary_pressed())
        && !over_selector
        && d.drag.is_none()
        && let Some(pointer) = pointer.filter(|p| p.distance(handle) <= 10.)
        && let Ok(world) = unproject(d.context, camera, canvas, pointer)
    {
        // Reject a grab whose actual pointer is indistinguishable from the pivot.
        if let PoseMotion::Angular { pivot, .. } = d.geometry.motion
            && world.distance(pivot) * camera.pixels_per_metre < 2.
        {
            return frame;
        }
        d.drag = Some(Drag {
            leaf: d.leaf,
            camera,
            canvas,
            drawing: drawing.identity(),
            offset: Point2::new(world.x - d.geometry.point.x, world.y - d.geometry.point.y),
            previous_angle: match d.geometry.motion {
                PoseMotion::Angular { pivot, .. } => {
                    (d.geometry.point.y - pivot.y).atan2(d.geometry.point.x - pivot.x)
                }
                PoseMotion::Sliding { .. } => 0.,
            },
            angular_delta: 0.,
        });
    }
    if d.drag.is_some() {
        let result = pointer
            .filter(|p| canvas.contains(*p))
            .ok_or_else(|| Error::Invalid("Pointer left the canvas".into()))
            .and_then(|p| unproject(d.context, camera, canvas, p))
            .and_then(|p| d.candidate(editor, p, ctx.input(|i| i.modifiers.shift)));
        match result {
            Ok((parameters, lines, geometry)) => {
                frame.point = project(d.context, camera, canvas, geometry.point);
                frame.label = d.label(geometry);
                frame.lines = Some((d.guard.id, lines));
                if ctx.input(|i| i.pointer.primary_released()) && frame.point.is_some() {
                    frame.commit = Some(parameters);
                }
            }
            Err(error) => {
                frame.label = error.to_string();
                frame.point = None;
            }
        }
        frame.released = ctx.input(|i| i.pointer.primary_released());
    }
    frame
}

fn leaf_label(leaf: PoseLeaf) -> &'static str {
    match leaf {
        PoseLeaf::Active => "Active leaf",
        PoseLeaf::Inactive => "Inactive leaf",
        PoseLeaf::StartSash => "Start sash",
        PoseLeaf::EndSash => "End sash",
    }
}

fn format_value(geometry: OpeningPoseHandle) -> String {
    match geometry.motion {
        PoseMotion::Angular { .. } => format!("{:.1}°", geometry.value),
        PoseMotion::Sliding { .. } => format!("{:.2} travel", geometry.value),
    }
}

impl DesktopApp {
    pub(crate) fn opening_pose_available(&self) -> bool {
        Draft::begin(self).ok().is_some_and(|draft| {
            self.plans
                .canvas_rect
                .zip(self.plans.cameras.get(&draft.context.view_id))
                .is_some_and(|(canvas, camera)| {
                    project(draft.context, *camera, canvas, draft.geometry.point).is_some()
                })
        })
    }

    pub(crate) fn begin_opening_pose(&mut self) {
        match Draft::begin(self) {
            Ok(draft) => {
                self.cancel_plan_authoring_for_modal();
                self.opening_draft = None;
                self.plans.opening_pose = Some(draft);
                self.status =
                    "Adjust opening pose · drag the handle · Shift quantizes · Escape exits".into();
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn validate_opening_pose(&mut self, ctx: &egui::Context) {
        if self.plans.opening_pose.is_some()
            && ctx.input(|i| i.pointer.primary_down() || i.pointer.primary_released())
        {
            self.plans.opening_pose_claimed = true;
        }
        if self.plans.opening_pose.as_ref().is_some_and(|d| {
            !d.current(self)
                || (d.drag.is_some()
                    && ctx.input(|i| !i.pointer.primary_down() && !i.pointer.primary_released()))
        }) || ctx.input(|i| {
            !i.focused
                || i.key_pressed(egui::Key::Escape)
                || i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::PointerGone | egui::Event::WindowFocused(false)
                    )
                })
        }) || self.plans.crop.mode.is_some()
            || self.wall_gesture.is_some()
            || self.opening_draft.is_some()
            || self.plans.opening_placement.is_some()
            || self.plans.dimension_draft.is_some()
            || self.plans.room_placement_active
            || self.plans.floor_sketch.is_some()
            || self.plans.floor_hole_sketch.is_some()
            || self.plans.section_placement.is_some()
            || self.plans.column_placement.is_some()
            || self.plans.casework_placement.is_some()
            || self.plans.curtain_draft.is_some()
            || self.plans.roof_draft.is_some()
            || self.plans.ceiling_draft.is_some()
            || self.plans.ramp_placement.is_some()
            || self.plans.stair_placement.is_some()
            || self.grid_draft.is_some()
            || self.plans.detail_line_draft.is_some()
            || self.plans.room_tag_draft.is_some()
            || self.plans.opening_tag_draft.is_some()
            || self.plans.room_separation_line_draft.is_some()
        {
            self.plans.opening_pose = None;
        }
    }

    pub(super) fn finish_opening_pose(
        &mut self,
        frame: Frame,
        painter: &egui::Painter,
        canvas: egui::Rect,
    ) {
        if let Some(point) = frame.point {
            painter.circle_filled(point, 6., theme::ACCENT);
            painter.circle_stroke(point, 8., egui::Stroke::new(1., theme::TEXT));
            painter.text(
                point + egui::vec2(12., -12.),
                egui::Align2::LEFT_BOTTOM,
                &frame.label,
                egui::FontId::proportional(13.),
                theme::TEXT,
            );
        }
        painter.text(
            canvas.left_top() + egui::vec2(12., 12.),
            egui::Align2::LEFT_TOP,
            format!(
                "Adjust opening pose · {} · Shift quantizes · Escape exits",
                frame.label
            ),
            egui::FontId::proportional(12.),
            theme::ACCENT,
        );
        // Paint controls after the canvas and symbol preview.
        for (rect, visuals, label) in &frame.selector {
            painter.rect(
                *rect,
                3.,
                visuals.bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(12.),
                visuals.text_color(),
            );
        }
        if frame.released {
            let result = if let Some(parameters) = frame.commit {
                self.plans
                    .opening_pose
                    .as_ref()
                    .filter(|d| d.current(self))
                    .map(|d| d.guard.id)
                    .ok_or_else(|| Error::Invalid("Pose gesture is stale".into()))
                    .and_then(|id| {
                        self.editor.command(
                            "Adjust opening pose",
                            Command::UpdateOpening { id, parameters },
                        )
                    })
            } else {
                Err(Error::Invalid(
                    "Invalid pose release; no changes applied".into(),
                ))
            };
            self.plans.opening_pose = None;
            self.report(result, "Opening pose updated.");
        }
    }
}
