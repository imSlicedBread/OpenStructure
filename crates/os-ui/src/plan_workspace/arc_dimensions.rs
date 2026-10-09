use super::*;

impl DesktopApp {
    pub(super) fn dimension_current(&self, draft: &DimensionDraft) -> bool {
        draft.session == self.editor.document.session_id()
            && draft.revision == self.editor.document.revision()
            && draft.wall_activation == self.editor.host.activation_id(os_walls::PLUGIN_ID)
            && draft.provider_signature == plan_provider_signature(&self.editor)
            && self.plans.active == Some(draft.context.view_id)
            && self.plans.active_sheet.is_none()
            && draft.displayed_drawing_identity.is_none_or(|id| {
                self.dimension_repair_drawing()
                    .is_some_and(|d| d.identity() == id)
            })
            && self.editor.native_plan_context(draft.context.view_id).ok() == Some(draft.context)
            && self.plans.drawing.as_ref().is_some_and(|d| {
                d.items(draft.context).is_ok()
                    && draft.drawing_identity.is_none_or(|id| id == d.identity())
            })
            && (draft.repair.is_none() || self.dimension_repair_current(draft))
    }

    pub(super) fn arc_length_parameters(
        draft: &DimensionDraft,
        model: &os_model::Model,
        placement: Point2,
    ) -> Result<DimensionParams> {
        os_core::ensure(
            point_in_plan_crop(draft.context, placement),
            "Arc placement outside crop",
        )?;
        let wall = draft
            .first
            .as_ref()
            .ok_or_else(|| Error::Invalid("Choose a circular wall".into()))?
            .reference
            .entity();
        let mut params = DimensionParams {
            layout: os_model::DimensionLayout::ArcLength,
            additional: Vec::new(),
            baseline_spacing_m: 0.25,
            view: draft.context.view_id,
            first: DimensionReference::WallEndpoint {
                wall,
                endpoint: DimensionEndpoint::Start,
            },
            second: DimensionReference::WallEndpoint {
                wall,
                endpoint: DimensionEndpoint::End,
            },
            offset_m: 0.0,
            orphan_hint: draft.context.basis.plane_to_world(placement)?,
        };
        params.place_arc_length(model, params.orphan_hint)?;
        Ok(params)
    }

    pub(super) fn arc_length_anchor_at(
        &self,
        drawing: &PlanDrawing,
        context: PlanContext,
        camera: PlanCamera,
        rect: egui::Rect,
        pointer: egui::Pos2,
    ) -> Result<DimensionAnchorDraft> {
        let model = self.editor.document.model();
        let level = model
            .views
            .get(&context.view_id)
            .and_then(|v| v.parameters.level)
            .ok_or_else(|| Error::Invalid("Arc dimension needs a floor plan".into()))?;
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let local = Point2::new(
            f64::from(pointer.x - rect.left()),
            f64::from(pointer.y - rect.top()),
        );
        let plane = camera.unproject(local, size)?;
        os_core::ensure(
            rect.contains(pointer) && point_in_plan_crop(context, plane),
            "Curve pick outside plan",
        )?;
        let foreground = drawing.pick_screen(context, camera, size, local, 0.01)?;
        let occludes = |hit: Option<Id>, wall: Id| {
            hit.is_some_and(|id| {
                id != wall
                    && model.walls.get(&id).is_none_or(|w| {
                        w.parameters.level != level || w.parameters.path.is_straight()
                    })
            })
        };
        let mut candidates = Vec::new();
        for item in drawing.items(context)? {
            let Some(wall) = model.walls.get(&item.entity) else {
                continue;
            };
            if wall.parameters.level != level
                || wall.parameters.path.is_straight()
                || occludes(foreground, item.entity)
            {
                continue;
            }
            // Acquire the actual clipped displayed body, never the hidden analytic extension.
            let mut nearest = (f64::INFINITY, plane);
            if item.footprint.contains(plane) {
                nearest = (0.0, plane);
            }
            let vertices = item.footprint.vertices();
            for (a, b) in vertices
                .iter()
                .zip(vertices.iter().cycle().skip(1))
                .take(vertices.len())
            {
                let sa = camera.project(*a, size)?;
                let sb = camera.project(*b, size)?;
                let dx = sb.x - sa.x;
                let dy = sb.y - sa.y;
                let length2 = dx * dx + dy * dy;
                let t = if length2 > 0.0 {
                    ((local.x - sa.x) * dx + (local.y - sa.y) * dy) / length2
                } else {
                    0.0
                }
                .clamp(0.0, 1.0);
                let screen = Point2::new(sa.x + t * dx, sa.y + t * dy);
                let distance = local.distance(screen);
                if distance < nearest.0 {
                    nearest = (distance, camera.unproject(screen, size)?);
                }
            }
            if nearest.0 <= 12.0 && point_in_plan_crop(context, nearest.1) {
                // An occluder at the candidate itself also prevents acquisition from nearby air.
                let hit = drawing.pick_screen(
                    context,
                    camera,
                    size,
                    camera.project(nearest.1, size)?,
                    0.01,
                )?;
                if !occludes(hit, item.entity) {
                    candidates.push((nearest.0, item.entity, nearest.1));
                }
            }
        }
        let (_, wall, point) = candidates
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .ok_or_else(|| Error::Invalid("Choose a visible circular wall within 12 px".into()))?;
        Ok(DimensionAnchorDraft {
            reference: DimensionReference::WallEndpoint {
                wall,
                endpoint: DimensionEndpoint::Start,
            },
            point,
        })
    }
}

pub(super) fn paint(
    painter: &egui::Painter,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    graphic: &os_render::plan::PlanArcLengthDimension,
    selected: bool,
) -> Result<()> {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let color = if graphic.diagnostic.is_some() {
        theme::ERROR
    } else if selected {
        theme::ACCENT
    } else {
        theme::TEXT
    };
    let screen = |p| -> Result<egui::Pos2> {
        let p = camera.project(p, size)?;
        os_core::ensure(
            p.x.abs() < f64::from(f32::MAX) / 2.0 && p.y.abs() < f64::from(f32::MAX) / 2.0,
            "Arc screen coordinate overflow",
        )?;
        Ok(rect.min + egui::vec2(p.x as f32, p.y as f32))
    };
    for (a, b) in graphic.segments(context)? {
        painter.line_segment(
            [screen(a)?, screen(b)?],
            egui::Stroke::new(if selected { 1.8 } else { 1.2 }, color),
        );
    }
    if let Some([a, b]) = graphic.label_bounds(context, camera, size)? {
        let bounds = egui::Rect::from_min_max(
            rect.min + egui::vec2(a.x as f32, a.y as f32),
            rect.min + egui::vec2(b.x as f32, b.y as f32),
        );
        painter
            .with_clip_rect(painter.clip_rect().intersect(bounds))
            .text(
                bounds.center(),
                egui::Align2::CENTER_CENTER,
                graphic.label(),
                egui::FontId::monospace(12.0),
                color,
            );
    }
    Ok(())
}
