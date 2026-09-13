//! Deterministic semantic polygon drawing and independent 2D navigation.
use os_core::{Id, Point2, Result, ensure};
use os_geometry::{
    Solid,
    plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, PlanRole, rectangular_plan},
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
mod grids;
mod provider_lines;
pub use grids::PlanGrid;
pub use provider_lines::PlanLine;

pub const MAX_PLAN_ELEMENTS: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanContext {
    pub session_id: Id,
    pub model_revision: u64,
    pub view_id: Id,
    pub settings_revision: u64,
    pub basis: HorizontalBasis,
    /// Absolute heights, not level-relative offsets.
    pub range: PlanRange,
    pub crop: Option<PlanCrop>,
    pub scale_denominator: f64,
    pub show_walls: bool,
    pub show_extensions: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem {
    pub entity: Id,
    pub footprint: PlanFootprint,
}

/// Bounded initial polygon IR. Drawing adapters can consume the same ordered list
/// as picking; screen/output adapters and curves/text/styles are later D/E work.
pub struct PlanDrawing {
    context: PlanContext,
    items: Vec<PlanItem>,
    unavailable: Vec<Id>,
    snaps: Option<crate::snapping::SnapScene>,
    source_ids: BTreeSet<Id>,
    grids: Vec<PlanGrid>,
    grid_segments: Vec<crate::snapping::SnapSegment>,
    lines: Vec<PlanLine>,
    line_segments: Vec<crate::snapping::SnapSegment>,
    provider_segment_count: usize,
    grid_segment_count: usize,
}
impl PlanDrawing {
    pub fn from_prisms(
        context: PlanContext,
        solids: &BTreeMap<Id, Solid>,
        mut unavailable: Vec<Id>,
    ) -> Result<Self> {
        ensure(
            !context.session_id.0.is_nil() && !context.view_id.0.is_nil(),
            "invalid plan identity",
        )?;
        ensure(
            solids.len().saturating_add(unavailable.len()) <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        context.basis.validate()?;
        context.range.validate()?;
        ensure(
            context.scale_denominator.is_finite()
                && (0.001..=1_000_000.0).contains(&context.scale_denominator),
            "invalid plan scale",
        )?;
        if let Some(crop) = context.crop {
            crop.validate()?;
        }
        unavailable.sort();
        ensure(
            unavailable
                .iter()
                .all(|id| !id.0.is_nil() && !solids.contains_key(id))
                && unavailable.windows(2).all(|ids| ids[0] != ids[1]),
            "invalid unavailable plan identities",
        )?;
        let mut items = Vec::new();
        for (id, solid) in solids {
            ensure(!id.0.is_nil(), "invalid plan element identity")?;
            if let Some(footprint) =
                rectangular_plan(solid, context.range, context.basis, context.crop)?
            {
                items.push(PlanItem {
                    entity: *id,
                    footprint,
                });
            }
        }
        // Cut polygons draw above projected/depth polygons. UUID order breaks
        // ties deterministically; picking checks the exact reverse draw order.
        items.sort_by_key(|item| {
            (
                match item.footprint.role {
                    PlanRole::Depth => 0,
                    PlanRole::Projected => 1,
                    PlanRole::Cut => 2,
                },
                item.entity,
            )
        });
        Ok(Self {
            source_ids: solids
                .keys()
                .copied()
                .chain(unavailable.iter().copied())
                .collect(),
            context,
            items,
            unavailable,
            snaps: None,
            grids: Vec::new(),
            grid_segments: Vec::new(),
            lines: Vec::new(),
            line_segments: Vec::new(),
            provider_segment_count: 0,
            grid_segment_count: 0,
        })
    }
    /// Attach semantic axes only for elements present in this checked visible drawing.
    pub fn with_snap_segments(
        mut self,
        mut segments: Vec<crate::snapping::SnapSegment>,
    ) -> Result<Self> {
        let visible: std::collections::BTreeSet<_> =
            self.items.iter().map(|item| item.entity).collect();
        ensure(
            segments.iter().all(|s| visible.contains(&s.entity)),
            "snap feature references an invisible or unavailable element",
        )?;
        segments.extend_from_slice(&self.grid_segments);
        segments.extend_from_slice(&self.line_segments);
        self.snaps = Some(
            crate::snapping::SnapScene::new(self.context, segments)?
                .with_grid_entities(self.grids.iter().map(|g| g.entity).collect())?,
        );
        Ok(self)
    }
    pub fn snap(
        &self,
        current: PlanContext,
        query: crate::snapping::SnapQuery,
    ) -> Result<crate::snapping::SnapResult> {
        self.items(current)?;
        self.snaps
            .as_ref()
            .ok_or_else(|| {
                os_core::Error::Unsupported("drawing has no semantic snap provider".into())
            })?
            .query(current, query)
    }
    pub fn items(&self, current: PlanContext) -> Result<&[PlanItem]> {
        ensure(
            self.context == current,
            "plan drawing is stale for this document/view/settings",
        )?;
        Ok(&self.items)
    }
    pub fn unavailable(&self, current: PlanContext) -> Result<&[Id]> {
        self.items(current)?;
        Ok(&self.unavailable)
    }
    pub fn pick(&self, current: PlanContext, point: Point2) -> Result<Option<Id>> {
        ensure(point.is_finite(), "invalid plan pick point")?;
        Ok(self
            .items(current)?
            .iter()
            .rev()
            .find(|item| item.footprint.contains(point))
            .map(|item| item.entity))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanCamera {
    pub center: Point2,
    /// Logical screen pixels per model metre, independent of paper scale/DPI.
    pub pixels_per_metre: f64,
}
impl Default for PlanCamera {
    fn default() -> Self {
        Self {
            center: Point2::default(),
            pixels_per_metre: 65.0,
        }
    }
}
impl PlanCamera {
    fn validate(self, size: [f64; 2]) -> Result<()> {
        ensure(
            self.center.is_finite()
                && self.pixels_per_metre.is_finite()
                && (0.001..=10_000.0).contains(&self.pixels_per_metre)
                && size.iter().all(|n| n.is_finite() && *n > 0.0),
            "invalid plan camera or viewport",
        )
    }
    pub fn project(self, point: Point2, size: [f64; 2]) -> Result<Point2> {
        self.validate(size)?;
        let result = Point2::new(
            size[0] * 0.5 + (point.x - self.center.x) * self.pixels_per_metre,
            size[1] * 0.5 - (point.y - self.center.y) * self.pixels_per_metre,
        );
        ensure(result.is_finite(), "plan screen projection overflow")?;
        Ok(result)
    }
    pub fn unproject(self, point: Point2, size: [f64; 2]) -> Result<Point2> {
        self.validate(size)?;
        let result = Point2::new(
            self.center.x + (point.x - size[0] * 0.5) / self.pixels_per_metre,
            self.center.y - (point.y - size[1] * 0.5) / self.pixels_per_metre,
        );
        ensure(result.is_finite(), "plan work-plane projection overflow")?;
        Ok(result)
    }
    pub fn pan(&mut self, delta: Point2, size: [f64; 2]) -> Result<()> {
        self.validate(size)?;
        let candidate = Self {
            center: Point2::new(
                self.center.x - delta.x / self.pixels_per_metre,
                self.center.y + delta.y / self.pixels_per_metre,
            ),
            ..*self
        };
        candidate.validate(size)?;
        *self = candidate;
        Ok(())
    }
    /// Cursor-anchored zoom. Failed navigation leaves the camera unchanged.
    pub fn zoom_at(&mut self, cursor: Point2, factor: f64, size: [f64; 2]) -> Result<()> {
        ensure(
            factor.is_finite() && factor > 0.0,
            "invalid plan zoom factor",
        )?;
        let anchor = self.unproject(cursor, size)?;
        let mut candidate = Self {
            pixels_per_metre: (self.pixels_per_metre * factor).clamp(0.001, 10_000.0),
            ..*self
        };
        let shifted = candidate.unproject(cursor, size)?;
        candidate.center.x += anchor.x - shifted.x;
        candidate.center.y += anchor.y - shifted.y;
        candidate.validate(size)?;
        *self = candidate;
        Ok(())
    }
}
