//! Straight ramps between two levels. Elevations and slope are derived.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

/// Modeling limits only; these do not express building-code compliance.
pub const MAX_RAMP_COORDINATE: f64 = 1e6;
pub const MIN_RAMP_DIMENSION: f64 = 1e-6;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RampParams {
    pub name: String,
    pub lower_level: Id,
    pub upper_level: Id,
    /// Centre of the lower walk-surface edge, in plan.
    pub start: Point2,
    /// Centre of the uphill (upper) walk-surface edge, in plan.
    pub end: Point2,
    pub width: f64,
    /// Vertical thickness below the sloped walk surface, not normal to it.
    pub structural_thickness: f64,
    pub material: Option<Id>,
}
pub type Ramp = Entity<RampParams>;

/// Measurements derived from endpoints and level elevations, never persisted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RampDimensions {
    pub rise: f64,
    pub run: f64,
    pub rise_per_run: f64,
}

impl RampParams {
    pub fn run(&self) -> f64 {
        self.start.distance(self.end)
    }

    /// Validate authored values independently of level and material references.
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "ramp name must be 1-256 bytes without control characters",
        )?;
        ensure(
            self.lower_level != self.upper_level,
            "ramp requires two distinct levels",
        )?;
        ensure(
            [self.start, self.end].iter().all(|p| {
                p.is_finite()
                    && p.x.abs() <= MAX_RAMP_COORDINATE
                    && p.y.abs() <= MAX_RAMP_COORDINATE
            }),
            "ramp endpoints exceed finite coordinate bounds",
        )?;
        let run = self.run();
        ensure(
            [run, self.width, self.structural_thickness]
                .iter()
                .all(|v| v.is_finite() && *v > MIN_RAMP_DIMENSION && *v <= MAX_RAMP_COORDINATE),
            "ramp dimensions must be positive and bounded",
        )?;
        let dx = (self.end.x - self.start.x) / run;
        let dy = (self.end.y - self.start.y) / run;
        for p in [self.start, self.end] {
            for side in [-self.width * 0.5, self.width * 0.5] {
                ensure(
                    (p.x - dy * side).abs() <= MAX_RAMP_COORDINATE
                        && (p.y + dx * side).abs() <= MAX_RAMP_COORDINATE,
                    "ramp footprint exceeds coordinate bounds",
                )?;
            }
        }
        Ok(())
    }

    /// Validate supplied elevations and derive the run, rise and slope.
    /// Geometry callers supply elevations; `dimensions_in` also checks references.
    pub fn dimensions(&self, lower_z: f64, upper_z: f64) -> Result<RampDimensions> {
        self.validate()?;
        ensure(
            [
                lower_z,
                upper_z,
                lower_z - self.structural_thickness,
                upper_z - self.structural_thickness,
            ]
            .iter()
            .all(|z| z.is_finite() && z.abs() <= MAX_RAMP_COORDINATE),
            "ramp elevations exceed finite coordinate bounds",
        )?;
        let rise = upper_z - lower_z;
        ensure(
            rise.is_finite() && rise > MIN_RAMP_DIMENSION && rise <= MAX_RAMP_COORDINATE,
            "ramp upper level must be above lower level with positive bounded rise",
        )?;
        let run = self.run();
        Ok(RampDimensions {
            rise,
            run,
            rise_per_run: rise / run,
        })
    }

    /// Resolve levels and material and require both levels in one existing building.
    pub fn dimensions_in(&self, model: &Model) -> Result<RampDimensions> {
        let lower = model
            .levels
            .get(&self.lower_level)
            .ok_or_else(|| os_core::Error::Invalid("ramp lower level missing".into()))?;
        let upper = model
            .levels
            .get(&self.upper_level)
            .ok_or_else(|| os_core::Error::Invalid("ramp upper level missing".into()))?;
        ensure(
            lower.parameters.building == upper.parameters.building
                && model.buildings.contains_key(&lower.parameters.building),
            "ramp levels must belong to the same existing building",
        )?;
        if let Some(material) = self.material {
            ensure(
                model.materials.contains_key(&material),
                "ramp material missing",
            )?;
        }
        self.dimensions(lower.parameters.elevation, upper.parameters.elevation)
    }

    pub fn validate_in(&self, model: &Model) -> Result<()> {
        self.dimensions_in(model).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Building, BuildingParams, Level, LevelParams, Material, MaterialParams};

    fn fixture() -> (Model, RampParams) {
        let mut model = Model::new("Ramps");
        let lower = model.levels.values().next().unwrap();
        let lower_level = lower.id();
        let upper = Level::new(
            "core.level",
            LevelParams {
                name: "Upper".into(),
                elevation: 3.0,
                building: lower.parameters.building,
            },
        );
        let upper_level = upper.id();
        model.levels.insert(upper_level, upper);
        (
            model,
            RampParams {
                name: "R1".into(),
                lower_level,
                upper_level,
                start: Point2::new(0.0, 0.0),
                end: Point2::new(12.0, 0.0),
                width: 1.5,
                structural_thickness: 0.2,
                material: None,
            },
        )
    }

    #[test]
    fn dimensions_follow_levels_and_direction_without_persisting_derived_values() {
        let (mut model, mut p) = fixture();
        assert_eq!(
            p.dimensions_in(&model).unwrap(),
            RampDimensions {
                rise: 3.0,
                run: 12.0,
                rise_per_run: 0.25
            }
        );
        std::mem::swap(&mut p.start, &mut p.end);
        assert_eq!(p.dimensions_in(&model).unwrap().rise_per_run, 0.25);
        model
            .levels
            .get_mut(&p.lower_level)
            .unwrap()
            .parameters
            .elevation = -2.0;
        model
            .levels
            .get_mut(&p.upper_level)
            .unwrap()
            .parameters
            .elevation = 4.0;
        assert_eq!(p.dimensions_in(&model).unwrap().rise_per_run, 0.5);
        let ramp = Ramp::new("core.ramp", p);
        let value = serde_json::to_value(&ramp).unwrap();
        let parameters = &value["parameters"];
        for derived in ["rise", "run", "rise_per_run", "mesh"] {
            assert!(parameters.get(derived).is_none());
        }
        assert_eq!(serde_json::from_value::<Ramp>(value).unwrap(), ramp);
        let mut unknown = serde_json::to_value(&ramp.parameters).unwrap();
        unknown["run"] = 12.0.into();
        assert!(serde_json::from_value::<RampParams>(unknown).is_err());
    }

    #[test]
    fn rejects_invalid_names_dimensions_and_footprint_bounds() {
        let (_, p) = fixture();
        for case in 0..19 {
            let mut bad = p.clone();
            match case {
                0 => bad.name = " \t".into(),
                1 => bad.name = "x".repeat(257),
                2 => bad.name = "bad\nname".into(),
                3 => bad.width = MIN_RAMP_DIMENSION,
                4 => bad.width = f64::NAN,
                5 => bad.width = 1e7,
                6 => bad.structural_thickness = -1.0,
                7 => bad.structural_thickness = f64::INFINITY,
                8 => bad.structural_thickness = MIN_RAMP_DIMENSION,
                9 => bad.structural_thickness = 1e7,
                10 => bad.end = bad.start,
                11 => bad.end.x = f64::NAN,
                12 => bad.start.y = f64::INFINITY,
                13 => bad.start.x = -1e7,
                14 => bad.end.x = MIN_RAMP_DIMENSION,
                15 => bad.upper_level = bad.lower_level,
                16 => {
                    bad.start.y = 1e6;
                    bad.end.y = 1e6;
                }
                17 => {
                    bad.start.x = -1e6;
                    bad.end.x = 1e6;
                }
                _ => {
                    bad.start = Point2::new(-1e6, -1e6);
                    bad.end = Point2::new(-999_990.0, -999_990.0);
                }
            }
            assert!(bad.dimensions(0.0, 3.0).is_err(), "case {case}");
        }
    }

    #[test]
    fn elevations_include_walk_surface_and_vertical_underside() {
        let (_, p) = fixture();
        for (lower, upper) in [
            (0.0, 0.0),
            (3.0, 0.0),
            (0.0, MIN_RAMP_DIMENSION),
            (f64::NAN, 3.0),
            (0.0, f64::INFINITY),
            (0.0, 1e7),
            (-1e6, 0.0),
            (-600_000.0, 600_000.0),
        ] {
            assert!(p.dimensions(lower, upper).is_err(), "{lower}, {upper}");
        }
        p.dimensions(-1e6 + p.structural_thickness, -999_990.0)
            .unwrap();
        p.dimensions(999_990.0, 1e6).unwrap();
        let mut boundary = p;
        boundary.start = Point2::new(0.0, 1e6 - boundary.width / 2.0);
        boundary.end = Point2::new(12.0, boundary.start.y);
        boundary.dimensions(0.0, 3.0).unwrap();
    }

    #[test]
    fn validates_level_building_and_material_references() {
        let (mut model, mut p) = fixture();
        let material = Material::new(
            "core.material",
            MaterialParams {
                name: "Concrete".into(),
                density_kg_m3: 2400.0,
                color: [150; 3],
            },
        );
        p.material = Some(material.id());
        assert!(p.validate_in(&model).is_err());
        model.materials.insert(material.id(), material);
        p.validate_in(&model).unwrap();
        for case in 0..6 {
            let mut bad = model.clone();
            match case {
                0 => {
                    bad.levels.remove(&p.lower_level);
                }
                1 => {
                    bad.levels.remove(&p.upper_level);
                }
                2 => {
                    bad.buildings.clear();
                }
                3 => {
                    let other = Building::new(
                        "core.building",
                        BuildingParams {
                            name: "Other".into(),
                            site: *bad.sites.keys().next().unwrap(),
                        },
                    );
                    bad.levels
                        .get_mut(&p.upper_level)
                        .unwrap()
                        .parameters
                        .building = other.id();
                    bad.buildings.insert(other.id(), other);
                }
                4 => {
                    bad.levels
                        .get_mut(&p.upper_level)
                        .unwrap()
                        .parameters
                        .elevation = 0.0;
                }
                _ => {
                    bad.materials.clear();
                }
            }
            assert!(p.validate_in(&bad).is_err(), "case {case}");
        }
    }
}
