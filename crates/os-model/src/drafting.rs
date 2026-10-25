//! Standalone annotation coordinates; deliberately has no model/level association.
use crate::PlanViewCrop;
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftingViewSettings {
    pub crop: PlanViewCrop,
    pub scale_denominator: f64,
}
impl Default for DraftingViewSettings {
    fn default() -> Self {
        Self {
            crop: PlanViewCrop {
                min: Point2::new(-5., -5.),
                max: Point2::new(5., 5.),
            },
            scale_denominator: 50.,
        }
    }
}
impl DraftingViewSettings {
    pub fn validate(self) -> Result<()> {
        ensure(
            [self.crop.min, self.crop.max]
                .iter()
                .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6)
                && self.crop.max.x - self.crop.min.x > 1e-6
                && self.crop.max.y - self.crop.min.y > 1e-6,
            "drafting crop must be a finite positive rectangle within ±1000000 m",
        )?;
        ensure(
            self.scale_denominator.is_finite() && (1.0..=10000.).contains(&self.scale_denominator),
            "drafting scale must be within 1:1–1:10000",
        )
    }
}
