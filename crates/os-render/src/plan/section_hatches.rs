//! Checked vector cut hatches. Scanline intervals respect fill compositing order.
use super::*;
use os_geometry::section::VerticalSectionPlane;
use os_model::MaterialPatternParams;

pub const MAX_SECTION_HATCH_SEGMENTS: usize = 50_000;
const MAX_WORK: usize = 2_000_000;
pub const SECTION_HATCH_WEIGHT_MM: f64 = 0.10;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanSectionHatch {
    pub entity: Id,
    pub component: u32,
    pub surface: os_geometry::SurfaceIdentity,
    pub start: Point2,
    pub end: Point2,
}

impl PlanDrawing {
    /// Pattern keys are material UUIDs. Origin is world XYZ; angle is measured
    /// in the section station/elevation plane. Zoom and paper scale never enter
    /// this calculation. A budget failure rejects the entire replacement.
    pub fn with_section_hatches(
        mut self,
        patterns: BTreeMap<Id, MaterialPatternParams>,
        plane: VerticalSectionPlane,
    ) -> Result<Self> {
        ensure(
            patterns.len() <= MAX_SECTION_HATCH_SEGMENTS,
            "too many hatch assignments",
        )?;
        let length = plane.direction.x.hypot(plane.direction.y);
        ensure(
            plane.origin.is_finite()
                && plane.origin.x.abs() <= 1e6
                && plane.origin.y.abs() <= 1e6
                && plane.direction.is_finite()
                && length.is_finite()
                && length > 1e-9,
            "invalid hatch section plane",
        )?;
        let axis = Point2::new(plane.direction.x / length, plane.direction.y / length);
        let mut output: Vec<PlanSectionHatch> = Vec::new();
        let mut work = 0usize;
        for (material, pattern) in patterns {
            ensure(!material.0.is_nil(), "invalid hatch material identity")?;
            pattern.validate()?;
            let anchor = Point2::new(
                (pattern.origin[0] - plane.origin.x) * axis.x
                    + (pattern.origin[1] - plane.origin.y) * axis.y,
                pattern.origin[2],
            );
            let (sin, cos) = pattern.angle_degrees.to_radians().sin_cos();
            let direction = Point2::new(cos, sin);
            let normal = Point2::new(-sin, cos);
            let station =
                |p: Point2| (p.x - anchor.x) * direction.x + (p.y - anchor.y) * direction.y;
            let offset = |p: Point2| (p.x - anchor.x) * normal.x + (p.y - anchor.y) * normal.y;
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for fill in self
                .section_fills
                .iter()
                .filter(|f| f.surface.material == Some(material))
            {
                for point in &fill.points {
                    min = min.min(offset(*point));
                    max = max.max(offset(*point));
                }
            }
            if min > max {
                continue;
            }
            let first = (min / pattern.spacing).ceil();
            let last = (max / pattern.spacing).floor();
            let count = (last - first + 1.0).max(0.0);
            ensure(
                count.is_finite() && count <= MAX_WORK as f64,
                "Section incomplete: hatch line budget exceeded",
            )?;
            work = work.saturating_add((count as usize).saturating_mul(self.section_fills.len()));
            ensure(
                work <= MAX_WORK,
                "Section incomplete: hatch clipping work budget exceeded",
            )?;
            for line in 0..count as usize {
                let distance = (first + line as f64) * pattern.spacing;
                let mut events = Vec::new();
                for (index, fill) in self.section_fills.iter().enumerate() {
                    let mut lo = f64::INFINITY;
                    let mut hi = f64::NEG_INFINITY;
                    for (&a, &b) in fill.points.iter().zip(fill.points.iter().cycle().skip(1)) {
                        let da = offset(a) - distance;
                        let db = offset(b) - distance;
                        if da == 0.0 {
                            lo = lo.min(station(a));
                            hi = hi.max(station(a));
                        }
                        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
                            // Symmetric interpolation keeps a shared triangle edge's
                            // intersection independent of its traversal direction.
                            let (a, b, da, db) =
                                if a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).is_gt() {
                                    (b, a, db, da)
                                } else {
                                    (a, b, da, db)
                                };
                            let t = da.abs() / (da.abs() + db.abs());
                            let value = station(a) * (1.0 - t) + station(b) * t;
                            lo = lo.min(value);
                            hi = hi.max(value);
                        }
                    }
                    if hi > lo {
                        events.push((lo, true, index));
                        events.push((hi, false, index));
                    }
                }
                events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                let mut active = BTreeSet::new();
                let mut previous = 0.0;
                let line_start = output.len();
                for (at, entering, index) in events {
                    if at > previous
                        && let Some(&top) = active.last()
                    {
                        let fill: &PlanSectionFillPolygon = &self.section_fills[top];
                        if fill.surface.material == Some(material) {
                            let project = |t| {
                                Point2::new(
                                    anchor.x + direction.x * t + normal.x * distance,
                                    anchor.y + direction.y * t + normal.y * distance,
                                )
                            };
                            let start = project(previous);
                            let end = project(at);
                            ensure(
                                start.is_finite() && end.is_finite(),
                                "hatch coordinate overflow",
                            )?;
                            let has_line = line_start < output.len();
                            if let Some(last) = output.last_mut().filter(|_| has_line) {
                                if last.entity == fill.entity
                                    && last.component == fill.component
                                    && last.surface == fill.surface
                                    && last.end == start
                                {
                                    last.end = end;
                                } else {
                                    output.push(PlanSectionHatch {
                                        entity: fill.entity,
                                        component: fill.component,
                                        surface: fill.surface,
                                        start,
                                        end,
                                    });
                                }
                            } else {
                                output.push(PlanSectionHatch {
                                    entity: fill.entity,
                                    component: fill.component,
                                    surface: fill.surface,
                                    start,
                                    end,
                                });
                            }
                            ensure(
                                output.len() <= MAX_SECTION_HATCH_SEGMENTS,
                                "Section incomplete: hatch segment budget exceeded",
                            )?;
                        }
                    }
                    if entering {
                        active.insert(index);
                    } else {
                        active.remove(&index);
                    }
                    previous = at;
                }
            }
        }
        self.section_hatches = output;
        Ok(self)
    }

    pub fn section_hatches(&self, current: PlanContext) -> Result<&[PlanSectionHatch]> {
        self.items(current)?;
        Ok(&self.section_hatches)
    }
}
