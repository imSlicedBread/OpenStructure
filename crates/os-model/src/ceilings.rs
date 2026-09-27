//! Horizontal native ceilings with a stable level-relative underside.
use crate::{Entity, FloorParams, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CeilingParams {
    pub name: String,
    pub level: Id,
    pub material: Option<Id>,
    /// Optional live boundary source. `boundary` remains the last saved outline
    /// so an unresolved association can be diagnosed, detached, or repaired.
    #[serde(default)]
    pub boundary_room: Option<Id>,
    pub boundary: Vec<Point2>,
    pub holes: Vec<Vec<Point2>>,
    pub thickness: f64,
    pub elevation_offset: f64,
}
pub type Ceiling = Entity<CeilingParams>;

impl CeilingParams {
    /// A slab-shaped view of the authored shell, useful for sharing the existing
    /// bounded ring validator without changing the ceiling's elevation meaning.
    pub fn slab(&self) -> FloorParams {
        FloorParams {
            name: self.name.clone(),
            level: self.level,
            material: self.material,
            boundary: self.boundary.clone(),
            holes: self.holes.clone(),
            thickness: self.thickness,
            top_offset: self.elevation_offset + self.thickness,
        }
    }

    pub fn validate_at(&self, level_z: f64) -> Result<()> {
        self.slab().validate()?;
        let bottom = level_z + self.elevation_offset;
        let top = bottom + self.thickness;
        ensure(
            level_z.is_finite()
                && bottom.is_finite()
                && top.is_finite()
                && bottom.abs() <= 1e6
                && top.abs() <= 1e6
                && top > bottom,
            "ceiling elevation exceeds finite coordinate bounds",
        )
    }

    pub fn validate_in(&self, model: &Model) -> Result<()> {
        let level = model
            .levels
            .get(&self.level)
            .ok_or_else(|| os_core::Error::Invalid("ceiling level missing".into()))?;
        ensure(
            self.material
                .is_none_or(|id| model.materials.contains_key(&id)),
            "ceiling material missing",
        )?;
        self.validate_at(level.parameters.elevation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameters(level: Id) -> CeilingParams {
        CeilingParams {
            name: "Ceiling".into(),
            level,
            material: None,
            boundary_room: None,
            boundary: vec![
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 0.0),
                Point2::new(8.0, 6.0),
                Point2::new(0.0, 6.0),
            ],
            holes: vec![vec![
                Point2::new(2.0, 2.0),
                Point2::new(4.0, 2.0),
                Point2::new(4.0, 4.0),
                Point2::new(2.0, 4.0),
            ]],
            thickness: 0.12,
            elevation_offset: 2.55,
        }
    }

    #[test]
    fn ceiling_validates_openings_and_level_relative_elevation() {
        let mut model = Model::new("Ceilings");
        let level = *model.levels.keys().next().unwrap();
        let ceiling = Ceiling::new("core.ceiling", parameters(level));
        let id = ceiling.id();
        model.ceilings.insert(id, ceiling);
        model.validate().unwrap();

        let mut invalid = model.clone();
        invalid
            .ceilings
            .get_mut(&id)
            .unwrap()
            .parameters
            .elevation_offset = f64::INFINITY;
        assert!(invalid.validate().is_err());
        let mut invalid = model.clone();
        invalid.ceilings.get_mut(&id).unwrap().parameters.holes[0][0] = Point2::new(20.0, 20.0);
        assert!(invalid.validate().is_err());
        let mut invalid = model;
        invalid.ceilings.get_mut(&id).unwrap().parameters.level = Id::new();
        assert!(invalid.validate().is_err());
    }
}
