//! Ordered screen hits for transient overlap selection.
use super::*;

impl PlanDrawing {
    /// Existing single-pick priority followed by every remaining selectable hit.
    /// Includes the native UI's column, room, floor and ceiling fallbacks.
    pub fn hits_screen(
        &self,
        current: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
        radius: f64,
    ) -> Result<Vec<Id>> {
        // Also validates context, camera and query. Keep historical ceiling and
        // grid tie priority exactly as the ordinary picker resolves them.
        let first = self.pick_screen(current, camera, viewport, pointer, radius)?;
        let point = camera.unproject(pointer, viewport)?;
        let in_crop = current.crop.is_none_or(|crop| point_in_crop(crop, point));
        let mut hits: Vec<Id> = first.into_iter().collect();
        for tag in self.opening_tags(current)?.iter().rev() {
            if tag.hit(current, camera, viewport, pointer)? {
                hits.push(tag.entity);
            }
        }
        for tag in self.room_tags(current)?.iter().rev() {
            if tag.hit(current, camera, viewport, pointer)? {
                hits.push(tag.entity);
            }
        }
        hits.extend(self.dimension_hits(current, camera, viewport, pointer, radius)?);
        let distance = |a, b| -> Result<f64> {
            let distance = point_segment_distance(
                pointer,
                camera.project(a, viewport)?,
                camera.project(b, viewport)?,
            );
            ensure(distance.is_finite(), "plan pick distance overflow")?;
            Ok(distance)
        };
        if in_crop {
            for line in self.detail_lines(current)?.iter().rev() {
                if distance(line.start, line.end)? <= radius {
                    hits.push(line.entity);
                }
            }
        }
        for line in self.provider_lines(current)?.iter().rev() {
            if self.is_native_line(line.entity) && distance(line.start, line.end)? <= radius {
                hits.push(line.entity);
            }
        }
        for item in self.items(current)?.iter().rev() {
            if item.footprint.contains(point) {
                hits.push(item.entity);
            }
        }
        for line in self.provider_lines(current)?.iter().rev() {
            if distance(line.start, line.end)? <= radius {
                hits.push(line.entity);
            }
        }
        let mut grids = Vec::new();
        for grid in self.grids(current)? {
            let d = distance(grid.start, grid.end)?;
            if d <= radius {
                grids.push((d, grid.entity));
            }
        }
        grids.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        hits.extend(grids.into_iter().map(|(_, id)| id));
        for item in self.columns(current)?.iter().rev() {
            if item.footprint.contains(point) {
                hits.push(item.entity);
            }
        }
        if in_crop {
            for room in self.rooms(current)?.iter().rev() {
                if room.contains(point) {
                    hits.push(room.entity);
                }
            }
            for floor in self
                .floors(current)?
                .iter()
                .rev()
                .chain(self.ceilings(current)?.iter().rev())
            {
                if floor.contains(point) {
                    hits.push(floor.entity);
                }
            }
        }
        let mut seen = BTreeSet::new();
        hits.retain(|id| seen.insert(*id));
        Ok(hits)
    }
}
