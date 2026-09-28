//! Versioned semantic view settings, independent of render/kernel/UI types.
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const PLAN_SETTINGS_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanViewType {
    #[default]
    FloorPlan,
    ReflectedCeilingPlan,
}

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

    /// Convert saved floor/RCP range semantics to the geometry kernel's
    /// bottom-to-top ordering, then make the range absolute to its level.
    pub fn at_level_for_view(self, view_type: PlanViewType, elevation: f64) -> Result<Self> {
        let ordered = match view_type {
            PlanViewType::FloorPlan => {
                self.validate()?;
                self
            }
            PlanViewType::ReflectedCeilingPlan => {
                ensure(
                    [self.bottom, self.cut, self.top, self.depth]
                        .iter()
                        .all(|value| value.is_finite())
                        && self.bottom <= self.cut
                        && self.cut <= self.top
                        && self.top <= self.depth
                        && self.depth - self.bottom > 1e-9
                        && (self.depth - self.bottom).is_finite(),
                    "RCP range needs finite bottom <= cut <= top <= depth and positive span",
                )?;
                Self {
                    top: self.depth,
                    cut: self.top,
                    bottom: self.cut,
                    depth: self.bottom,
                }
            }
        };
        ordered.at_level(elevation)
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
    pub doors: bool,
    pub windows: bool,
    pub ceilings: bool,
    pub extensions: bool,
}
impl Default for PlanVisibility {
    fn default() -> Self {
        Self {
            walls: true,
            doors: true,
            windows: true,
            ceilings: true,
            extensions: true,
        }
    }
}

impl PlanVisibility {
    pub fn shows_opening(self, kind: crate::OpeningKind) -> bool {
        match kind {
            crate::OpeningKind::Door => self.doors,
            crate::OpeningKind::Window => self.windows,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSettings {
    pub schema_version: u32,
    /// None follows the latest project phase; Some pins a stable phase identity.
    #[serde(deserialize_with = "required_phase")]
    pub target_phase: Option<os_core::Id>,
    pub phase_filter: crate::PhaseFilter,
    pub view_type: PlanViewType,
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
            target_phase: None,
            phase_filter: crate::PhaseFilter::ShowAll,
            view_type: PlanViewType::FloorPlan,
            range: PlanViewRange::default(),
            basis: PlanViewBasis::default(),
            crop: None,
            scale_denominator: 100.0,
            visibility: PlanVisibility::default(),
        }
    }
}

fn required_phase<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<os_core::Id>, D::Error> {
    Option::<os_core::Id>::deserialize(deserializer)
}
impl PlanSettings {
    pub fn reflected_ceiling() -> Self {
        Self {
            view_type: PlanViewType::ReflectedCeilingPlan,
            range: PlanViewRange {
                bottom: 0.0,
                cut: 1.2,
                top: 3.0,
                depth: 4.0,
            },
            ..Self::default()
        }
    }

    pub fn validate(self) -> Result<()> {
        ensure(
            self.schema_version == PLAN_SETTINGS_VERSION,
            "unsupported plan settings version",
        )?;
        match self.view_type {
            PlanViewType::FloorPlan => self.range.validate()?,
            PlanViewType::ReflectedCeilingPlan => ensure(
                [
                    self.range.bottom,
                    self.range.cut,
                    self.range.top,
                    self.range.depth,
                ]
                .iter()
                .all(|n| n.is_finite())
                    && self.range.bottom <= self.range.cut
                    && self.range.cut <= self.range.top
                    && self.range.top <= self.range.depth
                    && self.range.depth - self.range.bottom > 1e-9
                    && (self.range.depth - self.range.bottom).is_finite(),
                "RCP range needs finite bottom <= cut <= top <= depth and positive span",
            )?,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_visibility_defaults_and_required_booleans() {
        let settings = PlanSettings::default();
        assert_eq!(settings.schema_version, 4);
        assert!(settings.visibility.doors && settings.visibility.windows);
        for field in ["doors", "windows"] {
            let mut value = serde_json::to_value(settings).unwrap();
            value["visibility"].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<PlanSettings>(value).is_err());
        }
    }

    #[test]
    fn reflected_range_maps_upward_semantics_to_geometry_order() {
        let settings = PlanSettings::reflected_ceiling();
        settings.validate().unwrap();
        assert_eq!(
            settings
                .range
                .at_level_for_view(settings.view_type, 4.2)
                .unwrap(),
            PlanViewRange {
                top: 8.2,
                cut: 7.2,
                bottom: 5.4,
                depth: 4.2,
            }
        );
        assert!(settings.range.at_level(4.2).is_err());
    }
}
