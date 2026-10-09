//! World-oriented facade views, separate from host-local elevations.
use crate::ElevationSide;
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FacadeViewSettings {
    pub schema_version: u32,
    #[serde(deserialize_with = "strict_point")]
    pub origin: Point2,
    /// Unit horizontal world axis. Never follows a wall's edited direction.
    #[serde(deserialize_with = "strict_point")]
    pub axis: Point2,
    pub side: ElevationSide,
    pub marker_length: f64,
    pub min_station: f64,
    pub max_station: f64,
    pub bottom_elevation: f64,
    pub top_elevation: f64,
    /// Positive distance looking away from the baseline, opposite the viewer side.
    pub near_depth: f64,
    pub far_depth: f64,
}
impl FacadeViewSettings {
    pub fn new(start: Point2, end: Point2, bottom: f64, top: f64) -> Result<Self> {
        let length = start.distance(end);
        let result = Self {
            schema_version: 1,
            origin: start,
            axis: Point2::new((end.x - start.x) / length, (end.y - start.y) / length),
            side: ElevationSide::Left,
            marker_length: length,
            min_station: 0.,
            max_station: length,
            bottom_elevation: bottom,
            top_elevation: top,
            near_depth: 0.,
            far_depth: 30.,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn validate(self) -> Result<()> {
        ensure(
            self.schema_version == 1,
            "unsupported facade settings version",
        )?;
        ensure(
            self.origin.is_finite()
                && self.axis.is_finite()
                && (self.axis.x.hypot(self.axis.y) - 1.).abs() <= 1e-8
                && [
                    self.origin.x,
                    self.origin.y,
                    self.marker_length,
                    self.min_station,
                    self.max_station,
                    self.bottom_elevation,
                    self.top_elevation,
                    self.near_depth,
                    self.far_depth,
                ]
                .into_iter()
                .all(|v| v.is_finite() && v.abs() <= 1e6)
                && self.marker_length > 1e-6
                && self.max_station > self.min_station
                && self.top_elevation > self.bottom_elevation
                && self.near_depth >= 0.
                && self.far_depth > self.near_depth,
            "facade requires a finite unit world axis, positive crop/marker and ordered nonnegative near/far depths",
        )?;
        let end = self.marker_end();
        ensure(
            end.is_finite() && end.x.abs() <= 1e6 && end.y.abs() <= 1e6,
            "facade marker exceeds world coordinate envelope",
        )
    }
    pub fn marker_end(self) -> Point2 {
        Point2::new(
            self.origin.x + self.axis.x * self.marker_length,
            self.origin.y + self.axis.y * self.marker_length,
        )
    }
    pub fn world_point(self, station: f64, depth: f64) -> Point2 {
        let sign = if self.side == ElevationSide::Left {
            -1.
        } else {
            1.
        };
        Point2::new(
            self.origin.x + self.axis.x * station - self.axis.y * sign * depth,
            self.origin.y + self.axis.y * station + self.axis.x * sign * depth,
        )
    }
}
fn strict_point<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Point2, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Point {
        x: f64,
        y: f64,
    }
    let p = Point::deserialize(d)?;
    Ok(Point2::new(p.x, p.y))
}
