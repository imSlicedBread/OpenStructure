//! Independent geometry recipes. Coordinates are right-handed metres, Z up.
use serde::{Deserialize, Serialize};

/// A positive rectangular extrusion, rotated about local Z then translated.
/// Local lower corner is (0,0,0); no native kernel type crosses the wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularPrism {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    pub translation: [f64; 3],
    pub rotation_z: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", deny_unknown_fields)]
pub enum Recipe {
    RectangularPrism(RectangularPrism),
}

impl Recipe {
    pub fn validate(&self) -> crate::ProtocolResult<()> {
        let Self::RectangularPrism(prism) = self;
        crate::ensure(
            [prism.width, prism.depth, prism.height]
                .iter()
                .all(|v| v.is_finite() && *v > 1e-6),
            "prism dimensions must be finite and greater than one micrometre",
        )?;
        crate::ensure(
            prism.translation.iter().all(|v| v.is_finite()) && prism.rotation_z.is_finite(),
            "prism placement must be finite",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_recipe_rejects_nonfinite_and_degenerate_values() {
        let mut p = RectangularPrism {
            width: 0.4,
            depth: 0.6,
            height: 3.0,
            translation: [0.0; 3],
            rotation_z: 0.0,
        };
        assert!(Recipe::RectangularPrism(p.clone()).validate().is_ok());
        for value in [0.0, -1.0, 1e-6, f64::NAN, f64::INFINITY] {
            p.width = value;
            assert!(Recipe::RectangularPrism(p.clone()).validate().is_err());
        }
        p.width = 0.4;
        p.translation[2] = f64::INFINITY;
        assert!(Recipe::RectangularPrism(p).validate().is_err());
    }
}
