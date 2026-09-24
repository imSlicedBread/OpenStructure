//! Bounded derived angular graphics; the exact same segments and label bounds paint and pick.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanAngularDimension {
    pub entity: Id,
    pub center: Point2,
    pub radius: f64,
    pub start_radians: f64,
    pub sweep_radians: f64,
    pub orphan_hint: Point2,
    pub diagnostic: Option<String>,
}

impl PlanAngularDimension {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.entity.0.is_nil()
                && self.center.is_finite()
                && self.orphan_hint.is_finite()
                && self.radius.is_finite()
                && self.radius > 1e-6
                && self.radius <= 1_000_000.0
                && self.start_radians.is_finite()
                && self.sweep_radians.is_finite()
                && self.sweep_radians.abs() > 1e-6
                && self.sweep_radians.abs() < std::f64::consts::PI
                && self
                    .diagnostic
                    .as_ref()
                    .is_none_or(|d| !d.is_empty() && d.len() <= 256),
            "invalid angular graphic",
        )
    }
    pub fn point(&self, fraction: f64) -> Point2 {
        let a = self.start_radians + self.sweep_radians * fraction;
        Point2::new(
            self.center.x + self.radius * a.cos(),
            self.center.y + self.radius * a.sin(),
        )
    }
    pub fn label(&self) -> String {
        self.diagnostic.as_ref().map_or_else(
            || format!("{:.2}°", self.sweep_radians.abs().to_degrees()),
            |_| "Angle lost reference".into(),
        )
    }
    /// At most 64 arc chords and two radial witness lines, including the intersection.
    pub fn segments(&self, context: PlanContext) -> Result<Vec<(Point2, Point2)>> {
        self.validate()?;
        let raw: Vec<_> = if self.diagnostic.is_some() {
            // Model-space diagnostic cross, clipped identically for painting and picking.
            let p = self.orphan_hint;
            vec![
                (
                    Point2::new(p.x - 0.04, p.y - 0.04),
                    Point2::new(p.x + 0.04, p.y + 0.04),
                ),
                (
                    Point2::new(p.x - 0.04, p.y + 0.04),
                    Point2::new(p.x + 0.04, p.y - 0.04),
                ),
            ]
        } else {
            let mut lines: Vec<_> = (0..64)
                .map(|i| {
                    (
                        self.point(f64::from(i) / 64.0),
                        self.point(f64::from(i + 1) / 64.0),
                    )
                })
                .collect();
            lines.extend([
                (self.center, self.point(0.0)),
                (self.center, self.point(1.0)),
            ]);
            lines
        };
        raw.into_iter()
            .filter_map(|(a, b)| crop_dimension_segment(context, a, b).transpose())
            .collect()
    }
    pub fn label_bounds(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
    ) -> Result<Option<[Point2; 2]>> {
        let p = if self.diagnostic.is_some() {
            self.orphan_hint
        } else {
            self.point(0.5)
        };
        let p = camera.project(p, viewport)?;
        let half_width = self.label().chars().count() as f64 * 4.0;
        let bounds = [
            Point2::new(p.x - half_width, p.y - 23.0),
            Point2::new(p.x + half_width, p.y - 7.0),
        ];
        if let Some(crop) = context.crop {
            for corner in [
                bounds[0],
                bounds[1],
                Point2::new(bounds[0].x, bounds[1].y),
                Point2::new(bounds[1].x, bounds[0].y),
            ] {
                if !point_in_crop(crop, camera.unproject(corner, viewport)?) {
                    return Ok(None);
                }
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
        radius: f64,
    ) -> Result<bool> {
        for (a, b) in self.segments(context)? {
            if point_segment_distance(
                pointer,
                camera.project(a, viewport)?,
                camera.project(b, viewport)?,
            ) <= radius
            {
                return Ok(true);
            }
        }
        Ok(self
            .label_bounds(context, camera, viewport)?
            .is_some_and(|[a, b]| {
                pointer.x >= a.x && pointer.x <= b.x && pointer.y >= a.y && pointer.y <= b.y
            }))
    }
}
