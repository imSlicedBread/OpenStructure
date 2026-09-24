//! Fixed, bounded screen-space badges shared by paint, hover and pick.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanRoomTag {
    pub entity: Id,
    pub view: Id,
    pub room: Id,
    /// Plan-plane metres, transformed from world XY by the snapshot producer.
    pub anchor: Point2,
    pub label: String,
    pub diagnostic: Option<String>,
}

impl PlanRoomTag {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.entity.0.is_nil()
                && !self.view.0.is_nil()
                && !self.room.0.is_nil()
                && self.anchor.is_finite()
                && self.anchor.x.abs() <= 2e6
                && self.anchor.y.abs() <= 2e6
                && !self.label.is_empty()
                && self.label.len() <= 520
                && !self.label.chars().any(char::is_control)
                && self.diagnostic.as_ref().is_none_or(|d| {
                    !d.is_empty() && d.len() <= 256 && !d.chars().any(char::is_control)
                }),
            "invalid room tag graphic",
        )
    }

    /// The whole badge must fit in the crop. Text is clipped to this same badge.
    pub fn bounds(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
    ) -> Result<Option<[Point2; 2]>> {
        self.validate()?;
        ensure(
            self.view == context.view_id,
            "room tag belongs to another view",
        )?;
        let p = camera.project(self.anchor, viewport)?;
        let half = (self.label.chars().count() as f64 * 4.5 + 10.0).clamp(24.0, 160.0);
        let mut bounds = [
            Point2::new(p.x - half, p.y - 12.0),
            Point2::new(p.x + half, p.y + 12.0),
        ];
        ensure(
            bounds.iter().all(|p| p.x.abs() < 1e8 && p.y.abs() < 1e8),
            "room tag exceeds screen range",
        )?;
        if let Some(crop) = context.crop {
            // A symbol anchored in the crop remains visible at crop edges, with
            // its badge clipped to the same crop used for geometry and picking.
            if !point_in_crop(crop, self.anchor) {
                return Ok(None);
            }
            let crop_min = camera.project(Point2::new(crop.min.x, crop.max.y), viewport)?;
            let crop_max = camera.project(Point2::new(crop.max.x, crop.min.y), viewport)?;
            bounds[0].x = bounds[0].x.max(crop_min.x);
            bounds[0].y = bounds[0].y.max(crop_min.y);
            bounds[1].x = bounds[1].x.min(crop_max.x);
            bounds[1].y = bounds[1].y.min(crop_max.y);
            if bounds[0].x >= bounds[1].x || bounds[0].y >= bounds[1].y {
                return Ok(None);
            }
        }
        Ok(Some(bounds))
    }

    pub fn hit(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
    ) -> Result<bool> {
        ensure(pointer.is_finite(), "invalid room tag pointer")?;
        Ok(self
            .bounds(context, camera, viewport)?
            .is_some_and(|[a, b]| {
                pointer.x >= a.x && pointer.y >= a.y && pointer.x <= b.x && pointer.y <= b.y
            }))
    }
}

impl PlanDrawing {
    pub fn room_tags(&self, current: PlanContext) -> Result<&[PlanRoomTag]> {
        self.items(current)?;
        Ok(&self.room_tags)
    }
    pub fn with_room_tags(mut self, mut tags: Vec<PlanRoomTag>) -> Result<Self> {
        ensure(self.room_tags.is_empty(), "room tags already attached")?;
        ensure(
            tags.len()
                .saturating_add(self.items.len())
                .saturating_add(self.lines.len())
                .saturating_add(self.detail_segment_count)
                .saturating_add(self.grids.len())
                .saturating_add(self.floors.len())
                .saturating_add(self.rooms.len())
                .saturating_add(
                    self.dimensions
                        .iter()
                        .map(PlanDimensionItem::graphic_count)
                        .sum::<usize>(),
                )
                .saturating_add(self.angular_dimensions.len().saturating_mul(67))
                <= MAX_PLAN_ELEMENTS,
            "room tags exceed plan graphics budget",
        )?;
        let mut pairs = BTreeSet::new();
        for tag in &tags {
            tag.validate()?;
            ensure(
                tag.view == self.context.view_id && pairs.insert((tag.view, tag.room)),
                "invalid room tag view or duplicate target",
            )?;
            ensure(
                !self.rooms.iter().any(|d| d.entity == tag.entity)
                    && !self.dimensions.iter().any(|d| d.entity == tag.entity)
                    && !self
                        .angular_dimensions
                        .iter()
                        .any(|d| d.entity == tag.entity)
                    && self.source_ids.insert(tag.entity),
                "duplicate room tag identity",
            )?;
        }
        tags.sort_by_key(|tag| tag.entity);
        self.room_tags = tags;
        Ok(self)
    }
    pub fn pick_room_tag_screen(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
    ) -> Result<Option<Id>> {
        for tag in self.room_tags(context)?.iter().rev() {
            if tag.hit(context, camera, viewport, pointer)? {
                return Ok(Some(tag.entity));
            }
        }
        Ok(None)
    }
}
