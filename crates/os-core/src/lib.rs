//! Stable identity, explicit SI conversion and shared errors.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Persistent identity. Array positions are never identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub uuid::Uuid);

impl Id {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid data: {0}")]
    Invalid(String),
    #[error("Permission denied: {0}")]
    Permission(String),
    #[error("Unsupported: {0}")]
    Unsupported(String),
    #[error("Storage error: {0}")]
    Storage(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn ensure(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}

/// Explicit conversion boundary; model coordinates and dimensions are metres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    Metres,
    Millimetres,
    Feet,
}
impl LengthUnit {
    pub fn to_metres(self, value: f64) -> Result<f64> {
        ensure(value.is_finite(), "length must be finite")?;
        let metres = value
            * match self {
                Self::Metres => 1.0,
                Self::Millimetres => 0.001,
                Self::Feet => 0.3048,
            };
        ensure(metres.is_finite(), "converted length overflow")?;
        Ok(metres)
    }
    pub fn from_metres(self, value: f64) -> Result<f64> {
        ensure(value.is_finite(), "length must be finite")?;
        let value = value / self.to_metres(1.0)?;
        ensure(value.is_finite(), "converted length overflow")?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}
impl Point2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_and_units() {
        let a = Id::new();
        assert_ne!(a, Id::new());
        assert_eq!(a.0.get_version_num(), 4);
        assert_eq!(LengthUnit::Millimetres.to_metres(1000.0).unwrap(), 1.0);
        assert!((LengthUnit::Feet.from_metres(0.3048).unwrap() - 1.0).abs() < 1e-12);
        assert!(LengthUnit::Metres.to_metres(f64::NAN).is_err());
    }
}
