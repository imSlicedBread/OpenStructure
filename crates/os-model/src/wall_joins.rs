//! Explicit connectivity for straight, equal-profile end-to-end walls.
use crate::{Entity, Model, WallParams};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_WALL_JOINS: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WallEndpoint {
    Start,
    End,
}
impl WallEndpoint {
    pub fn point(self, wall: &WallParams) -> Point2 {
        match self {
            Self::Start => wall.start,
            Self::End => wall.end,
        }
    }
    pub fn opposite(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallAnchor {
    pub wall: Id,
    pub endpoint: WallEndpoint,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ButtJoinParams {
    pub a: WallAnchor,
    pub b: WallAnchor,
}
pub type ButtJoin = Entity<ButtJoinParams>;

/// Authored connectivity; material geometry is derived, never persisted twice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum WallJoinParams {
    Butt {
        a: WallAnchor,
        b: WallAnchor,
    },
    Corner {
        a: WallAnchor,
        b: WallAnchor,
        owner: Id,
    },
    Tee {
        host: Id,
        station: f64,
        branch: WallAnchor,
    },
}
pub type WallJoin = Entity<WallJoinParams>;

impl From<ButtJoinParams> for WallJoinParams {
    fn from(p: ButtJoinParams) -> Self {
        Self::Butt { a: p.a, b: p.b }
    }
}

impl WallJoinParams {
    pub fn members(&self) -> [Id; 2] {
        match *self {
            Self::Butt { a, b } | Self::Corner { a, b, .. } => [a.wall, b.wall],
            Self::Tee { host, branch, .. } => [host, branch.wall],
        }
    }
    pub fn anchors(&self) -> Vec<WallAnchor> {
        match *self {
            Self::Butt { a, b } | Self::Corner { a, b, .. } => vec![a, b],
            Self::Tee { branch, .. } => vec![branch],
        }
    }
    pub fn validate(&self, model: &Model) -> Result<()> {
        let [a, b] = self.members();
        ensure(a != b, "join requires different walls")?;
        let (wa, wb) = (&wall(model, a)?, &wall(model, b)?);
        wa.validate()?;
        wb.validate()?;
        ensure(
            wa.level == wb.level && wa.height == wb.height && wa.thickness == wb.thickness,
            "wall join requires the same level, base, height and thickness",
        )?;
        if let Self::Butt { a, b } = *self {
            return ButtJoinParams { a, b }.validate(model);
        }
        let (u, v) = (axis(wa), axis(wb));
        ensure(
            (u.x * v.x + u.y * v.y).abs() <= 1e-12,
            "corner and tee walls must be exactly perpendicular",
        )?;
        let node = match *self {
            Self::Corner { a, b, owner } => {
                ensure(
                    owner == a.wall || owner == b.wall,
                    "corner owner must be a member",
                )?;
                let node = a.endpoint.point(wa);
                ensure(
                    node == b.endpoint.point(wb),
                    "corner endpoints must coincide exactly",
                )?;
                node
            }
            Self::Tee {
                station, branch, ..
            } => {
                ensure(
                    station.is_finite()
                        && station > wa.thickness / 2.0 + 0.001
                        && station < wa.length() - wa.thickness / 2.0 - 0.001,
                    "tee station must be inside the continuous host",
                )?;
                let node = wall_point(wa, station, 0.0);
                ensure(
                    node == branch.endpoint.point(wb),
                    "tee branch must meet the exact host station",
                )?;
                node
            }
            Self::Butt { .. } => unreachable!(),
        };
        // Any third centerline crossing the junction's material square is
        // ambiguous. This also rejects crosses and nearby competing contacts.
        for (id, other) in &model.walls {
            if *id == a || *id == b || other.parameters.level != wa.level {
                continue;
            }
            let w = &model.resolve_wall(*id)?.parameters;
            let d = axis(w);
            let t =
                ((node.x - w.start.x) * d.x + (node.y - w.start.y) * d.y).clamp(0.0, w.length());
            ensure(
                node.distance(wall_point(w, t, 0.0)) > (wa.thickness + w.thickness) / 2.0 + 1e-9,
                "join has an ambiguous third wall at its junction",
            )?;
        }
        Ok(())
    }
}

fn wall(model: &Model, id: Id) -> Result<WallParams> {
    Ok(model.resolve_wall(id)?.parameters)
}
pub fn axis(w: &WallParams) -> Point2 {
    Point2::new(
        (w.end.x - w.start.x) / w.length(),
        (w.end.y - w.start.y) / w.length(),
    )
}
pub fn wall_point(w: &WallParams, station: f64, across: f64) -> Point2 {
    let d = axis(w);
    Point2::new(
        w.start.x + d.x * station - d.y * across,
        w.start.y + d.y * station + d.x * across,
    )
}

/// Effective material stations in the authored axis, shared by all consumers.
pub fn wall_station_limits(model: &Model, id: Id) -> Result<(f64, f64)> {
    let w = &wall(model, id)?;
    let mut limits = (0.0, w.length());
    for join in model.wall_joins.values() {
        for anchor in join
            .parameters
            .anchors()
            .into_iter()
            .filter(|a| a.wall == id)
        {
            let trim = match join.parameters {
                WallJoinParams::Butt { .. } => 0.0,
                WallJoinParams::Corner { owner, .. } if owner == id => -w.thickness / 2.0,
                _ => w.thickness / 2.0,
            };
            match anchor.endpoint {
                WallEndpoint::Start => limits.0 += trim,
                WallEndpoint::End => limits.1 -= trim,
            }
        }
    }
    ensure(
        limits.1 - limits.0 >= 0.001,
        "joined wall is over-trimmed or below minimum length",
    )?;
    Ok(limits)
}

/// Shared material face in world XY; the branch/trimmed wall owns the end face.
pub fn wall_join_trace(model: &Model, p: &WallJoinParams) -> Result<(Point2, Point2)> {
    let anchor = match *p {
        WallJoinParams::Butt { a, .. } => a,
        WallJoinParams::Corner { a, b, owner } => {
            if owner == a.wall {
                b
            } else {
                a
            }
        }
        WallJoinParams::Tee { branch, .. } => branch,
    };
    let w = &wall(model, anchor.wall)?;
    let (start, end) = wall_station_limits(model, anchor.wall)?;
    let t = if anchor.endpoint == WallEndpoint::Start {
        start
    } else {
        end
    };
    Ok((
        wall_point(w, t, -w.thickness / 2.0),
        wall_point(w, t, w.thickness / 2.0),
    ))
}

impl ButtJoinParams {
    pub fn validate(&self, model: &Model) -> Result<()> {
        ensure(
            self.a.wall != self.b.wall,
            "butt join requires two different walls",
        )?;
        let (a, b) = (&wall(model, self.a.wall)?, &wall(model, self.b.wall)?);
        a.validate()?;
        b.validate()?;
        ensure(
            a.level == b.level && a.height == b.height && a.thickness == b.thickness,
            "butt join requires the same level, base, height and thickness",
        )?;
        let node = self.a.endpoint.point(a);
        ensure(
            node == self.b.endpoint.point(b),
            "butt join endpoints must coincide exactly",
        )?;
        let away = |w: &WallParams, e: WallEndpoint| {
            let p = e.opposite().point(w);
            Point2::new((p.x - node.x) / w.length(), (p.y - node.y) / w.length())
        };
        let (u, v) = (away(a, self.a.endpoint), away(b, self.b.endpoint));
        ensure(
            (u.x * v.y - u.y * v.x).abs() <= 1e-12 && u.x * v.x + u.y * v.y < 0.0,
            "butt join requires collinear opposing ends; corners, tees and overlaps are unsupported",
        )?;
        // A third wall through the junction would make this a tee/cross/competing
        // contact. Only explicit join nodes impose this restriction on the model.
        for (id, other) in &model.walls {
            if *id == self.a.wall || *id == self.b.wall || other.parameters.level != a.level {
                continue;
            }
            let w = &model.resolve_wall(*id)?.parameters;
            let dx = w.end.x - w.start.x;
            let dy = w.end.y - w.start.y;
            let length = dx.hypot(dy);
            let along = ((node.x - w.start.x) * dx + (node.y - w.start.y) * dy) / length;
            let across = ((node.x - w.start.x) * dy - (node.y - w.start.y) * dx) / length;
            ensure(
                !(across.abs() <= 1e-9 && along >= -1e-9 && along <= length + 1e-9),
                "butt join has a competing wall at its junction",
            )?;
        }
        Ok(())
    }
}

pub fn validate_wall_joins(model: &Model) -> Result<()> {
    ensure(
        model.wall_joins.len() <= MAX_WALL_JOINS,
        "wall join limit exceeded",
    )?;
    let mut anchors = BTreeSet::new();
    for join in model.wall_joins.values() {
        join.parameters.validate(model)?;
        for anchor in join.parameters.anchors() {
            ensure(
                anchors.insert(anchor),
                "competing joins at one wall endpoint",
            )?;
        }
    }
    let mut stations: BTreeMap<Id, Vec<f64>> = BTreeMap::new();
    for join in model.wall_joins.values() {
        if let WallJoinParams::Tee { host, station, .. } = join.parameters {
            let w = &wall(model, host)?;
            let (start, end) = wall_station_limits(model, host)?;
            let half = w.thickness / 2.0;
            ensure(
                station - half >= start + 0.001 && station + half <= end - 0.001,
                "tee conflicts with a trimmed host end",
            )?;
            let others = stations.entry(host).or_default();
            ensure(
                others
                    .iter()
                    .all(|s| (s - station).abs() >= w.thickness + 0.001),
                "competing tee stations",
            )?;
            others.push(station);
        }
    }
    let members: BTreeSet<_> = model
        .wall_joins
        .values()
        .flat_map(|j| j.parameters.members())
        .collect();
    for id in members {
        let (start, end) = wall_station_limits(model, id)?;
        let w = &wall(model, id)?;
        for opening in model.openings.values().filter(|o| o.parameters.host == id) {
            let o = model.resolve_opening(&opening.parameters)?;
            o.validate_host(w)?;
            ensure(
                o.offset >= start + 0.001 && o.offset + o.width <= end - 0.001,
                "opening clearance intersects a joined wall end",
            )?;
            for join in model
                .wall_joins
                .values()
                .filter(|j| j.parameters.members().contains(&id))
            {
                let (a, b) = wall_join_trace(model, &join.parameters)?;
                let d = axis(w);
                let project = |p: Point2| (p.x - w.start.x) * d.x + (p.y - w.start.y) * d.y;
                let (a, b) = (project(a), project(b));
                if (a - b).abs() > 1e-9 {
                    ensure(
                        o.offset + o.width + 0.001 <= a.min(b) || o.offset - 0.001 >= a.max(b),
                        "opening clearance intersects a join contact face",
                    )?;
                }
            }
            if let Some(stations) = stations.get(&id) {
                ensure(
                    stations.iter().all(|s| {
                        o.offset + o.width + 0.001 <= s - w.thickness / 2.0
                            || o.offset - 0.001 >= s + w.thickness / 2.0
                    }),
                    "opening clearance intersects a tee junction",
                )?;
            }
        }
    }
    validate_joined_cells(model)?;
    Ok(())
}

/// A clear cycle is legal. Reject material overlap anywhere within a connected
/// join group, including conflicts away from the authored junction nodes.
fn validate_joined_cells(model: &Model) -> Result<()> {
    let mut neighbors: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for join in model.wall_joins.values() {
        let [a, b] = join.parameters.members();
        neighbors.entry(a).or_default().push(b);
        neighbors.entry(b).or_default().push(a);
    }
    let mut groups = BTreeMap::new();
    for &root in neighbors.keys() {
        if groups.contains_key(&root) {
            continue;
        }
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if groups.insert(id, root).is_none() {
                pending.extend(neighbors[&id].iter().filter(|id| !groups.contains_key(id)));
            }
        }
    }
    let mut cells = Vec::new();
    let used_anchors: BTreeSet<_> = model
        .wall_joins
        .values()
        .flat_map(|join| join.parameters.anchors())
        .collect();
    for (&id, &group) in &groups {
        let w = &wall(model, id)?;
        let (a, b) = wall_station_limits(model, id)?;
        let h = w.thickness / 2.;
        let points = [
            wall_point(w, a, -h),
            wall_point(w, b, -h),
            wall_point(w, b, h),
            wall_point(w, a, h),
        ];
        let min = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let max = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        cells.push((group, id, points, axis(w), min, max));
    }
    cells.sort_by(|a, b| a.4.total_cmp(&b.4));
    let mut comparisons = 0usize;
    for (i, a) in cells.iter().enumerate() {
        for b in cells.iter().skip(i + 1).take_while(|b| b.4 < a.5 - 1e-9) {
            comparisons += 1;
            ensure(
                comparisons <= 1_000_000,
                "join graph exceeds geometric comparison budget",
            )?;
            if a.0 != b.0 {
                continue;
            }
            let separate = [
                a.3,
                Point2::new(-a.3.y, a.3.x),
                b.3,
                Point2::new(-b.3.y, b.3.x),
            ]
            .into_iter()
            .any(|d| {
                let range = |points: &[Point2; 4]| {
                    let values = points.map(|p| (p.x - a.2[0].x) * d.x + (p.y - a.2[0].y) * d.y);
                    (
                        values.into_iter().fold(f64::INFINITY, f64::min),
                        values.into_iter().fold(f64::NEG_INFINITY, f64::max),
                    )
                };
                let (ra, rb) = (range(&a.2), range(&b.2));
                ra.1.min(rb.1) - ra.0.max(rb.0) <= 1e-9
            });
            ensure(
                separate || pending_corner_contact(model, a.1, b.1, &used_anchors),
                format!(
                    "joined wall group has overlapping material cells ({}, {})",
                    a.1, b.1
                ),
            )?;
        }
    }
    Ok(())
}

/// While a loop is being authored one transaction at a time, its final pair of
/// perpendicular endpoints still has the untrimmed overlap that the explicit
/// corner join will resolve. Permit only that exact, unused endpoint contact;
/// arbitrary intersections and already-owned anchors remain invalid.
fn pending_corner_contact(model: &Model, a_id: Id, b_id: Id, used: &BTreeSet<WallAnchor>) -> bool {
    let (Ok(a), Ok(b)) = (model.resolve_wall(a_id), model.resolve_wall(b_id)) else {
        return false;
    };
    let (a, b) = (&a.parameters, &b.parameters);
    if a.level != b.level || a.height != b.height || a.thickness != b.thickness {
        return false;
    }
    let (u, v) = (axis(a), axis(b));
    if (u.x * v.x + u.y * v.y).abs() > 1e-12 {
        return false;
    }
    for a_endpoint in [WallEndpoint::Start, WallEndpoint::End] {
        let a_anchor = WallAnchor {
            wall: a_id,
            endpoint: a_endpoint,
        };
        if used.contains(&a_anchor) {
            continue;
        }
        for b_endpoint in [WallEndpoint::Start, WallEndpoint::End] {
            let b_anchor = WallAnchor {
                wall: b_id,
                endpoint: b_endpoint,
            };
            if used.contains(&b_anchor) {
                continue;
            }
            if a_endpoint.point(a) == b_endpoint.point(b) {
                return true;
            }
        }
    }
    false
}
