//! Architectural straight datums, independent of a view's decorative lattice.
use crate::Entity;
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub type Grid = Entity<GridParams>;

/// Building-scoped vertical datum plane through a directed XY axis in metres.
/// Endpoints define finite model extents, not a wall, elevation or view override.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridParams {
    pub name: String,
    pub building: Id,
    #[serde(with = "point")]
    pub start: Point2,
    #[serde(with = "point")]
    pub end: Point2,
}

impl GridParams {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.is_empty()
                && self.name.trim() == self.name
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "grid name must be 1-256 bytes, trimmed and without control characters",
        )?;
        ensure(!self.building.0.is_nil(), "grid building is nil")?;
        ensure(
            self.start.is_finite() && self.end.is_finite(),
            "grid endpoints must be finite",
        )?;
        let length = self.start.distance(self.end);
        ensure(
            length.is_finite() && length > 1e-6,
            "grid extent must exceed one micrometre",
        )
    }
}

// Do not silently discard a supplied z coordinate or other unsupported fields.
mod point {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct XY {
        x: f64,
        y: f64,
    }
    pub fn serialize<S: serde::Serializer>(
        p: &Point2,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        XY { x: p.x, y: p.y }.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Point2, D::Error> {
        let p = XY::deserialize(d)?;
        Ok(Point2::new(p.x, p.y))
    }
}
