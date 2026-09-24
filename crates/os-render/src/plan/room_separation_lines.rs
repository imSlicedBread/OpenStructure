//! Checked native level-owned room boundary layer; coordinates are in the plan plane.
use super::*;
use crate::snapping::SnapSegment;

/// Room separator stroke weight in paper millimetres.
pub const ROOM_SEPARATOR_WEIGHT_MM: f64 = 0.25;

pub fn clip_room_separation_line(
    context: PlanContext,
    start: Point2,
    end: Point2,
) -> Result<Option<(Point2, Point2)>> {
    grids::clip(start, end, context.crop)
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanRoomSeparationLine {
    pub entity: Id,
    pub view: Id,
    pub level: Id,
    pub start: Point2,
    pub end: Point2,
}

impl PlanDrawing {
    pub fn with_room_separation_lines(
        mut self,
        level: Id,
        lines: Vec<PlanRoomSeparationLine>,
    ) -> Result<Self> {
        ensure(
            self.snaps.is_none(),
            "attach room separators before snap features",
        )?;
        ensure(
            self.source_ids.len().saturating_add(lines.len()) <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        ensure(
            self.provider_segment_count
                .saturating_add(self.grid_segment_count)
                .saturating_add(self.detail_segment_count)
                .saturating_add(self.separator_segment_count)
                .saturating_add(lines.len())
                <= MAX_PLAN_ELEMENTS,
            "plan lines exceed 10000 segments",
        )?;
        self.separator_segment_count += lines.len();
        for mut line in lines {
            ensure(
                !line.entity.0.is_nil()
                    && self.source_ids.insert(line.entity)
                    && line.view == self.context.view_id
                    && !level.0.is_nil()
                    && line.level == level,
                "invalid room separator identity or view",
            )?;
            ensure(
                [line.start, line.end]
                    .iter()
                    .all(|p| p.is_finite() && p.x.abs() <= 3e6 && p.y.abs() <= 3e6),
                "invalid room separator coordinates",
            )?;
            ensure(
                line.start.distance(line.end) > 0.0,
                "degenerate room separator",
            )?;
            let raw = SnapSegment {
                entity: line.entity,
                feature: 0,
                start: line.start,
                end: line.end,
            };
            if let Some((a, b)) = grids::clip(line.start, line.end, self.context.crop)? {
                line.start = a;
                line.end = b;
                self.room_separation_lines.push(line);
                self.separator_segments.push(raw);
            }
        }
        self.room_separation_lines.sort_by_key(|line| line.entity);
        Ok(self)
    }

    pub fn room_separation_lines(&self, current: PlanContext) -> Result<&[PlanRoomSeparationLine]> {
        self.items(current)?;
        Ok(&self.room_separation_lines)
    }

    /// Room separators paint over physical geometry; reverse UUID order wins picking.
    pub fn pick_room_separation_line_screen(
        &self,
        current: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
        radius: f64,
    ) -> Result<Option<Id>> {
        ensure(
            pointer.is_finite() && radius.is_finite() && radius > 0.0 && radius <= 64.0,
            "invalid room separator pick",
        )?;
        self.items(current)?;
        let plane = camera.unproject(pointer, viewport)?;
        if current.crop.is_some_and(|crop| !point_in_crop(crop, plane)) {
            return Ok(None);
        }
        for line in self.room_separation_lines(current)?.iter().rev() {
            let a = camera.project(line.start, viewport)?;
            let b = camera.project(line.end, viewport)?;
            let length = a.distance(b);
            if length <= 0.0 {
                continue;
            }
            let t = (((pointer.x - a.x) * (b.x - a.x) + (pointer.y - a.y) * (b.y - a.y))
                / (length * length))
                .clamp(0.0, 1.0);
            if pointer.distance(Point2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y))) <= radius
            {
                return Ok(Some(line.entity));
            }
        }
        Ok(None)
    }
}
