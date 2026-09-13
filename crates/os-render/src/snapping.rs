//! Checked semantic-segment snapping in view-plane metres, acquired in logical pixels.
use crate::plan::{MAX_PLAN_ELEMENTS, PlanCamera, PlanContext};
use os_core::{Id, Point2, Result, ensure};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug)]
pub struct SnapSegment {
    pub entity: Id,
    /// Stable semantic feature key supplied by the host/provider, never a pixel index.
    pub feature: u32,
    pub start: Point2,
    pub end: Point2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SnapKind {
    Endpoint,
    Intersection,
    Perpendicular,
    Midpoint,
    GridAxis,
    Nearest,
    AxisExtension,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapQuery {
    pub camera: PlanCamera,
    pub viewport: [f64; 2],
    pub pointer: Point2,
    pub radius_pixels: f64,
    pub endpoints: bool,
    pub midpoints: bool,
    pub intersections: bool,
    /// Gesture anchor in view-plane metres; None disables perpendicular acquisition.
    pub perpendicular_from: Option<Point2>,
    pub nearest: bool,
    /// Optional construction axes beyond finite segment endpoints, not geometry picking.
    pub axis_extensions: bool,
    /// Editing targets are excluded after acquiring a base point. Part of query identity.
    pub exclude_entity: Option<Id>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapCandidate {
    pub entity: Id,
    pub feature: u32,
    /// Second semantic feature for an intersection, ordered after the first key.
    pub other: Option<(Id, u32)>,
    pub kind: SnapKind,
    /// 0 = start, 1 = end for endpoint snaps; zero for other kinds.
    pub endpoint: u8,
    pub point: Point2,
    pub distance_pixels: f64,
}
pub struct SnapResult {
    context: PlanContext,
    query: SnapQuery,
    candidate: Option<SnapCandidate>,
}
impl SnapResult {
    pub fn candidate(
        &self,
        context: PlanContext,
        query: SnapQuery,
    ) -> Result<Option<SnapCandidate>> {
        ensure(
            self.context == context && self.query == query,
            "snap result is stale for this view or navigation query",
        )?;
        Ok(self.candidate)
    }
}

pub struct SnapScene {
    context: PlanContext,
    segments: Vec<PreparedSegment>,
    grids: BTreeSet<Id>,
}

/// Immutable derived geometry, rebuilt with the revision-bound scene. In particular,
/// the local intersection pair loop must not normalize both lines for every pair.
struct PreparedSegment {
    segment: SnapSegment,
    length: f64,
    ux: f64,
    uy: f64,
}
impl SnapScene {
    pub fn new(context: PlanContext, segments: Vec<SnapSegment>) -> Result<Self> {
        ensure(
            segments.len() <= MAX_PLAN_ELEMENTS,
            "snap scene exceeds 10000 segments",
        )?;
        ensure(
            !context.session_id.0.is_nil() && !context.view_id.0.is_nil(),
            "invalid snap context identity",
        )?;
        context.basis.validate()?;
        context.range.validate()?;
        if let Some(crop) = context.crop {
            crop.validate()?;
        }
        ensure(
            context.scale_denominator.is_finite()
                && (0.001..=1_000_000.0).contains(&context.scale_denominator),
            "invalid snap context scale",
        )?;
        let mut keys = BTreeSet::new();
        for segment in &segments {
            let length = segment.start.distance(segment.end);
            ensure(
                !segment.entity.0.is_nil() && keys.insert((segment.entity, segment.feature)),
                "invalid or duplicate snap feature",
            )?;
            ensure(
                segment.start.is_finite()
                    && segment.end.is_finite()
                    && length.is_finite()
                    && length > 1e-9,
                "snap segment must have finite nonzero length",
            )?;
        }
        Ok(Self {
            context,
            segments: segments
                .into_iter()
                .map(|segment| {
                    let length = segment.start.distance(segment.end);
                    PreparedSegment {
                        ux: (segment.end.x - segment.start.x) / length,
                        uy: (segment.end.y - segment.start.y) / length,
                        segment,
                        length,
                    }
                })
                .collect(),
            grids: BTreeSet::new(),
        })
    }

    /// Host classification, not a provider-controlled arbitrary priority.
    pub fn with_grid_entities(mut self, grids: BTreeSet<Id>) -> Result<Self> {
        let entities: BTreeSet<_> = self.segments.iter().map(|s| s.segment.entity).collect();
        ensure(grids.is_subset(&entities), "grid snap identity missing")?;
        self.grids = grids;
        Ok(self)
    }

    pub fn query(&self, current: PlanContext, query: SnapQuery) -> Result<SnapResult> {
        ensure(
            self.context == current,
            "snap scene is stale for this document/view/settings",
        )?;
        ensure(
            query.pointer.is_finite()
                && query.radius_pixels.is_finite()
                && query.radius_pixels > 0.0
                && query.radius_pixels <= 64.0,
            "snap radius must be within (0,64] logical pixels",
        )?;
        let pointer = query.camera.unproject(query.pointer, query.viewport)?;
        ensure(
            query.perpendicular_from.is_none_or(|p| p.is_finite()),
            "invalid perpendicular anchor",
        )?;
        let mut best: Option<SnapCandidate> = None;
        let mut offer =
            |segment: &SnapSegment, other, kind, endpoint, point: Point2| -> Result<()> {
                ensure(point.is_finite(), "snap geometry overflow")?;
                if let Some(crop) = current.crop
                    && (point.x < crop.min.x
                        || point.x > crop.max.x
                        || point.y < crop.min.y
                        || point.y > crop.max.y)
                {
                    return Ok(());
                }
                let screen = query.camera.project(point, query.viewport)?;
                let distance = screen.distance(query.pointer);
                ensure(distance.is_finite(), "snap screen distance overflow")?;
                if distance > query.radius_pixels {
                    return Ok(());
                }
                let candidate = SnapCandidate {
                    entity: segment.entity,
                    feature: segment.feature,
                    other,
                    kind,
                    endpoint,
                    point,
                    distance_pixels: distance,
                };
                if best.is_none_or(|old| {
                    kind.cmp(&old.kind)
                        .then_with(|| distance.total_cmp(&old.distance_pixels))
                        .then_with(|| {
                            (
                                candidate.entity,
                                candidate.feature,
                                candidate.other,
                                endpoint,
                            )
                                .cmp(&(
                                    old.entity,
                                    old.feature,
                                    old.other,
                                    old.endpoint,
                                ))
                        })
                        .is_lt()
                }) {
                    best = Some(candidate);
                }
                Ok(())
            };
        let mut nearby = Vec::new();
        for prepared in &self.segments {
            let segment = &prepared.segment;
            if query.exclude_entity == Some(segment.entity) {
                continue;
            }
            let dx = segment.end.x - segment.start.x;
            let dy = segment.end.y - segment.start.y;
            let (length, ux, uy) = (prepared.length, prepared.ux, prepared.uy);
            if query.axis_extensions {
                let along = (pointer.x - segment.start.x) * ux + (pointer.y - segment.start.y) * uy;
                ensure(along.is_finite(), "axis extension projection overflow")?;
                if along < 0.0 || along > length {
                    let point =
                        Point2::new(segment.start.x + ux * along, segment.start.y + uy * along);
                    offer(segment, None, SnapKind::AxisExtension, 0, point)?;
                }
            }
            if let Some(anchor) = query.perpendicular_from {
                let along = (anchor.x - segment.start.x) * ux + (anchor.y - segment.start.y) * uy;
                ensure(along.is_finite(), "perpendicular projection overflow")?;
                // Do not clamp: a foot outside the finite feature is not supported.
                if (0.0..=length).contains(&along) {
                    let point =
                        Point2::new(segment.start.x + ux * along, segment.start.y + uy * along);
                    ensure(point.is_finite(), "perpendicular point overflow")?;
                    if point.distance(anchor) > 1e-9 {
                        offer(segment, None, SnapKind::Perpendicular, 0, point)?;
                    }
                }
            }
            if query.intersections {
                let along = (pointer.x - segment.start.x) * ux + (pointer.y - segment.start.y) * uy;
                ensure(along.is_finite(), "intersection acquisition overflow")?;
                let along = along.clamp(0.0, length);
                let closest =
                    Point2::new(segment.start.x + ux * along, segment.start.y + uy * along);
                if closest.distance(pointer) <= query.radius_pixels / query.camera.pixels_per_metre
                {
                    ensure(
                        nearby.len() < 256,
                        "intersection acquisition exceeds 256 nearby segments; zoom in",
                    )?;
                    nearby.push(prepared);
                }
            }
            if query.endpoints {
                offer(segment, None, SnapKind::Endpoint, 0, segment.start)?;
                offer(segment, None, SnapKind::Endpoint, 1, segment.end)?;
            }
            if query.midpoints {
                offer(
                    segment,
                    None,
                    SnapKind::Midpoint,
                    0,
                    Point2::new(segment.start.x + dx * 0.5, segment.start.y + dy * 0.5),
                )?;
            }
            if query.nearest {
                // Unit direction avoids squaring large coordinates/lengths.
                let distance =
                    (pointer.x - segment.start.x) * ux + (pointer.y - segment.start.y) * uy;
                ensure(distance.is_finite(), "snap nearest-point overflow")?;
                let mut lo: f64 = 0.0;
                let mut hi: f64 = length;
                if let Some(crop) = current.crop {
                    for (origin, direction, min, max) in [
                        (segment.start.x, ux, crop.min.x, crop.max.x),
                        (segment.start.y, uy, crop.min.y, crop.max.y),
                    ] {
                        if direction == 0.0 {
                            if origin < min || origin > max {
                                hi = -1.0;
                            }
                        } else {
                            let a = (min - origin) / direction;
                            let b = (max - origin) / direction;
                            lo = lo.max(a.min(b));
                            hi = hi.min(a.max(b));
                        }
                    }
                }
                if lo <= hi {
                    let distance = distance.clamp(lo, hi);
                    let mut point = Point2::new(
                        segment.start.x + ux * distance,
                        segment.start.y + uy * distance,
                    );
                    // Remove rounding overshoot at a clipped boundary, without inventing an endpoint.
                    if let Some(crop) = current.crop {
                        point.x = point.x.clamp(crop.min.x, crop.max.x);
                        point.y = point.y.clamp(crop.min.y, crop.max.y);
                    }
                    let kind = if self.grids.contains(&segment.entity) {
                        SnapKind::GridAxis
                    } else {
                        SnapKind::Nearest
                    };
                    offer(segment, None, kind, 0, point)?;
                }
            }
        }
        // Bounded local pair search; never truncate to a misleading partial result.
        nearby.sort_by_key(|s| (s.segment.entity, s.segment.feature));
        for (index, a) in nearby.iter().enumerate() {
            for b in &nearby[index + 1..] {
                if let Some(point) = intersection(a, b)? {
                    offer(
                        &a.segment,
                        Some((b.segment.entity, b.segment.feature)),
                        SnapKind::Intersection,
                        0,
                        point,
                    )?;
                }
            }
        }
        Ok(SnapResult {
            context: current,
            query,
            candidate: best,
        })
    }
}

/// Finite segment intersections only. Collinear overlaps have no unique point.
/// Unit-vector determinant avoids squared lengths and defines a 1e-12 angular
/// conditioning threshold; endpoints are never extended by acquisition tolerance.
fn intersection(a: &PreparedSegment, b: &PreparedSegment) -> Result<Option<Point2>> {
    let (al, ax, ay) = (a.length, a.ux, a.uy);
    let (bl, bx, by) = (b.length, b.ux, b.uy);
    let (a, b) = (&a.segment, &b.segment);
    let determinant = ax * by - ay * bx;
    if determinant.abs() <= 1e-12 {
        return Ok(None);
    }
    let dx = b.start.x - a.start.x;
    let dy = b.start.y - a.start.y;
    let t = (dx * by - dy * bx) / determinant;
    let u = (dx * ay - dy * ax) / determinant;
    ensure(
        t.is_finite() && u.is_finite(),
        "intersection geometry overflow",
    )?;
    if t < 0.0 || t > al || u < 0.0 || u > bl {
        return Ok(None);
    }
    let point = Point2::new(a.start.x + ax * t, a.start.y + ay * t);
    ensure(point.is_finite(), "intersection point overflow")?;
    Ok(Some(point))
}
