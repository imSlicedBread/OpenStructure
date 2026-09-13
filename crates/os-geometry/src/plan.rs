//! Horizontal cuts/projections for the existing validated rectangular prisms.
//! Not a general section kernel, wall join solver or arbitrary plugin plan provider.
use crate::{GeometryKernel, PrismKernel, Solid};
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};

/// Metres, not machine epsilon or a screen-space acquisition radius.
pub const PLAN_TOLERANCE: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanRange {
    pub top: f64,
    pub cut: f64,
    pub bottom: f64,
    pub depth: f64,
}
impl Default for PlanRange {
    fn default() -> Self {
        Self {
            top: 2.5,
            cut: 1.2,
            bottom: 0.0,
            depth: -1.0,
        }
    }
}
impl PlanRange {
    pub fn validate(self) -> Result<()> {
        ensure(
            [self.top, self.cut, self.bottom, self.depth]
                .iter()
                .all(|v| v.is_finite())
                && self.depth <= self.bottom
                && self.bottom <= self.cut
                && self.cut <= self.top
                && self.top - self.depth > PLAN_TOLERANCE
                && (self.top - self.depth).is_finite(),
            "plan range needs finite depth <= bottom <= cut <= top and positive depth span",
        )
    }

    /// Convert level-relative settings to absolute heights; reject overflow.
    pub fn at_level(self, elevation: f64) -> Result<Self> {
        self.validate()?;
        ensure(elevation.is_finite(), "plan level elevation is not finite")?;
        let absolute = Self {
            top: self.top + elevation,
            cut: self.cut + elevation,
            bottom: self.bottom + elevation,
            depth: self.depth + elevation,
        };
        absolute.validate()?;
        Ok(absolute)
    }
}

/// Right-handed horizontal plane: +X right, +Y up, +Z normal; yaw is radians.
/// The XY origin is subtracted before rotation to retain large-coordinate detail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HorizontalBasis {
    pub origin: Point2,
    pub rotation: f64,
}
impl HorizontalBasis {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.origin.is_finite() && self.rotation.is_finite(),
            "invalid horizontal plan basis",
        )
    }
    pub fn world_to_plane(self, point: Point2) -> Result<Point2> {
        self.validate()?;
        ensure(point.is_finite(), "invalid world point")?;
        let (s, c) = self.rotation.sin_cos();
        let x = point.x - self.origin.x;
        let y = point.y - self.origin.y;
        let result = Point2::new(c * x + s * y, -s * x + c * y);
        ensure(result.is_finite(), "plan coordinate overflow")?;
        Ok(result)
    }
    pub fn plane_to_world(self, point: Point2) -> Result<Point2> {
        self.validate()?;
        ensure(point.is_finite(), "invalid plane point")?;
        let (s, c) = self.rotation.sin_cos();
        let result = Point2::new(
            c * point.x - s * point.y + self.origin.x,
            s * point.x + c * point.y + self.origin.y,
        );
        ensure(result.is_finite(), "world coordinate overflow")?;
        Ok(result)
    }
}

/// Axis-aligned in view-plane coordinates; rotating the basis rotates the crop.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanCrop {
    pub min: Point2,
    pub max: Point2,
}
impl PlanCrop {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.min.is_finite()
                && self.max.is_finite()
                && self.min.x < self.max.x
                && self.min.y < self.max.y
                && (self.max.x - self.min.x).is_finite()
                && (self.max.y - self.min.y).is_finite(),
            "invalid plan crop",
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum PlanRole {
    Cut,
    Projected,
    Depth,
}

/// Checked convex polygon, CCW in view-plane metres. Maximum eight vertices.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanFootprint {
    pub role: PlanRole,
    vertices: Vec<Point2>,
}
impl PlanFootprint {
    pub fn vertices(&self) -> &[Point2] {
        &self.vertices
    }
    pub fn area(&self) -> f64 {
        signed_area(&self.vertices)
    }

    /// Hit-test only the visible clipped polygon. Screen acquisition belongs to
    /// the viewport, not this geometric containment test.
    pub fn contains(&self, point: Point2) -> bool {
        point.is_finite()
            && self
                .vertices
                .iter()
                .zip(self.vertices.iter().cycle().skip(1))
                .all(|(a, b)| {
                    let cross = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
                    cross.is_finite() && cross >= -PLAN_TOLERANCE * a.distance(*b)
                })
    }
}

/// Derive a true horizontal cut or downward projection, never a camera screenshot.
/// Range is absolute; use `at_level` for level-relative settings. Above-cut-only
/// prisms are hidden in this initial wall policy (no overhead/symbolic inference).
pub fn rectangular_plan(
    solid: &Solid,
    range: PlanRange,
    basis: HorizontalBasis,
    crop: Option<PlanCrop>,
) -> Result<Option<PlanFootprint>> {
    range.validate()?;
    basis.validate()?;
    if let Some(crop) = crop {
        crop.validate()?;
    }
    // Validate even hidden input, so invalid geometry cannot masquerade as a
    // successful empty drawing. The existing kernel rejects non-rectangular forms.
    let mesh = PrismKernel.tessellate(solid)?;
    let base = solid.transform.translation.z;
    let top = base + solid.height;
    ensure(top.is_finite(), "plan prism height overflow")?;
    if top <= range.depth + PLAN_TOLERANCE || base >= range.top - PLAN_TOLERANCE {
        return Ok(None);
    }
    let role = if base <= range.cut + PLAN_TOLERANCE && top > range.cut + PLAN_TOLERANCE {
        PlanRole::Cut
    } else if top <= range.cut + PLAN_TOLERANCE && top > range.bottom + PLAN_TOLERANCE {
        PlanRole::Projected
    } else if top <= range.bottom + PLAN_TOLERANCE && top > range.depth + PLAN_TOLERANCE {
        PlanRole::Depth
    } else {
        return Ok(None);
    };
    let mut vertices = mesh.vertices[..4]
        .iter()
        .map(|p| basis.world_to_plane(Point2::new(p.x, p.y)))
        .collect::<Result<Vec<_>>>()?;
    let original_area = signed_area(&vertices);
    ensure(
        original_area.is_finite() && original_area > PLAN_TOLERANCE * PLAN_TOLERANCE,
        "plan basis loses geometry precision or overflows",
    )?;
    if let Some(crop) = crop {
        for (axis, boundary, keep_greater) in [
            (0, crop.min.x, true),
            (0, crop.max.x, false),
            (1, crop.min.y, true),
            (1, crop.max.y, false),
        ] {
            vertices = clip(&vertices, axis, boundary, keep_greater)?;
        }
    }
    deduplicate(&mut vertices);
    if vertices.len() < 3 {
        return Ok(None);
    }
    let area = signed_area(&vertices);
    ensure(
        vertices.len() <= 8 && area.is_finite() && area >= 0.0,
        "invalid clipped plan polygon",
    )?;
    if area <= PLAN_TOLERANCE * PLAN_TOLERANCE {
        return Ok(None);
    }
    Ok(Some(PlanFootprint { role, vertices }))
}

fn clip(input: &[Point2], axis: usize, boundary: f64, keep_greater: bool) -> Result<Vec<Point2>> {
    let mut output = Vec::with_capacity(8);
    let coord = |point: Point2| if axis == 0 { point.x } else { point.y };
    let inside = |point: Point2| {
        if keep_greater {
            coord(point) >= boundary
        } else {
            coord(point) <= boundary
        }
    };
    for (&a, &b) in input.iter().zip(input.iter().cycle().skip(1)) {
        if inside(a) != inside(b) {
            let t = (boundary - coord(a)) / (coord(b) - coord(a));
            let mut point = Point2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
            if axis == 0 {
                point.x = boundary;
            } else {
                point.y = boundary;
            }
            ensure(
                t.is_finite() && (0.0..=1.0).contains(&t) && point.is_finite(),
                "plan crop intersection overflow",
            )?;
            output.push(point);
        }
        if inside(b) {
            output.push(b);
        }
    }
    // Crossing exactly onto a corner can emit both an intersection and the
    // existing vertex. Remove repeats before the next clip/count check.
    deduplicate(&mut output);
    ensure(
        output.len() <= 8,
        "plan crop polygon exceeds eight vertices",
    )?;
    Ok(output)
}

fn deduplicate(vertices: &mut Vec<Point2>) {
    vertices.dedup_by(|a, b| a.distance(*b) <= PLAN_TOLERANCE);
    if vertices.len() > 1 && vertices[0].distance(*vertices.last().unwrap()) <= PLAN_TOLERANCE {
        vertices.pop();
    }
}

fn signed_area(vertices: &[Point2]) -> f64 {
    let origin = vertices[0];
    vertices[1..]
        .windows(2)
        .map(|pair| {
            let a = Point2::new(pair[0].x - origin.x, pair[0].y - origin.y);
            let b = Point2::new(pair[1].x - origin.x, pair[1].y - origin.y);
            (a.x * b.y - a.y * b.x) * 0.5
        })
        .sum()
}
