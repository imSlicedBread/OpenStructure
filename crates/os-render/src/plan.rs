//! Deterministic semantic polygon drawing and independent 2D navigation.
use os_core::{Id, Point2, Result, ensure};
use os_geometry::{
    Solid,
    plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, PlanRole, rectangular_plan},
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
mod angular;
mod columns;
pub use columns::PlanColumnItem;
mod detail_lines;
mod room_separation_lines;
pub use room_separation_lines::{
    PlanRoomSeparationLine, ROOM_SEPARATOR_WEIGHT_MM, clip_room_separation_line,
};
mod room_tags;
pub use detail_lines::{DETAIL_LINE_WEIGHT_MM, PlanDetailLine, clip_detail_line};
pub use room_tags::PlanRoomTag;
mod grids;
mod provider_lines;
pub use angular::PlanAngularDimension;
pub use grids::PlanGrid;
pub use provider_lines::PlanLine;

pub const MAX_PLAN_ELEMENTS: usize = 10_000;
/// Bound triangle-based fills so many maximally detailed floors cannot stall
/// one plan frame or exhaust the rendering worker.
pub const MAX_PLAN_FLOOR_TRIANGLES: usize = 50_000;

/// Compute polygon area after translating to the first vertex. This avoids
/// catastrophic cancellation from large absolute plan coordinates.
fn polygon_area(boundary: &[Point2]) -> f64 {
    let Some(origin) = boundary.first() else {
        return 0.0;
    };
    let twice_area = boundary
        .iter()
        .skip(1)
        .zip(boundary.iter().skip(2))
        .map(|(a, b)| {
            let ax = a.x - origin.x;
            let ay = a.y - origin.y;
            let bx = b.x - origin.x;
            let by = b.y - origin.y;
            ax * by - ay * bx
        })
        .sum::<f64>()
        .abs();
    twice_area * 0.5
}

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
    pub surface: os_geometry::SurfaceIdentity,
    pub entity: Id,
    pub footprint: PlanFootprint,
    /// Persisted butt-join seams affect strokes only, never fill/pick polygons.
    pub hidden_edges: BTreeSet<usize>,
    /// Exposed portions of edges split by a partial wall contact.
    pub split_edges: Vec<(Point2, Point2)>,
}

/// Paper-space stroke resolved by the owning model adapter. Kept independent
/// of model types so the renderer does not depend on `os-model`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanStroke {
    pub color: [u8; 3],
    pub weight_mm: f64,
    pub dashed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlanElementAppearance {
    pub cut: Option<PlanStroke>,
    pub projected: Option<PlanStroke>,
}
impl PlanElementAppearance {
    pub fn for_role(self, role: PlanRole) -> Option<PlanStroke> {
        match role {
            PlanRole::Cut => self.cut,
            PlanRole::Projected => self.projected,
            PlanRole::Depth => None,
        }
    }
}
impl PlanItem {
    pub fn outline(&self) -> impl Iterator<Item = (Point2, Point2)> + '_ {
        let vertices = self.footprint.vertices();
        (0..vertices.len())
            .filter(|i| !self.hidden_edges.contains(i))
            .map(|i| (vertices[i], vertices[(i + 1) % vertices.len()]))
            .chain(self.split_edges.iter().copied())
    }
}

/// Model-derived space graphics. The boundary is expressed in plan-plane metres;
/// it is never persisted and does not change the reported area when the view crops.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanRoomItem {
    pub entity: Id,
    pub number: String,
    pub name: String,
    pub seed: Point2,
    pub boundary: Vec<Point2>,
    pub area_m2: f64,
    /// Persisted topology identity. Geometry is derived from current boundaries.
    pub boundary_signature: Vec<(Id, bool)>,
    /// A missing/ambiguous enclosure remains visible as a diagnostic, not a stale polygon.
    pub diagnostic: Option<String>,
}

/// Unoccupied, currently enclosed face available to the room placement tool.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanRoomFace {
    pub boundary: Vec<Point2>,
    pub area_m2: f64,
    pub boundary_signature: Vec<(Id, bool)>,
}

/// Derived annotation geometry. The displayed measurement is recomputed from
/// the two current wall anchors; an orphan retains only its creation hint.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanDimensionItem {
    /// Additional measurement spans owned by this same semantic entity; never nested.
    pub spans: Vec<PlanDimensionItem>,
    pub shared_start_witness: bool,
    pub entity: Id,
    pub witness_start: Point2,
    pub witness_end: Point2,
    pub line_start: Point2,
    pub line_end: Point2,
    pub value_m: Option<f64>,
    pub orphan_hint: Point2,
    pub diagnostic: Option<String>,
}

impl PlanDimensionItem {
    pub fn graphics(&self) -> impl DoubleEndedIterator<Item = &Self> {
        std::iter::once(self).chain(self.spans.iter())
    }

    pub fn lines(&self) -> impl Iterator<Item = (Point2, Point2)> {
        [
            (!self.shared_start_witness).then_some((self.witness_start, self.line_start)),
            Some((self.witness_end, self.line_end)),
            Some((self.line_start, self.line_end)),
        ]
        .into_iter()
        .flatten()
    }

    /// Shared clipped label bounds in logical screen pixels, used for paint and pick.
    pub fn label_bounds(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
    ) -> Result<Option<[Point2; 2]>> {
        let (anchor, width, height) = if let Some(value) = self.value_m {
            let Some((start, end)) =
                crop_dimension_segment(context, self.line_start, self.line_end)?
            else {
                return Ok(None);
            };
            let center = camera.project(
                Point2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5),
                viewport,
            )?;
            let width = format!("{value:.3} m").len() as f64 * 8.0;
            (
                Point2::new(center.x - width * 0.5, center.y - 25.0),
                width,
                16.0,
            )
        } else {
            let center = camera.project(self.orphan_hint, viewport)?;
            (
                Point2::new(center.x + 9.0, center.y - 8.0),
                self.label().len() as f64 * 8.0,
                16.0,
            )
        };
        let bounds = [anchor, Point2::new(anchor.x + width, anchor.y + height)];
        if let Some(crop) = context.crop {
            for corner in [
                bounds[0],
                bounds[1],
                Point2::new(bounds[0].x, bounds[1].y),
                Point2::new(bounds[1].x, bounds[0].y),
            ] {
                if !point_in_crop(crop, camera.unproject(corner, viewport)?) {
                    return Ok(None);
                }
            }
        }
        Ok(Some(bounds))
    }

    pub fn label(&self) -> String {
        self.value_m.map_or_else(
            || match self.diagnostic.as_deref() {
                Some(reason) if reason.contains('(') => {
                    format!("Dimension lost reference: {reason}")
                }
                _ => "Dimension lost reference".into(),
            },
            |value| format!("{value:.3} m"),
        )
    }

    pub fn graphic_count(&self) -> usize {
        self.graphics()
            .map(|span| {
                if span.value_m.is_some() {
                    span.lines().count() + 3
                } else {
                    2
                }
            })
            .sum()
    }
}

/// A native horizontal floor outline and deterministic triangulation projected
/// into the plan plane. The authored boundary remains the picking/render edge;
/// triangles are fill-only so internal tessellation seams are never stroked.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanFloorItem {
    pub entity: Id,
    pub boundary: Vec<Point2>,
    pub triangles: Vec<[u32; 3]>,
    pub area_m2: f64,
}

impl PlanFloorItem {
    pub fn contains(&self, point: Point2) -> bool {
        if !point.is_finite() || self.boundary.len() < 3 {
            return false;
        }
        let mut inside = false;
        for (a, b) in self
            .boundary
            .iter()
            .zip(self.boundary.iter().cycle().skip(1))
            .take(self.boundary.len())
        {
            let cross = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
            if cross.abs() <= 1e-9
                && point.x >= a.x.min(b.x) - 1e-9
                && point.x <= a.x.max(b.x) + 1e-9
                && point.y >= a.y.min(b.y) - 1e-9
                && point.y <= a.y.max(b.y) + 1e-9
            {
                return true;
            }
            if (a.y > point.y) != (b.y > point.y)
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
            {
                inside = !inside;
            }
        }
        inside
    }
}

impl PlanRoomItem {
    pub fn contains(&self, point: Point2) -> bool {
        if !point.is_finite() || self.boundary.len() < 3 {
            return false;
        }
        let mut inside = false;
        for (a, b) in self
            .boundary
            .iter()
            .zip(self.boundary.iter().cycle().skip(1))
            .take(self.boundary.len())
        {
            let cross = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
            let scale = (b.x - a.x).abs().max((b.y - a.y).abs()).max(1.0);
            let on_segment = cross.abs() <= 1e-9 * scale
                && point.x >= a.x.min(b.x) - 1e-9
                && point.x <= a.x.max(b.x) + 1e-9
                && point.y >= a.y.min(b.y) - 1e-9
                && point.y <= a.y.max(b.y) + 1e-9;
            if on_segment {
                return true;
            }
            if (a.y > point.y) != (b.y > point.y)
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
            {
                inside = !inside;
            }
        }
        inside
    }
}

/// Bounded initial polygon IR. Drawing adapters can consume the same ordered list
/// as picking; screen/output adapters and curves/text/styles are later D/E work.
pub struct PlanDrawing {
    line_surfaces: BTreeMap<(Id, u32), os_geometry::SurfaceIdentity>,
    identity: Id,
    context: PlanContext,
    items: Vec<PlanItem>,
    unavailable: Vec<Id>,
    snaps: Option<crate::snapping::SnapScene>,
    source_ids: BTreeSet<Id>,
    grids: Vec<PlanGrid>,
    grid_segments: Vec<crate::snapping::SnapSegment>,
    lines: Vec<PlanLine>,
    rooms: Vec<PlanRoomItem>,
    room_tags: Vec<PlanRoomTag>,
    detail_lines: Vec<PlanDetailLine>,
    detail_segments: Vec<crate::snapping::SnapSegment>,
    detail_segment_count: usize,
    room_separation_lines: Vec<PlanRoomSeparationLine>,
    separator_segments: Vec<crate::snapping::SnapSegment>,
    separator_segment_count: usize,
    room_faces: Vec<PlanRoomFace>,
    dimensions: Vec<PlanDimensionItem>,
    angular_dimensions: Vec<PlanAngularDimension>,
    floors: Vec<PlanFloorItem>,
    columns: Vec<PlanColumnItem>,
    room_boundary_diagnostic: Option<String>,
    line_segments: Vec<crate::snapping::SnapSegment>,
    provider_segment_count: usize,
    grid_segment_count: usize,
    native_line_ids: BTreeSet<Id>,
    appearances: BTreeMap<Id, PlanElementAppearance>,
}
impl PlanDrawing {
    pub fn with_line_surfaces(
        mut self,
        surfaces: BTreeMap<(Id, u32), os_geometry::SurfaceIdentity>,
    ) -> Self {
        self.line_surfaces = surfaces;
        self
    }
    pub fn line_surface(&self, entity: Id, feature: u32) -> os_geometry::SurfaceIdentity {
        self.line_surfaces
            .get(&(entity, feature))
            .copied()
            .unwrap_or_default()
    }
    pub fn identity(&self) -> Id {
        self.identity
    }

    pub fn from_prisms(
        context: PlanContext,
        solids: &BTreeMap<Id, Solid>,
        unavailable: Vec<Id>,
    ) -> Result<Self> {
        Self::from_segmented_prisms(
            context,
            &solids
                .iter()
                .map(|(id, solid)| (*id, vec![solid.clone()]))
                .collect(),
            unavailable,
        )
    }

    /// Multiple convex cells may share one stable semantic wall identity.
    pub fn from_segmented_prisms(
        context: PlanContext,
        solids: &BTreeMap<Id, Vec<Solid>>,
        unavailable: Vec<Id>,
    ) -> Result<Self> {
        Self::from_layered_prisms(context, solids, &BTreeMap::new(), unavailable)
    }
    pub fn from_layered_prisms(
        context: PlanContext,
        solids: &BTreeMap<Id, Vec<Solid>>,
        surfaces: &BTreeMap<Id, Vec<os_geometry::SurfaceIdentity>>,
        unavailable: Vec<Id>,
    ) -> Result<Self> {
        let mut footprints = BTreeMap::new();
        for (id, cells) in solids {
            if let Some(tags) = surfaces.get(id) {
                ensure(
                    tags.len() == cells.len(),
                    "plan layer identity count mismatch",
                )?;
            }
            let mut parts = Vec::new();
            for (index, solid) in cells.iter().enumerate() {
                if let Some(footprint) =
                    rectangular_plan(solid, context.range, context.basis, context.crop)?
                {
                    parts.push((
                        surfaces.get(id).map(|tags| tags[index]).unwrap_or_default(),
                        footprint,
                    ));
                }
            }
            footprints.insert(*id, parts);
        }
        Self::from_layered_footprints(context, &footprints, unavailable)
    }

    /// Footprints already derived and clipped in this exact plan context.
    pub fn from_layered_footprints(
        context: PlanContext,
        solids: &BTreeMap<
            Id,
            Vec<(
                os_geometry::SurfaceIdentity,
                os_geometry::plan::PlanFootprint,
            )>,
        >,
        mut unavailable: Vec<Id>,
    ) -> Result<Self> {
        ensure(
            !context.session_id.0.is_nil() && !context.view_id.0.is_nil(),
            "invalid plan identity",
        )?;
        ensure(
            solids
                .values()
                .fold(unavailable.len(), |n, cells| n.saturating_add(cells.len()))
                <= MAX_PLAN_ELEMENTS,
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
        for (id, cells) in solids {
            ensure(!id.0.is_nil(), "invalid plan element identity")?;
            for (surface, footprint) in cells {
                items.push(PlanItem {
                    surface: *surface,
                    entity: *id,
                    footprint: footprint.clone(),
                    hidden_edges: BTreeSet::new(),
                    split_edges: Vec::new(),
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
            line_surfaces: BTreeMap::new(),
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
            rooms: Vec::new(),
            room_tags: Vec::new(),
            detail_lines: Vec::new(),
            detail_segments: Vec::new(),
            detail_segment_count: 0,
            room_separation_lines: Vec::new(),
            separator_segments: Vec::new(),
            separator_segment_count: 0,
            room_faces: Vec::new(),
            dimensions: Vec::new(),
            angular_dimensions: Vec::new(),
            identity: Id::new(),
            floors: Vec::new(),
            columns: Vec::new(),
            room_boundary_diagnostic: None,
            line_segments: Vec::new(),
            provider_segment_count: 0,
            grid_segment_count: 0,
            native_line_ids: BTreeSet::new(),
            appearances: BTreeMap::new(),
        })
    }
    /// Hide only the portion of an explicitly joined end face left after crop.
    pub fn without_wall_seams(
        mut self,
        seams: &BTreeMap<Id, Vec<(Point2, Point2)>>,
    ) -> Result<Self> {
        for item in &mut self.items {
            let Some(seams) = seams.get(&item.entity) else {
                continue;
            };
            let vertices = item.footprint.vertices();
            for (i, a) in vertices.iter().enumerate() {
                let b = vertices[(i + 1) % vertices.len()];
                let original = (*a, b);
                let mut edges = vec![original];
                for &(start, end) in seams {
                    let length = start.distance(end);
                    ensure(length.is_finite() && length > 1e-6, "invalid wall seam")?;
                    edges = edges
                        .into_iter()
                        .flat_map(|e| os_geometry::walls::subtract_segment(e, (start, end)))
                        .collect();
                }
                if edges != [original] {
                    item.hidden_edges.insert(i);
                    item.split_edges.extend(edges);
                }
            }
        }
        Ok(self)
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
        segments.extend_from_slice(&self.detail_segments);
        segments.extend_from_slice(&self.separator_segments);
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
    /// Attach already-resolved model styles only to native geometry in this
    /// drawing. This appearance map never participates in geometry or picking.
    pub fn with_appearances(
        mut self,
        appearances: BTreeMap<Id, PlanElementAppearance>,
    ) -> Result<Self> {
        ensure(
            appearances.len() <= MAX_PLAN_ELEMENTS,
            "plan appearance budget exceeded",
        )?;
        for (entity, appearance) in &appearances {
            let valid_stroke = |stroke: PlanStroke| {
                stroke.weight_mm.is_finite() && (0.05..=2.0).contains(&stroke.weight_mm)
            };
            ensure(
                appearance.cut.is_none_or(valid_stroke)
                    && appearance.projected.is_none_or(valid_stroke),
                "invalid plan appearance weight",
            )?;
            let cut_exists = self
                .items
                .iter()
                .any(|item| item.entity == *entity && item.footprint.role == PlanRole::Cut)
                || (self.native_line_ids.contains(entity)
                    && self
                        .lines
                        .iter()
                        .any(|line| line.entity == *entity && line.role == PlanRole::Cut));
            let projected_exists =
                self.items.iter().any(|item| {
                    item.entity == *entity && item.footprint.role == PlanRole::Projected
                }) || (self.native_line_ids.contains(entity)
                    && self
                        .lines
                        .iter()
                        .any(|line| line.entity == *entity && line.role == PlanRole::Projected))
                    || self.floors.iter().any(|floor| floor.entity == *entity);
            ensure(
                (appearance.cut.is_none() || cut_exists)
                    && (appearance.projected.is_none() || projected_exists)
                    && (appearance.cut.is_some() || appearance.projected.is_some()),
                "plan appearance references missing native geometry",
            )?;
        }
        self.appearances = appearances;
        Ok(self)
    }
    pub fn appearance(
        &self,
        current: PlanContext,
        entity: Id,
        role: PlanRole,
    ) -> Result<Option<PlanStroke>> {
        self.items(current)?;
        Ok(self
            .appearances
            .get(&entity)
            .copied()
            .and_then(|a| a.for_role(role)))
    }
    /// Attach deterministic model-derived room graphics after geometry derivation.
    /// Invalid/stale room polygons are rejected rather than drawn or picked.
    pub fn with_rooms(mut self, mut rooms: Vec<PlanRoomItem>) -> Result<Self> {
        ensure(
            self.items
                .len()
                .saturating_add(self.floors.len())
                .saturating_add(self.columns.len())
                .saturating_add(rooms.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        let mut ids = BTreeSet::new();
        for room in &rooms {
            ensure(
                !room.entity.0.is_nil()
                    && ids.insert(room.entity)
                    && !self.source_ids.contains(&room.entity)
                    && !self.grids.iter().any(|grid| grid.entity == room.entity)
                    && !self.lines.iter().any(|line| line.entity == room.entity),
                "invalid or duplicate room plan identity",
            )?;
            ensure(
                room.number.len() <= 256
                    && room.name.len() <= 256
                    && !room.number.chars().any(char::is_control)
                    && !room.name.chars().any(char::is_control),
                "invalid room plan label",
            )?;
            ensure(
                room.diagnostic.as_ref().is_none_or(|message| {
                    message.len() <= 512 && !message.chars().any(char::is_control)
                }),
                "invalid room plan diagnostic",
            )?;
            ensure(
                (3..=1024).contains(&room.boundary_signature.len())
                    && room.boundary_signature.iter().all(|(id, _)| !id.0.is_nil())
                    && room
                        .boundary_signature
                        .iter()
                        .map(|(id, _)| *id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == room.boundary_signature.len(),
                "invalid room boundary signature",
            )?;
            ensure(
                room.seed.is_finite()
                    && room.boundary.len() <= 4096
                    && room.boundary.iter().all(|p| p.is_finite())
                    && room.area_m2.is_finite()
                    && room.area_m2 >= 0.0,
                "invalid room plan geometry",
            )?;
            if room.boundary.len() >= 3 && room.diagnostic.is_none() {
                let boundary_area = polygon_area(&room.boundary);
                ensure(
                    boundary_area.is_finite()
                        && boundary_area > 0.0
                        && (boundary_area - room.area_m2).abs()
                            <= 1e-8_f64.max(boundary_area * 1e-8),
                    "room area does not match its boundary",
                )?;
            }
            ensure(
                (room.boundary.len() >= 3 && room.area_m2 > 0.0 && room.diagnostic.is_none())
                    || (room.boundary.is_empty()
                        && room.area_m2 == 0.0
                        && room.diagnostic.is_some()),
                "room plan needs either a boundary or an enclosure diagnostic",
            )?;
        }
        rooms.sort_by_key(|room| room.entity);
        self.rooms = rooms;
        Ok(self)
    }
    pub fn with_room_faces(mut self, mut faces: Vec<PlanRoomFace>) -> Result<Self> {
        ensure(
            self.items
                .len()
                .saturating_add(self.floors.len())
                .saturating_add(self.columns.len())
                .saturating_add(self.rooms.len())
                .saturating_add(faces.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        let mut signatures = BTreeSet::new();
        for face in &faces {
            let signature: Vec<_> = face.boundary_signature.clone();
            let area = polygon_area(&face.boundary);
            ensure(
                face.boundary.len() >= 3
                    && face.boundary.len() <= 4096
                    && face.boundary.iter().all(|p| p.is_finite())
                    && face.area_m2.is_finite()
                    && face.area_m2 > 0.0
                    && area.is_finite()
                    && area > 0.0
                    && (area - face.area_m2).abs() <= 1e-8_f64.max(area * 1e-8)
                    && (3..=1024).contains(&signature.len())
                    && signature.iter().all(|(id, _)| !id.0.is_nil())
                    && signature
                        .iter()
                        .map(|(id, _)| *id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == signature.len()
                    && signatures.insert(signature),
                "invalid or duplicate room face",
            )?;
        }
        faces.sort_by(|a, b| a.boundary_signature.cmp(&b.boundary_signature));
        self.room_faces = faces;
        Ok(self)
    }
    pub fn with_dimensions(mut self, mut dimensions: Vec<PlanDimensionItem>) -> Result<Self> {
        ensure(
            self.items
                .len()
                .saturating_add(self.floors.len())
                .saturating_add(self.columns.len())
                .saturating_add(self.rooms.len())
                .saturating_add(self.room_faces.len())
                .saturating_add(self.grids.len())
                .saturating_add(self.lines.len())
                .saturating_add(
                    dimensions
                        .iter()
                        .map(PlanDimensionItem::graphic_count)
                        .sum::<usize>(),
                )
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        let mut ids = BTreeSet::new();
        for dimension in &dimensions {
            ensure(
                !dimension.entity.0.is_nil()
                    && ids.insert(dimension.entity)
                    && !self.source_ids.contains(&dimension.entity)
                    && !self
                        .rooms
                        .iter()
                        .any(|room| room.entity == dimension.entity)
                    && !self
                        .grids
                        .iter()
                        .any(|grid| grid.entity == dimension.entity)
                    && !self
                        .lines
                        .iter()
                        .any(|line| line.entity == dimension.entity),
                "invalid or duplicate dimension identity",
            )?;
            ensure(
                dimension.spans.len() <= 126
                    && (dimension.spans.is_empty() || dimension.value_m.is_some())
                    && dimension.spans.iter().all(|span| {
                        span.spans.is_empty()
                            && span.entity == dimension.entity
                            && span.value_m.is_some()
                            && span.diagnostic.is_none()
                    }),
                "invalid dimension spans",
            )?;
            for dimension in dimension.graphics() {
                ensure(
                    dimension.witness_start.is_finite()
                        && dimension.witness_end.is_finite()
                        && dimension.line_start.is_finite()
                        && dimension.line_end.is_finite()
                        && dimension.orphan_hint.is_finite()
                        && dimension.diagnostic.as_ref().is_none_or(|message| {
                            message.len() <= 512 && !message.chars().any(char::is_control)
                        }),
                    "invalid dimension graphics",
                )?;
                match (dimension.value_m, dimension.diagnostic.as_ref()) {
                    (Some(value), None) => {
                        let measured = dimension.witness_start.distance(dimension.witness_end);
                        ensure(
                            value.is_finite()
                                && value > 1e-6
                                && measured.is_finite()
                                && measured > 1e-6
                                && (measured - value).abs() <= 1e-8_f64.max(measured * 1e-8)
                                && dimension
                                    .line_start
                                    .distance(dimension.line_end)
                                    .is_finite()
                                && dimension.line_start.distance(dimension.line_end) > 1e-6,
                            "dimension value does not match its anchors",
                        )?;
                    }
                    (None, Some(_)) => {}
                    _ => return Err(os_core::Error::Invalid("invalid dimension state".into())),
                }
            }
        }
        dimensions.sort_by_key(|dimension| dimension.entity);
        self.dimensions = dimensions;
        Ok(self)
    }
    pub fn with_floors(mut self, mut floors: Vec<PlanFloorItem>) -> Result<Self> {
        ensure(
            self.items
                .len()
                .saturating_add(self.rooms.len())
                .saturating_add(self.room_faces.len())
                .saturating_add(self.grids.len())
                .saturating_add(self.lines.len())
                .saturating_add(
                    self.dimensions
                        .iter()
                        .map(PlanDimensionItem::graphic_count)
                        .sum::<usize>(),
                )
                .saturating_add(floors.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        ensure(
            self.source_ids.len().saturating_add(floors.len()) <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 source elements",
        )?;
        ensure(
            floors.iter().fold(0usize, |total, floor| {
                total.saturating_add(floor.triangles.len())
            }) <= MAX_PLAN_FLOOR_TRIANGLES,
            "floor plan fills exceed the 50000 triangle limit",
        )?;
        let mut ids = BTreeSet::new();
        for floor in &floors {
            ensure(
                !floor.entity.0.is_nil()
                    && ids.insert(floor.entity)
                    && !self.source_ids.contains(&floor.entity)
                    && !self.rooms.iter().any(|room| room.entity == floor.entity)
                    && !self.grids.iter().any(|grid| grid.entity == floor.entity)
                    && !self.lines.iter().any(|line| line.entity == floor.entity)
                    && !self
                        .dimensions
                        .iter()
                        .any(|dimension| dimension.entity == floor.entity),
                "invalid or duplicate floor plan identity",
            )?;
            ensure(
                (3..=256).contains(&floor.boundary.len())
                    && floor.boundary.iter().all(|point| point.is_finite())
                    && floor.area_m2.is_finite()
                    && floor.area_m2 > 1e-8
                    && floor.triangles.len() == floor.boundary.len() - 2
                    && floor
                        .triangles
                        .iter()
                        .flatten()
                        .all(|index| (*index as usize) < floor.boundary.len())
                    && (polygon_area(&floor.boundary) - floor.area_m2).abs()
                        <= 1e-8_f64.max(floor.area_m2 * 1e-8),
                "invalid floor plan geometry",
            )?;
            ensure(
                os_geometry::floors::triangulate_floor(&floor.boundary)? == floor.triangles,
                "floor plan triangulation does not match its boundary",
            )?;
        }
        floors.sort_by_key(|floor| floor.entity);
        self.source_ids
            .extend(floors.iter().map(|floor| floor.entity));
        self.floors = floors;
        Ok(self)
    }
    pub fn rooms(&self, current: PlanContext) -> Result<&[PlanRoomItem]> {
        self.items(current)?;
        Ok(&self.rooms)
    }
    pub fn room_faces(&self, current: PlanContext) -> Result<&[PlanRoomFace]> {
        self.items(current)?;
        Ok(&self.room_faces)
    }
    pub fn dimensions(&self, current: PlanContext) -> Result<&[PlanDimensionItem]> {
        self.items(current)?;
        Ok(&self.dimensions)
    }

    pub fn angular_dimensions(&self, current: PlanContext) -> Result<&[PlanAngularDimension]> {
        self.items(current)?;
        Ok(&self.angular_dimensions)
    }

    pub fn with_angular_dimensions(mut self, mut items: Vec<PlanAngularDimension>) -> Result<Self> {
        let mut ids = BTreeSet::new();
        ensure(
            items.len().saturating_mul(67)
                + self
                    .dimensions
                    .iter()
                    .map(PlanDimensionItem::graphic_count)
                    .sum::<usize>()
                + self.items.len()
                + self.lines.len()
                + self.detail_segment_count
                + self.separator_segment_count
                + self.grids.len()
                + self.floors.len()
                + self.columns.len()
                + self.rooms.len()
                <= MAX_PLAN_ELEMENTS,
            "angular graphics exceed plan budget",
        )?;
        for item in &items {
            item.validate()?;
            ensure(
                ids.insert(item.entity)
                    && !self.source_ids.contains(&item.entity)
                    && !self.dimensions.iter().any(|d| d.entity == item.entity)
                    && !self.rooms.iter().any(|d| d.entity == item.entity)
                    && !self.grids.iter().any(|d| d.entity == item.entity)
                    && !self.lines.iter().any(|d| d.entity == item.entity)
                    && !self.floors.iter().any(|d| d.entity == item.entity),
                "duplicate angular dimension identity",
            )?;
        }
        items.sort_by_key(|item| item.entity);
        self.angular_dimensions = items;
        Ok(self)
    }
    pub fn floors(&self, current: PlanContext) -> Result<&[PlanFloorItem]> {
        self.items(current)?;
        Ok(&self.floors)
    }
    pub fn with_room_boundary_diagnostic(mut self, diagnostic: Option<String>) -> Result<Self> {
        ensure(
            diagnostic.as_ref().is_none_or(|message| {
                message.len() <= 512 && !message.chars().any(char::is_control)
            }),
            "invalid room boundary diagnostic",
        )?;
        self.room_boundary_diagnostic = diagnostic;
        Ok(self)
    }
    pub fn room_boundary_diagnostic(&self, current: PlanContext) -> Result<Option<&str>> {
        self.items(current)?;
        Ok(self.room_boundary_diagnostic.as_deref())
    }
    /// Return only a strict interior hit. Clicks on wall/shared boundaries do
    /// not place rooms; overlapping candidate faces are an explicit ambiguity.
    pub fn room_face_at(
        &self,
        current: PlanContext,
        point: Point2,
    ) -> Result<Option<&PlanRoomFace>> {
        ensure(point.is_finite(), "invalid room placement point")?;
        let mut candidates = self
            .room_faces(current)?
            .iter()
            .filter(|face| strictly_contains(&face.boundary, point));
        let Some(face) = candidates.next() else {
            return Ok(None);
        };
        ensure(
            candidates.next().is_none(),
            "room placement point is inside overlapping boundaries",
        )?;
        Ok(Some(face))
    }
    pub fn pick_room(&self, current: PlanContext, point: Point2) -> Result<Option<Id>> {
        ensure(point.is_finite(), "invalid room pick point")?;
        Ok(self
            .rooms(current)?
            .iter()
            .rev()
            .find(|room| room.contains(point))
            .map(|room| room.entity))
    }
    pub fn pick_floor(&self, current: PlanContext, point: Point2) -> Result<Option<Id>> {
        ensure(point.is_finite(), "invalid floor pick point")?;
        Ok(self
            .floors(current)?
            .iter()
            .rev()
            .find(|floor| {
                current.crop.is_none_or(|crop| point_in_crop(crop, point)) && floor.contains(point)
            })
            .map(|floor| floor.entity))
    }
    pub fn pick_dimension_screen(
        &self,
        current: PlanContext,
        camera: PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
        radius_pixels: f64,
    ) -> Result<Option<Id>> {
        ensure(
            pointer.is_finite()
                && radius_pixels.is_finite()
                && radius_pixels > 0.0
                && radius_pixels <= 64.0,
            "invalid dimension pick query",
        )?;
        let dimensions = self.dimensions(current)?;
        if let Some(crop) = current.crop
            && !point_in_crop(crop, camera.unproject(pointer, viewport)?)
        {
            return Ok(None);
        }
        for dimension in self.angular_dimensions.iter().rev() {
            if dimension.hit(current, camera, viewport, pointer, radius_pixels)? {
                return Ok(Some(dimension.entity));
            }
        }
        for dimension in dimensions
            .iter()
            .rev()
            .flat_map(|item| item.graphics().rev())
        {
            let hit = if dimension.value_m.is_some() {
                let mut near_line = false;
                for (start, end) in dimension.lines() {
                    if let Some((start, end)) = crop_dimension_segment(current, start, end)? {
                        let a = camera.project(start, viewport)?;
                        let b = camera.project(end, viewport)?;
                        near_line |= point_segment_distance(pointer, a, b) <= radius_pixels;
                    }
                }
                let center_hit = if let Some((a, b)) =
                    crop_dimension_segment(current, dimension.line_start, dimension.line_end)?
                {
                    pointer.distance(
                        camera
                            .project(Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5), viewport)?,
                    ) <= radius_pixels + 16.0
                } else {
                    false
                };
                near_line
                    || center_hit
                    || dimension
                        .label_bounds(current, camera, viewport)?
                        .is_some_and(|[min, max]| {
                            pointer.x >= min.x - radius_pixels
                                && pointer.x <= max.x + radius_pixels
                                && pointer.y >= min.y - radius_pixels
                                && pointer.y <= max.y + radius_pixels
                        })
            } else {
                current
                    .crop
                    .is_none_or(|crop| point_in_crop(crop, dimension.orphan_hint))
                    && (pointer.distance(camera.project(dimension.orphan_hint, viewport)?)
                        <= radius_pixels + 8.0
                        || dimension
                            .label_bounds(current, camera, viewport)?
                            .is_some_and(|[min, max]| {
                                pointer.x >= min.x
                                    && pointer.x <= max.x
                                    && pointer.y >= min.y
                                    && pointer.y <= max.y
                            }))
            };
            if hit {
                return Ok(Some(dimension.entity));
            }
        }
        Ok(None)
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

fn point_in_crop(crop: PlanCrop, point: Point2) -> bool {
    point.x >= crop.min.x && point.x <= crop.max.x && point.y >= crop.min.y && point.y <= crop.max.y
}

fn crop_dimension_segment(
    context: PlanContext,
    start: Point2,
    end: Point2,
) -> Result<Option<(Point2, Point2)>> {
    if start.distance(end) <= 1e-9 {
        return Ok(context
            .crop
            .is_none_or(|crop| point_in_crop(crop, start))
            .then_some((start, end)));
    }
    grids::clip(start, end, context.crop)
}

fn point_segment_distance(point: Point2, start: Point2, end: Point2) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return point.distance(start);
    }
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    point.distance(Point2::new(start.x + t * dx, start.y + t * dy))
}

fn strictly_contains(boundary: &[Point2], point: Point2) -> bool {
    let mut inside = false;
    for (a, b) in boundary
        .iter()
        .zip(boundary.iter().cycle().skip(1))
        .take(boundary.len())
    {
        let cross = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
        let scale = (b.x - a.x).abs().max((b.y - a.y).abs()).max(1.0);
        if cross.abs() <= 1e-9 * scale
            && point.x >= a.x.min(b.x) - 1e-9
            && point.x <= a.x.max(b.x) + 1e-9
            && point.y >= a.y.min(b.y) - 1e-9
            && point.y <= a.y.max(b.y) + 1e-9
        {
            return false;
        }
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
    }
    inside
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
