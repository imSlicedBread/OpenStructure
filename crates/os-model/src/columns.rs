//! Native vertical, axis-aligned rectangular architectural columns.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnParams {
    pub name: String,
    pub level: Id,
    pub center: Point2,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    pub base_offset: f64,
    pub material: Option<Id>,
}
pub type Column = Entity<ColumnParams>;

impl ColumnParams {
    pub fn boundary(&self) -> [Point2; 4] {
        let (x, y, w, d) = (
            self.center.x,
            self.center.y,
            self.width / 2.0,
            self.depth / 2.0,
        );
        [
            Point2::new(x - w, y - d),
            Point2::new(x + w, y - d),
            Point2::new(x + w, y + d),
            Point2::new(x - w, y + d),
        ]
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "column name must be 1-256 bytes without control characters",
        )?;
        ensure(
            [self.width, self.depth, self.height]
                .iter()
                .all(|v| v.is_finite() && *v > 1e-6 && *v <= 1e6),
            "column dimensions must be positive and bounded",
        )?;
        ensure(
            self.width * self.depth > 1e-8,
            "column footprint is negligible",
        )?;
        ensure(
            self.base_offset.is_finite() && self.base_offset.abs() <= 1e6,
            "invalid column base offset",
        )?;
        ensure(
            self.center.is_finite()
                && self
                    .boundary()
                    .iter()
                    .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
            "column footprint exceeds coordinate bounds",
        )
    }
    pub fn elevations(&self, level_elevation: f64) -> Result<(f64, f64)> {
        let bottom = level_elevation + self.base_offset;
        let top = bottom + self.height;
        ensure(
            bottom.is_finite() && top.is_finite() && top > bottom,
            "column elevation overflow",
        )?;
        Ok((bottom, top))
    }
    pub fn validate_in(&self, model: &Model) -> Result<()> {
        self.validate()?;
        let level = model
            .levels
            .get(&self.level)
            .ok_or_else(|| os_core::Error::Invalid("column level missing".into()))?;
        self.elevations(level.parameters.elevation)?;
        if let Some(material) = self.material {
            ensure(
                model.materials.contains_key(&material),
                "column material missing",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(level: Id) -> ColumnParams {
        ColumnParams {
            name: "C1".into(),
            level,
            center: Point2::new(2.0, 3.0),
            width: 0.4,
            depth: 0.6,
            height: 3.2,
            base_offset: 0.15,
            material: None,
        }
    }

    #[test]
    fn validates_dimensions_coordinates_references_and_elevations() {
        let mut model = Model::new("Columns");
        let level = *model.levels.keys().next().unwrap();
        let valid = column(level);
        valid.validate_in(&model).unwrap();
        assert_eq!(valid.elevations(3.0).unwrap(), (3.15, 6.35));

        for bad in [
            ColumnParams {
                width: 0.0,
                ..valid.clone()
            },
            ColumnParams {
                depth: f64::INFINITY,
                ..valid.clone()
            },
            ColumnParams {
                height: 1e7,
                ..valid.clone()
            },
            ColumnParams {
                base_offset: f64::NAN,
                ..valid.clone()
            },
            ColumnParams {
                center: Point2::new(1e7, 0.0),
                ..valid.clone()
            },
            ColumnParams {
                name: "bad\nname".into(),
                ..valid.clone()
            },
            ColumnParams {
                level: Id::new(),
                ..valid.clone()
            },
            ColumnParams {
                material: Some(Id::new()),
                ..valid.clone()
            },
        ] {
            assert!(bad.validate_in(&model).is_err(), "accepted {bad:?}");
        }
        assert!(valid.elevations(f64::MAX).is_err());

        let material = crate::Material::new(
            "core.material",
            crate::MaterialParams {
                name: "Steel".into(),
                density_kg_m3: 7850.0,
            },
        );
        let material_id = material.id();
        model.materials.insert(material_id, material);
        let mut with_material = valid;
        with_material.material = Some(material_id);
        with_material.validate_in(&model).unwrap();
    }

    #[test]
    fn native_map_has_unique_core_identity_and_round_trips() {
        let mut model = Model::new("Columns");
        let level = *model.levels.keys().next().unwrap();
        let column = Column::new("core.column", column(level));
        let id = column.id();
        model.columns.insert(id, column.clone());
        model.validate().unwrap();
        assert_eq!(
            serde_json::from_slice::<Model>(&serde_json::to_vec(&model).unwrap()).unwrap(),
            model
        );

        let mut duplicate = model.clone();
        duplicate.columns.get_mut(&id).unwrap().header.id = duplicate.project.id();
        assert!(duplicate.validate().is_err());
        let mut wrong_type = model;
        wrong_type.columns.get_mut(&id).unwrap().header.type_id = "core.floor".into();
        assert!(wrong_type.validate().is_err());
    }
}
