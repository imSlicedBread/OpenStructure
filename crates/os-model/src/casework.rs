//! Native rectangular cabinet envelopes with shared project types.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_COORDINATE: f64 = 1e6;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseworkTypeParams {
    pub name: String,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    pub material: Option<Id>,
}
pub type CaseworkType = Entity<CaseworkTypeParams>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseworkParams {
    pub name: String,
    pub type_id: Id,
    pub level: Id,
    pub center: Point2,
    /// Counter-clockwise rotation about +Z, in radians.
    pub yaw: f64,
    pub base_offset: f64,
}
pub type Casework = Entity<CaseworkParams>;

fn validate_name(name: &str) -> Result<()> {
    ensure(
        !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
        "casework name must be 1-256 bytes without control characters",
    )
}

impl CaseworkTypeParams {
    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        ensure(
            [self.width, self.depth, self.height]
                .iter()
                .all(|v| v.is_finite() && *v > 1e-6 && *v <= MAX_COORDINATE),
            "casework dimensions must be positive and bounded",
        )?;
        ensure(
            self.width * self.depth > 1e-8,
            "casework footprint is negligible",
        )
    }

    pub fn validate_in(&self, model: &Model) -> Result<()> {
        self.validate()?;
        if let Some(material) = self.material {
            ensure(
                model.materials.contains_key(&material),
                "casework material missing",
            )?;
        }
        Ok(())
    }
}

impl CaseworkParams {
    /// Checks instance data alone. Use `validate_in` before committing an edit.
    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        ensure(
            self.center.is_finite()
                && self.center.x.abs() <= MAX_COORDINATE
                && self.center.y.abs() <= MAX_COORDINATE,
            "casework center exceeds coordinate bounds",
        )?;
        ensure(self.yaw.is_finite(), "casework yaw must be finite")?;
        ensure(
            self.base_offset.is_finite() && self.base_offset.abs() <= MAX_COORDINATE,
            "invalid casework base offset",
        )
    }

    /// Four CCW world-space corners, without a repeated closing vertex.
    /// Validates both the instance and the proposed type before resolving corners.
    pub fn boundary(&self, casework_type: &CaseworkTypeParams) -> Result<[Point2; 4]> {
        self.validate()?;
        casework_type.validate()?;
        let (w, d) = (casework_type.width / 2.0, casework_type.depth / 2.0);
        let (s, c) = self.yaw.sin_cos();
        let corners = [(-w, -d), (w, -d), (w, d), (-w, d)].map(|(x, y)| {
            Point2::new(c * x - s * y + self.center.x, s * x + c * y + self.center.y)
        });
        ensure(
            corners.iter().all(|p| {
                p.is_finite() && p.x.abs() <= MAX_COORDINATE && p.y.abs() <= MAX_COORDINATE
            }),
            "casework footprint exceeds coordinate bounds",
        )?;
        Ok(corners)
    }

    pub fn validate_with_type(&self, casework_type: &CaseworkTypeParams) -> Result<()> {
        self.boundary(casework_type).map(|_| ())
    }

    /// Absolute bottom/top in metres; both must remain within model bounds.
    pub fn elevations(
        &self,
        casework_type: &CaseworkTypeParams,
        level_elevation: f64,
    ) -> Result<(f64, f64)> {
        self.validate_with_type(casework_type)?;
        let bottom = level_elevation + self.base_offset;
        let top = bottom + casework_type.height;
        ensure(
            level_elevation.is_finite()
                && bottom.is_finite()
                && top.is_finite()
                && bottom.abs() <= MAX_COORDINATE
                && top.abs() <= MAX_COORDINATE
                && top > bottom,
            "casework elevations exceed coordinate bounds or collapse",
        )?;
        Ok((bottom, top))
    }

    /// Resolve and validate references against a candidate type map.
    /// Passing the map explicitly also supports validating a proposed shared type
    /// edit against every affected instance before a document transaction commits.
    pub fn resolve_in<'a>(
        &self,
        model: &Model,
        types: &'a BTreeMap<Id, CaseworkType>,
    ) -> Result<&'a CaseworkTypeParams> {
        self.validate()?;
        let casework_type = types
            .get(&self.type_id)
            .ok_or_else(|| os_core::Error::Invalid("casework type missing".into()))?;
        ensure(
            casework_type.id() == self.type_id
                && casework_type.header.type_id == "core.casework_type",
            "casework type identity mismatch",
        )?;
        casework_type.parameters.validate_in(model)?;
        let level = model
            .levels
            .get(&self.level)
            .ok_or_else(|| os_core::Error::Invalid("casework level missing".into()))?;
        self.elevations(&casework_type.parameters, level.parameters.elevation)?;
        Ok(&casework_type.parameters)
    }

    pub fn validate_in(&self, model: &Model, types: &BTreeMap<Id, CaseworkType>) -> Result<()> {
        self.resolve_in(model, types).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cabinet_type() -> CaseworkTypeParams {
        CaseworkTypeParams {
            name: "Base cabinet".into(),
            width: 0.6,
            depth: 0.6,
            height: 0.9,
            material: None,
        }
    }

    fn fixture() -> (Model, BTreeMap<Id, CaseworkType>, CaseworkParams) {
        let model = Model::new("Casework");
        let cabinet_type = CaseworkType::new("core.casework_type", cabinet_type());
        let instance = CaseworkParams {
            name: "Cabinet 1".into(),
            type_id: cabinet_type.id(),
            level: *model.levels.keys().next().unwrap(),
            center: Point2::new(2.0, 3.0),
            yaw: 0.0,
            base_offset: 0.15,
        };
        (
            model,
            BTreeMap::from([(cabinet_type.id(), cabinet_type)]),
            instance,
        )
    }

    #[test]
    fn rejects_invalid_type_dimensions_and_names() {
        for bad in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            0.0,
            1e-6,
            1e6 + 1.0,
        ] {
            for dimension in 0..3 {
                let mut parameters = cabinet_type();
                match dimension {
                    0 => parameters.width = bad,
                    1 => parameters.depth = bad,
                    _ => parameters.height = bad,
                }
                assert!(parameters.validate().is_err());
            }
        }
        let mut parameters = cabinet_type();
        parameters.width = 2e-6;
        parameters.depth = 2e-6;
        assert!(parameters.validate().is_err());
        for name in [
            "".into(),
            "  ".into(),
            "bad\nname".into(),
            "bad\u{7f}".into(),
            "a".repeat(257),
        ] {
            assert!(
                CaseworkTypeParams {
                    name,
                    ..cabinet_type()
                }
                .validate()
                .is_err()
            );
        }
        CaseworkTypeParams {
            name: "a".repeat(256),
            ..cabinet_type()
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn rejects_invalid_instance_data() {
        let (model, types, valid) = fixture();
        valid.validate_in(&model, &types).unwrap();
        for bad in [
            CaseworkParams {
                name: "\t".into(),
                ..valid.clone()
            },
            CaseworkParams {
                name: "a".repeat(257),
                ..valid.clone()
            },
            CaseworkParams {
                center: Point2::new(f64::NAN, 0.0),
                ..valid.clone()
            },
            CaseworkParams {
                center: Point2::new(0.0, f64::INFINITY),
                ..valid.clone()
            },
            CaseworkParams {
                yaw: f64::NAN,
                ..valid.clone()
            },
            CaseworkParams {
                yaw: f64::INFINITY,
                ..valid.clone()
            },
            CaseworkParams {
                base_offset: f64::NAN,
                ..valid.clone()
            },
            CaseworkParams {
                base_offset: 1e6 + 1.0,
                ..valid.clone()
            },
        ] {
            assert!(bad.validate_in(&model, &types).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn rotated_corners_and_bounds_use_the_resolved_type() {
        let (_, _, mut instance) = fixture();
        let parameters = CaseworkTypeParams {
            width: 2.0,
            depth: 1.0,
            ..cabinet_type()
        };
        instance.yaw = std::f64::consts::FRAC_PI_2;
        let corners = instance.boundary(&parameters).unwrap();
        let expected = [(2.5, 2.0), (2.5, 4.0), (1.5, 4.0), (1.5, 2.0)];
        for (actual, (x, y)) in corners.iter().zip(expected) {
            assert!(actual.distance(Point2::new(x, y)) < 1e-12);
        }
        instance.center = Point2::new(1e6 - 1.0, 0.0);
        instance.yaw = 0.0;
        instance.boundary(&parameters).unwrap();
        instance.yaw = std::f64::consts::FRAC_PI_4;
        assert!(instance.boundary(&parameters).is_err());
        instance.center.x = -1e6 + 1.0;
        assert!(instance.boundary(&parameters).is_err());
    }

    #[test]
    fn elevation_validation_checks_absolute_extents() {
        let (_, _, mut instance) = fixture();
        let parameters = cabinet_type();
        let (bottom, top) = instance.elevations(&parameters, 3.0).unwrap();
        assert!((bottom - 3.15).abs() < 1e-12 && (top - 4.05).abs() < 1e-12);
        for elevation in [f64::NAN, f64::INFINITY, f64::MAX, 1e6, -1e6 - 1.0] {
            assert!(instance.elevations(&parameters, elevation).is_err());
        }
        instance.base_offset = -0.9;
        assert_eq!(instance.elevations(&parameters, 1e6).unwrap().1, 1e6);
        instance.base_offset = -1.0;
        assert!(instance.elevations(&parameters, -1e6).is_err());
    }

    #[test]
    fn references_material_and_candidate_shared_type_are_validated() {
        let (mut model, mut types, instance) = fixture();
        assert!(instance.validate_in(&model, &BTreeMap::new()).is_err());
        let mut bad = instance.clone();
        bad.level = Id::new();
        assert!(bad.validate_in(&model, &types).is_err());
        let material = crate::Material::new(
            "core.material",
            crate::MaterialParams {
                name: "Oak".into(),
                density_kg_m3: 700.0,
                color: [180, 140, 90],
            },
        );
        types
            .get_mut(&instance.type_id)
            .unwrap()
            .parameters
            .material = Some(material.id());
        assert!(instance.validate_in(&model, &types).is_err());
        model.materials.insert(material.id(), material);
        instance.validate_in(&model, &types).unwrap();
        let mut near_edge = instance.clone();
        near_edge.center.x = 1e6 - 0.4;
        near_edge.validate_in(&model, &types).unwrap();
        let mut candidate = types.clone();
        candidate
            .get_mut(&instance.type_id)
            .unwrap()
            .parameters
            .width = 1.0;
        instance.validate_in(&model, &candidate).unwrap();
        assert!(near_edge.validate_in(&model, &candidate).is_err());
        near_edge.validate_in(&model, &types).unwrap();
        types.get_mut(&instance.type_id).unwrap().header.id = Id::new();
        assert!(instance.validate_in(&model, &types).is_err());
        types.get_mut(&instance.type_id).unwrap().header.id = instance.type_id;
        types.get_mut(&instance.type_id).unwrap().header.type_id = "core.column".into();
        assert!(instance.validate_in(&model, &types).is_err());
    }

    #[test]
    fn entities_round_trip_with_shared_ids_and_reject_unknown_parameters() {
        let (_, types, parameters) = fixture();
        let first = Casework::new("core.casework", parameters.clone());
        let second = Casework::new(
            "core.casework",
            CaseworkParams {
                name: "Cabinet 2".into(),
                center: Point2::new(3.0, 3.0),
                ..parameters
            },
        );
        assert_ne!(first.id(), second.id());
        let original = (types, vec![first, second]);
        let json = serde_json::to_vec(&original).unwrap();
        let decoded: (BTreeMap<Id, CaseworkType>, Vec<Casework>) =
            serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(
            decoded.1[0].parameters.type_id,
            decoded.1[1].parameters.type_id
        );
        let mut value = serde_json::to_value(&decoded.1[0].parameters).unwrap();
        value["width"] = 1.0.into();
        assert!(serde_json::from_value::<CaseworkParams>(value).is_err());
        let mut value = serde_json::to_value(cabinet_type()).unwrap();
        value["yaw"] = 1.0.into();
        assert!(serde_json::from_value::<CaseworkTypeParams>(value).is_err());
    }
}
