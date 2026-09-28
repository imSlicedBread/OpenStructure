//! Analytic wall axes. Display samples are derived and never persisted.
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

pub const WALL_DISPLAY_SAGITTA: f64 = 0.001;
pub const MAX_WALL_ARC_SEGMENTS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum WallPath {
    Straight {
        start: Point2,
        end: Point2,
    },
    CircularArc {
        center: Point2,
        radius: f64,
        start_angle_rad: f64,
        signed_sweep_rad: f64,
    },
}

// Decode through JSON Value before variant dispatch. serde's internal-tag content
// buffer otherwise treats arbitrary-precision JSON numbers as maps in native RPC.
impl<'de> Deserialize<'de> for WallPath {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Straight {
            start: Point2,
            end: Point2,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Arc {
            center: Point2,
            radius: f64,
            start_angle_rad: f64,
            signed_sweep_rad: f64,
        }
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let kind = value
            .as_object_mut()
            .and_then(|v| v.remove("kind"))
            .ok_or_else(|| D::Error::custom("wall path kind missing"))?;
        match kind.as_str() {
            Some("Straight") => {
                let p: Straight = serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(Self::Straight {
                    start: p.start,
                    end: p.end,
                })
            }
            Some("CircularArc") => {
                let p: Arc = serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(Self::CircularArc {
                    center: p.center,
                    radius: p.radius,
                    start_angle_rad: p.start_angle_rad,
                    signed_sweep_rad: p.signed_sweep_rad,
                })
            }
            _ => Err(D::Error::custom("unknown wall path kind")),
        }
    }
}

impl WallPath {
    pub fn straight_start_mut(&mut self) -> Result<&mut Point2> {
        match self {
            Self::Straight { start, .. } => Ok(start),
            _ => Err(os_core::Error::Invalid(
                "straight endpoint editing cannot edit a circular wall; use arc controls".into(),
            )),
        }
    }
    pub fn straight_end_mut(&mut self) -> Result<&mut Point2> {
        match self {
            Self::Straight { end, .. } => Ok(end),
            _ => Err(os_core::Error::Invalid(
                "straight endpoint editing cannot edit a circular wall; use arc controls".into(),
            )),
        }
    }
    pub fn is_straight(self) -> bool {
        matches!(self, Self::Straight { .. })
    }
    pub fn length(self) -> f64 {
        match self {
            Self::Straight { start, end } => start.distance(end),
            Self::CircularArc {
                radius,
                signed_sweep_rad,
                ..
            } => radius * signed_sweep_rad.abs(),
        }
    }
    pub fn start(self) -> Point2 {
        self.point(0.0)
    }
    pub fn end(self) -> Point2 {
        self.point(self.length())
    }
    /// Station in metres of travel from the stored start, for either sweep sign.
    pub fn point(self, station: f64) -> Point2 {
        match self {
            Self::Straight { start, end } => {
                if station == 0.0 {
                    return start;
                }
                let length = start.distance(end);
                if station == length {
                    return end;
                }
                Point2::new(
                    start.x + (end.x - start.x) * station / length,
                    start.y + (end.y - start.y) * station / length,
                )
            }
            Self::CircularArc {
                center,
                radius,
                start_angle_rad,
                signed_sweep_rad,
            } => {
                let angle = start_angle_rad + station / radius * signed_sweep_rad.signum();
                Point2::new(
                    center.x + radius * angle.cos(),
                    center.y + radius * angle.sin(),
                )
            }
        }
    }
    pub fn tangent(self, station: f64) -> Point2 {
        match self {
            Self::Straight { start, end } => Point2::new(
                (end.x - start.x) / self.length(),
                (end.y - start.y) / self.length(),
            ),
            Self::CircularArc {
                radius,
                start_angle_rad,
                signed_sweep_rad,
                ..
            } => {
                let sign = signed_sweep_rad.signum();
                let a = start_angle_rad + sign * station / radius;
                Point2::new(-a.sin() * sign, a.cos() * sign)
            }
        }
    }
    pub fn offset_point(self, station: f64, left: f64) -> Point2 {
        let p = self.point(station);
        let t = self.tangent(station);
        Point2::new(p.x - t.y * left, p.y + t.x * left)
    }
    /// Closest bounded station; endpoints win ties, including the circle centre.
    pub fn project(self, point: Point2) -> f64 {
        match self {
            Self::Straight { start, .. } => {
                let t = self.tangent(0.0);
                ((point.x - start.x) * t.x + (point.y - start.y) * t.y).clamp(0.0, self.length())
            }
            Self::CircularArc {
                center,
                radius,
                start_angle_rad,
                signed_sweep_rad,
            } => {
                if point == center {
                    return 0.0;
                }
                let angle = (point.y - center.y).atan2(point.x - center.x);
                let travel =
                    ((angle - start_angle_rad) * signed_sweep_rad.signum()).rem_euclid(TAU);
                if travel <= signed_sweep_rad.abs() {
                    travel * radius
                } else if point.distance(self.start()) <= point.distance(self.end()) {
                    0.0
                } else {
                    self.length()
                }
            }
        }
    }
    /// Segment count for the outermost display face. Never silently relax tolerance.
    pub fn display_segments(self, half_thickness: f64) -> Result<usize> {
        if let Self::CircularArc {
            radius,
            signed_sweep_rad,
            ..
        } = self
        {
            let outer = radius + half_thickness;
            let step =
                (2.0 * (1.0 - WALL_DISPLAY_SAGITTA / outer).clamp(-1.0, 1.0).acos()).min(PI / 18.0);
            let count = (signed_sweep_rad.abs() / step).ceil();
            ensure(
                count.is_finite() && count >= 1.0 && count <= MAX_WALL_ARC_SEGMENTS as f64,
                "circular wall exceeds 4096 segments at 1 mm display tolerance",
            )?;
            Ok(count as usize)
        } else {
            Ok(1)
        }
    }
    pub fn validate(self, thickness: f64) -> Result<()> {
        match self {
            Self::Straight { start, end } => ensure(
                start.is_finite() && end.is_finite(),
                "wall endpoints must be finite",
            )?,
            Self::CircularArc {
                center,
                radius,
                start_angle_rad,
                signed_sweep_rad,
            } => {
                ensure(
                    center.is_finite()
                        && radius.is_finite()
                        && start_angle_rad.is_finite()
                        && signed_sweep_rad.is_finite(),
                    "circular wall values must be finite",
                )?;
                ensure(
                    radius > thickness / 2.0 + 1e-6,
                    "circular wall inner radius must exceed one micrometre after resolved thickness",
                )?;
                ensure(
                    start_angle_rad.abs() <= TAU
                        && signed_sweep_rad.abs() > 1e-6
                        && signed_sweep_rad.abs() < TAU - 1e-6,
                    "circular wall needs a bounded nonzero sweep below a full circle and a normalized start angle",
                )?;
                self.display_segments(thickness / 2.0)?;
            }
        }
        ensure(
            self.length().is_finite()
                && self.length() > 1e-6
                && self.start().is_finite()
                && self.end().is_finite(),
            "wall length must exceed one micrometre without overflow",
        )
    }
    /// Circle through start, an interior point, and end; supports major/minor arcs.
    pub fn through_three_points(start: Point2, bulge: Point2, end: Point2) -> Result<Self> {
        ensure(
            start.is_finite() && bulge.is_finite() && end.is_finite(),
            "arc points must be finite",
        )?;
        let (bx, by, ex, ey) = (
            bulge.x - start.x,
            bulge.y - start.y,
            end.x - start.x,
            end.y - start.y,
        );
        let d = 2.0 * (bx * ey - by * ex);
        ensure(
            d.is_finite()
                && d.abs() > 1e-9 * bulge.distance(start).max(end.distance(start)).powi(2),
            "arc points must be distinct and non-collinear",
        )?;
        let (b2, e2) = (bx * bx + by * by, ex * ex + ey * ey);
        let center = Point2::new(
            start.x + (ey * b2 - by * e2) / d,
            start.y + (bx * e2 - ex * b2) / d,
        );
        let a = (start.y - center.y).atan2(start.x - center.x);
        let b = ((bulge.y - center.y).atan2(bulge.x - center.x) - a).rem_euclid(TAU);
        let e = ((end.y - center.y).atan2(end.x - center.x) - a).rem_euclid(TAU);
        let path = Self::CircularArc {
            center,
            radius: start.distance(center),
            start_angle_rad: a,
            signed_sweep_rad: if b < e { e } else { e - TAU },
        };
        path.validate(0.0)?;
        Ok(path)
    }
    pub fn translated(self, delta: Point2) -> Self {
        let add = |p: Point2| Point2::new(p.x + delta.x, p.y + delta.y);
        match self {
            Self::Straight { start, end } => Self::Straight {
                start: add(start),
                end: add(end),
            },
            Self::CircularArc {
                center,
                radius,
                start_angle_rad,
                signed_sweep_rad,
            } => Self::CircularArc {
                center: add(center),
                radius,
                start_angle_rad,
                signed_sweep_rad,
            },
        }
    }
}
