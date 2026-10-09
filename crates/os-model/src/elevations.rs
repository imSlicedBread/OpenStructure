//! Persisted crops for elevations linked to a native straight wall.
use crate::{Model, SCHEMA_VERSION, WallParams};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};

pub const ELEVATION_SETTINGS_VERSION: u32 = 1;

/// Wall-local side of the current directed host. Reversing start/end flips the
/// physical viewed face; it does not preserve a world-facing direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElevationSide {
    Left,
    Right,
}

/// Metres along the current wall start -> end on both sides, and absolute project
/// elevations. The host UUID survives moves, rotations and endpoint reversal.
/// On reversal station zero moves to the new start and crop offsets keep their
/// numeric values relative to that start. No world-facing direction is stored.
/// Crops may include padding beyond the host. No derived geometry is persisted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElevationViewSettings {
    pub schema_version: u32,
    pub host: Id,
    pub side: ElevationSide,
    pub min_station: f64,
    pub max_station: f64,
    pub bottom_elevation: f64,
    pub top_elevation: f64,
}

impl ElevationViewSettings {
    pub fn new(
        host: Id,
        side: ElevationSide,
        min_station: f64,
        max_station: f64,
        bottom_elevation: f64,
        top_elevation: f64,
    ) -> Self {
        Self {
            schema_version: ELEVATION_SETTINGS_VERSION,
            host,
            side,
            min_station,
            max_station,
            bottom_elevation,
            top_elevation,
        }
    }

    /// Checks authored data only, allowing missing or subsequently curved hosts
    /// to remain repairable persisted orphans.
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.schema_version == ELEVATION_SETTINGS_VERSION,
            "unsupported elevation settings version",
        )?;
        ensure(
            !self.host.0.is_nil(),
            "elevation host identity must not be nil",
        )?;
        ensure(
            [
                self.min_station,
                self.max_station,
                self.bottom_elevation,
                self.top_elevation,
            ]
            .into_iter()
            .all(|value| value.is_finite() && value.abs() <= 1e6)
                && self.max_station > self.min_station
                && self.top_elevation > self.bottom_elevation,
            "elevation crop must be finite, positive and within +/- 1000000 metres",
        )
    }

    /// Resolve current effective native parameters, including compound-wall thickness.
    /// Creation, edits and derivation call this; passive model validation does not.
    pub fn resolve_host(&self, model: &Model) -> Result<WallParams> {
        self.validate()?;
        let wall = model
            .walls
            .get(&self.host)
            .ok_or_else(|| os_core::Error::Invalid("elevation host wall missing".into()))?;
        ensure(
            wall.id() == self.host
                && wall.header.type_id == "org.openstructure.walls.wall"
                && wall.header.schema_version == SCHEMA_VERSION,
            "elevation host must be a current native wall",
        )?;
        ensure(
            wall.parameters.path.is_straight(),
            "elevation host wall must be straight",
        )?;
        let parameters = model.resolve_wall(self.host)?.parameters;
        ensure(
            model.levels.contains_key(&parameters.level),
            "elevation host level missing",
        )?;
        Ok(parameters)
    }
}
