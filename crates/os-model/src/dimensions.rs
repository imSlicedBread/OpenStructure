//! View-owned reporting dimensions. Measurements are derived from live anchors.
use crate::{Entity, Model, ViewKind};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_DIMENSION_METRES: f64 = 1_000_000.0;
pub const MAX_DIMENSION_ANCHORS: usize = 128;
/// Absolute model-space perpendicular tolerance, independent of view scale.
pub const DIMENSION_COLLINEAR_TOLERANCE_M: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionLayout {
    Aligned,
    Chain,
    Baseline,
    Angular,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionEndpoint {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionReference {
    pub wall: Id,
    pub endpoint: DimensionEndpoint,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionParams {
    pub layout: DimensionLayout,
    pub additional: Vec<DimensionReference>,
    pub baseline_spacing_m: f64,
    pub view: Id,
    pub first: DimensionReference,
    pub second: DimensionReference,
    /// Metres along the left normal of the directed first-to-second segment.
    pub offset_m: f64,
    /// Clicked model-space position; only a placement/orphan marker hint.
    pub orphan_hint: Point2,
}
pub type Dimension = Entity<DimensionParams>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimensionDiagnostic {
    MissingPlanView,
    MissingWall,
    WrongLevel,
    CoincidentAnchors,
    InvalidGeometry,
    OffAxis(usize),
    Backtracking(usize),
    MissingWallAt(usize),
    WrongLevelAt(usize),
    ParallelWalls,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedDimension {
    pub first: Point2,
    pub second: Point2,
    pub length_metres: f64,
}

/// Live infinite wall-axis intersection and the smaller directed sector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedAngularDimension {
    pub center: Point2,
    pub radius: f64,
    pub start_radians: f64,
    pub sweep_radians: f64,
}

impl ResolvedAngularDimension {
    pub fn degrees(self) -> f64 {
        self.sweep_radians.abs().to_degrees()
    }
    pub fn point(self, fraction: f64) -> Point2 {
        let angle = self.start_radians + fraction * self.sweep_radians;
        Point2::new(
            self.center.x + self.radius * angle.cos(),
            self.center.y + self.radius * angle.sin(),
        )
    }
    pub fn contains_placement(self, point: Point2) -> bool {
        let angle = (point.y - self.center.y).atan2(point.x - self.center.x);
        let delta = (angle - self.start_radians + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        delta * self.sweep_radians > 0.0
            && delta.abs() > 1e-6
            && delta.abs() < self.sweep_radians.abs() - 1e-6
    }
}

impl DimensionParams {
    pub fn resolve_angular_with(
        &self,
        mut wall: impl FnMut(Id) -> std::result::Result<(Point2, Point2), DimensionDiagnostic>,
    ) -> std::result::Result<ResolvedAngularDimension, DimensionDiagnostic> {
        if self.layout != DimensionLayout::Angular || self.validate().is_err() {
            return Err(DimensionDiagnostic::InvalidGeometry);
        }
        let (a, b) = wall(self.first.wall)?;
        let (c, d) = wall(self.second.wall)?;
        let unit = |a: Point2, b: Point2| {
            let length = a.distance(b);
            if !length.is_finite() || length <= 1e-6 {
                return Err(DimensionDiagnostic::InvalidGeometry);
            }
            Ok(Point2::new((b.x - a.x) / length, (b.y - a.y) / length))
        };
        let u = unit(a, b)?;
        let v = unit(c, d)?;
        let cross = u.x * v.y - u.y * v.x;
        if cross.abs() <= 1e-6 {
            return Err(DimensionDiagnostic::ParallelWalls);
        }
        let t = ((c.x - a.x) * v.y - (c.y - a.y) * v.x) / cross;
        let center = Point2::new(a.x + t * u.x, a.y + t * u.y);
        let sign = |endpoint| {
            if endpoint == DimensionEndpoint::End {
                1.0
            } else {
                -1.0
            }
        };
        let start = (u.y * sign(self.first.endpoint)).atan2(u.x * sign(self.first.endpoint));
        let end = (v.y * sign(self.second.endpoint)).atan2(v.x * sign(self.second.endpoint));
        let sweep = (end - start + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        if !center.is_finite()
            || center.x.abs() + self.offset_m > MAX_DIMENSION_METRES
            || center.y.abs() + self.offset_m > MAX_DIMENSION_METRES
        {
            return Err(DimensionDiagnostic::InvalidGeometry);
        }
        Ok(ResolvedAngularDimension {
            center,
            radius: self.offset_m,
            start_radians: start,
            sweep_radians: sweep,
        })
    }

    pub fn resolve_angular(
        &self,
        model: &Model,
    ) -> std::result::Result<ResolvedAngularDimension, DimensionDiagnostic> {
        let level = model
            .views
            .get(&self.view)
            .filter(|v| v.parameters.kind == ViewKind::Plan)
            .and_then(|v| v.parameters.level)
            .ok_or(DimensionDiagnostic::MissingPlanView)?;
        self.resolve_angular_with(|id| {
            let wall = model
                .walls
                .get(&id)
                .ok_or(DimensionDiagnostic::MissingWall)?;
            if wall.parameters.level != level {
                return Err(DimensionDiagnostic::WrongLevel);
            }
            Ok((wall.parameters.start, wall.parameters.end))
        })
    }

    /// Choose one of four non-reflex sectors; placement on either axis is invalid.
    pub fn place_angular(&mut self, model: &Model, placement: Point2) -> Result<()> {
        let mut candidate = self.clone();
        candidate.offset_m = 1.0;
        candidate.orphan_hint = placement;
        for first in [DimensionEndpoint::Start, DimensionEndpoint::End] {
            for second in [DimensionEndpoint::Start, DimensionEndpoint::End] {
                candidate.first.endpoint = first;
                candidate.second.endpoint = second;
                if let Ok(geometry) = candidate.resolve_angular(model) {
                    candidate.offset_m = geometry.center.distance(placement);
                    if geometry.contains_placement(placement)
                        && candidate.validate_creation(model).is_ok()
                    {
                        *self = candidate;
                        return Ok(());
                    }
                    candidate.offset_m = 1.0;
                }
            }
        }
        Err(os_core::Error::Invalid(
            "Choose a point inside a nonparallel wall angle, away from its rays".into(),
        ))
    }
    pub fn references(&self) -> impl Iterator<Item = DimensionReference> + '_ {
        [self.first, self.second]
            .into_iter()
            .chain(self.additional.iter().copied())
    }

    /// Resolve all anchors atomically, retaining a one-based failing anchor index.
    pub fn resolve_anchors_with(
        &self,
        mut endpoint: impl FnMut(DimensionReference) -> std::result::Result<Point2, DimensionDiagnostic>,
    ) -> std::result::Result<Vec<Point2>, DimensionDiagnostic> {
        if self.layout == DimensionLayout::Angular || self.validate().is_err() {
            return Err(DimensionDiagnostic::InvalidGeometry);
        }
        let points = self
            .references()
            .enumerate()
            .map(|(i, r)| {
                endpoint(r).map_err(|reason| {
                    if self.layout == DimensionLayout::Aligned {
                        reason
                    } else {
                        match reason {
                            DimensionDiagnostic::MissingWall => {
                                DimensionDiagnostic::MissingWallAt(i + 1)
                            }
                            DimensionDiagnostic::WrongLevel => {
                                DimensionDiagnostic::WrongLevelAt(i + 1)
                            }
                            other => other,
                        }
                    }
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let first = points[0];
        let length = first.distance(points[1]);
        if points.iter().any(|p| !p.is_finite()) || !length.is_finite() {
            return Err(DimensionDiagnostic::InvalidGeometry);
        }
        if length <= 1e-6 {
            return Err(DimensionDiagnostic::CoincidentAnchors);
        }
        let dx = (points[1].x - first.x) / length;
        let dy = (points[1].y - first.y) / length;
        let mut previous = length;
        for (i, point) in points.iter().enumerate().skip(2) {
            let x = point.x - first.x;
            let y = point.y - first.y;
            let projection = x * dx + y * dy;
            if !projection.is_finite() || (x * dy - y * dx).abs() > DIMENSION_COLLINEAR_TOLERANCE_M
            {
                return Err(DimensionDiagnostic::OffAxis(i + 1));
            }
            if projection <= previous || points[..i].iter().any(|p| p.distance(*point) <= 1e-6) {
                return Err(DimensionDiagnostic::Backtracking(i + 1));
            }
            previous = projection;
        }
        Ok(points)
    }

    /// Validate persistent syntax only: broken anchors remain preservable.
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.additional.len() <= MAX_DIMENSION_ANCHORS - 2,
            "dimension exceeds 128 anchors",
        )?;
        ensure(
            match self.layout {
                DimensionLayout::Aligned | DimensionLayout::Angular => self.additional.is_empty(),
                DimensionLayout::Chain | DimensionLayout::Baseline => !self.additional.is_empty(),
            },
            "invalid dimension anchor count for layout",
        )?;
        if self.layout == DimensionLayout::Angular {
            ensure(
                self.first.wall != self.second.wall,
                "angular dimension requires distinct walls",
            )?;
            ensure(
                self.offset_m > 1e-6 && self.baseline_spacing_m == 0.25,
                "invalid angular radius or spacing",
            )?;
        }
        ensure(
            self.baseline_spacing_m.is_finite()
                && self.baseline_spacing_m > 0.0
                && self.baseline_spacing_m <= MAX_DIMENSION_METRES,
            "invalid baseline spacing",
        )?;
        let references: Vec<_> = self.references().collect();
        for (i, reference) in references.iter().enumerate() {
            ensure(
                !reference.wall.0.is_nil() && !references[..i].contains(reference),
                "nil or repeated dimension reference",
            )?;
        }
        ensure(!self.view.0.is_nil(), "dimension view ID is nil")?;
        ensure(
            !self.first.wall.0.is_nil() && !self.second.wall.0.is_nil(),
            "dimension wall ID is nil",
        )?;
        ensure(
            self.first != self.second,
            "dimension references must differ",
        )?;
        ensure(
            self.offset_m.is_finite() && self.offset_m.abs() <= MAX_DIMENSION_METRES,
            "dimension offset must be finite and within +/- 1000000 metres",
        )?;
        ensure(
            self.orphan_hint.is_finite()
                && self.orphan_hint.x.abs() <= MAX_DIMENSION_METRES
                && self.orphan_hint.y.abs() <= MAX_DIMENSION_METRES,
            "dimension placement hint must be finite and within +/- 1000000 metres",
        )
    }

    /// Pure model-space resolution. Does not depend on visibility, crop or zoom.
    pub fn resolve(
        &self,
        model: &Model,
    ) -> std::result::Result<ResolvedDimension, DimensionDiagnostic> {
        let points = self.resolve_points(model)?;
        Ok(ResolvedDimension {
            first: points[0],
            second: points[1],
            length_metres: points[0].distance(points[1]),
        })
    }

    pub fn resolve_points(
        &self,
        model: &Model,
    ) -> std::result::Result<Vec<Point2>, DimensionDiagnostic> {
        let view = model
            .views
            .get(&self.view)
            .filter(|v| v.parameters.kind == ViewKind::Plan)
            .ok_or(DimensionDiagnostic::MissingPlanView)?;
        let level = view
            .parameters
            .level
            .ok_or(DimensionDiagnostic::MissingPlanView)?;
        let anchor = |reference: DimensionReference| {
            let wall = model
                .walls
                .get(&reference.wall)
                .ok_or(DimensionDiagnostic::MissingWall)?;
            if wall.parameters.level != level {
                return Err(DimensionDiagnostic::WrongLevel);
            }
            Ok(match reference.endpoint {
                DimensionEndpoint::Start => wall.parameters.start,
                DimensionEndpoint::End => wall.parameters.end,
            })
        };
        self.resolve_anchors_with(anchor)
    }

    pub fn validate_creation(&self, model: &Model) -> Result<()> {
        self.validate()?;
        if self.layout == DimensionLayout::Angular {
            return self.resolve_angular(model).map(|_| ()).map_err(|reason| {
                os_core::Error::Invalid(format!("angular references cannot resolve: {reason:?}"))
            });
        }
        self.resolve(model).map_err(|reason| {
            os_core::Error::Invalid(format!("dimension anchors cannot resolve: {reason:?}"))
        })?;
        Ok(())
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    for dimension in model.dimensions.values() {
        dimension.parameters.validate()?;
        ensure(
            model
                .views
                .get(&dimension.parameters.view)
                .is_some_and(|view| view.parameters.kind == ViewKind::Plan),
            "dimension requires an existing floor-plan view",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{View, ViewParams};

    #[test]
    fn angular_sectors_constraints_and_orphan_recovery() {
        let mut model = Model::new("Angles");
        let level = *model.levels.keys().next().unwrap();
        let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
        let view_id = view.id();
        model.views.insert(view_id, view);
        let make_wall = |end| {
            crate::Wall::new(
                "org.openstructure.walls.wall",
                crate::WallParams {
                    name: "Axis".into(),
                    start: Point2::new(0.0, 0.0),
                    end,
                    thickness: 0.2,
                    height: 3.0,
                    level,
                    material: None,
                },
            )
        };
        let a = make_wall(Point2::new(4.0, 0.0));
        let b = make_wall(Point2::new(3.0, 3.0));
        let (aid, bid) = (a.id(), b.id());
        model.walls.insert(aid, a);
        model.walls.insert(bid, b.clone());
        let mut p = DimensionParams {
            layout: DimensionLayout::Angular,
            additional: vec![],
            baseline_spacing_m: 0.25,
            view: view_id,
            first: DimensionReference {
                wall: aid,
                endpoint: DimensionEndpoint::End,
            },
            second: DimensionReference {
                wall: bid,
                endpoint: DimensionEndpoint::End,
            },
            offset_m: 1.0,
            orphan_hint: Point2::new(1.0, 0.5),
        };
        for (click, degrees) in [
            (Point2::new(2.0, 1.0), 45.0),
            (Point2::new(-1.0, 2.0), 135.0),
            (Point2::new(-2.0, -1.0), 45.0),
            (Point2::new(1.0, -2.0), 135.0),
        ] {
            p.place_angular(&model, click).unwrap();
            let g = p.resolve_angular(&model).unwrap();
            assert!(g.center.distance(Point2::new(0.0, 0.0)) < 1e-12);
            assert!((g.degrees() - degrees).abs() < 1e-10);
            assert!((g.radius - 5.0_f64.sqrt()).abs() < 1e-10);
            assert!(g.contains_placement(click));
        }
        let saved = p.clone();
        for click in [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(f64::NAN, 1.0),
        ] {
            assert!(p.place_angular(&model, click).is_err());
            assert_eq!(p, saved);
        }
        let mut same = p.clone();
        same.second.wall = aid;
        assert!(same.validate_creation(&model).is_err());
        for (end, diagnostic) in [
            (Point2::new(3.0, 0.0), DimensionDiagnostic::ParallelWalls),
            (Point2::new(3.0, 1e-7), DimensionDiagnostic::ParallelWalls),
            (Point2::new(0.0, 0.0), DimensionDiagnostic::InvalidGeometry),
        ] {
            model.walls.get_mut(&bid).unwrap().parameters.end = end;
            assert_eq!(p.resolve_angular(&model), Err(diagnostic));
            assert!(p.validate_creation(&model).is_err());
            p.validate().unwrap();
            model.walls.insert(bid, b.clone());
            assert!(p.resolve_angular(&model).is_ok());
        }
        model.walls.get_mut(&bid).unwrap().parameters.level = Id::new();
        assert_eq!(
            p.resolve_angular(&model),
            Err(DimensionDiagnostic::WrongLevel)
        );
        assert!(p.validate_creation(&model).is_err());
        model.walls.remove(&bid);
        assert_eq!(
            p.resolve_angular(&model),
            Err(DimensionDiagnostic::MissingWall)
        );
        model.walls.insert(bid, b);
        p.validate_creation(&model).unwrap();
        assert_eq!(p, saved);
    }

    #[test]
    fn intent_validation_preserves_missing_anchors_but_rejects_bad_syntax() {
        let mut model = Model::new("Dimensions");
        let view = View::new(
            "core.view",
            ViewParams::floor_plan("Plan", *model.levels.keys().next().unwrap()),
        );
        let dimension = Dimension::new(
            "core.dimension",
            DimensionParams {
                layout: DimensionLayout::Aligned,
                additional: Vec::new(),
                baseline_spacing_m: 0.25,
                view: view.id(),
                first: DimensionReference {
                    wall: Id::new(),
                    endpoint: DimensionEndpoint::Start,
                },
                second: DimensionReference {
                    wall: Id::new(),
                    endpoint: DimensionEndpoint::End,
                },
                offset_m: -2.0,
                orphan_hint: Point2::new(3.0, -2.0),
            },
        );
        model.views.insert(view.id(), view);
        model.dimensions.insert(dimension.id(), dimension.clone());
        model.validate().unwrap();
        assert_eq!(
            dimension.parameters.resolve(&model),
            Err(DimensionDiagnostic::MissingWall)
        );
        for case in 0..10 {
            let mut bad = model.clone();
            let d = bad.dimensions.get_mut(&dimension.id()).unwrap();
            match case {
                0 => d.parameters.first = d.parameters.second,
                1 => d.parameters.offset_m = f64::NAN,
                2 => d.parameters.offset_m = MAX_DIMENSION_METRES + 1.0,
                3 => d.parameters.orphan_hint.x = f64::INFINITY,
                4 => d.parameters.orphan_hint.y = -MAX_DIMENSION_METRES - 1.0,
                5 => d.parameters.view = Id::new(),
                6 => {
                    d.parameters.view = *model
                        .views
                        .iter()
                        .find(|(_, v)| v.parameters.kind == ViewKind::Perspective)
                        .unwrap()
                        .0
                }
                7 => d.header.id = Id::new(),
                8 => d.header.type_id = "wrong.dimension".into(),
                _ => d.header.schema_version = 6,
            }
            assert!(bad.validate().is_err(), "case {case}");
        }
        let mut value = serde_json::to_value(dimension).unwrap();
        assert!(value["parameters"].get("length_metres").is_none());
        value["parameters"]["length_metres"] = 3.0.into();
        assert!(serde_json::from_value::<Dimension>(value).is_err());
    }

    #[test]
    fn chain_and_baseline_anchor_lists_are_collinear_forward_and_orphan_safe() {
        let first = DimensionReference {
            wall: Id::new(),
            endpoint: DimensionEndpoint::Start,
        };
        let second = DimensionReference {
            wall: Id::new(),
            endpoint: DimensionEndpoint::End,
        };
        let third = DimensionReference {
            wall: Id::new(),
            endpoint: DimensionEndpoint::Start,
        };
        for layout in [DimensionLayout::Chain, DimensionLayout::Baseline] {
            let params = DimensionParams {
                layout,
                additional: vec![third],
                baseline_spacing_m: 0.25,
                view: Id::new(),
                first,
                second,
                offset_m: 0.5,
                orphan_hint: Point2::new(0.0, 0.5),
            };
            let resolve = |last: std::result::Result<Point2, DimensionDiagnostic>| {
                params.resolve_anchors_with(|reference| {
                    if reference == first {
                        Ok(Point2::new(0.0, 0.0))
                    } else if reference == second {
                        Ok(Point2::new(4.0, 0.0))
                    } else {
                        last
                    }
                })
            };
            assert_eq!(
                resolve(Ok(Point2::new(5.0, 0.0))).unwrap(),
                vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(4.0, 0.0),
                    Point2::new(5.0, 0.0)
                ]
            );
            assert_eq!(
                resolve(Ok(Point2::new(5.0, 2e-6))),
                Err(DimensionDiagnostic::OffAxis(3))
            );
            assert_eq!(
                resolve(Ok(Point2::new(3.0, 0.0))),
                Err(DimensionDiagnostic::Backtracking(3))
            );
            assert_eq!(
                resolve(Err(DimensionDiagnostic::MissingWall)),
                Err(DimensionDiagnostic::MissingWallAt(3))
            );

            let mut repeated = params.clone();
            repeated.additional[0] = second;
            assert!(repeated.validate().is_err());
        }
    }
}
