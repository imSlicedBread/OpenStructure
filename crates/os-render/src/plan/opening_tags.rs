//! Fixed, bounded screen-space badges shared by paint, hover and pick.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanOpeningTag {
    pub entity: Id,
    pub view: Id,
    pub opening: Id,
    /// Plan-plane metres, transformed from world XY by the snapshot producer.
    pub anchor: Point2,
    /// Derived opening center in the owning plan plane; absent for orphan tags.
    pub leader_source: Option<Point2>,
    pub label: String,
    pub diagnostic: Option<String>,
}

impl PlanOpeningTag {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.entity.0.is_nil()
                && !self.view.0.is_nil()
                && !self.opening.0.is_nil()
                && self.anchor.is_finite()
                && self.anchor.x.abs() <= 2e6
                && self.anchor.y.abs() <= 2e6
                && self
                    .leader_source
                    .is_none_or(|p| p.is_finite() && p.x.abs() <= 2e6 && p.y.abs() <= 2e6)
                && !self.label.is_empty()
                && self.label.len() <= 600
                && !self.label.chars().any(char::is_control)
                && self.diagnostic.as_ref().is_none_or(|d| {
                    !d.is_empty() && d.len() <= 256 && !d.chars().any(char::is_control)
                }),
            "invalid opening tag graphic",
        )
    }

    /// Visible badge bounds, also used for text clipping and badge-only picking.
    pub fn bounds(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
    ) -> Result<Option<[Point2; 2]>> {
        self.validate()?;
        ensure(
            self.view == context.view_id,
            "opening tag belongs to another view",
        )?;
        let p = camera.project(self.anchor, viewport)?;
        let half = (self.label.chars().count() as f64 * 4.5 + 10.0).clamp(24.0, 160.0);
        let mut bounds = [
            Point2::new(p.x - half, p.y - 12.0),
            Point2::new(p.x + half, p.y + 12.0),
        ];
        ensure(
            bounds.iter().all(|p| p.x.abs() < 1e8 && p.y.abs() < 1e8),
            "opening tag exceeds screen range",
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

    /// Screen-space leader, clipped to both the view crop and the canvas.
    /// Use the visible badge box, matching the centered text at crop edges.
    pub fn leader(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
    ) -> Result<Option<(Point2, Point2)>> {
        let Some(bounds) = self.bounds(context, camera, viewport)? else {
            return Ok(None);
        };
        let Some(source) = self.leader_source.filter(|_| self.diagnostic.is_none()) else {
            return Ok(None);
        };
        let Some((a, b)) = Self::leader_to_box(camera.project(source, viewport)?, bounds, 0.5)
        else {
            return Ok(None);
        };
        let mut clip = PlanCrop {
            min: Point2::default(),
            max: Point2::new(viewport[0], viewport[1]),
        };
        if let Some(crop) = context.crop {
            let min = camera.project(Point2::new(crop.min.x, crop.max.y), viewport)?;
            let max = camera.project(Point2::new(crop.max.x, crop.min.y), viewport)?;
            clip.min.x = clip.min.x.max(min.x);
            clip.min.y = clip.min.y.max(min.y);
            clip.max.x = clip.max.x.min(max.x);
            clip.max.y = clip.max.y.min(max.y);
        }
        if clip.min.x >= clip.max.x || clip.min.y >= clip.max.y {
            return Ok(None);
        }
        grids::clip(a, b, Some(clip))
    }

    /// Intersect the source-to-box-center ray with the nearest box side.
    /// Coordinates and touch tolerance use the caller's units (pixels or mm).
    pub fn leader_to_box(
        source: Point2,
        [min, max]: [Point2; 2],
        touch: f64,
    ) -> Option<(Point2, Point2)> {
        if !source.is_finite()
            || !min.is_finite()
            || !max.is_finite()
            || min.x >= max.x
            || min.y >= max.y
            || !touch.is_finite()
            || touch < 0.0
            || (source.x >= min.x - touch
                && source.x <= max.x + touch
                && source.y >= min.y - touch
                && source.y <= max.y + touch)
        {
            return None;
        }
        let center = Point2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);
        let dx = source.x - center.x;
        let dy = source.y - center.y;
        let ratio = ((max.x - min.x) * 0.5 / dx.abs()).min((max.y - min.y) * 0.5 / dy.abs());
        Some((
            source,
            Point2::new(center.x + dx * ratio, center.y + dy * ratio),
        ))
    }

    pub fn hit(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
    ) -> Result<bool> {
        ensure(pointer.is_finite(), "invalid opening tag pointer")?;
        Ok(self
            .bounds(context, camera, viewport)?
            .is_some_and(|[a, b]| {
                pointer.x >= a.x && pointer.y >= a.y && pointer.x <= b.x && pointer.y <= b.y
            }))
    }
}

impl PlanDrawing {
    pub fn opening_tags(&self, current: PlanContext) -> Result<&[PlanOpeningTag]> {
        self.items(current)?;
        Ok(&self.opening_tags)
    }
    pub fn with_opening_tags(mut self, mut tags: Vec<PlanOpeningTag>) -> Result<Self> {
        ensure(
            self.opening_tags.is_empty(),
            "opening tags already attached",
        )?;
        ensure(
            tags.len()
                .saturating_add(self.items.len())
                .saturating_add(self.lines.len())
                .saturating_add(self.detail_segment_count)
                .saturating_add(self.grids.len())
                .saturating_add(self.floors.len())
                .saturating_add(self.rooms.len())
                .saturating_add(self.room_tags.len())
                .saturating_add(
                    self.dimensions
                        .iter()
                        .map(PlanDimensionItem::graphic_count)
                        .sum::<usize>(),
                )
                .saturating_add(self.angular_dimensions.len().saturating_mul(67))
                <= MAX_PLAN_ELEMENTS,
            "opening tags exceed plan graphics budget",
        )?;
        let mut pairs = BTreeSet::new();
        for tag in &tags {
            tag.validate()?;
            ensure(
                tag.view == self.context.view_id && pairs.insert((tag.view, tag.opening)),
                "invalid opening tag view or duplicate target",
            )?;
            ensure(
                !self.rooms.iter().any(|d| d.entity == tag.entity)
                    && !self.dimensions.iter().any(|d| d.entity == tag.entity)
                    && !self
                        .angular_dimensions
                        .iter()
                        .any(|d| d.entity == tag.entity)
                    && self.source_ids.insert(tag.entity),
                "duplicate opening tag identity",
            )?;
        }
        tags.sort_by_key(|tag| tag.entity);
        self.opening_tags = tags;
        Ok(self)
    }
    pub fn pick_opening_tag_screen(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
    ) -> Result<Option<Id>> {
        for tag in self.opening_tags(context)?.iter().rev() {
            if tag.hit(context, camera, viewport, pointer)? {
                return Ok(Some(tag.entity));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_tag_leader_box_sides_corners_and_touch() {
        let bounds = [Point2::new(-10., -5.), Point2::new(10., 5.)];
        for (source, end) in [
            (Point2::new(-30., 0.), Point2::new(-10., 0.)),
            (Point2::new(30., 0.), Point2::new(10., 0.)),
            (Point2::new(0., -30.), Point2::new(0., -5.)),
            (Point2::new(0., 30.), Point2::new(0., 5.)),
            (Point2::new(30., 15.), Point2::new(10., 5.)),
        ] {
            assert_eq!(
                PlanOpeningTag::leader_to_box(source, bounds, 0.5),
                Some((source, end))
            );
        }
        for source in [
            Point2::default(),
            Point2::new(10., 3.),
            Point2::new(-10.4, 0.),
            Point2::new(0., 5.5),
        ] {
            assert_eq!(PlanOpeningTag::leader_to_box(source, bounds, 0.5), None);
        }
    }

    #[test]
    fn opening_tag_leader_crop_canvas_orphan_and_badge_only_pick() {
        let mut context = PlanContext {
            session_id: Id::new(),
            model_revision: 0,
            view_id: Id::new(),
            settings_revision: 0,
            basis: Default::default(),
            range: Default::default(),
            crop: None,
            scale_denominator: 100.,
            show_walls: true,
            show_extensions: true,
        };
        let camera = PlanCamera {
            center: Point2::default(),
            pixels_per_metre: 100.,
        };
        let size = [800., 600.];
        let mut tag = PlanOpeningTag {
            entity: Id::new(),
            view: context.view_id,
            opening: Id::new(),
            anchor: Point2::default(),
            leader_source: Some(Point2::new(-10., 0.)),
            label: "D1".into(),
            diagnostic: None,
        };
        assert_eq!(
            tag.leader(context, camera, size).unwrap(),
            Some((Point2::new(0., 300.), Point2::new(376., 300.)))
        );
        context.crop = Some(PlanCrop {
            min: Point2::new(-2., -2.),
            max: Point2::new(2., 2.),
        });
        assert_eq!(
            tag.leader(context, camera, size).unwrap(),
            Some((Point2::new(200., 300.), Point2::new(376., 300.)))
        );
        let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_opening_tags(vec![tag.clone()])
            .unwrap();
        assert_eq!(
            drawing
                .pick_opening_tag_screen(context, camera, size, Point2::new(300., 300.))
                .unwrap(),
            None
        );
        assert_eq!(
            drawing
                .pick_opening_tag_screen(context, camera, size, Point2::new(400., 300.))
                .unwrap(),
            Some(tag.entity)
        );
        tag.anchor = Point2::new(3., 0.);
        assert!(tag.bounds(context, camera, size).unwrap().is_none());
        assert!(tag.leader(context, camera, size).unwrap().is_none());
        tag.anchor = Point2::default();
        tag.leader_source = None;
        tag.diagnostic = Some("Missing opening".into());
        assert!(tag.bounds(context, camera, size).unwrap().is_some());
        assert!(tag.leader(context, camera, size).unwrap().is_none());
        tag.leader_source = Some(Point2::new(f64::NAN, 0.));
        assert!(tag.validate().is_err());
    }
}
