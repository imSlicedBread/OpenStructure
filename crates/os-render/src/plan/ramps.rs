//! Continuous straight ramp slabs in plan; arrows are graphics, never snap axes.
use super::*;
use crate::snapping::SnapSegment;
use os_model::RampParams;

const MAX_COORDINATE: f64 = 1e6;
const TOLERANCE: f64 = os_geometry::plan::PLAN_TOLERANCE;

#[derive(Clone, Debug, PartialEq)]
struct Projection {
    start: Point2,
    end: Point2,
    width: f64,
    thickness: f64,
    lower: f64,
    upper: f64,
    context: PlanContext,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanRampItem {
    pub entity: Id,
    pub material: Option<Id>,
    /// Full uncropped plan rectangle; absent for section-only items.
    pub footprint_corners: Option<[Point2; 4]>,
    pub footprints: Vec<PlanFootprint>,
    /// Visible area outlines, including range/crop boundaries, for painting only.
    pub visible_boundary_lines: Vec<PlanLine>,
    pub ascent_direction: Option<Point2>,
    /// Cropped ascent arrow, excluded from semantic snapping.
    pub arrow: Vec<PlanLine>,
    /// Plan arrows or section cuts, consumed by canvas and paper rendering.
    pub lines: Vec<PlanLine>,
    source: Option<Projection>,
    semantic_edges: Vec<PlanLine>,
}

impl PlanRampItem {
    pub fn contains(&self, point: Point2) -> bool {
        self.footprints
            .iter()
            .any(|footprint| footprint.contains(point))
    }

    pub fn visible_strokes(&self) -> impl Iterator<Item = &PlanLine> {
        self.visible_boundary_lines.iter().chain(self.lines.iter())
    }

    /// Classification wrapper; ownership, feature IDs and coordinates are
    /// checked by `PlanDrawing::with_ramps`, which also clips to its context.
    pub fn from_section_lines(entity: Id, material: Option<Id>, lines: Vec<PlanLine>) -> Self {
        Self {
            entity,
            material,
            footprint_corners: None,
            footprints: Vec::new(),
            visible_boundary_lines: Vec::new(),
            ascent_direction: None,
            arrow: Vec::new(),
            lines,
            source: None,
            semantic_edges: Vec::new(),
        }
    }

    /// `lower_z` and `upper_z` are absolute walk-surface elevations.
    /// Levels/material references are resolved by the caller. Invalid inputs
    /// error; fully range/crop-hidden slabs return `None`.
    pub fn from_plan_data(
        entity: Id,
        params: &RampParams,
        lower_z: f64,
        upper_z: f64,
        context: PlanContext,
    ) -> Result<Option<Self>> {
        params.dimensions(lower_z, upper_z)?;
        let item = Self::derive(
            entity,
            params.start,
            params.end,
            params.width,
            params.structural_thickness,
            params.material,
            lower_z,
            upper_z,
            context,
        )?;
        Ok((!item.footprints.is_empty()).then_some(item))
    }

    /// Scalar projection API matching native straight-slab semantics. Hidden
    /// geometry retains its identity with empty visible areas and strokes.
    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        entity: Id,
        start: Point2,
        end: Point2,
        width: f64,
        structural_thickness: f64,
        material: Option<Id>,
        lower_z: f64,
        upper_z: f64,
        context: PlanContext,
    ) -> Result<Self> {
        let run = start.distance(end);
        let rise = upper_z - lower_z;
        ensure(
            !entity.0.is_nil()
                && material.is_none_or(|id| !id.0.is_nil())
                && [start, end].iter().all(|p| bounded_point(*p))
                && [width, structural_thickness, run, rise]
                    .iter()
                    .all(|v| v.is_finite() && *v > 1e-6 && *v <= MAX_COORDINATE)
                && [
                    lower_z,
                    upper_z,
                    lower_z - structural_thickness,
                    upper_z - structural_thickness,
                ]
                .iter()
                .all(|z| z.is_finite() && z.abs() <= MAX_COORDINATE),
            "invalid ramp plan geometry",
        )?;
        context.basis.validate()?;
        context.range.validate()?;
        if let Some(crop) = context.crop {
            crop.validate()?;
        }
        let direction = Point2::new((end.x - start.x) / run, (end.y - start.y) / run);
        let at = |station: f64, lateral: f64| {
            let world = Point2::new(
                start.x + direction.x * station - direction.y * lateral,
                start.y + direction.y * station + direction.x * lateral,
            );
            ensure(
                bounded_point(world),
                "ramp footprint exceeds coordinate bounds",
            )?;
            context.basis.world_to_plane(world)
        };
        let corners = |near: f64, far: f64| -> Result<Vec<Point2>> {
            let half = width * 0.5;
            [(near, half), (near, -half), (far, -half), (far, half)]
                .into_iter()
                .map(|(s, l)| at(s, l))
                .collect()
        };
        let footprint_corners = corners(0.0, run)?
            .try_into()
            .map_err(|_| os_core::Error::Invalid("invalid ramp corners".into()))?;
        let plane_start = at(0.0, 0.0)?;
        let plane_end = at(run, 0.0)?;
        let ascent_direction = Point2::new(
            (plane_end.x - plane_start.x) / run,
            (plane_end.y - plane_start.y) / run,
        );
        // A station is visible when its top is above depth and its underside
        // below top. Split continuous slope at bottom, cut and cut+thickness.
        let station = |z: f64| ((z - lower_z) / rise * run).clamp(0.0, run);
        let range = context.range;
        let near = station(range.depth);
        let far = station(range.top + structural_thickness);
        let mut footprints = Vec::new();
        let mut visible_extent: Option<(f64, f64)> = None;
        let bands = [
            (near, station(range.bottom), PlanRole::Depth),
            (
                station(range.bottom),
                station(range.cut),
                PlanRole::Projected,
            ),
            (
                station(range.cut),
                station(range.cut + structural_thickness),
                PlanRole::Cut,
            ),
            (
                station(range.cut + structural_thickness),
                far,
                PlanRole::Projected,
            ),
        ];
        for (a, b, role) in bands {
            let a = a.max(near);
            let b = b.min(far);
            if b - a <= os_geometry::plan::PLAN_TOLERANCE {
                continue;
            }
            if let Some(footprint) = PlanFootprint::from_convex(role, corners(a, b)?, context.crop)?
            {
                for point in footprint.vertices() {
                    let world = context.basis.plane_to_world(*point)?;
                    let along =
                        (world.x - start.x) * direction.x + (world.y - start.y) * direction.y;
                    visible_extent = Some(match visible_extent {
                        Some((min, max)) => (min.min(along), max.max(along)),
                        None => (along, along),
                    });
                }
                footprints.push(footprint);
            }
        }
        let mut semantic_edges = Vec::new();
        if !footprints.is_empty() {
            let mut add = |feature, a, b| -> Result<()> {
                if grids::clip(a, b, context.crop)?.is_some() {
                    semantic_edges.push(PlanLine {
                        entity,
                        feature,
                        start: a,
                        end: b,
                        role: PlanRole::Projected,
                    });
                }
                Ok(())
            };
            let half = width * 0.5;
            if near == 0.0 {
                add(0, at(0.0, half)?, at(0.0, -half)?)?;
            }
            add(1, at(near, -half)?, at(far, -half)?)?;
            if far == run {
                add(2, at(run, -half)?, at(run, half)?)?;
            }
            add(3, at(far, half)?, at(near, half)?)?;
        }
        let mut visible_boundary_lines = Vec::new();
        for footprint in &footprints {
            let vertices = footprint.vertices();
            for (start, end) in vertices
                .iter()
                .copied()
                .zip(vertices.iter().copied().cycle().skip(1))
                .take(vertices.len())
            {
                visible_boundary_lines.push(PlanLine {
                    entity,
                    feature: visible_boundary_lines.len() as u32,
                    start,
                    end,
                    role: footprint.role,
                });
            }
        }
        let mut arrow = Vec::new();
        if let Some((a, b)) = visible_extent {
            let span = b - a;
            let start = a + span * 0.25;
            let end = a + span * 0.75;
            let head = (span * 0.1).min(width * 0.25);
            for (p, q) in [
                (at(start, 0.0)?, at(end, 0.0)?),
                (at(end, 0.0)?, at(end - head, head)?),
                (at(end, 0.0)?, at(end - head, -head)?),
            ] {
                if let Some((start, end)) = grids::clip(p, q, context.crop)? {
                    arrow.push(PlanLine {
                        entity,
                        feature: (visible_boundary_lines.len() + arrow.len()) as u32,
                        start,
                        end,
                        role: PlanRole::Projected,
                    });
                }
            }
        }
        Ok(Self {
            entity,
            material,
            footprint_corners: Some(footprint_corners),
            footprints,
            visible_boundary_lines,
            ascent_direction: Some(ascent_direction),
            lines: arrow.clone(),
            arrow,
            source: Some(Projection {
                start,
                end,
                width,
                thickness: structural_thickness,
                lower: lower_z,
                upper: upper_z,
                context,
            }),
            semantic_edges,
        })
    }
}

fn bounded_point(p: Point2) -> bool {
    p.is_finite() && p.x.abs() <= MAX_COORDINATE && p.y.abs() <= MAX_COORDINATE
}

impl PlanDrawing {
    /// Attach before snap features. Footprints supply fills/interior picking;
    /// arrows reach all line consumers but never enter the snap scene.
    pub fn with_ramps(mut self, mut ramps: Vec<PlanRampItem>) -> Result<Self> {
        ensure(self.snaps.is_none(), "attach ramps before snap features")?;
        ensure(self.ramps.is_empty(), "ramps already attached")?;
        let footprint_count = ramps
            .iter()
            .fold(0usize, |n, r| n.saturating_add(r.footprints.len()));
        let line_count = ramps.iter().fold(0usize, |n, r| {
            n.saturating_add(r.lines.len())
                .saturating_add(r.semantic_edges.len())
        });
        ensure(
            self.source_ids.len().saturating_add(ramps.len()) <= MAX_PLAN_ELEMENTS
                && self.items.len().saturating_add(footprint_count) <= MAX_PLAN_ELEMENTS
                && self
                    .provider_segment_count
                    .saturating_add(line_count)
                    .saturating_add(self.grid_segment_count)
                    .saturating_add(self.detail_segment_count)
                    .saturating_add(self.separator_segment_count)
                    <= MAX_PLAN_ELEMENTS,
            "ramp plan exceeds drawing budget",
        )?;
        ramps.sort_by_key(|r| r.entity);
        for ramp in &ramps {
            ensure(
                !ramp.entity.0.is_nil()
                    && self.source_ids.insert(ramp.entity)
                    && ramp.material.is_none_or(|id| !id.0.is_nil()),
                "invalid or duplicate ramp identity",
            )?;
            ensure(
                ramp.footprints.len() <= 4
                    && ramp.visible_boundary_lines.len() <= 32
                    && ramp.lines.len() <= 4096
                    && ramp.semantic_edges.len() <= 4,
                "ramp exceeds per-element budget",
            )?;
            if let Some(p) = &ramp.source {
                ensure(
                    p.context == self.context,
                    "ramp projection context is stale",
                )?;
                let expected = PlanRampItem::derive(
                    ramp.entity,
                    p.start,
                    p.end,
                    p.width,
                    p.thickness,
                    ramp.material,
                    p.lower,
                    p.upper,
                    p.context,
                )?;
                ensure(
                    *ramp == expected,
                    "ramp graphics do not match checked projection",
                )?;
            } else {
                ensure(
                    ramp.footprint_corners.is_none() && ramp.semantic_edges.is_empty(),
                    "invalid section ramp source",
                )?;
            }
            if let Some(corners) = ramp.footprint_corners {
                ensure(
                    corners.iter().all(|p| p.is_finite())
                        && ramp
                            .ascent_direction
                            .is_some_and(|d| d.is_finite() && (d.x.hypot(d.y) - 1.0).abs() <= 1e-8)
                        && ramp.arrow == ramp.lines
                        && ramp.lines.len() <= 3,
                    "invalid ramp plan projection",
                )?;
            } else {
                ensure(
                    ramp.footprints.is_empty()
                        && ramp.visible_boundary_lines.is_empty()
                        && ramp.ascent_direction.is_none()
                        && ramp.arrow.is_empty(),
                    "section ramp cannot carry a plan footprint",
                )?;
            }
            let expected = ramp
                .footprints
                .iter()
                .flat_map(|footprint| {
                    let vertices = footprint.vertices();
                    vertices
                        .iter()
                        .copied()
                        .zip(vertices.iter().copied().cycle().skip(1))
                        .take(vertices.len())
                        .map(move |(a, b)| (a, b, footprint.role))
                })
                .collect::<Vec<_>>();
            ensure(
                expected.len() == ramp.visible_boundary_lines.len()
                    && ramp
                        .visible_boundary_lines
                        .iter()
                        .zip(&expected)
                        .enumerate()
                        .all(|(i, (line, (a, b, role)))| {
                            line.entity == ramp.entity
                                && line.feature == i as u32
                                && line.start == *a
                                && line.end == *b
                                && line.role == *role
                        }),
                "ramp boundary strokes do not match footprints",
            )?;
            for footprint in &ramp.footprints {
                ensure(
                    footprint.vertices().iter().all(|p| p.is_finite())
                        && footprint.area().is_finite()
                        && footprint.area() > 0.0,
                    "invalid ramp footprint",
                )?;
                self.items.push(PlanItem {
                    entity: ramp.entity,
                    footprint: footprint.clone(),
                    surface: os_geometry::SurfaceIdentity {
                        layer: None,
                        material: ramp.material,
                    },
                    hidden_edges: BTreeSet::new(),
                    split_edges: Vec::new(),
                });
            }
            let mut features = BTreeSet::new();
            for line in ramp.visible_boundary_lines.iter().chain(&ramp.lines) {
                ensure(
                    line.entity == ramp.entity
                        && features.insert(line.feature)
                        && line.start.is_finite()
                        && line.end.is_finite()
                        && line.start.distance(line.end).is_finite()
                        && line.start.distance(line.end) > TOLERANCE,
                    "invalid ramp line",
                )?;
                self.line_surfaces.insert(
                    (ramp.entity, line.feature),
                    os_geometry::SurfaceIdentity {
                        layer: None,
                        material: ramp.material,
                    },
                );
            }
            for line in &ramp.lines {
                if let Some((start, end)) = grids::clip(line.start, line.end, self.context.crop)? {
                    self.lines.push(PlanLine {
                        start,
                        end,
                        ..line.clone()
                    });
                    if ramp.source.is_none() {
                        self.line_segments.push(SnapSegment {
                            entity: ramp.entity,
                            feature: line.feature,
                            start: line.start,
                            end: line.end,
                        });
                    }
                }
            }
            for line in &ramp.semantic_edges {
                self.line_segments.push(SnapSegment {
                    entity: ramp.entity,
                    feature: line.feature,
                    start: line.start,
                    end: line.end,
                });
            }
            self.native_line_ids.insert(ramp.entity);
        }
        self.provider_segment_count += line_count;
        self.items
            .sort_by_key(|item| (role_order(item.footprint.role), item.entity));
        self.lines
            .sort_by_key(|line| (role_order(line.role), line.entity, line.feature));
        self.ramps = ramps;
        Ok(self)
    }

    pub fn ramps(&self, current: PlanContext) -> Result<&[PlanRampItem]> {
        self.items(current)?;
        Ok(&self.ramps)
    }

    pub fn pick_ramp(&self, current: PlanContext, point: Point2) -> Result<Option<Id>> {
        ensure(point.is_finite(), "invalid ramp pick point")?;
        self.ramps(current)?;
        Ok(self
            .items
            .iter()
            .rev()
            .find(|i| {
                i.footprint.contains(point)
                    && self
                        .ramps
                        .binary_search_by_key(&i.entity, |r| r.entity)
                        .is_ok()
            })
            .map(|i| i.entity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};
    use crate::snapping::{SnapKind, SnapQuery};

    fn context() -> PlanContext {
        PlanContext {
            session_id: Id::new(),
            model_revision: 1,
            view_id: Id::new(),
            settings_revision: 1,
            basis: HorizontalBasis::default(),
            range: PlanRange {
                top: 3.0,
                cut: 1.2,
                bottom: 0.4,
                depth: -1.0,
            },
            crop: None,
            scale_denominator: 100.0,
            show_walls: true,
            show_extensions: false,
        }
    }

    fn ramp(id: Id, context: PlanContext) -> PlanRampItem {
        PlanRampItem::derive(
            id,
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            1.0,
            0.2,
            None,
            0.0,
            2.0,
            context,
        )
        .unwrap()
    }

    fn drawing(context: PlanContext, item: PlanRampItem) -> PlanDrawing {
        PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new())
            .unwrap()
            .with_ramps(vec![item])
            .unwrap()
    }

    fn query(point: Point2, nearest: bool) -> SnapQuery {
        let camera = PlanCamera {
            center: Point2::new(2.0, 0.0),
            pixels_per_metre: 100.0,
        };
        let viewport = [800.0, 600.0];
        SnapQuery {
            camera,
            viewport,
            pointer: camera.project(point, viewport).unwrap(),
            radius_pixels: 2.0,
            endpoints: !nearest,
            midpoints: false,
            intersections: false,
            perpendicular_from: None,
            nearest,
            axis_extensions: false,
            exclude_entity: None,
        }
    }

    #[test]
    fn continuous_slab_splits_at_top_and_underside_thresholds() {
        let context = context();
        let id = Id::new();
        let item = ramp(id, context);
        assert_eq!(item, ramp(id, context));
        assert_eq!(item.footprints.len(), 4);
        let expected = [
            (PlanRole::Depth, 0.0, 0.8),
            (PlanRole::Projected, 0.8, 2.4),
            (PlanRole::Cut, 2.4, 2.8),
            (PlanRole::Projected, 2.8, 4.0),
        ];
        for (area, (role, a, b)) in item.footprints.iter().zip(expected) {
            assert_eq!(area.role, role);
            assert!((area.vertices()[0].x - a).abs() < 1e-9);
            assert!((area.vertices()[2].x - b).abs() < 1e-9);
        }
        assert_eq!(item.ascent_direction, Some(Point2::new(1.0, 0.0)));
        assert_eq!(item.arrow.len(), 3);
        assert_eq!(item.arrow, item.lines);
        assert!(item.arrow[0].end.x > item.arrow[0].start.x);
        assert!((item.footprints.iter().map(PlanFootprint::area).sum::<f64>() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn depth_top_and_thickness_bound_visibility() {
        let mut context = context();
        context.range = PlanRange {
            top: 1.0,
            cut: 0.8,
            bottom: 0.4,
            depth: 0.2,
        };
        let item = ramp(Id::new(), context);
        assert!((item.footprints.first().unwrap().vertices()[0].x - 0.4).abs() < 1e-9);
        assert!((item.footprints.last().unwrap().vertices()[2].x - 2.4).abs() < 1e-9);
        assert!(!item.contains(Point2::new(0.2, 0.0)));
        assert!(!item.contains(Point2::new(2.6, 0.0)));
        for range in [
            PlanRange {
                top: 10.0,
                cut: 9.0,
                bottom: 8.0,
                depth: 7.0,
            },
            PlanRange {
                top: -0.2,
                cut: -1.0,
                bottom: -2.0,
                depth: -3.0,
            },
        ] {
            context.range = range;
            let item = ramp(Id::new(), context);
            assert!(
                item.footprints.is_empty()
                    && item.lines.is_empty()
                    && item.semantic_edges.is_empty()
            );
            let drawing = drawing(context, item);
            assert_eq!(
                drawing.pick_ramp(context, Point2::new(2.0, 0.0)).unwrap(),
                None
            );
        }
    }

    #[test]
    fn crop_clips_areas_symbols_and_picking_without_inventing_snap_edges() {
        let mut context = context();
        context.crop = Some(PlanCrop {
            min: Point2::new(0.4, -0.3),
            max: Point2::new(1.6, 0.3),
        });
        let id = Id::new();
        let item = ramp(id, context);
        assert!(item.semantic_edges.is_empty()); // Every physical edge lies outside this interior crop.
        assert_eq!(item.lines.len(), 3);
        for line in item.visible_strokes() {
            for p in [line.start, line.end] {
                assert!(
                    p.x >= 0.4 - 1e-9
                        && p.x <= 1.6 + 1e-9
                        && p.y >= -0.3 - 1e-9
                        && p.y <= 0.3 + 1e-9
                );
            }
        }
        let drawing = drawing(context, item).with_snap_segments(vec![]).unwrap();
        assert_eq!(
            drawing.pick_ramp(context, Point2::new(0.8, 0.0)).unwrap(),
            Some(id)
        );
        assert_eq!(
            drawing.pick(context, Point2::new(0.8, 0.0)).unwrap(),
            Some(id)
        );
        assert_eq!(
            drawing.pick_ramp(context, Point2::new(0.2, 0.0)).unwrap(),
            None
        );
        let q = query(Point2::new(1.0, 0.0), true);
        assert!(
            drawing
                .snap(context, q)
                .unwrap()
                .candidate(context, q)
                .unwrap()
                .is_none()
        );
        context.crop = Some(PlanCrop {
            min: Point2::new(8.0, 8.0),
            max: Point2::new(9.0, 9.0),
        });
        let hidden = ramp(id, context);
        assert!(
            hidden.footprints.is_empty()
                && hidden.lines.is_empty()
                && hidden.semantic_edges.is_empty()
        );
    }

    #[test]
    fn physical_edges_snap_but_arrow_and_range_partition_do_not() {
        let context = context();
        let id = Id::new();
        let item = ramp(id, context);
        let arrow_tip = item.arrow[0].end;
        let drawing = drawing(context, item).with_snap_segments(vec![]).unwrap();
        for (point, nearest, expected) in [
            (Point2::new(0.0, 0.5), false, Some(SnapKind::Endpoint)),
            (Point2::new(2.0, 0.5), true, Some(SnapKind::Nearest)),
            (arrow_tip, false, None),
            (arrow_tip, true, None),
            (Point2::new(2.4, 0.0), true, None),
        ] {
            let q = query(point, nearest);
            let hit = drawing
                .snap(context, q)
                .unwrap()
                .candidate(context, q)
                .unwrap();
            assert_eq!(hit.map(|h| h.kind), expected);
            if let Some(hit) = hit {
                assert_eq!(hit.entity, id);
                assert!(hit.feature < 4);
            }
        }
        let mut cropped = context;
        cropped.crop = Some(PlanCrop {
            min: Point2::new(0.4, -1.0),
            max: Point2::new(1.6, 1.0),
        });
        let drawing = super::tests::drawing(cropped, ramp(id, cropped))
            .with_snap_segments(vec![])
            .unwrap();
        let q = query(Point2::new(0.4, 0.5), false);
        assert!(
            drawing
                .snap(cropped, q)
                .unwrap()
                .candidate(cropped, q)
                .unwrap()
                .is_none()
        );
        let q = query(Point2::new(1.0, 0.5), true);
        assert!(
            drawing
                .snap(cropped, q)
                .unwrap()
                .candidate(cropped, q)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn rotation_translation_and_reversed_endpoints_preserve_uphill() {
        let mut context = context();
        context.basis = HorizontalBasis {
            origin: Point2::new(10.0, -5.0),
            rotation: std::f64::consts::FRAC_PI_2,
        };
        let item = PlanRampItem::derive(
            Id::new(),
            Point2::new(10.0, -1.0),
            Point2::new(10.0, -5.0),
            1.0,
            0.2,
            None,
            0.0,
            2.0,
            context,
        )
        .unwrap();
        assert!(item.arrow[0].end.x < item.arrow[0].start.x);
        assert!(item.ascent_direction.unwrap().x < -0.999999);
        assert!(item.contains(Point2::new(2.0, 0.0)));
        assert!((item.footprints.iter().map(PlanFootprint::area).sum::<f64>() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn checked_attachment_rejects_mutation_duplicates_stale_and_late_snaps() {
        let context = context();
        let id = Id::new();
        let empty = || PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap();
        let mut forged = ramp(id, context);
        forged.arrow[0].start.x += 0.1;
        assert!(empty().with_ramps(vec![forged]).is_err());
        assert!(
            empty()
                .with_ramps(vec![ramp(id, context), ramp(id, context)])
                .is_err()
        );
        let mut stale = context;
        stale.model_revision += 1;
        assert!(empty().with_ramps(vec![ramp(id, stale)]).is_err());
        assert!(
            empty()
                .with_snap_segments(vec![])
                .unwrap()
                .with_ramps(vec![ramp(id, context)])
                .is_err()
        );
        let drawing = drawing(context, ramp(id, context));
        assert!(drawing.ramps(stale).is_err());
        assert!(drawing.pick_ramp(stale, Point2::new(1.0, 0.0)).is_err());
        assert!(
            drawing
                .pick_ramp(context, Point2::new(f64::NAN, 0.0))
                .is_err()
        );
        for (width, thickness, lower, upper, end) in [
            (0.0, 0.2, 0.0, 2.0, Point2::new(4.0, 0.0)),
            (1.0, 0.0, 0.0, 2.0, Point2::new(4.0, 0.0)),
            (1.0, 0.2, 2.0, 0.0, Point2::new(4.0, 0.0)),
            (1.0, 0.2, 0.0, 2.0, Point2::new(0.0, 0.0)),
        ] {
            assert!(
                PlanRampItem::derive(
                    id,
                    Point2::new(0.0, 0.0),
                    end,
                    width,
                    thickness,
                    None,
                    lower,
                    upper,
                    context
                )
                .is_err()
            );
        }
        assert!(
            PlanRampItem::derive(
                id,
                Point2::new(0.0, MAX_COORDINATE),
                Point2::new(4.0, MAX_COORDINATE),
                1.0,
                0.2,
                None,
                0.0,
                2.0,
                context
            )
            .is_err()
        );
    }

    #[test]
    fn material_and_symbol_reach_shared_sheet_and_pdf_paths() {
        let context = context();
        let id = Id::new();
        let material = Id::new();
        let item = PlanRampItem::derive(
            id,
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            1.0,
            0.2,
            Some(material),
            0.0,
            2.0,
            context,
        )
        .unwrap();
        let drawing = drawing(context, item);
        assert!(
            drawing
                .items(context)
                .unwrap()
                .iter()
                .all(|i| i.surface.material == Some(material))
        );
        assert!(
            drawing
                .provider_lines(context)
                .unwrap()
                .iter()
                .all(|l| drawing.line_surface(l.entity, l.feature).material == Some(material))
        );
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 297.0,
                height_mm: 210.0,
                number: "A1",
                name: "Ramp",
            },
            "Plan",
            PaperViewport {
                center_mm: Point2::new(148.5, 105.0),
                width_mm: 200.0,
                height_mm: 140.0,
                model_center_m: Point2::new(2.0, 0.0),
                scale_denominator: 100.0,
            },
            context,
            &drawing,
        )
        .unwrap();
        let paths = page
            .marks()
            .iter()
            .filter(|m| matches!(m.kind, PaperMarkKind::Path { .. }))
            .count();
        assert!(
            paths
                >= drawing.items(context).unwrap().len()
                    + drawing.provider_lines(context).unwrap().len()
        );
        assert!(page.to_pdf().unwrap().starts_with(b"%PDF-"));
    }

    #[test]
    fn section_items_are_checked_cropped_material_aware_and_snappable() {
        let mut context = context();
        context.crop = Some(PlanCrop {
            min: Point2::new(0.4, -1.0),
            max: Point2::new(1.6, 1.0),
        });
        let id = Id::new();
        let material = Id::new();
        let line = PlanLine {
            entity: id,
            feature: 0,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(2.0, 0.0),
            role: PlanRole::Cut,
        };
        let item = PlanRampItem::from_section_lines(id, Some(material), vec![line.clone()]);
        let drawing = drawing(context, item).with_snap_segments(vec![]).unwrap();
        let lines = drawing.provider_lines(context).unwrap();
        assert_eq!(lines[0].start, Point2::new(0.4, 0.0));
        assert_eq!(lines[0].end, Point2::new(1.6, 0.0));
        assert_eq!(drawing.line_surface(id, 0).material, Some(material));
        let q = query(Point2::new(1.0, 0.0), true);
        assert!(
            drawing
                .snap(context, q)
                .unwrap()
                .candidate(context, q)
                .unwrap()
                .is_some()
        );
        let bad = PlanRampItem::from_section_lines(id, None, vec![line.clone(), line]);
        assert!(
            PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
                .unwrap()
                .with_ramps(vec![bad])
                .is_err()
        );
    }
}

fn role_order(role: PlanRole) -> u8 {
    match role {
        PlanRole::Depth => 0,
        PlanRole::Projected => 1,
        PlanRole::Cut => 2,
    }
}

#[cfg(test)]
mod plan_data_tests {
    use super::*;
    use crate::snapping::{SnapKind, SnapQuery};

    fn context() -> PlanContext {
        PlanContext {
            session_id: Id::new(),
            model_revision: 1,
            view_id: Id::new(),
            settings_revision: 1,
            basis: HorizontalBasis::default(),
            range: PlanRange {
                top: 4.0,
                cut: 1.2,
                bottom: 0.5,
                depth: -1.0,
            },
            crop: None,
            scale_denominator: 100.0,
            show_walls: true,
            show_extensions: false,
        }
    }

    fn params() -> RampParams {
        RampParams {
            name: "Ramp".into(),
            lower_level: Id::new(),
            upper_level: Id::new(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(12.0, 0.0),
            width: 2.0,
            structural_thickness: 0.2,
            material: Some(Id::new()),
        }
    }

    fn empty(context: PlanContext) -> PlanDrawing {
        PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new()).unwrap()
    }

    #[test]
    fn continuous_range_projection_matches_slab_and_preserves_material() {
        let ctx = context();
        let p = params();
        let id = Id::new();
        let ramp = PlanRampItem::from_plan_data(id, &p, 0.0, 3.0, ctx)
            .unwrap()
            .unwrap();
        assert_eq!(ramp.footprints.len(), 4);
        assert_eq!(
            ramp.footprints.iter().map(|f| f.role).collect::<Vec<_>>(),
            vec![
                PlanRole::Depth,
                PlanRole::Projected,
                PlanRole::Cut,
                PlanRole::Projected
            ]
        );
        let cut = &ramp.footprints[2];
        assert!((cut.area() - 1.6).abs() < 1e-9);
        assert!(
            cut.vertices()
                .iter()
                .all(|p| p.x >= 4.8 - 1e-9 && p.x <= 5.6 + 1e-9)
        );
        assert_eq!(ramp.ascent_direction, Some(Point2::new(1.0, 0.0)));
        assert_eq!(ramp.arrow.len(), 3);
        let drawing = empty(ctx).with_ramps(vec![ramp]).unwrap();
        assert_eq!(drawing.pick(ctx, Point2::new(8.0, 0.0)).unwrap(), Some(id));
        assert_eq!(
            drawing.pick_ramp(ctx, Point2::new(8.0, 0.0)).unwrap(),
            Some(id)
        );
        assert!(
            drawing
                .items(ctx)
                .unwrap()
                .iter()
                .all(|i| i.surface.material == p.material)
        );
        assert!(
            drawing
                .provider_lines(ctx)
                .unwrap()
                .iter()
                .all(|l| drawing.line_surface(id, l.feature).material == p.material)
        );
    }

    #[test]
    fn hidden_crop_tangencies_rotated_basis_and_reversed_baseline() {
        let mut ctx = context();
        let mut p = params();
        let id = Id::new();
        for (lower, upper) in [(5.0, 8.0), (-5.0, -1.0)] {
            assert!(
                PlanRampItem::from_plan_data(id, &p, lower, upper, ctx)
                    .unwrap()
                    .is_none()
            );
        }
        ctx.crop = Some(PlanCrop {
            min: Point2::new(20.0, 0.0),
            max: Point2::new(21.0, 1.0),
        });
        assert!(
            PlanRampItem::from_plan_data(id, &p, 0.0, 3.0, ctx)
                .unwrap()
                .is_none()
        );
        ctx.crop = Some(PlanCrop {
            min: Point2::new(2.0, 1.0),
            max: Point2::new(3.0, 2.0),
        });
        assert!(
            PlanRampItem::from_plan_data(id, &p, 0.0, 3.0, ctx)
                .unwrap()
                .is_none()
        );
        ctx.crop = Some(PlanCrop {
            min: Point2::new(6.0, -0.5),
            max: Point2::new(8.0, 0.5),
        });
        let ramp = PlanRampItem::from_plan_data(id, &p, 0.0, 3.0, ctx)
            .unwrap()
            .unwrap();
        assert!(ramp.footprints.iter().all(|f| {
            f.vertices()
                .iter()
                .all(|p| (6.0..=8.0).contains(&p.x) && (-0.5..=0.5).contains(&p.y))
        }));
        let drawing = empty(ctx).with_ramps(vec![ramp]).unwrap();
        assert_eq!(drawing.pick_ramp(ctx, Point2::new(5.0, 0.0)).unwrap(), None);
        let mut stale = ctx;
        stale.model_revision += 1;
        assert!(drawing.ramps(stale).is_err());
        assert!(drawing.pick_ramp(stale, Point2::new(7.0, 0.0)).is_err());
        ctx.crop = None;
        ctx.basis.rotation = std::f64::consts::FRAC_PI_2;
        std::mem::swap(&mut p.start, &mut p.end);
        let ramp = PlanRampItem::from_plan_data(id, &p, 0.0, 3.0, ctx)
            .unwrap()
            .unwrap();
        let d = ramp.ascent_direction.unwrap();
        assert!(d.x.abs() < 1e-9 && (d.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn arrows_are_rendered_but_only_boundaries_snap() {
        let ctx = context();
        let id = Id::new();
        let ramp = PlanRampItem::from_plan_data(id, &params(), 0.0, 3.0, ctx)
            .unwrap()
            .unwrap();
        let arrow_end = ramp.arrow[0].end;
        let boundary = ramp.visible_boundary_lines[0].start;
        let drawing = empty(ctx)
            .with_ramps(vec![ramp])
            .unwrap()
            .with_snap_segments(Vec::new())
            .unwrap();
        let camera = PlanCamera {
            center: Point2::new(6.0, 0.0),
            pixels_per_metre: 100.0,
        };
        let query = |p| SnapQuery {
            camera,
            viewport: [1280.0, 800.0],
            pointer: camera.project(p, [1280.0, 800.0]).unwrap(),
            radius_pixels: 3.0,
            endpoints: true,
            midpoints: true,
            intersections: true,
            perpendicular_from: None,
            nearest: true,
            axis_extensions: false,
            exclude_entity: None,
        };
        let q = query(arrow_end);
        assert!(
            drawing
                .snap(ctx, q)
                .unwrap()
                .candidate(ctx, q)
                .unwrap()
                .is_none()
        );
        let q = query(boundary);
        let candidate = drawing
            .snap(ctx, q)
            .unwrap()
            .candidate(ctx, q)
            .unwrap()
            .unwrap();
        assert_eq!(candidate.entity, id);
        assert_eq!(candidate.kind, SnapKind::Endpoint);
        assert!(
            drawing
                .provider_lines(ctx)
                .unwrap()
                .iter()
                .any(|l| l.end == arrow_end)
        );
    }

    #[test]
    fn sections_validate_clip_and_snap_without_plan_footprints() {
        let mut ctx = context();
        ctx.crop = Some(PlanCrop {
            min: Point2::new(0.0, 0.0),
            max: Point2::new(2.0, 2.0),
        });
        let id = Id::new();
        let material = Some(Id::new());
        let line = PlanLine {
            entity: id,
            feature: 7,
            start: Point2::new(-1.0, 1.0),
            end: Point2::new(3.0, 1.0),
            role: PlanRole::Cut,
        };
        let ramp = PlanRampItem::from_section_lines(id, material, vec![line.clone()]);
        let drawing = empty(ctx)
            .with_ramps(vec![ramp.clone()])
            .unwrap()
            .with_snap_segments(Vec::new())
            .unwrap();
        assert!(drawing.items(ctx).unwrap().is_empty());
        assert!(drawing.ramps(ctx).unwrap()[0].arrow.is_empty());
        assert_eq!(drawing.line_segments.len(), 1);
        let clipped = &drawing.provider_lines(ctx).unwrap()[0];
        assert_eq!((clipped.start.x, clipped.end.x), (0.0, 2.0));
        assert_eq!(drawing.line_surface(id, 7).material, material);
        assert!(empty(ctx).with_ramps(vec![ramp.clone(), ramp]).is_err());
        let bad = PlanRampItem::from_section_lines(id, None, vec![line.clone(), line]);
        assert!(empty(ctx).with_ramps(vec![bad]).is_err());
        let mut bad = params();
        bad.width = f64::NAN;
        assert!(PlanRampItem::from_plan_data(id, &bad, 0.0, 3.0, ctx).is_err());
        let ramp = PlanRampItem::from_plan_data(id, &params(), 0.0, 3.0, ctx)
            .unwrap()
            .unwrap();
        assert!(
            empty(ctx)
                .with_snap_segments(Vec::new())
                .unwrap()
                .with_ramps(vec![ramp])
                .is_err()
        );
    }
}
