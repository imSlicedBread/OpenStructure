//! Versioned semantic view settings, independent of render/kernel/UI types.
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const PLAN_SETTINGS_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanViewRange {
    pub top: f64,
    pub cut: f64,
    pub bottom: f64,
    pub depth: f64,
}
impl Default for PlanViewRange {
    fn default() -> Self {
        Self {
            top: 2.5,
            cut: 1.2,
            bottom: 0.0,
            depth: -1.0,
        }
    }
}
impl PlanViewRange {
    pub fn validate(self) -> Result<()> {
        ensure(
            [self.top, self.cut, self.bottom, self.depth]
                .iter()
                .all(|n| n.is_finite())
                && self.depth <= self.bottom
                && self.bottom <= self.cut
                && self.cut <= self.top
                && self.top - self.depth > 1e-9
                && (self.top - self.depth).is_finite(),
            "plan range needs finite depth <= bottom <= cut <= top and positive span",
        )
    }
    pub fn at_level(self, elevation: f64) -> Result<Self> {
        self.validate()?;
        ensure(elevation.is_finite(), "plan level elevation is not finite")?;
        let absolute = Self {
            top: self.top + elevation,
            cut: self.cut + elevation,
            bottom: self.bottom + elevation,
            depth: self.depth + elevation,
        };
        absolute.validate()?;
        Ok(absolute)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanViewBasis {
    #[serde(deserialize_with = "strict_point")]
    pub origin: Point2,
    /// Right-handed horizontal rotation in radians, not a navigation camera.
    pub rotation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanViewCrop {
    #[serde(deserialize_with = "strict_point")]
    pub min: Point2,
    #[serde(deserialize_with = "strict_point")]
    pub max: Point2,
}

fn strict_point<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Point2, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Point {
        x: f64,
        y: f64,
    }
    let point = Point::deserialize(deserializer)?;
    Ok(Point2::new(point.x, point.y))
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanVisibility {
    pub walls: bool,
    pub extensions: bool,
}
impl Default for PlanVisibility {
    fn default() -> Self {
        Self {
            walls: true,
            extensions: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSettings {
    pub schema_version: u32,
    pub range: PlanViewRange,
    pub basis: PlanViewBasis,
    pub crop: Option<PlanViewCrop>,
    /// Paper/model ratio 1:N, distinct from navigation zoom and screen DPI.
    pub scale_denominator: f64,
    pub visibility: PlanVisibility,
}
impl Default for PlanSettings {
    fn default() -> Self {
        Self {
            schema_version: PLAN_SETTINGS_VERSION,
            range: PlanViewRange::default(),
            basis: PlanViewBasis::default(),
            crop: None,
            scale_denominator: 100.0,
            visibility: PlanVisibility::default(),
        }
    }
}
impl PlanSettings {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.schema_version == PLAN_SETTINGS_VERSION,
            "unsupported plan settings version",
        )?;
        self.range.validate()?;
        ensure(
            self.basis.origin.is_finite() && self.basis.rotation.is_finite(),
            "invalid plan basis",
        )?;
        if let Some(crop) = self.crop {
            ensure(
                crop.min.is_finite()
                    && crop.max.is_finite()
                    && crop.min.x < crop.max.x
                    && crop.min.y < crop.max.y
                    && (crop.max.x - crop.min.x).is_finite()
                    && (crop.max.y - crop.min.y).is_finite(),
                "invalid plan crop",
            )?;
        }
        ensure(
            self.scale_denominator.is_finite()
                && (0.001..=1_000_000.0).contains(&self.scale_denominator),
            "plan scale denominator must be between 0.001 and 1000000",
        )
    }
}
