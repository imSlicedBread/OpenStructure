//! Native straight-flight plan graphics shared by canvas and paper output.
use super::*;
use crate::snapping::SnapSegment;

/// A stair's visible tread areas and authored symbol strokes in view coordinates.
/// Each footprint and stroke carries the same stable stair identity.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanStairItem {
    pub entity: Id,
    pub material: Option<Id>,
    /// Full projected flight rectangle in view coordinates, before crop.
    /// Section-only line items have no horizontal footprint.
    pub footprint_corners: Option<[Point2; 4]>,
    /// Cropped tread regions. Their outlines are painted by every plan consumer.
    pub footprints: Vec<PlanFootprint>,
    /// Cropped tread, riser and border strokes with their actual view-range roles.
    pub visible_tread_lines: Vec<PlanLine>,
    /// Unit vector from first riser toward upper arrival, in view coordinates.
    pub ascent_direction: Option<Point2>,
    /// Cropped direction arrow strokes. Also present in `lines` for PlanDrawing.
    pub arrow: Vec<PlanLine>,
    /// Additional native lines; plan items use this for the arrow, sections for cuts.
    pub lines: Vec<PlanLine>,
}

impl PlanStairItem {
    pub fn contains(&self, point: Point2) -> bool {
        self.footprints
            .iter()
            .any(|footprint| footprint.contains(point))
    }

    /// Linework for a placement preview. Matches the footprint outlines and
    /// native arrow lines used by the committed PlanDrawing consumers.
    pub fn visible_strokes(&self) -> impl Iterator<Item = &PlanLine> {
        self.visible_tread_lines.iter().chain(self.arrow.iter())
    }

    pub fn from_section_lines(entity: Id, material: Option<Id>, lines: Vec<PlanLine>) -> Self {
        Self {
            entity,
            material,
            footprint_corners: None,
            footprints: Vec::new(),
            visible_tread_lines: Vec::new(),
            ascent_direction: None,
            arrow: Vec::new(),
            lines,
        }
    }

    /// The last of N equal goings is the upper arrival tread. The top of tread
    /// `i` is at lower + (i + 1) * rise. The structural underside slopes below it.
    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        entity: Id,
        start: Point2,
        end: Point2,
        width: f64,
        riser_count: usize,
        structural_thickness: f64,
        material: Option<Id>,
        lower_z: f64,
        upper_z: f64,
        context: PlanContext,
    ) -> Result<Self> {
        context.basis.validate()?;
        context.range.validate()?;
        if let Some(crop) = context.crop {
            crop.validate()?;
        }
        let run = start.distance(end);
        ensure(
            !entity.0.is_nil()
                && start.is_finite()
                && end.is_finite()
                && run.is_finite()
                && run > 1e-6
                && width.is_finite()
                && width > 1e-6
                && structural_thickness.is_finite()
                && structural_thickness > 1e-6
                && lower_z.is_finite()
                && upper_z.is_finite()
                && upper_z > lower_z
                && (1..=256).contains(&riser_count),
            "invalid stair plan geometry",
        )?;
        let direction = Point2::new((end.x - start.x) / run, (end.y - start.y) / run);
        let side = Point2::new(-direction.y * width * 0.5, direction.x * width * 0.5);
        let rise = (upper_z - lower_z) / riser_count as f64;
        let going = run / riser_count as f64;
        let at = |along: f64, lateral: f64| {
            Point2::new(
                start.x + direction.x * along + side.x * lateral,
                start.y + direction.y * along + side.y * lateral,
            )
        };
        let footprint_corners = [at(0.0, 1.0), at(0.0, -1.0), at(run, -1.0), at(run, 1.0)]
            .map(|point| context.basis.world_to_plane(point))
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| os_core::Error::Invalid("invalid stair footprint corners".into()))?;
        let plane_start = context.basis.world_to_plane(start)?;
        let plane_end = context.basis.world_to_plane(end)?;
        let ascent_direction = Point2::new(
            (plane_end.x - plane_start.x) / run,
            (plane_end.y - plane_start.y) / run,
        );
        ensure(
            ascent_direction.is_finite(),
            "invalid stair ascent direction",
        )?;
        let mut footprints = Vec::new();
        let mut lines = Vec::new();
        let mut feature = 0u32;
        let mut push_line = |a: Point2, b: Point2, role: PlanRole| -> Result<()> {
            let start = context.basis.world_to_plane(a)?;
            let end = context.basis.world_to_plane(b)?;
            if start.distance(end) > 1e-9
                && let Some((start, end)) = grids::clip(start, end, context.crop)?
            {
                lines.push(PlanLine {
                    entity,
                    feature,
                    start,
                    end,
                    role,
                });
                feature += 1;
            }
            Ok(())
        };
        let mut visible_extent: Option<(f64, f64)> = None;
        let mut add_region = |near: f64, far: f64, role: PlanRole| -> Result<()> {
            if far - near <= 1e-9 {
                return Ok(());
            }
            let corners = [at(near, 1.0), at(near, -1.0), at(far, -1.0), at(far, 1.0)];
            let corners = corners
                .map(|point| context.basis.world_to_plane(point))
                .into_iter()
                .collect::<Result<Vec<_>>>()?;
            if let Some(footprint) = PlanFootprint::from_convex(role, corners, context.crop)? {
                for vertex in footprint.vertices() {
                    let world = context.basis.plane_to_world(*vertex)?;
                    let along =
                        (world.x - start.x) * direction.x + (world.y - start.y) * direction.y;
                    visible_extent = Some(match visible_extent {
                        Some((min, max)) => (min.min(along), max.max(along)),
                        None => (along, along),
                    });
                }
                footprints.push(footprint);
            }
            Ok(())
        };
        for i in 0..riser_count {
            let near = i as f64 * going;
            let far = (i + 1) as f64 * going;
            let tread_top = lower_z + (i + 1) as f64 * rise;
            if tread_top <= context.range.depth + 1e-9 {
                continue;
            }
            // The underside rises continuously from the lower to upper level.
            // Split at a crossed cut plane instead of marking the whole tread
            // cut when only its lower fraction intersects that plane.
            let at_elevation =
                |z: f64| run * (z - lower_z + structural_thickness) / (upper_z - lower_z);
            let visible_far = far.min(at_elevation(context.range.top));
            if visible_far <= near + 1e-9 {
                continue;
            }
            if tread_top <= context.range.bottom + 1e-9 {
                add_region(near, visible_far, PlanRole::Depth)?;
            } else if tread_top <= context.range.cut + 1e-9 {
                add_region(near, visible_far, PlanRole::Projected)?;
            } else {
                let cut_far = visible_far.min(at_elevation(context.range.cut));
                add_region(near, cut_far, PlanRole::Cut)?;
                add_region(cut_far.max(near), visible_far, PlanRole::Projected)?;
            }
        }
        if let Some((visible_start, visible_end)) = visible_extent {
            let visible_run = visible_end - visible_start;
            // Keep the arrow in the visible longitudinal span after crop.
            if visible_run > 1e-6 {
                let arrow_start = visible_start + visible_run * 0.25;
                let arrow_end = visible_start + visible_run * 0.75;
                let head = going.min(width * 0.25).min(visible_run * 0.1);
                push_line(
                    at(arrow_start, 0.0),
                    at(arrow_end, 0.0),
                    PlanRole::Projected,
                )?;
                push_line(
                    at(arrow_end, 0.0),
                    at(arrow_end - head, 0.22),
                    PlanRole::Projected,
                )?;
                push_line(
                    at(arrow_end, 0.0),
                    at(arrow_end - head, -0.22),
                    PlanRole::Projected,
                )?;
            }
        }
        ensure(
            footprints.len() <= 512 && lines.len() <= 772,
            "stair plan exceeds geometry limit",
        )?;
        let mut visible_tread_lines = Vec::new();
        for footprint in &footprints {
            let vertices = footprint.vertices();
            for (start, end) in vertices
                .iter()
                .copied()
                .zip(vertices.iter().copied().cycle().skip(1))
                .take(vertices.len())
            {
                visible_tread_lines.push(PlanLine {
                    entity,
                    feature: visible_tread_lines.len() as u32,
                    start,
                    end,
                    role: footprint.role,
                });
            }
        }
        ensure(
            visible_tread_lines.len() <= 4096,
            "stair plan exceeds tread stroke limit",
        )?;
        for (index, line) in lines.iter_mut().enumerate() {
            line.feature = (visible_tread_lines.len() + index) as u32;
        }
        Ok(Self {
            entity,
            material,
            footprint_corners: Some(footprint_corners),
            footprints,
            visible_tread_lines,
            ascent_direction: Some(ascent_direction),
            arrow: lines.clone(),
            lines,
        })
    }
}

impl PlanDrawing {
    /// Attach stair fills, interior picking and symbol lines as one checked
    /// model-derived representation. Native lines reach canvas, sheet and PDF.
    pub fn with_stairs(mut self, mut stairs: Vec<PlanStairItem>) -> Result<Self> {
        ensure(self.snaps.is_none(), "attach stairs before snap features")?;
        ensure(self.stairs.is_empty(), "stairs already attached")?;
        let footprint_count = stairs.iter().map(|s| s.footprints.len()).sum::<usize>();
        let line_count = stairs.iter().map(|s| s.lines.len()).sum::<usize>();
        ensure(
            self.source_ids.len().saturating_add(stairs.len()) <= MAX_PLAN_ELEMENTS
                && self.items.len().saturating_add(footprint_count) <= MAX_PLAN_ELEMENTS
                && self
                    .provider_segment_count
                    .saturating_add(line_count)
                    .saturating_add(self.grid_segment_count)
                    .saturating_add(self.detail_segment_count)
                    .saturating_add(self.separator_segment_count)
                    <= MAX_PLAN_ELEMENTS,
            "stair plan exceeds drawing budget",
        )?;
        stairs.sort_by_key(|stair| stair.entity);
        for stair in &stairs {
            ensure(
                !stair.entity.0.is_nil() && self.source_ids.insert(stair.entity),
                "invalid or duplicate stair identity",
            )?;
            ensure(
                stair.footprints.len() <= 512
                    && stair.visible_tread_lines.len() <= 4096
                    && stair.lines.len() <= 772,
                "stair plan exceeds per-element budget",
            )?;
            if let Some(corners) = stair.footprint_corners {
                ensure(
                    corners.iter().all(|point| point.is_finite())
                        && stair.ascent_direction.is_some_and(|direction| {
                            direction.is_finite()
                                && (direction.x.hypot(direction.y) - 1.0).abs() <= 1e-8
                        })
                        && stair.arrow == stair.lines,
                    "invalid stair plan projection",
                )?;
            } else {
                ensure(
                    stair.footprints.is_empty()
                        && stair.visible_tread_lines.is_empty()
                        && stair.ascent_direction.is_none()
                        && stair.arrow.is_empty(),
                    "section stair cannot carry a plan footprint",
                )?;
            }
            let expected_tread_lines = stair
                .footprints
                .iter()
                .flat_map(|footprint| {
                    let vertices = footprint.vertices();
                    vertices
                        .iter()
                        .copied()
                        .zip(vertices.iter().copied().cycle().skip(1))
                        .take(vertices.len())
                        .map(move |(start, end)| (start, end, footprint.role))
                })
                .collect::<Vec<_>>();
            ensure(
                expected_tread_lines.len() == stair.visible_tread_lines.len()
                    && stair
                        .visible_tread_lines
                        .iter()
                        .zip(expected_tread_lines.iter())
                        .enumerate()
                        .all(|(feature, (line, (start, end, role)))| {
                            line.entity == stair.entity
                                && line.feature == feature as u32
                                && line.start == *start
                                && line.end == *end
                                && line.role == *role
                        }),
                "stair tread strokes do not match visible footprints",
            )?;
            let mut features = BTreeSet::new();
            for footprint in &stair.footprints {
                ensure(
                    footprint.vertices().iter().all(|p| p.is_finite())
                        && footprint.area().is_finite()
                        && footprint.area() > 0.0,
                    "invalid stair footprint",
                )?;
                self.items.push(PlanItem {
                    entity: stair.entity,
                    footprint: footprint.clone(),
                    surface: os_geometry::SurfaceIdentity {
                        layer: None,
                        material: stair.material,
                    },
                    hidden_edges: BTreeSet::new(),
                    split_edges: Vec::new(),
                });
            }
            for line in &stair.lines {
                ensure(
                    line.entity == stair.entity
                        && features.insert(line.feature)
                        && line.start.is_finite()
                        && line.end.is_finite(),
                    "invalid stair line",
                )?;
                let raw = SnapSegment {
                    entity: stair.entity,
                    feature: line.feature,
                    start: line.start,
                    end: line.end,
                };
                self.line_surfaces.insert(
                    (stair.entity, line.feature),
                    os_geometry::SurfaceIdentity {
                        layer: None,
                        material: stair.material,
                    },
                );
                if let Some((start, end)) = grids::clip(line.start, line.end, self.context.crop)? {
                    self.lines.push(PlanLine {
                        start,
                        end,
                        ..line.clone()
                    });
                    self.line_segments.push(raw);
                }
            }
            self.native_line_ids.insert(stair.entity);
        }
        self.provider_segment_count += line_count;
        self.items.sort_by_key(|item| {
            (
                match item.footprint.role {
                    PlanRole::Depth => 0,
                    PlanRole::Projected => 1,
                    PlanRole::Cut => 2,
                },
                item.entity,
            )
        });
        self.lines.sort_by_key(|line| {
            (
                match line.role {
                    PlanRole::Depth => 0,
                    PlanRole::Projected => 1,
                    PlanRole::Cut => 2,
                },
                line.entity,
                line.feature,
            )
        });
        self.stairs = stairs;
        Ok(self)
    }

    pub fn stairs(&self, current: PlanContext) -> Result<&[PlanStairItem]> {
        self.items(current)?;
        Ok(&self.stairs)
    }

    /// Pick only a visible cropped stair tread, retaining the model UUID.
    pub fn pick_stair(&self, current: PlanContext, point: Point2) -> Result<Option<Id>> {
        ensure(point.is_finite(), "invalid stair pick point")?;
        Ok(self
            .stairs(current)?
            .iter()
            .rev()
            .find(|stair| stair.contains(point))
            .map(|stair| stair.entity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};

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
                bottom: 0.0,
                depth: -1.0,
            },
            crop: None,
            scale_denominator: 100.0,
            show_walls: true,
            show_extensions: false,
        }
    }

    fn flight(entity: Id, context: PlanContext) -> PlanStairItem {
        PlanStairItem::derive(
            entity,
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            1.0,
            4,
            0.2,
            None,
            0.0,
            2.0,
            context,
        )
        .unwrap()
    }

    #[test]
    fn flight_projection_uses_elevations_and_has_a_stable_arrow() {
        let context = context();
        let entity = Id::new();
        let stair = flight(entity, context);
        assert_eq!(stair, flight(entity, context));
        assert_eq!(stair.footprints.len(), 5);
        assert_eq!(stair.footprints[0].role, PlanRole::Projected);
        assert_eq!(stair.footprints[2].role, PlanRole::Cut);
        assert_eq!(stair.footprints[3].role, PlanRole::Projected);
        assert!((stair.footprints[2].vertices()[2].x - 2.8).abs() < 1e-9);
        assert_eq!(stair.footprint_corners.unwrap()[0], Point2::new(0.0, 0.5));
        assert_eq!(stair.ascent_direction, Some(Point2::new(1.0, 0.0)));
        assert_eq!(
            stair.visible_tread_lines.len(),
            stair
                .footprints
                .iter()
                .map(|part| part.vertices().len())
                .sum::<usize>()
        );
        assert!(
            stair
                .visible_tread_lines
                .iter()
                .any(|line| line.role == PlanRole::Cut)
        );
        assert_eq!(stair.lines.len(), 3);
        assert_eq!(stair.arrow, stair.lines);
        assert_eq!(
            stair.visible_strokes().count(),
            stair.visible_tread_lines.len() + 3
        );
        assert!(stair.lines.iter().all(|line| line.entity == entity));
        let arrow = &stair.lines;
        assert!(arrow[0].end.x > arrow[0].start.x);
        assert!(arrow[1].end.x < arrow[1].start.x);
        assert!(arrow[2].end.x < arrow[2].start.x);
    }

    #[test]
    fn cropped_stair_is_pickable_only_in_visible_footprint() {
        let mut context = context();
        context.crop = Some(PlanCrop {
            min: Point2::new(0.4, -0.3),
            max: Point2::new(1.6, 0.3),
        });
        let entity = Id::new();
        let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new())
            .unwrap()
            .with_stairs(vec![flight(entity, context)])
            .unwrap();
        assert_eq!(
            drawing.pick(context, Point2::new(0.8, 0.0)).unwrap(),
            Some(entity)
        );
        assert_eq!(
            drawing.pick_stair(context, Point2::new(0.8, 0.0)).unwrap(),
            Some(entity)
        );
        assert_eq!(drawing.pick(context, Point2::new(0.2, 0.0)).unwrap(), None);
        assert_eq!(
            drawing.pick_stair(context, Point2::new(0.2, 0.0)).unwrap(),
            None
        );
        assert!(drawing.provider_lines(context).unwrap().iter().all(|line| {
            [line.start, line.end].iter().all(|point| {
                point.x >= 0.4 - 1e-9
                    && point.x <= 1.6 + 1e-9
                    && point.y >= -0.3 - 1e-9
                    && point.y <= 0.3 + 1e-9
            })
        }));
        let mut stale = context;
        stale.model_revision += 1;
        assert!(drawing.stairs(stale).is_err());
        assert!(drawing.pick_stair(stale, Point2::new(0.8, 0.0)).is_err());
    }

    #[test]
    fn reversed_flight_and_hidden_range_keep_direction_and_visibility() {
        let context = context();
        let entity = Id::new();
        let reversed = PlanStairItem::derive(
            entity,
            Point2::new(4.0, 0.0),
            Point2::new(0.0, 0.0),
            1.0,
            4,
            0.2,
            None,
            0.0,
            2.0,
            context,
        )
        .unwrap();
        assert!(reversed.lines[0].end.x < reversed.lines[0].start.x);
        let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new())
            .unwrap()
            .with_stairs(vec![reversed])
            .unwrap();
        assert_eq!(
            drawing.pick(context, Point2::new(3.0, 0.0)).unwrap(),
            Some(entity)
        );

        let mut high = context;
        high.range = PlanRange {
            top: 10.0,
            cut: 9.0,
            bottom: 8.0,
            depth: 7.0,
        };
        let hidden = flight(entity, high);
        assert!(hidden.footprints.is_empty() && hidden.lines.is_empty());
        let drawing = PlanDrawing::from_prisms(high, &BTreeMap::new(), Vec::new())
            .unwrap()
            .with_stairs(vec![hidden])
            .unwrap();
        assert_eq!(drawing.pick(high, Point2::new(2.0, 0.0)).unwrap(), None);
        assert!(drawing.unavailable(high).unwrap().is_empty());
    }

    #[test]
    fn sheet_and_pdf_consume_the_same_stair_drawing() {
        let context = context();
        let entity = Id::new();
        let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new())
            .unwrap()
            .with_stairs(vec![flight(entity, context)])
            .unwrap();
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 297.0,
                height_mm: 210.0,
                number: "A1",
                name: "Stair",
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
            .filter(|mark| matches!(mark.kind, PaperMarkKind::Path { .. }))
            .count();
        assert!(
            paths
                >= drawing.items(context).unwrap().len()
                    + drawing.provider_lines(context).unwrap().len()
        );
        assert!(page.to_pdf().unwrap().starts_with(b"%PDF-"));
    }
}
