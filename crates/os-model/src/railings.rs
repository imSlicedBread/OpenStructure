//! Host and shared type references for straight stair railings.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_STAIR_RAILINGS: usize = 256;
pub const MAX_STAIR_RAILING_POSTS: f64 = 512.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StairRailingSide {
    /// Left/right when looking from the lower start toward the upper end.
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StairRailingTypeParams {
    pub name: String,
    pub top_rail_height: f64,
    pub top_rail_width: f64,
    pub top_rail_depth: f64,
    pub post_width: f64,
    pub post_depth: f64,
    pub max_post_spacing: f64,
    pub material: Option<Id>,
}
pub type StairRailingType = Entity<StairRailingTypeParams>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StairRailingParams {
    pub name: String,
    pub stair: Id,
    pub railing_type: Id,
    pub side: StairRailingSide,
}
pub type StairRailing = Entity<StairRailingParams>;

impl StairRailingTypeParams {
    /// Validate the shared, level-independent type dimensions.
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "railing type name must be 1-256 bytes without control characters",
        )?;
        ensure(
            [
                self.top_rail_height,
                self.top_rail_width,
                self.top_rail_depth,
                self.post_width,
                self.post_depth,
                self.max_post_spacing,
            ]
            .into_iter()
            .all(|v| v.is_finite() && (0.005..=10.0).contains(&v)),
            "railing type dimensions must be finite and between 0.005 and 10 metres",
        )?;
        ensure(
            self.top_rail_height > self.top_rail_depth,
            "railing top height must exceed its vertical depth",
        )
    }

    pub fn validate_in(&self, model: &Model) -> Result<()> {
        self.validate()?;
        ensure(
            self.material
                .is_none_or(|id| model.materials.contains_key(&id)),
            "railing material missing",
        )
    }
}

impl StairRailingParams {
    /// Resolve host, shared type, material and bounded post count against the model.
    pub fn validate_in(&self, model: &Model) -> Result<()> {
        let ty = model
            .railing_types
            .get(&self.railing_type)
            .ok_or_else(|| os_core::Error::Invalid("railing type missing".into()))?;
        self.validate_with_type(&ty.parameters, model)
    }

    /// Validate an edit against a staged shared type without mutating the model.
    pub fn validate_with_type(&self, ty: &StairRailingTypeParams, model: &Model) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "railing name must be 1-256 bytes without control characters",
        )?;
        ensure(
            model.railing_types.contains_key(&self.railing_type),
            "railing type missing",
        )?;
        let stair = model
            .stairs
            .get(&self.stair)
            .ok_or_else(|| os_core::Error::Invalid("railing stair host missing".into()))?;
        ty.validate_in(model)?;
        let dimensions = stair.parameters.dimensions_in(model)?;
        ensure(
            ty.top_rail_width <= stair.parameters.width
                && ty.post_width <= stair.parameters.width
                && ty.post_depth <= stair.parameters.width
                && ty.post_width < dimensions.going
                && ty.max_post_spacing >= dimensions.going,
            "railing members do not fit stair width and going",
        )?;
        let post_count = f64::from(stair.parameters.riser_count + 1);
        ensure(
            post_count.is_finite() && post_count <= MAX_STAIR_RAILING_POSTS,
            "railing post count exceeds 512",
        )?;

        // Posts extend half a post width beyond each landing. Validate their
        // outer corners now so every accepted model remains drawable in plan
        // and section, including at the stair coordinate boundary.
        let run = dimensions.run;
        let direction = Point2::new(
            (stair.parameters.end.x - stair.parameters.start.x) / run,
            (stair.parameters.end.y - stair.parameters.start.y) / run,
        );
        let normal = Point2::new(-direction.y, direction.x);
        let half_post = ty.post_width * 0.5;
        let half_stair = stair.parameters.width * 0.5;
        for center in [stair.parameters.start, stair.parameters.end] {
            for along in [-half_post, half_post] {
                for across in [-half_stair, half_stair] {
                    let x = center.x + direction.x * along + normal.x * across;
                    let y = center.y + direction.y * along + normal.y * across;
                    ensure(
                        x.is_finite()
                            && y.is_finite()
                            && x.abs() <= crate::MAX_STAIR_COORDINATE
                            && y.abs() <= crate::MAX_STAIR_COORDINATE,
                        "railing posts exceed finite coordinate bounds",
                    )?;
                }
            }
        }
        let upper_elevation = model.levels[&stair.parameters.upper_level]
            .parameters
            .elevation;
        ensure(
            (upper_elevation + ty.top_rail_height).is_finite()
                && (upper_elevation + ty.top_rail_height).abs() <= crate::MAX_STAIR_COORDINATE,
            "railing top rail exceeds finite elevation bounds",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Level, LevelParams, Stair, StairParams};
    use os_core::Point2;

    fn fixture() -> (Model, StairRailing) {
        let mut model = Model::new("Railings");
        let lower = model.levels.values().next().unwrap().clone();
        let upper = Level::new(
            "core.level",
            LevelParams {
                name: "Upper".into(),
                elevation: 3.0,
                building: lower.parameters.building,
            },
        );
        let upper_id = upper.id();
        model.levels.insert(upper_id, upper);
        let stair = Stair::new(
            "core.stair",
            StairParams {
                name: "Flight".into(),
                lower_level: lower.id(),
                upper_level: upper_id,
                start: Point2::new(0.0, 0.0),
                end: Point2::new(4.5, 0.0),
                width: 1.2,
                riser_count: 15,
                structural_thickness: 0.2,
                material: None,
            },
        );
        let stair_id = stair.id();
        model.stairs.insert(stair_id, stair);
        let ty = StairRailingType::new(
            "core.railing_type",
            StairRailingTypeParams {
                name: "Standard".into(),
                top_rail_height: 0.95,
                top_rail_width: 0.05,
                top_rail_depth: 0.05,
                post_width: 0.08,
                post_depth: 0.05,
                max_post_spacing: 0.6,
                material: None,
            },
        );
        let type_id = ty.id();
        model.railing_types.insert(type_id, ty);
        let railing = StairRailing::new(
            "core.railing",
            StairRailingParams {
                name: "Left".into(),
                stair: stair_id,
                railing_type: type_id,
                side: StairRailingSide::Left,
            },
        );
        model.railings.insert(railing.id(), railing.clone());
        (model, railing)
    }

    #[test]
    fn railing_has_checked_host_type_and_stable_serialized_identity() {
        let (model, railing) = fixture();
        model.validate().unwrap();
        railing.parameters.validate_in(&model).unwrap();
        let json = serde_json::to_value(&railing).unwrap();
        let decoded: StairRailing = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, railing);
        assert_eq!(decoded.id(), railing.id());
    }

    #[test]
    fn railing_rejects_orphan_hosts_bad_types_tight_spacing_and_invalid_dimensions() {
        let (model, railing) = fixture();
        let mut bad_host = railing.parameters.clone();
        bad_host.stair = Id::new();
        assert!(bad_host.validate_in(&model).is_err());
        let mut bad_type = railing.parameters.clone();
        bad_type.railing_type = Id::new();
        assert!(bad_type.validate_in(&model).is_err());

        let mut bad = model.clone();
        bad.railing_types
            .get_mut(&railing.parameters.railing_type)
            .unwrap()
            .parameters
            .max_post_spacing = 0.1;
        assert!(bad.validate().is_err());

        let mut bad = model;
        let ty = bad
            .railing_types
            .get_mut(&railing.parameters.railing_type)
            .unwrap();
        ty.parameters.top_rail_height = ty.parameters.top_rail_depth;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn railing_rejects_post_and_top_rail_extents_outside_coordinate_envelope() {
        let (mut model, railing) = fixture();
        let stair = model.stairs.get_mut(&railing.parameters.stair).unwrap();
        stair.parameters.end.x = crate::MAX_STAIR_COORDINATE - 0.001;
        stair.parameters.start.x = stair.parameters.end.x - 4.5;
        assert!(railing.parameters.validate_in(&model).is_err());

        let (mut model, railing) = fixture();
        let upper = model.stairs[&railing.parameters.stair]
            .parameters
            .upper_level;
        model.levels.get_mut(&upper).unwrap().parameters.elevation =
            crate::MAX_STAIR_COORDINATE - 0.25;
        assert!(railing.parameters.validate_in(&model).is_err());
    }
}
