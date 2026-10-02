//! Project-owned Length drivers for native opening type dimensions.
use crate::{Entity, Model, OpeningKind, OpeningTypeParams};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_LENGTH_PARAMETERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    Metres,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LengthParameterParams {
    pub name: String,
    pub unit: LengthUnit,
    pub value: f64,
}
pub type LengthParameter = Entity<LengthParameterParams>;

impl LengthParameterParams {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "Length parameter name must be 1-256 bytes without control characters",
        )?;
        ensure(
            self.value.is_finite() && self.value >= 0.,
            "Length parameter must be finite and nonnegative",
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningTypeLengthBindings {
    #[serde(deserialize_with = "required_binding")]
    pub width: Option<Id>,
    #[serde(deserialize_with = "required_binding")]
    pub height: Option<Id>,
    #[serde(deserialize_with = "required_binding")]
    pub sill: Option<Id>,
}
fn required_binding<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Id>, D::Error> {
    Option::<Id>::deserialize(d)
}
impl OpeningTypeLengthBindings {
    pub fn references(self) -> std::collections::BTreeSet<Id> {
        [self.width, self.height, self.sill]
            .into_iter()
            .flatten()
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthSource {
    TypeLiteral,
    ProjectParameter(Id),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedOpeningType {
    pub parameters: OpeningTypeParams,
    /// Width, height and sill sources, in that order. Instance pins are applied
    /// by resolve_opening after resolving the type.
    pub sources: [LengthSource; 3],
}

impl Model {
    pub fn resolve_opening_type(&self, id: Id) -> Result<ResolvedOpeningType> {
        let mut parameters = self
            .opening_types
            .get(&id)
            .ok_or_else(|| os_core::Error::Invalid("opening type missing".into()))?
            .parameters
            .clone();
        let bindings = self
            .opening_type_length_bindings
            .get(&id)
            .copied()
            .unwrap_or_default();
        ensure(
            parameters.kind != OpeningKind::Door || bindings.sill.is_none(),
            "doors cannot bind a sill parameter",
        )?;
        let mut sources = [LengthSource::TypeLiteral; 3];
        for (index, (binding, value)) in [
            (bindings.width, &mut parameters.width),
            (bindings.height, &mut parameters.height),
            (bindings.sill, &mut parameters.sill),
        ]
        .into_iter()
        .enumerate()
        {
            // Inactive literals remain well-formed for persistence, but family
            // proportions must be checked against the effective dimensions.
            ensure(
                value.is_finite() && *value >= 0.,
                "invalid literal Length fallback",
            )?;
            if let Some(parameter) = binding {
                let driver = self.length_parameters.get(&parameter).ok_or_else(|| {
                    os_core::Error::Invalid("shared Length parameter missing".into())
                })?;
                driver.parameters.validate()?;
                *value = driver.parameters.value;
                sources[index] = LengthSource::ProjectParameter(parameter);
            }
        }
        parameters.validate()?;
        Ok(ResolvedOpeningType {
            parameters,
            sources,
        })
    }

    /// Reports type identities and dimensions, including unplaced types.
    pub fn length_parameter_uses(&self, id: Id) -> Vec<(Id, &'static str)> {
        let mut uses = Vec::new();
        for (ty, b) in &self.opening_type_length_bindings {
            for (binding, dimension) in [(b.width, "width"), (b.height, "height"), (b.sill, "sill")]
            {
                if binding == Some(id) {
                    uses.push((*ty, dimension));
                }
            }
        }
        uses
    }

    pub fn validate_opening_lengths(&self) -> Result<()> {
        ensure(
            self.length_parameters.len() <= MAX_LENGTH_PARAMETERS,
            "project Length parameter limit (64) exceeded",
        )?;
        for p in self.length_parameters.values() {
            p.parameters.validate()?;
        }
        for (id, bindings) in &self.opening_type_length_bindings {
            ensure(
                self.opening_types.contains_key(id),
                "Length binding type missing",
            )?;
            ensure(
                *bindings != OpeningTypeLengthBindings::default(),
                "empty Length bindings must be omitted",
            )?;
        }
        for id in self.opening_types.keys() {
            self.resolve_opening_type(*id)?;
        }
        Ok(())
    }

    /// Changing a binding to literal freezes its currently resolved dimension.
    pub fn set_opening_type_length_bindings(
        &mut self,
        id: Id,
        bindings: OpeningTypeLengthBindings,
    ) -> Result<()> {
        let before = self
            .opening_type_length_bindings
            .get(&id)
            .copied()
            .unwrap_or_default();
        ensure(
            self.opening_types.contains_key(&id),
            "Length binding type missing",
        )?;
        if (before.width.is_some() && bindings.width.is_none())
            || (before.height.is_some() && bindings.height.is_none())
            || (before.sill.is_some() && bindings.sill.is_none())
        {
            let resolved = self.resolve_opening_type(id)?.parameters;
            let p = &mut self.opening_types.get_mut(&id).unwrap().parameters;
            if before.width.is_some() && bindings.width.is_none() {
                p.width = resolved.width;
            }
            if before.height.is_some() && bindings.height.is_none() {
                p.height = resolved.height;
            }
            if before.sill.is_some() && bindings.sill.is_none() {
                p.sill = resolved.sill;
            }
        }
        if bindings == OpeningTypeLengthBindings::default() {
            self.opening_type_length_bindings.remove(&id);
        } else {
            self.opening_type_length_bindings.insert(id, bindings);
        }
        Ok(())
    }
}
