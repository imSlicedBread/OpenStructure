//! One-plane roofs. Thickness is vertical; slope is signed rise per horizontal run.
use crate::{Entity, FloorParams, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoofParams {
    pub name: String,
    pub level: Id,
    pub material: Option<Id>,
    pub boundary: Vec<Point2>,
    pub holes: Vec<Vec<Point2>>,
    pub thickness: f64,
    pub top_offset: f64,
    /// Top surface here is exactly level elevation + top_offset.
    pub slope_start: Point2,
    /// Direction only: arrow length does not change the gradient.
    pub slope_end: Point2,
    pub rise_per_run: f64,
}
pub type Roof = Entity<RoofParams>;

impl RoofParams {
    pub fn footprint(&self) -> FloorParams {
        FloorParams {
            name: self.name.clone(),
            level: self.level,
            material: self.material,
            boundary: self.boundary.clone(),
            holes: self.holes.clone(),
            thickness: self.thickness,
            top_offset: self.top_offset,
        }
    }

    pub fn top_at(&self, point: Point2, level_z: f64) -> f64 {
        let run = self.slope_start.distance(self.slope_end);
        level_z
            + self.top_offset
            + self.rise_per_run
                * ((point.x - self.slope_start.x) * (self.slope_end.x - self.slope_start.x) / run
                    + (point.y - self.slope_start.y) * (self.slope_end.y - self.slope_start.y)
                        / run)
    }

    pub fn validate_at(&self, level_z: f64) -> Result<()> {
        self.footprint().validate()?;
        ensure(
            [self.slope_start, self.slope_end]
                .iter()
                .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
            "invalid roof slope arrow coordinates",
        )?;
        let run = self.slope_start.distance(self.slope_end);
        ensure(
            run > 1e-6 && run <= 1e6,
            "roof slope arrow needs a bounded nonzero run",
        )?;
        ensure(
            self.rise_per_run.is_finite() && self.rise_per_run.abs() <= 100.0,
            "roof slope must be finite and within +/-100 rise per run",
        )?;
        ensure(
            level_z.is_finite() && level_z.abs() <= 1e6,
            "invalid roof level elevation",
        )?;
        for point in self
            .boundary
            .iter()
            .chain(self.holes.iter().flatten())
            .chain([&self.slope_start, &self.slope_end])
        {
            let top = self.top_at(*point, level_z);
            let bottom = top - self.thickness;
            ensure(
                top.is_finite()
                    && bottom.is_finite()
                    && top.abs() <= 1e6
                    && bottom.abs() <= 1e6
                    && top > bottom,
                "roof elevations exceed finite coordinate bounds",
            )?;
        }
        Ok(())
    }

    pub fn validate_in(&self, model: &Model) -> Result<()> {
        let level = model
            .levels
            .get(&self.level)
            .ok_or_else(|| os_core::Error::Invalid("roof level missing".into()))?;
        ensure(
            self.material
                .is_none_or(|id| model.materials.contains_key(&id)),
            "roof material missing",
        )?;
        self.validate_at(level.parameters.elevation)
    }
}
