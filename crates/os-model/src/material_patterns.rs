//! Project-owned model-scale parallel cut hatches for native sections.
use super::*;

pub const MAX_MATERIAL_PATTERNS: usize = 256;
pub type MaterialPattern = Entity<MaterialPatternParams>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialPatternParams {
    pub name: String,
    /// Counterclockwise from section station toward elevation, in degrees.
    pub angle_degrees: f64,
    /// Perpendicular distance between parallel lines, in model metres.
    pub spacing: f64,
    /// Shared world XYZ anchor, projected into each section's station/elevation.
    pub origin: [f64; 3],
}

impl MaterialPatternParams {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "pattern name must be 1-256 bytes without control characters",
        )?;
        ensure(
            self.angle_degrees.is_finite() && (0.0..180.0).contains(&self.angle_degrees),
            "pattern angle must be finite and in [0, 180) degrees",
        )?;
        ensure(
            self.spacing.is_finite() && (0.001..=1000.0).contains(&self.spacing),
            "pattern spacing must be 0.001-1000 model metres",
        )?;
        ensure(
            self.origin
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.0),
            "pattern origin must be finite and within ±1000000 metres",
        )
    }
}
