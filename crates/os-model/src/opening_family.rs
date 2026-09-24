//! Versioned, bounded opening components and independent wall-cut profiles.
use os_core::{Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningHostCut {
    Rectangular,
    Profile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningComponentOperation {
    ProfileExtrusion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningFamily {
    pub version: u32,
    pub host_cut: OpeningHostCut,
    pub operation: OpeningComponentOperation,
    /// One simple closed ring (closure implicit), in width/height fractions.
    pub profile: Vec<Point2>,
    /// Independent host void, in width/height fractions (implicit closure).
    pub cut_profile: Vec<Point2>,
    /// Maximum extrusion depth in metres; limited to 20% of host/width at resolution.
    pub depth: f64,
    /// Frame rail width in metres. Zero disables the authored frame.
    pub frame_width: f64,
    /// Frame extrusion depth in metres, capped by the host wall thickness.
    pub frame_depth: f64,
}

impl Default for OpeningFamily {
    fn default() -> Self {
        Self {
            version: 3,
            host_cut: OpeningHostCut::Rectangular,
            operation: OpeningComponentOperation::ProfileExtrusion,
            profile: vec![
                Point2::new(0., 0.),
                Point2::new(1., 0.),
                Point2::new(1., 1.),
                Point2::new(0., 1.),
            ],
            depth: 0.025,
            cut_profile: Self::rectangle(),
            frame_width: 0.0,
            frame_depth: 0.05,
        }
    }
}

fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

impl OpeningFamily {
    pub const MAX_VERTICES: usize = 32;

    pub fn rectangle() -> Vec<Point2> {
        vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(1., 1.),
            Point2::new(0., 1.),
        ]
    }

    pub fn rectangular_cut(&self) -> bool {
        // Equivalent windings/starting vertices/forward collinear edges retain
        // the historical rectangle mesh and remain compatible with frames.
        self.cut_profile
            .iter()
            .all(|p| p.x == 0. || p.x == 1. || p.y == 0. || p.y == 1.)
            && Self::rectangle()
                .iter()
                .all(|p| self.cut_profile.contains(p))
    }

    pub fn validate(&self) -> Result<()> {
        ensure(self.version == 3, "unsupported opening family version")?;
        ensure(
            self.depth.is_finite() && (0.001..=0.2).contains(&self.depth),
            "component depth must be 1-200 mm",
        )?;
        ensure(
            self.frame_width.is_finite()
                && (self.frame_width == 0.0 || (0.001..=0.3).contains(&self.frame_width)),
            "frame width must be zero or 1-300 mm",
        )?;
        ensure(
            self.frame_depth.is_finite() && (0.001..=0.5).contains(&self.frame_depth),
            "frame depth must be 1-500 mm",
        )?;
        Self::validate_ring(&self.profile)?;
        Self::validate_ring(&self.cut_profile)?;
        ensure(
            self.host_cut != OpeningHostCut::Rectangular || self.rectangular_cut(),
            "rectangular host cut requires the full rectangular cut profile",
        )
    }

    fn validate_ring(p: &[Point2]) -> Result<()> {
        ensure(
            (3..=Self::MAX_VERTICES).contains(&p.len()),
            "profile needs 3-32 vertices",
        )?;
        ensure(
            p.iter()
                .all(|p| p.is_finite() && (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y)),
            "profile points must be finite width/height fractions from 0 to 1",
        )?;
        // Distance-to-segment checks reject crossings, touching, overlapping and
        // nearly coincident nonadjacent edges. Adjacent forward collinearity is OK.
        let distance = |q: Point2, a: Point2, b: Point2| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let t = (((q.x - a.x) * dx + (q.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
            q.distance(Point2::new(a.x + t * dx, a.y + t * dy))
        };
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            ensure(a.distance(b) >= 1e-4, "profile edge is too short")?;
            let prev = p[(i + p.len() - 1) % p.len()];
            ensure(
                cross(prev, a, b).abs() > 1e-10
                    || (a.x - prev.x) * (b.x - a.x) + (a.y - prev.y) * (b.y - a.y) > 0.,
                "profile edge doubles back",
            )?;
            for j in i + 1..p.len() {
                ensure(
                    p[i].distance(p[j]) >= 1e-4,
                    "profile has duplicate or near-coincident vertices",
                )?;
                if j == i + 1 || (i == 0 && j == p.len() - 1) {
                    continue;
                }
                let (c, d) = (p[j], p[(j + 1) % p.len()]);
                let crossing = cross(a, b, c) * cross(a, b, d) <= 0.
                    && cross(c, d, a) * cross(c, d, b) <= 0.
                    && a.x.min(b.x) <= c.x.max(d.x)
                    && c.x.min(d.x) <= a.x.max(b.x)
                    && a.y.min(b.y) <= c.y.max(d.y)
                    && c.y.min(d.y) <= a.y.max(b.y);
                ensure(
                    !crossing
                        && [
                            distance(c, a, b),
                            distance(d, a, b),
                            distance(a, c, d),
                            distance(b, c, d),
                        ]
                        .into_iter()
                        .all(|v| v >= 1e-6),
                    "profile edges cross, touch or overlap",
                )?;
            }
        }
        let area = p[1..]
            .windows(2)
            .map(|q| cross(p[0], q[0], q[1]))
            .sum::<f64>()
            .abs()
            * 0.5;
        ensure(area >= 1e-4, "profile area is too small")
    }

    pub fn validate_for(&self, width: f64, height: f64, kind: crate::OpeningKind) -> Result<()> {
        self.validate()?;
        ensure(
            width.is_finite() && height.is_finite() && width > 0. && height > 0.,
            "invalid opening dimensions",
        )?;
        ensure(
            self.frame_width == 0.0 || self.rectangular_cut(),
            "rectangular frame rails require a full rectangular host cut; remove the frame before shaping the cut",
        )?;
        ensure(
            ring_contains_ring(
                &self.cut_profile,
                &self.component_profile(width, height, kind),
            ),
            "component profile must fit within the host cut",
        )?;
        if self.frame_width > 0.0 {
            let horizontal = width - 2.0 * self.frame_width;
            let vertical = height
                - self.frame_width
                    * if kind == crate::OpeningKind::Window {
                        2.0
                    } else {
                        1.0
                    };
            ensure(
                horizontal >= 0.001 && vertical >= 0.001,
                "frame leaves less than 1 mm for the opening component",
            )?;
        }
        Ok(())
    }

    /// Inner panel/pane profile after reserving the frame rails. Door frames
    /// have no threshold; window frames have both a sill and a head rail.
    pub fn component_profile(
        &self,
        width: f64,
        height: f64,
        kind: crate::OpeningKind,
    ) -> Vec<Point2> {
        if self.frame_width == 0.0 {
            return self.profile.clone();
        }
        let x_inset = self.frame_width / width;
        let y_bottom = if kind == crate::OpeningKind::Window {
            self.frame_width / height
        } else {
            0.0
        };
        let y_top = self.frame_width / height;
        self.profile
            .iter()
            .map(|p| {
                Point2::new(
                    x_inset + p.x * (1.0 - 2.0 * x_inset),
                    y_bottom + p.y * (1.0 - y_bottom - y_top),
                )
            })
            .collect()
    }

    /// Horizontal section intervals in normalized profile space. Half-open edge
    /// crossings avoid counting a shared vertex twice; top boundary uses its limit.
    pub fn section_spans(&self, height: f64) -> Vec<(f64, f64)> {
        let top = self
            .profile
            .iter()
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let y = if height == top {
            height - 1e-10
        } else {
            height
        };
        let mut xs = Vec::new();
        for (a, b) in self
            .profile
            .iter()
            .zip(self.profile.iter().cycle().skip(1))
            .take(self.profile.len())
        {
            if (a.y <= y && y < b.y) || (b.y <= y && y < a.y) {
                xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }
        xs.sort_by(f64::total_cmp);
        xs.chunks_exact(2)
            .filter(|p| p[1] - p[0] > 1e-9)
            .map(|p| (p[0], p[1]))
            .collect()
    }
}

/// Test every interval of every component edge, including crossings of a concave
/// cut. Vertex-only containment would accept edges spanning a notch.
fn ring_contains_ring(outer: &[Point2], inner: &[Point2]) -> bool {
    let contains = |p: Point2| {
        let mut inside = false;
        for (a, b) in outer
            .iter()
            .zip(outer.iter().cycle().skip(1))
            .take(outer.len())
        {
            if cross(*a, *b, p).abs() <= 1e-10
                && p.x >= a.x.min(b.x) - 1e-10
                && p.x <= a.x.max(b.x) + 1e-10
                && p.y >= a.y.min(b.y) - 1e-10
                && p.y <= a.y.max(b.y) + 1e-10
            {
                return true;
            }
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
                inside = !inside;
            }
        }
        inside
    };
    for (a, b) in inner
        .iter()
        .zip(inner.iter().cycle().skip(1))
        .take(inner.len())
    {
        if !contains(*a) {
            return false;
        }
        let mut ts = vec![0., 1.];
        for (c, d) in outer
            .iter()
            .zip(outer.iter().cycle().skip(1))
            .take(outer.len())
        {
            let denominator = (b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x);
            if denominator.abs() > 1e-14 {
                let t = ((c.x - a.x) * (d.y - c.y) - (c.y - a.y) * (d.x - c.x)) / denominator;
                let u = ((c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)) / denominator;
                if (0. ..=1.).contains(&t) && (0. ..=1.).contains(&u) {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        if ts.windows(2).any(|pair| {
            let t = (pair[0] + pair[1]) * 0.5;
            !contains(Point2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)))
        }) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_containment_checks_edges_through_concave_notches_and_frame_compatibility() {
        let mut family = OpeningFamily {
            host_cut: OpeningHostCut::Profile,
            cut_profile: vec![
                Point2::new(0., 0.),
                Point2::new(1., 0.),
                Point2::new(1., 1.),
                Point2::new(0.6, 1.),
                Point2::new(0.6, 0.4),
                Point2::new(0.4, 0.4),
                Point2::new(0.4, 1.),
                Point2::new(0., 1.),
            ],
            ..Default::default()
        };
        // All rectangle vertices are inside; its top edge crosses the notch.
        assert!(
            family
                .validate_for(1., 2., crate::OpeningKind::Window)
                .is_err()
        );
        family.profile = family.cut_profile.clone();
        family
            .validate_for(1., 2., crate::OpeningKind::Window)
            .unwrap();
        family.frame_width = 0.05;
        assert!(
            family
                .validate_for(1., 2., crate::OpeningKind::Window)
                .is_err()
        );
        family.frame_width = 0.;
        family.cut_profile[1] = family.cut_profile[0];
        assert!(family.validate().is_err());
    }

    #[test]
    fn opening_family_bounds_winding_sections_and_wire_versions() {
        let mut family = OpeningFamily::default();
        family.profile.insert(1, Point2::new(0.5, 0.));
        for _ in 0..2 {
            family.validate().unwrap();
            assert_eq!(family.section_spans(0.5), vec![(0., 1.)]);
            family.profile.reverse();
        }
        for key in [
            "version",
            "host_cut",
            "operation",
            "depth",
            "profile",
            "cut_profile",
            "frame_width",
            "frame_depth",
        ] {
            let mut value = serde_json::to_value(&family).unwrap();
            value.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<OpeningFamily>(value).is_err());
        }
        let mut future = serde_json::to_value(&family).unwrap();
        future["operation"] = serde_json::json!("Sweep");
        assert!(serde_json::from_value::<OpeningFamily>(future).is_err());
        family.version = 4;
        assert!(family.validate().is_err());
        family.version = 3;
        family.frame_width = 0.3;
        assert!(
            family
                .validate_for(0.5, 0.5, crate::OpeningKind::Window)
                .is_err()
        );
        family.frame_width = 0.05;
        assert_eq!(
            family.component_profile(1., 1., crate::OpeningKind::Door)[0],
            Point2::new(0.05, 0.)
        );
        assert_eq!(
            family.component_profile(1., 1., crate::OpeningKind::Window)[0],
            Point2::new(0.05, 0.05)
        );
        family.version = 1;
        family.profile[1] = family.profile[0];
        assert!(family.validate().is_err());
    }
}
