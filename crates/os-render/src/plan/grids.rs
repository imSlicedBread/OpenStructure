//! Checked finite datum lines, separate from cut polygons and derived mesh edges.
use super::*;
use crate::snapping::{SnapQuery, SnapSegment};

#[derive(Clone, Debug, PartialEq)]
pub struct PlanGrid {
    pub entity: Id,
    pub name: String,
    /// View-plane metres. Drawing endpoints may be clipped; snap endpoints are not.
    pub start: Point2,
    pub end: Point2,
}

impl PlanDrawing {
    /// Inputs are already transformed from model coordinates by the host adapter.
    /// Add before wall snap features. Hidden source IDs also participate in bounds
    /// and collision checks so cropping cannot bypass the input budget.
    pub fn with_grids(mut self, grids: Vec<PlanGrid>) -> Result<Self> {
        ensure(self.snaps.is_none(), "attach grids before snap features")?;
        ensure(
            self.provider_segment_count
                .saturating_add(self.detail_segment_count)
                .saturating_add(self.separator_segment_count)
                .saturating_add(self.grid_segment_count)
                .saturating_add(grids.len())
                <= MAX_PLAN_ELEMENTS,
            "plan lines exceed 10000 segments",
        )?;
        self.grid_segment_count += grids.len();
        ensure(
            self.source_ids.len().saturating_add(grids.len()) <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        for mut grid in grids {
            ensure(
                !grid.entity.0.is_nil() && self.source_ids.insert(grid.entity),
                "invalid or duplicate grid drawing identity",
            )?;
            ensure(
                !grid.name.is_empty()
                    && grid.name.trim() == grid.name
                    && grid.name.len() <= 256
                    && !grid.name.chars().any(char::is_control),
                "invalid grid drawing label",
            )?;
            let raw = SnapSegment {
                entity: grid.entity,
                feature: 0,
                start: grid.start,
                end: grid.end,
            };
            let Some((a, b)) = clip(grid.start, grid.end, self.context.crop)? else {
                continue;
            };
            grid.start = a;
            grid.end = b;
            self.grids.push(grid);
            self.grid_segments.push(raw);
        }
        self.grids.sort_by_key(|g| g.entity);
        Ok(self)
    }

    pub fn grids(&self, current: PlanContext) -> Result<&[PlanGrid]> {
        self.items(current)?;
        Ok(&self.grids)
    }

    /// Polygon interiors have foreground priority. Otherwise acquire a datum at
    /// a fixed logical-pixel distance, using the same cropped lines as drawing.
    pub fn pick_screen(
        &self,
        current: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
        radius_pixels: f64,
    ) -> Result<Option<Id>> {
        ensure(
            pointer.is_finite()
                && radius_pixels.is_finite()
                && radius_pixels > 0.0
                && radius_pixels <= 64.0,
            "invalid plan pick query",
        )?;
        if let Some(id) = self.pick_room_tag_screen(current, camera, viewport, pointer)? {
            return Ok(Some(id));
        }
        if let Some(id) =
            self.pick_dimension_screen(current, camera, viewport, pointer, radius_pixels)?
        {
            return Ok(Some(id));
        }
        let point = camera.unproject(pointer, viewport)?;
        if let Some(id) =
            self.pick_detail_line_screen(current, camera, viewport, pointer, radius_pixels)?
        {
            return Ok(Some(id));
        }
        if let Some(id) =
            self.pick_provider_line(current, camera, viewport, pointer, radius_pixels, true)?
        {
            return Ok(Some(id));
        }
        if let Some(id) = self.pick(current, point)? {
            return Ok(Some(id));
        }
        if let Some(id) =
            self.pick_provider_line(current, camera, viewport, pointer, radius_pixels, false)?
        {
            return Ok(Some(id));
        }
        let scene = crate::snapping::SnapScene::new(
            current,
            self.grids(current)?
                .iter()
                .map(|g| SnapSegment {
                    entity: g.entity,
                    feature: 0,
                    start: g.start,
                    end: g.end,
                })
                .collect(),
        )?;
        let query = SnapQuery {
            camera,
            viewport,
            pointer,
            radius_pixels,
            endpoints: false,
            midpoints: false,
            intersections: false,
            perpendicular_from: None,
            nearest: true,
            axis_extensions: false,
            exclude_entity: None,
        };
        Ok(scene
            .query(current, query)?
            .candidate(current, query)?
            .map(|hit| hit.entity))
    }
}

pub(super) fn clip(
    start: Point2,
    end: Point2,
    crop: Option<PlanCrop>,
) -> Result<Option<(Point2, Point2)>> {
    let length = start.distance(end);
    ensure(
        start.is_finite() && end.is_finite() && length.is_finite() && length > 1e-9,
        "grid drawing needs finite nonzero extents",
    )?;
    let Some(crop) = crop else {
        return Ok(Some((start, end)));
    };
    crop.validate()?;
    let ux = (end.x - start.x) / length;
    let uy = (end.y - start.y) / length;
    let mut lo: f64 = 0.0;
    let mut hi: f64 = length;
    for (origin, direction, min, max) in [
        (start.x, ux, crop.min.x, crop.max.x),
        (start.y, uy, crop.min.y, crop.max.y),
    ] {
        if direction == 0.0 {
            if origin < min || origin > max {
                return Ok(None);
            }
        } else {
            let a = (min - origin) / direction;
            let b = (max - origin) / direction;
            // Infinite interval ends may mean wholly outside. NaN is never useful.
            ensure(!a.is_nan() && !b.is_nan(), "grid crop overflow")?;
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
        }
    }
    if hi - lo <= 1e-9 {
        return Ok(None);
    }
    let at = |distance| -> Result<Point2> {
        let p = Point2::new(start.x + ux * distance, start.y + uy * distance);
        ensure(p.is_finite(), "grid crop coordinate overflow")?;
        Ok(Point2::new(
            p.x.clamp(crop.min.x, crop.max.x),
            p.y.clamp(crop.min.y, crop.max.y),
        ))
    };
    let a = at(lo)?;
    let b = at(hi)?;
    if a.distance(b) <= 1e-9 {
        return Ok(None);
    }
    Ok(Some((a, b)))
}
