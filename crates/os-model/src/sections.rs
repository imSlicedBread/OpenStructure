//! Persisted, model-space definition for a linked vertical section view.
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const SECTION_SETTINGS_VERSION: u32 = 1;

/// A vertical section plane and its authored view extents, in metres.
///
/// `start` to `end` is the positive horizontal view axis used in section space;
/// `bottom_elevation` and `top_elevation` are absolute project elevations. The
/// same line is the marker shown in the associated plan view.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionViewSettings {
    pub schema_version: u32,
    #[serde(deserialize_with = "strict_point")]
    pub start: Point2,
    #[serde(deserialize_with = "strict_point")]
    pub end: Point2,
    pub bottom_elevation: f64,
    pub top_elevation: f64,
}

impl SectionViewSettings {
    pub fn new(start: Point2, end: Point2, bottom_elevation: f64, top_elevation: f64) -> Self {
        Self {
            schema_version: SECTION_SETTINGS_VERSION,
            start,
            end,
            bottom_elevation,
            top_elevation,
        }
    }

    pub fn validate(self) -> Result<()> {
        ensure(
            self.schema_version == SECTION_SETTINGS_VERSION,
            "unsupported section settings version",
        )?;
        ensure(
            self.start.is_finite()
                && self.end.is_finite()
                && self.start.x.abs() <= 1e6
                && self.start.y.abs() <= 1e6
                && self.end.x.abs() <= 1e6
                && self.end.y.abs() <= 1e6,
            "section endpoints must be finite and within the architectural coordinate envelope",
        )?;
        let length = self.start.distance(self.end);
        ensure(
            length.is_finite() && length > 1e-6 && length <= 1e7,
            "section line length must be between one micrometre and ten thousand kilometres",
        )?;
        ensure(
            self.bottom_elevation.is_finite()
                && self.top_elevation.is_finite()
                && self.bottom_elevation.abs() <= 1e6
                && self.top_elevation.abs() <= 1e6
                && self.top_elevation > self.bottom_elevation
                && (self.top_elevation - self.bottom_elevation).is_finite()
                && self.top_elevation - self.bottom_elevation <= 1e7,
            "section elevation range must be finite, positive and bounded",
        )
    }

    pub fn length(self) -> Result<f64> {
        self.validate()?;
        Ok(self.start.distance(self.end))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_settings_validate_and_round_trip() {
        let settings =
            SectionViewSettings::new(Point2::new(1.0, 2.0), Point2::new(4.0, 6.0), -1.0, 8.0);
        settings.validate().unwrap();
        assert_eq!(settings.length().unwrap(), 5.0);
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<SectionViewSettings>(&json).unwrap(),
            settings
        );
    }

    #[test]
    fn rejects_degenerate_non_finite_and_unbounded_sections() {
        assert!(
            SectionViewSettings::new(Point2::new(0.0, 0.0), Point2::new(0.0, 0.0), 0.0, 3.0)
                .validate()
                .is_err()
        );
        assert!(
            SectionViewSettings::new(
                Point2::new(0.0, 0.0),
                Point2::new(f64::INFINITY, 0.0),
                0.0,
                3.0,
            )
            .validate()
            .is_err()
        );
        assert!(
            SectionViewSettings::new(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0), 3.0, 3.0)
                .validate()
                .is_err()
        );
        assert!(
            SectionViewSettings::new(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0), 0.0, f64::MAX,)
                .validate()
                .is_err()
        );
    }

    #[test]
    fn section_points_reject_unknown_coordinates() {
        let json = r#"{"schema_version":1,"start":{"x":0.0,"y":0.0,"z":1.0},"end":{"x":1.0,"y":0.0},"bottom_elevation":0.0,"top_elevation":3.0}"#;
        assert!(serde_json::from_str::<SectionViewSettings>(json).is_err());
    }

    #[test]
    fn section_view_edits_require_the_linked_section_and_marker_level() {
        let section =
            SectionViewSettings::new(Point2::new(0.0, 0.0), Point2::new(8.0, 0.0), -1.0, 9.0);
        let mut view = crate::ViewParams {
            name: "Section A".into(),
            kind: crate::ViewKind::Section,
            level: Some(os_core::Id::new()),
            settings_revision: 0,
            plan: None,
            section: Some(section),
        };
        view.validate_edit().unwrap();
        view.section = None;
        assert!(view.validate_edit().is_err());
        view.section = Some(section);
        view.level = None;
        assert!(view.validate_edit().is_err());
    }
}
