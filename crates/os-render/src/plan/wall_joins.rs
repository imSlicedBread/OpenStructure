//! Appearance-aware cleanup of explicitly authored native wall contacts.
use super::*;
use os_geometry::SurfaceIdentity;

/// View-plane trace and stroke owner supplied by the native model adapter.
/// This is derived data, never a plugin connectivity inference.
#[derive(Clone, Copy, Debug)]
pub struct PlanWallContact {
    pub members: [Id; 2],
    pub owner: Id,
    pub trace: (Point2, Point2),
}

type Footprints = BTreeMap<Id, Vec<(SurfaceIdentity, PlanFootprint)>>;

// One budget for the entire drawing, including provenance comparisons,
// interval partition scans, and residual-edge subtraction. Fail atomically
// instead of silently returning partially cleaned graphics.
const MAX_JOIN_WORK: usize = 1_000_000;
struct WorkBudget(usize);
impl WorkBudget {
    fn charge(&mut self, work: usize) -> Result<()> {
        self.0 = self.0.checked_sub(work).ok_or_else(|| {
            os_core::Error::Invalid("wall join graphics work budget exceeded".into())
        })?;
        Ok(())
    }
}

impl PlanDrawing {
    /// Run after material colors and all role appearances have been resolved.
    /// Uncropped boundaries prove provenance: a crop-created edge is not a
    /// contact, even when it happens to lie on the persisted trace.
    pub fn with_wall_join_graphics(
        mut self,
        contacts: &[PlanWallContact],
        uncropped: &Footprints,
    ) -> Result<Self> {
        ensure(
            contacts.len() <= MAX_PLAN_ELEMENTS,
            "wall contact budget exceeded",
        )?;
        let mut budget = WorkBudget(MAX_JOIN_WORK);
        let mut by_entity = BTreeMap::<Id, Vec<usize>>::new();
        for (index, item) in self.items.iter().enumerate() {
            by_entity.entry(item.entity).or_default().push(index);
        }
        let strokes: Vec<_> = self
            .items
            .iter()
            .map(|item| {
                self.appearances
                    .get(&item.entity)
                    .copied()
                    .unwrap_or_default()
                    .for_role(item.footprint.role)
                    .or_else(|| {
                        (item.footprint.role == PlanRole::Overhead).then_some(PlanStroke {
                            color: [88, 88, 88],
                            weight_mm: 0.18,
                            dashed: true,
                        })
                    })
            })
            .collect();
        let mut cuts = BTreeMap::<usize, Vec<(Point2, Point2)>>::new();
        for contact in contacts {
            budget.charge(1)?;
            let [a, b] = contact.members;
            ensure(
                a != b && contact.members.contains(&contact.owner),
                "invalid wall contact members",
            )?;
            let trace = Trace::new(contact.trace)?;
            // Even a genuine contact may coincide with the view boundary.
            // The visible crop edge takes precedence over contact cleanup.
            if self
                .context
                .crop
                .is_some_and(|crop| trace.on_crop_boundary(crop))
            {
                continue;
            }
            let mut contributions = Vec::new();
            for member in contact.members {
                let Some(originals) = uncropped.get(&member) else {
                    continue;
                };
                for &index in by_entity.get(&member).into_iter().flatten() {
                    let item = &self.items[index];
                    for (surface, original) in originals {
                        budget.charge(1)?;
                        if *surface != item.surface || original.role != item.footprint.role {
                            continue;
                        }
                        let vertices = original.vertices();
                        for i in 0..vertices.len() {
                            budget.charge(1)?;
                            let Some(raw) =
                                trace.interval((vertices[i], vertices[(i + 1) % vertices.len()]))
                            else {
                                continue;
                            };
                            for edge in item.outline() {
                                budget.charge(1)?;
                                if let Some(visible) = trace.interval(edge) {
                                    let interval = (raw.0.max(visible.0), raw.1.min(visible.1));
                                    if interval.1 - interval.0 > 1e-9 {
                                        ensure(
                                            contributions.len() < MAX_PLAN_ELEMENTS,
                                            "wall contact interval budget exceeded",
                                        )?;
                                        contributions.push((index, interval));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Partition at every visible cell/layer/crop boundary. Compatibility
            // is decided for the whole interval, not pairwise (which could erase
            // a discontinuity in overlapping projected profile cells).
            ensure(
                contributions.len() <= MAX_PLAN_ELEMENTS,
                "wall contact interval budget exceeded",
            )?;
            let mut stations: Vec<_> = contributions
                .iter()
                .flat_map(|(_, (lo, hi))| [*lo, *hi])
                .collect();
            stations.sort_by(f64::total_cmp);
            stations.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
            // Each span scans contributions and participants a bounded number
            // of times. Reserve that potentially quadratic work before starting.
            budget.charge(
                stations
                    .len()
                    .saturating_mul(contributions.len())
                    .saturating_mul(8),
            )?;
            for span in stations.windows(2) {
                let mid = (span[0] + span[1]) * 0.5;
                let participants: BTreeSet<_> = contributions
                    .iter()
                    .filter(|(_, (lo, hi))| *lo < mid && mid < *hi)
                    .map(|(index, _)| *index)
                    .collect();
                if ![a, b]
                    .iter()
                    .all(|id| participants.iter().any(|i| self.items[*i].entity == *id))
                {
                    continue;
                }
                let first_index = *participants.first().unwrap();
                let first = &self.items[first_index];
                let compatible = participants.iter().all(|i| {
                    let item = &self.items[*i];
                    item.footprint.role == first.footprint.role
                        && self.surface_color(item.surface) == self.surface_color(first.surface)
                        && strokes[*i] == strokes[first_index]
                });
                let keeper = (!compatible).then(|| {
                    participants
                        .iter()
                        .copied()
                        .find(|i| self.items[*i].entity == contact.owner)
                        .unwrap()
                });
                for index in participants {
                    if Some(index) != keeper {
                        cuts.entry(index)
                            .or_default()
                            .push((trace.point(span[0]), trace.point(span[1])));
                    }
                }
            }
        }
        for (index, cuts) in cuts {
            let item = &mut self.items[index];
            let mut subtract = |mut edges: Vec<_>| -> Result<Vec<_>> {
                for &cut in &cuts {
                    budget.charge(edges.len())?;
                    edges = edges
                        .into_iter()
                        .flat_map(|edge| os_geometry::walls::subtract_segment(edge, cut))
                        .collect();
                }
                Ok(edges)
            };
            item.split_edges = subtract(std::mem::take(&mut item.split_edges))?;
            let vertices = item.footprint.vertices();
            for i in 0..vertices.len() {
                if item.hidden_edges.contains(&i) {
                    continue;
                }
                let original = (vertices[i], vertices[(i + 1) % vertices.len()]);
                let edges = subtract(vec![original])?;
                if edges != [original] {
                    item.hidden_edges.insert(i);
                    item.split_edges.extend(edges);
                }
            }
        }
        Ok(self)
    }
}

struct Trace {
    start: Point2,
    direction: Point2,
    length: f64,
}
impl Trace {
    fn on_crop_boundary(&self, crop: PlanCrop) -> bool {
        let points = [
            crop.min,
            Point2::new(crop.max.x, crop.min.y),
            crop.max,
            Point2::new(crop.min.x, crop.max.y),
        ];
        (0..4).any(|i| self.interval((points[i], points[(i + 1) % 4])).is_some())
    }
    fn new((start, end): (Point2, Point2)) -> Result<Self> {
        let length = start.distance(end);
        ensure(
            start.is_finite() && end.is_finite() && length.is_finite() && length > 1e-6,
            "invalid wall contact trace",
        )?;
        Ok(Self {
            start,
            direction: Point2::new((end.x - start.x) / length, (end.y - start.y) / length),
            length,
        })
    }
    fn point(&self, station: f64) -> Point2 {
        Point2::new(
            self.start.x + station * self.direction.x,
            self.start.y + station * self.direction.y,
        )
    }
    fn interval(&self, (a, b): (Point2, Point2)) -> Option<(f64, f64)> {
        let project = |p: Point2| {
            let (x, y) = (p.x - self.start.x, p.y - self.start.y);
            (
                x * self.direction.x + y * self.direction.y,
                x * self.direction.y - y * self.direction.x,
            )
        };
        let (a, b) = (project(a), project(b));
        if a.1.abs() > 1e-8 || b.1.abs() > 1e-8 {
            return None;
        }
        let (lo, hi) = (a.0.min(b.0).max(0.0), a.0.max(b.0).min(self.length));
        (hi - lo > 1e-9).then_some((lo, hi))
    }
}
