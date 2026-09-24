//! Level-owned straight XY room boundaries. No thickness or solid geometry.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_ROOM_SEPARATION_LINES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomSeparationLineParams {
    pub level: Id,
    pub start: Point2,
    pub end: Point2,
}
pub type RoomSeparationLine = Entity<RoomSeparationLineParams>;

impl RoomSeparationLineParams {
    pub fn validate(&self, model: &Model) -> Result<()> {
        ensure(
            model.levels.contains_key(&self.level),
            "room separator level missing",
        )?;
        ensure(
            [self.start, self.end]
                .iter()
                .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
            "room separator coordinates must be finite and within +/- 1000000 metres",
        )?;
        let lattice = |p: Point2| ((p.x / 1e-6).round() as i64, (p.y / 1e-6).round() as i64);
        ensure(
            lattice(self.start) != lattice(self.end),
            "room separator endpoints must differ on the 1 micrometre lattice",
        )
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    ensure(
        model.room_separation_lines.len() <= MAX_ROOM_SEPARATION_LINES,
        "room separators exceed 10000 elements",
    )?;
    for line in model.room_separation_lines.values() {
        line.parameters.validate(model)?;
    }
    // Enclosure and the combined 1024-segment topology budget are derived
    // diagnostics. Neither may prevent otherwise valid model edits.
    Ok(())
}
