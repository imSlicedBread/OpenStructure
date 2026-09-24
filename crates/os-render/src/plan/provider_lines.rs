//! Host-owned clipping/drawing/picking of checked semantic provider lines.
use super::*;
use crate::snapping::SnapSegment;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanLine {
    pub entity: Id,
    pub feature: u32,
    pub start: Point2,
    pub end: Point2,
    pub role: PlanRole,
}

impl PlanDrawing {
    /// Host symbols use the same checked clipping and line geometry as providers,
    /// but draw/pick above projected host sills.
    pub fn with_native_lines(mut self, lines: BTreeMap<Id, Vec<PlanLine>>) -> Result<Self> {
        self.native_line_ids.extend(lines.keys().copied());
        self.with_provider_lines(lines)
    }

    pub fn is_native_line(&self, entity: Id) -> bool {
        self.native_line_ids.contains(&entity)
    }
    /// Resolve unavailable entities only after provider results have passed the
    /// host's document/view/activation checks. Empty output is a valid invisible
    /// representation. Native polygon/grid identities cannot be overwritten.
    /// Attach before snaps; original semantic endpoints are kept independently
    /// from clipped drawing endpoints. No plugin/runtime dependency enters render.
    pub fn with_provider_lines(mut self, providers: BTreeMap<Id, Vec<PlanLine>>) -> Result<Self> {
        ensure(
            self.snaps.is_none(),
            "attach provider lines before snap features",
        )?;
        let count = providers
            .values()
            .fold(0usize, |n, lines| n.saturating_add(lines.len()));
        ensure(
            count
                .saturating_add(self.provider_segment_count)
                .saturating_add(self.grid_segment_count)
                .saturating_add(self.detail_segment_count)
                .saturating_add(self.separator_segment_count)
                <= MAX_PLAN_ELEMENTS,
            "plan provider lines exceed 10000 segments",
        )?;
        self.provider_segment_count += count;
        for (entity, lines) in providers {
            ensure(
                self.unavailable.contains(&entity),
                "provider target is not unavailable",
            )?;
            ensure(lines.len() <= 256, "provider exceeds 256 lines per element")?;
            let mut features = BTreeSet::new();
            for mut line in lines {
                ensure(
                    line.entity == entity && features.insert(line.feature),
                    "invalid or duplicate provider feature",
                )?;
                let raw = SnapSegment {
                    entity,
                    feature: line.feature,
                    start: line.start,
                    end: line.end,
                };
                if let Some((start, end)) = grids::clip(line.start, line.end, self.context.crop)? {
                    line.start = start;
                    line.end = end;
                    self.lines.push(line);
                    self.line_segments.push(raw);
                }
            }
            self.unavailable.retain(|id| *id != entity);
        }
        self.lines.sort_by_key(|line| {
            (
                match line.role {
                    PlanRole::Depth => 0,
                    PlanRole::Projected => 1,
                    PlanRole::Cut => 2,
                },
                line.entity,
                line.feature,
            )
        });
        Ok(self)
    }

    /// Draw in this order above grids and below polygon bodies. Screen picking
    /// gives polygon interiors precedence, then reverse provider-line order,
    /// then grid acquisition. This is the initial fixed composition policy.
    pub fn provider_lines(&self, current: PlanContext) -> Result<&[PlanLine]> {
        self.items(current)?;
        Ok(&self.lines)
    }

    pub(super) fn pick_provider_line(
        &self,
        current: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
        radius: f64,
        native_only: bool,
    ) -> Result<Option<Id>> {
        for line in self.provider_lines(current)?.iter().rev() {
            if native_only && !self.native_line_ids.contains(&line.entity) {
                continue;
            }
            let a = camera.project(line.start, viewport)?;
            let b = camera.project(line.end, viewport)?;
            let length = a.distance(b);
            ensure(length.is_finite(), "provider pick projection overflow")?;
            let distance = if length == 0.0 {
                a.distance(pointer)
            } else {
                let (ux, uy) = ((b.x - a.x) / length, (b.y - a.y) / length);
                let along = ((pointer.x - a.x) * ux + (pointer.y - a.y) * uy).clamp(0.0, length);
                pointer.distance(Point2::new(a.x + ux * along, a.y + uy * along))
            };
            ensure(distance.is_finite(), "provider pick distance overflow")?;
            if distance <= radius {
                return Ok(Some(line.entity));
            }
        }
        Ok(None)
    }
}
