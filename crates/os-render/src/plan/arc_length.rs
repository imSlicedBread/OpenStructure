//! Full analytic wall-axis length with bounded, shared paint/pick strokes.
use super::*;

pub const ARC_LENGTH_SEGMENTS: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanArcLengthDimension {
    pub entity: Id,
    pub center: Point2,
    pub axis_radius: f64,
    pub radius: f64,
    pub start_radians: f64,
    pub sweep_radians: f64,
    pub value_m: f64,
    pub orphan_hint: Point2,
    pub diagnostic: Option<String>,
}

impl PlanArcLengthDimension {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.entity.0.is_nil()
                && self.center.is_finite()
                && self.orphan_hint.is_finite()
                && self.axis_radius.is_finite()
                && self.axis_radius > 1e-6
                && self.radius.is_finite()
                && self.radius > 1e-6
                && self.radius <= 1_000_000.0
                && self.start_radians.is_finite()
                && self.sweep_radians.is_finite()
                && self.sweep_radians.abs() > 1e-6
                && self.sweep_radians.abs() < std::f64::consts::TAU
                && self.value_m.is_finite()
                && self.value_m > 1e-6
                && self
                    .diagnostic
                    .as_ref()
                    .is_none_or(|d| !d.is_empty() && d.len() <= 256),
            "invalid arc-length graphic",
        )
    }
    pub fn point(&self, fraction: f64) -> Point2 {
        self.at_radius(fraction, self.radius)
    }
    fn at_radius(&self, fraction: f64, radius: f64) -> Point2 {
        let a = self.start_radians + self.sweep_radians * fraction;
        Point2::new(
            self.center.x + radius * a.cos(),
            self.center.y + radius * a.sin(),
        )
    }
    pub fn label(&self) -> String {
        if self.diagnostic.is_some() {
            "Arc length lost reference".into()
        } else {
            format!("{:.3} m", self.value_m)
        }
    }
    pub fn segments(&self, context: PlanContext) -> Result<Vec<(Point2, Point2)>> {
        self.validate()?;
        let raw = if self.diagnostic.is_some() {
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
            let mut lines: Vec<_> = (0..ARC_LENGTH_SEGMENTS)
                .map(|i| {
                    (
                        self.point(i as f64 / ARC_LENGTH_SEGMENTS as f64),
                        self.point((i + 1) as f64 / ARC_LENGTH_SEGMENTS as f64),
                    )
                })
                .collect();
            for t in [0.0, 1.0] {
                lines.push((self.at_radius(t, self.axis_radius), self.point(t)));
            }
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
        let p = camera.project(
            if self.diagnostic.is_some() {
                self.orphan_hint
            } else {
                self.point(0.5)
            },
            viewport,
        )?;
        let half = self.label().chars().count() as f64 * 4.0;
        let bounds = [
            Point2::new(p.x - half, p.y - 23.0),
            Point2::new(p.x + half, p.y - 7.0),
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
