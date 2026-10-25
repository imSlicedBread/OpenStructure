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
        if current.annotation_crop.is_some() {
            hits.extend(self.annotation_hits(
                current,
                camera,
                viewport,
                pointer,
                radius,
                |_| true,
            )?);
        }
        for tag in self
            .opening_tags(current)?
            .iter()
            .rev()
            .filter(|_| current.annotation_crop.is_none())
        {
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
        let outline_hit = |segments: Vec<(Point2, Point2)>| -> Result<bool> {
            for (a, b) in segments {
                if distance(a, b)? <= radius {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        if current
            .annotation_bounds()
            .is_none_or(|c| point_in_crop(c, point))
        {
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
            let hit = if item.footprint.role == PlanRole::Overhead {
                outline_hit(item.outline().collect())?
            } else {
                item.footprint.contains(point)
            };
            if hit {
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
            let hit = if item.footprint.role == PlanRole::Overhead {
                outline_hit(item.outline().collect())?
            } else {
                item.footprint.contains(point)
            };
            if hit {
                hits.push(item.entity);
            }
        }
        for item in self.casework(current)?.iter().rev() {
            let hit = if item.footprint.role == PlanRole::Overhead {
                outline_hit(item.outline().collect())?
            } else {
                item.footprint.contains(point)
            };
            if hit {
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
                let hit = if floor.role == PlanRole::Overhead {
                    let mut segments = Vec::new();
                    for ring in std::iter::once(&floor.boundary).chain(floor.holes.iter()) {
                        for (start, end) in ring
                            .iter()
                            .copied()
                            .zip(ring.iter().copied().cycle().skip(1))
                            .take(ring.len())
                        {
                            if let Some(segment) = grids::clip(start, end, current.crop)? {
                                segments.push(segment);
                            }
                        }
                    }
                    outline_hit(segments)?
                } else {
                    floor.contains(point)
                };
                if hit {
                    hits.push(floor.entity);
                }
            }
        }
        let mut seen = BTreeSet::new();
        hits.retain(|id| seen.insert(*id));
        Ok(hits)
    }
}
