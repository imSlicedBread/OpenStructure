//! Stable identities and disposable, atomic previews for bounded native wall copies.
use super::wall_set_move::Candidate;
use os_core::{Error, Id, Point2, Result, ensure};
use os_document::{Command, Document};
use os_model::{Header, Model, WallJoinParams};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DEPENDENTS: usize = 4096;
const MAX_COORDINATE: f64 = 1e6;
const MAX_COLLISION_CELLS: usize = 65_536;
const MAX_COLLISION_COMPARISONS: usize = 1_000_000;
const CONTACT_TOLERANCE: f64 = 1e-9;

pub(super) struct Identities {
    pub(super) walls: BTreeMap<Id, Id>,
    openings: BTreeMap<Id, Id>,
    joins: BTreeMap<Id, Id>,
    pub(super) tags: BTreeMap<Id, Id>,
    clearances: usize,
}

impl Identities {
    pub(super) fn new(model: &Model, ids: &BTreeSet<Id>, view: Id) -> Result<Self> {
        ensure(
            (1..=64).contains(&ids.len()),
            "Copy walls requires 1–64 native straight walls",
        )?;
        let plan = model
            .views
            .get(&view)
            .ok_or_else(|| Error::Invalid("Copy plan is missing".into()))?;
        ensure(
            plan.parameters.kind == os_model::ViewKind::Plan
                && plan
                    .parameters
                    .plan
                    .is_some_and(|p| p.view_type == os_model::PlanViewType::FloorPlan),
            "Copy walls requires a floor plan",
        )?;
        for id in ids {
            let wall = model
                .walls
                .get(id)
                .ok_or_else(|| Error::Invalid("Copy source wall is missing".into()))?;
            ensure(
                wall.header.type_id == os_walls::WALL_TYPE
                    && wall.parameters.path.is_straight()
                    && Some(wall.parameters.level) == plan.parameters.level,
                "Copy walls requires native straight walls on the active plan level",
            )?;
        }

        // Count before allocating: neither a hidden opening nor a hidden tag in
        // this view may be silently dropped to fit the operation's budget.
        let openings: BTreeSet<_> = model
            .openings
            .values()
            .filter(|o| ids.contains(&o.parameters.host))
            .take(MAX_DEPENDENTS + 1)
            .map(|o| o.id())
            .collect();
        let mut joins = BTreeSet::new();
        for join in model.wall_joins.values() {
            let members = join.parameters.members();
            if members.iter().any(|id| ids.contains(id)) {
                ensure(
                    members.iter().all(|id| ids.contains(id)),
                    "Copy walls requires complete wall-join membership",
                )?;
                joins.insert(join.id());
                ensure(
                    openings.len() + joins.len() <= MAX_DEPENDENTS,
                    "Copy walls exceeds the 4096 dependent limit",
                )?;
            }
        }
        let tags: BTreeSet<_> = model
            .opening_tags
            .values()
            .filter(|t| t.parameters.view == view && openings.contains(&t.parameters.opening))
            .take(MAX_DEPENDENTS + 1)
            .map(|t| t.id())
            .collect();
        ensure(
            openings.len() + joins.len() + tags.len() <= MAX_DEPENDENTS,
            "Copy walls exceeds the 4096 dependent limit (openings, joins and tags)",
        )?;
        let clearances = openings
            .iter()
            .filter(|id| model.opening_clearances.contains_key(id))
            .count();
        let allocate = |source: &BTreeSet<Id>| source.iter().map(|id| (*id, Id::new())).collect();
        Ok(Self {
            walls: allocate(ids),
            openings: allocate(&openings),
            joins: allocate(&joins),
            tags: allocate(&tags),
            clearances,
        })
    }

    pub(super) fn summary(&self) -> String {
        format!(
            "{} walls, {} openings, {} joins, {} active-plan tags, {} clearance locks",
            self.walls.len(),
            self.openings.len(),
            self.joins.len(),
            self.tags.len(),
            self.clearances,
        )
    }

    fn header(&self, header: &mut Header, id: Id) {
        header.id = id;
        for target in header.relationships.values_mut().flatten() {
            if let Some(copy) = self
                .walls
                .get(target)
                .or_else(|| self.openings.get(target))
                .or_else(|| self.joins.get(target))
                .or_else(|| self.tags.get(target))
            {
                *target = *copy;
            }
        }
    }

    pub(super) fn candidate(&self, original: &Model, delta: Point2) -> Result<Candidate> {
        check_point(delta)?;
        ensure(
            delta.x != 0.0 || delta.y != 0.0,
            "Copy walls requires a nonzero displacement",
        )?;
        let shift = |point: Point2| -> Result<Point2> {
            let point = Point2::new(point.x + delta.x, point.y + delta.y);
            check_point(point)?;
            Ok(point)
        };
        let mut commands = Vec::new();
        for (&source, &id) in &self.walls {
            let mut wall = original
                .walls
                .get(&source)
                .ok_or_else(|| Error::Invalid("Copy source wall is missing".into()))?
                .clone();
            self.header(&mut wall.header, id);
            let start = shift(wall.parameters.start())?;
            let end = shift(wall.parameters.end())?;
            ensure(
                start != wall.parameters.start() || end != wall.parameters.end(),
                "Copy displacement is below coordinate precision",
            )?;
            *wall.parameters.path.straight_start_mut()? = start;
            *wall.parameters.path.straight_end_mut()? = end;
            commands.push(Command::AddWall(wall));
            if let Some(assignment) = original.wall_type_assignments.get(&source) {
                commands.push(Command::AssignWallType {
                    wall: id,
                    assignment: Some(*assignment),
                });
            }
            copy_lifecycle(original, source, id, &mut commands);
        }
        for (&source, &id) in &self.openings {
            let mut opening = original
                .openings
                .get(&source)
                .ok_or_else(|| Error::Invalid("Copy source opening is missing".into()))?
                .clone();
            self.header(&mut opening.header, id);
            opening.parameters.host = copied_wall(&self.walls, opening.parameters.host)?;
            commands.push(Command::AddOpening(opening));
            copy_lifecycle(original, source, id, &mut commands);
        }
        for (&source, &id) in &self.openings {
            if let Some(clearance) = original.opening_clearances.get(&source) {
                commands.push(Command::SetOpeningClearance {
                    id,
                    clearance: Some(*clearance),
                });
            }
        }
        for (&source, &id) in &self.joins {
            let mut join = original
                .wall_joins
                .get(&source)
                .ok_or_else(|| Error::Invalid("Copy source join is missing".into()))?
                .clone();
            self.header(&mut join.header, id);
            match &mut join.parameters {
                WallJoinParams::Butt { a, b } => {
                    a.wall = copied_wall(&self.walls, a.wall)?;
                    b.wall = copied_wall(&self.walls, b.wall)?;
                }
                WallJoinParams::Corner { a, b, owner } => {
                    a.wall = copied_wall(&self.walls, a.wall)?;
                    b.wall = copied_wall(&self.walls, b.wall)?;
                    *owner = copied_wall(&self.walls, *owner)?;
                }
                WallJoinParams::Tee { host, branch, .. } => {
                    *host = copied_wall(&self.walls, *host)?;
                    branch.wall = copied_wall(&self.walls, branch.wall)?;
                }
            }
            commands.push(Command::AddWallJoin(join));
        }
        for (&source, &id) in &self.tags {
            let mut tag = original
                .opening_tags
                .get(&source)
                .ok_or_else(|| Error::Invalid("Copy source tag is missing".into()))?
                .clone();
            self.header(&mut tag.header, id);
            tag.parameters.opening = *self
                .openings
                .get(&tag.parameters.opening)
                .ok_or_else(|| Error::Invalid("Copy tag opening is missing".into()))?;
            tag.parameters.position = shift(tag.parameters.position)?;
            commands.push(Command::AddOpeningTag(tag));
        }

        let document = Document::from_model(original.clone())?;
        let model = document.preview_commands(commands.clone())?;
        ensure(model != *original, "Copy walls made no change")?;
        for id in self.walls.values() {
            os_geometry::walls::NativeWall::from_model(&model, *id)?.mesh()?;
        }
        for id in self.openings.values() {
            crate::opening_tools::panel_mesh(&model, *id)?;
        }
        validate_collisions(&model, &self.walls.values().copied().collect())?;
        validate_rooms(original, &model)?;
        Ok(Candidate { model, commands })
    }
}

fn copied_wall(walls: &BTreeMap<Id, Id>, source: Id) -> Result<Id> {
    walls
        .get(&source)
        .copied()
        .ok_or_else(|| Error::Invalid("Copy dependency refers to an unselected wall".into()))
}

fn copy_lifecycle(original: &Model, source: Id, id: Id, commands: &mut Vec<Command>) {
    // A newly created document element defaults to the latest phase. Copying
    // an unassigned legacy element must instead retain its effective Existing
    // lifecycle, so carry the resolved lifecycle even when it was implicit.
    if let Some(lifecycle) = original.element_lifecycle(source) {
        commands.push(Command::SetElementLifecycle {
            element: id,
            lifecycle,
        });
    }
}

fn check_point(point: Point2) -> Result<()> {
    ensure(
        point.is_finite() && point.x.abs() <= MAX_COORDINATE && point.y.abs() <= MAX_COORDINATE,
        "Copy coordinates must be finite and within +/- 1000000 metres",
    )
}

fn validate_rooms(original: &Model, candidate: &Model) -> Result<()> {
    ensure(
        original.rooms == candidate.rooms
            && original.room_tags == candidate.room_tags
            && original.dimensions == candidate.dimensions,
        "Copy walls must preserve rooms, room tags and dimensions",
    )?;
    let levels: BTreeSet<_> = original
        .rooms
        .values()
        .map(|r| r.parameters.level)
        .collect();
    for level in levels {
        let faces = |model: &Model| {
            os_geometry::rooms::derive_faces(&model.room_boundary_segments(level)?).map_err(|e| {
                Error::Invalid(format!("Copy walls cannot resolve room boundaries: {e:?}"))
            })
        };
        let before = faces(original)?;
        let after = faces(candidate)?;
        for room in original
            .rooms
            .values()
            .filter(|r| r.parameters.level == level)
        {
            let old = before.assign_seed(room.parameters.seed).map_err(|e| {
                Error::Invalid(format!("Source room {} is unresolved: {e:?}", room.id()))
            })?;
            let new = after.assign_seed(room.parameters.seed).map_err(|e| {
                Error::Invalid(format!("Copy would invalidate room {}: {e:?}", room.id()))
            })?;
            ensure(
                old.key.as_signature() == room.parameters.boundary_signature && old == new,
                format!(
                    "Copy would change the accepted boundary of room {}",
                    room.id()
                ),
            )?;
        }
    }
    Ok(())
}

/// Collision envelopes reserve the full host footprint, including its apertures.
/// Explicit joins trim/extend the footprint; boundary-only contact is legal.
struct Footprint {
    points: [Point2; 4],
    bottom: f64,
    top: f64,
}

fn footprints(model: &Model, id: Id) -> Result<Vec<Footprint>> {
    let wall = model.resolve_wall(id)?.parameters;
    let half = wall.thickness / 2.0;
    let bottom = model.levels[&wall.level].parameters.elevation;
    let top = bottom + wall.height;
    let (start, end) = os_model::wall_station_limits(model, id)?;
    if wall.path.is_straight() {
        return Ok(vec![Footprint {
            points: [
                wall.path.offset_point(start, -half),
                wall.path.offset_point(end, -half),
                wall.path.offset_point(end, half),
                wall.path.offset_point(start, half),
            ],
            bottom,
            top,
        }]);
    }
    // Existing circular walls are obstacles too. Each short annular sector is
    // enclosed in its midpoint tangent rectangle. This is conservative at the
    // native 1 mm display tolerance, without missing the bulge between samples.
    let count = wall.path.display_segments(half)?;
    let mut cells = Vec::with_capacity(count);
    for i in 0..count {
        let a = wall.length() * i as f64 / count as f64;
        let b = wall.length() * (i + 1) as f64 / count as f64;
        let mid = (a + b) / 2.0;
        let origin = wall.path.point(mid);
        let u = wall.path.tangent(mid);
        let v = Point2::new(-u.y, u.x);
        let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
        let mut max = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for station in [a, mid, b] {
            for offset in [-half, half] {
                let p = wall.path.offset_point(station, offset);
                let x = (p.x - origin.x) * u.x + (p.y - origin.y) * u.y;
                let y = (p.x - origin.x) * v.x + (p.y - origin.y) * v.y;
                min.x = min.x.min(x);
                min.y = min.y.min(y);
                max.x = max.x.max(x);
                max.y = max.y.max(y);
            }
        }
        let world = |x, y| Point2::new(origin.x + u.x * x + v.x * y, origin.y + u.y * x + v.y * y);
        cells.push(Footprint {
            points: [
                world(min.x, min.y),
                world(max.x, min.y),
                world(max.x, max.y),
                world(min.x, max.y),
            ],
            bottom,
            top,
        });
    }
    Ok(cells)
}

fn overlaps(a: &Footprint, b: &Footprint) -> bool {
    if a.top.min(b.top) - a.bottom.max(b.bottom) <= CONTACT_TOLERANCE {
        return false;
    }
    // Separating-axis test in a local frame keeps large coordinates stable.
    for points in [&a.points, &b.points] {
        for index in [1, 3] {
            let dx = points[index].x - points[0].x;
            let dy = points[index].y - points[0].y;
            let length = dx.hypot(dy);
            let axis = Point2::new(dx / length, dy / length);
            let range = |points: &[Point2; 4]| {
                let projected =
                    points.map(|p| (p.x - a.points[0].x) * axis.x + (p.y - a.points[0].y) * axis.y);
                (
                    projected.into_iter().fold(f64::INFINITY, f64::min),
                    projected.into_iter().fold(f64::NEG_INFINITY, f64::max),
                )
            };
            let (ra, rb) = (range(&a.points), range(&b.points));
            if ra.1.min(rb.1) - ra.0.max(rb.0) <= CONTACT_TOLERANCE {
                return false;
            }
        }
    }
    true
}

fn validate_collisions(model: &Model, copied: &BTreeSet<Id>) -> Result<()> {
    let mut cells = BTreeMap::new();
    let mut total = 0;
    for &id in model.walls.keys() {
        let footprint = footprints(model, id)?;
        total += footprint.len();
        ensure(
            total <= MAX_COLLISION_CELLS,
            "Copy wall collision check exceeds 65536 cells",
        )?;
        cells.insert(id, footprint);
    }
    let mut comparisons = 0;
    for &id in copied {
        for (&other, other_cells) in &cells {
            if other == id || (copied.contains(&other) && other < id) {
                continue;
            }
            for a in &cells[&id] {
                for b in other_cells {
                    comparisons += 1;
                    ensure(
                        comparisons <= MAX_COLLISION_COMPARISONS,
                        "Copy wall collision check exceeds 1000000 comparisons",
                    )?;
                    ensure(
                        !overlaps(a, b),
                        format!("Copied wall {id} overlaps wall {other}"),
                    )?;
                }
            }
        }
    }
    Ok(())
}
