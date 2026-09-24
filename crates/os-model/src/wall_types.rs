//! Project-owned compound profiles. Order is physical; UUIDs survive reorder.
use crate::{Entity, Model, WallParams};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_WALL_TYPES: usize = 1024;
pub const MAX_WALL_LAYERS: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerFunction {
    Structure,
    Substrate,
    Insulation,
    Finish,
    Other,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallLayer {
    pub id: Id,
    pub name: String,
    pub thickness: f64,
    pub function: LayerFunction,
    pub material: Option<Id>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallTypeParams {
    pub name: String,
    pub layers: Vec<WallLayer>,
}
pub type WallType = Entity<WallTypeParams>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallTypeAssignment {
    pub type_id: Id,
    pub flipped: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedWallLayer {
    pub id: Option<Id>,
    pub name: String,
    pub function: LayerFunction,
    pub material: Option<Id>,
    pub density_kg_m3: Option<f64>,
    pub min: f64,
    pub max: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedWall {
    pub parameters: WallParams,
    pub layers: Vec<ResolvedWallLayer>,
}
fn name_valid(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
}
impl WallTypeParams {
    pub fn validate(&self, model: &Model) -> Result<()> {
        ensure(
            name_valid(&self.name),
            "wall type name must be 1-256 bytes without controls",
        )?;
        ensure(
            !self.layers.is_empty() && self.layers.len() <= MAX_WALL_LAYERS,
            "wall type needs 1-32 layers",
        )?;
        let mut ids = BTreeSet::new();
        let mut total = 0.0;
        for layer in &self.layers {
            ensure(
                !layer.id.0.is_nil() && ids.insert(layer.id),
                "duplicate or nil wall layer ID",
            )?;
            ensure(name_valid(&layer.name), "invalid wall layer name")?;
            ensure(
                layer.thickness.is_finite()
                    && layer.thickness >= 0.00001
                    && layer.thickness <= 10.0,
                "layer thickness must be 0.00001-10 m",
            )?;
            total += layer.thickness;
            if let Some(id) = layer.material {
                ensure(
                    model.materials.contains_key(&id),
                    "wall layer material missing",
                )?;
            }
        }
        ensure(total <= 10.0, "compound wall thickness exceeds 10 m")
    }
}
impl Model {
    /// Sole profile resolver. Legacy parameters remain independent and untouched.
    /// Unflipped order runs from the axis's right side (-normal) to left (+normal).
    pub fn resolve_wall(&self, id: Id) -> Result<ResolvedWall> {
        let wall = self
            .walls
            .get(&id)
            .ok_or_else(|| os_core::Error::Invalid("wall missing".into()))?;
        let mut parameters = wall.parameters.clone();
        let layers = if let Some(assignment) = self.wall_type_assignments.get(&id) {
            let ty = self
                .wall_types
                .get(&assignment.type_id)
                .ok_or_else(|| os_core::Error::Invalid("wall type missing".into()))?;
            ty.parameters.validate(self)?;
            parameters.thickness = ty.parameters.layers.iter().map(|l| l.thickness).sum();
            parameters.material = None;
            let mut offset = -parameters.thickness / 2.0;
            ty.parameters
                .layers
                .iter()
                .map(|layer| {
                    let min = offset;
                    offset += layer.thickness;
                    let (min, max) = if assignment.flipped {
                        (-offset, -min)
                    } else {
                        (min, offset)
                    };
                    ResolvedWallLayer {
                        id: Some(layer.id),
                        name: layer.name.clone(),
                        function: layer.function,
                        material: layer.material,
                        density_kg_m3: layer
                            .material
                            .map(|id| self.materials[&id].parameters.density_kg_m3),
                        min,
                        max,
                    }
                })
                .collect()
        } else {
            vec![ResolvedWallLayer {
                id: None,
                name: parameters.name.clone(),
                function: LayerFunction::Other,
                material: parameters.material,
                density_kg_m3: parameters
                    .material
                    .and_then(|id| self.materials.get(&id))
                    .map(|m| m.parameters.density_kg_m3),
                min: -parameters.thickness / 2.0,
                max: parameters.thickness / 2.0,
            }]
        };
        parameters.validate()?;
        Ok(ResolvedWall { parameters, layers })
    }
}
