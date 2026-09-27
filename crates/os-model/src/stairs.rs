//! Straight stair flights. Levels own the elevations; geometry is derived.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

/// Modeling limits only; these do not express building-code compliance.
pub const MAX_STAIR_RISERS: u32 = 256;
pub const MAX_STAIR_COORDINATE: f64 = 1e6;
pub const MIN_STAIR_DIMENSION: f64 = 1e-6;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StairParams {
    pub name: String,
    pub lower_level: Id,
    pub upper_level: Id,
    /// Centre of the first riser, in plan.
    pub start: Point2,
    /// Centre of the far edge of the upper arrival tread, in plan.
    pub end: Point2,
    pub width: f64,
    /// N risers, N-1 intermediate treads and one upper arrival tread.
    pub riser_count: u32,
    /// Vertical distance below the line joining the lower and upper levels.
    /// The underside is continuous and sloped; thickness is not normal to it.
    pub structural_thickness: f64,
    pub material: Option<Id>,
}
pub type Stair = Entity<StairParams>;

/// Measurements are derived, never stored in the document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StairDimensions {
    pub rise: f64,
    pub run: f64,
    pub riser_height: f64,
    pub going: f64,
}

impl StairParams {
    pub fn run(&self) -> f64 {
        self.start.distance(self.end)
    }

    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "stair name must be 1-256 bytes without control characters",
        )?;
        ensure(
            self.lower_level != self.upper_level,
            "stair requires two distinct levels",
        )?;
        ensure(
            (1..=MAX_STAIR_RISERS).contains(&self.riser_count),
            "stair requires 1-256 risers",
        )?;
        ensure(
            [self.start, self.end].iter().all(|p| {
                p.is_finite()
                    && p.x.abs() <= MAX_STAIR_COORDINATE
                    && p.y.abs() <= MAX_STAIR_COORDINATE
            }),
            "stair endpoints exceed finite coordinate bounds",
        )?;
        let run = self.run();
        ensure(
            [self.width, self.structural_thickness, run]
                .iter()
                .all(|v| v.is_finite() && *v > MIN_STAIR_DIMENSION && *v <= MAX_STAIR_COORDINATE),
            "stair dimensions must be positive and bounded",
        )?;
        ensure(
            run / f64::from(self.riser_count) > MIN_STAIR_DIMENSION,
            "stair going must exceed one micrometre",
        )?;
        let half_width = self.width * 0.5;
        let dx = (self.end.x - self.start.x) / run;
        let dy = (self.end.y - self.start.y) / run;
        for p in [self.start, self.end] {
            for side in [-half_width, half_width] {
                ensure(
                    (p.x - dy * side).abs() <= MAX_STAIR_COORDINATE
                        && (p.y + dx * side).abs() <= MAX_STAIR_COORDINATE,
                    "stair footprint exceeds coordinate bounds",
                )?;
            }
        }
        Ok(())
    }

    /// Validate geometry inputs and derive equal risers and treads. Geometry
    /// callers supply elevations; model callers use `dimensions_in` to resolve
    /// and validate level and material references as well.
    pub fn dimensions(&self, lower_z: f64, upper_z: f64) -> Result<StairDimensions> {
        self.validate()?;
        ensure(
            [lower_z, upper_z, lower_z - self.structural_thickness]
                .iter()
                .all(|z| z.is_finite() && z.abs() <= MAX_STAIR_COORDINATE),
            "stair elevations exceed finite coordinate bounds",
        )?;
        let rise = upper_z - lower_z;
        let count = f64::from(self.riser_count);
        ensure(
            rise.is_finite() && rise <= MAX_STAIR_COORDINATE && rise / count > MIN_STAIR_DIMENSION,
            "stair upper level must be above lower level with positive bounded risers",
        )?;
        Ok(StairDimensions {
            rise,
            run: self.run(),
            riser_height: rise / count,
            going: self.run() / count,
        })
    }

    pub fn dimensions_in(&self, model: &Model) -> Result<StairDimensions> {
        let lower = model
            .levels
            .get(&self.lower_level)
            .ok_or_else(|| os_core::Error::Invalid("stair lower level missing".into()))?;
        let upper = model
            .levels
            .get(&self.upper_level)
            .ok_or_else(|| os_core::Error::Invalid("stair upper level missing".into()))?;
        ensure(
            lower.parameters.building == upper.parameters.building
                && model.buildings.contains_key(&lower.parameters.building),
            "stair levels must belong to the same existing building",
        )?;
        if let Some(material) = self.material {
            ensure(
                model.materials.contains_key(&material),
                "stair material missing",
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

    fn fixture() -> (Model, StairParams) {
        let mut model = Model::new("Stairs");
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
            StairParams {
                name: "S1".into(),
                lower_level,
                upper_level,
                start: Point2::new(0.0, 0.0),
                end: Point2::new(4.5, 0.0),
                width: 1.2,
                riser_count: 15,
                structural_thickness: 0.2,
                material: None,
            },
        )
    }

    #[test]
    fn stair_dimensions_follow_levels_and_endpoint_direction() {
        let (mut model, mut p) = fixture();
        assert_eq!(
            p.dimensions_in(&model).unwrap(),
            StairDimensions {
                rise: 3.0,
                run: 4.5,
                riser_height: 0.2,
                going: 0.3,
            }
        );
        std::mem::swap(&mut p.start, &mut p.end);
        assert_eq!(p.dimensions_in(&model).unwrap().going, 0.3);
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
        assert_eq!(p.dimensions_in(&model).unwrap().riser_height, 0.4);
        std::mem::swap(&mut p.lower_level, &mut p.upper_level);
        assert!(p.validate_in(&model).is_err());
        let value = serde_json::to_value(&p).unwrap();
        for derived in ["rise", "run", "going", "riser_height", "mesh"] {
            assert!(value.get(derived).is_none());
        }
        assert_eq!(serde_json::from_value::<StairParams>(value).unwrap(), p);
    }

    #[test]
    fn stair_rejects_invalid_names_dimensions_counts_and_coordinates() {
        let (_, p) = fixture();
        for case in 0..20 {
            let mut bad = p.clone();
            match case {
                0 => bad.name = "  ".into(),
                1 => bad.name = "x".repeat(257),
                2 => bad.name = "bad\nname".into(),
                3 => bad.width = 0.0,
                4 => bad.width = f64::NAN,
                5 => bad.width = 1e7,
                6 => bad.structural_thickness = -1.0,
                7 => bad.structural_thickness = f64::INFINITY,
                8 => bad.structural_thickness = 1e7,
                9 => bad.riser_count = 0,
                10 => bad.riser_count = MAX_STAIR_RISERS + 1,
                11 => bad.end = bad.start,
                12 => bad.end.x = f64::NAN,
                13 => bad.start.y = f64::INFINITY,
                14 => bad.start.x = 1e7,
                15 => bad.end.x = 1e-7,
                16 => bad.end.x = 1e-5, // individually positive run, negligible going
                17 => bad.upper_level = bad.lower_level,
                18 => {
                    bad.start.y = 1e6;
                    bad.end.y = 1e6;
                }
                _ => {
                    bad.start.x = -1e6;
                    bad.end.x = 1e6;
                }
            }
            assert!(bad.dimensions(0.0, 3.0).is_err(), "case {case}");
        }
        for (lower, upper) in [
            (0.0, 0.0),
            (3.0, 0.0),
            (0.0, 1e-7),
            (f64::NAN, 3.0),
            (0.0, f64::INFINITY),
            (0.0, 1e7),
            (-1e6, 0.0),
            (-600_000.0, 600_000.0),
        ] {
            assert!(p.dimensions(lower, upper).is_err());
        }
        for count in [1, MAX_STAIR_RISERS] {
            let mut limit = p.clone();
            limit.riser_count = count;
            limit.dimensions(0.0, 3.0).unwrap();
        }
    }

    #[test]
    fn stair_map_checks_identity_references_and_round_trip() {
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
        model.materials.insert(material.id(), material);
        let stair = Stair::new("core.stair", p);
        let id = stair.id();
        model.stairs.insert(id, stair);
        model.validate().unwrap();
        assert_eq!(
            serde_json::from_slice::<Model>(&serde_json::to_vec(&model).unwrap()).unwrap(),
            model
        );
        for case in 0..9 {
            let mut bad = model.clone();
            let stair = bad.stairs.get_mut(&id).unwrap();
            match case {
                0 => stair.header.id = Id::new(),
                1 => stair.header.type_id = "core.floor".into(),
                2 => stair.header.schema_version = 0,
                3 => stair.parameters.lower_level = Id::new(),
                4 => stair.parameters.upper_level = Id::new(),
                5 => stair.parameters.material = Some(Id::new()),
                6 => {
                    bad.levels.remove(&stair.parameters.upper_level);
                }
                7 => {
                    bad.levels
                        .get_mut(&stair.parameters.upper_level)
                        .unwrap()
                        .parameters
                        .elevation = 0.0;
                }
                _ => {
                    let other = Building::new(
                        "core.building",
                        BuildingParams {
                            name: "Other".into(),
                            site: *bad.sites.keys().next().unwrap(),
                        },
                    );
                    bad.levels
                        .get_mut(&stair.parameters.upper_level)
                        .unwrap()
                        .parameters
                        .building = other.id();
                    bad.buildings.insert(other.id(), other);
                }
            }
            assert!(bad.validate().is_err(), "case {case}");
        }
        let mut duplicate = model;
        let mut stair = duplicate.stairs.remove(&id).unwrap();
        stair.header.id = duplicate.project.id();
        duplicate.stairs.insert(stair.id(), stair);
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn stair_owned_memory_includes_map_name_and_header_payloads() {
        let (mut model, p) = fixture();
        let baseline = model.estimated_memory_bytes();
        let stair = Stair::new("core.stair", p);
        let id = stair.id();
        model.stairs.insert(id, stair);
        let initial = model.estimated_memory_bytes();
        assert!(initial > baseline + std::mem::size_of::<Stair>());
        model
            .stairs
            .get_mut(&id)
            .unwrap()
            .parameters
            .name
            .reserve(4096);
        assert!(model.estimated_memory_bytes() >= initial + 4096);
        let before = model.estimated_memory_bytes();
        let stair = model.stairs.get_mut(&id).unwrap();
        stair
            .header
            .properties
            .insert("note".into(), serde_json::Value::String("x".repeat(2048)));
        stair
            .header
            .relationships
            .insert("reference".into(), vec![stair.parameters.lower_level; 100]);
        assert!(model.estimated_memory_bytes() >= before + 2048 + 100 * std::mem::size_of::<Id>());
    }
}
