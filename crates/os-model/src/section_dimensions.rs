//! Semantic nominal extents in a section's station / absolute-elevation frame.
//! No tessellation IDs are persisted. Depth clipping is analytic; crop never
//! changes the measured value. Start/End mean minimum/maximum section station.
use crate::{
    DimensionDiagnostic as D, DimensionLayout as L, DimensionParams, DimensionReference as R,
    ElevationDimensionFeature as F, Model, ResolvedDimension, SectionDisplayMode, ViewKind,
};
use os_core::{Id, Point2};

impl R {
    /// Nominal rectangular wall or aperture bounds intersected with the section
    /// depth volume. The opening reference follows its UUID and current host.
    pub fn section_bounds(self, model: &Model, view: Id) -> Result<[f64; 4], D> {
        let v = model
            .views
            .get(&view)
            .filter(|v| v.parameters.kind == ViewKind::Section)
            .ok_or(D::MissingSectionView)?;
        let s = v.parameters.section.ok_or(D::MissingSectionView)?;
        s.validate().map_err(|_| D::InvalidGeometry)?;
        let (host, opening) = match self {
            R::SectionWall { wall, .. } => (wall, None),
            R::SectionOpening { opening, .. } => {
                let p = &model
                    .openings
                    .get(&opening)
                    .ok_or(D::MissingOpening)?
                    .parameters;
                (p.host, Some(p))
            }
            _ => return Err(D::InvalidGeometry),
        };
        let w = model.walls.get(&host).ok_or(if opening.is_some() {
            D::MissingOpeningHost
        } else {
            D::MissingWall
        })?;
        if !w.parameters.path.is_straight() {
            return Err(D::InvalidGeometry);
        }
        let w = model
            .resolve_wall(host)
            .map_err(|_| D::InvalidGeometry)?
            .parameters;
        let level = model.levels.get(&w.level).ok_or(D::WrongLevel)?;
        if v.parameters
            .level
            .and_then(|id| model.levels.get(&id))
            .is_none_or(|l| l.parameters.building != level.parameters.building)
        {
            return Err(D::WrongLevel);
        }
        let z = level.parameters.elevation;
        let (lo, hi, bottom, top) = if let Some(p) = opening {
            if let crate::OpeningDefinition::Typed { type_id } = p.definition
                && !model
                    .opening_types
                    .get(&type_id)
                    .ok_or(D::InvalidGeometry)?
                    .parameters
                    .family
                    .rectangular_cut()
            {
                return Err(D::InvalidGeometry);
            }
            let o = model.resolve_opening(p).map_err(|_| D::InvalidGeometry)?;
            o.validate_host(&w).map_err(|_| D::InvalidGeometry)?;
            (
                o.offset,
                o.offset + o.width,
                z + o.sill,
                z + o.sill + o.height,
            )
        } else {
            (0., w.length(), z, z + w.height)
        };
        let length = s.start.distance(s.end);
        let axis = Point2::new(
            (s.end.x - s.start.x) / length,
            (s.end.y - s.start.y) / length,
        );
        let depth_point = s.world_point(0., 1.);
        let normal = Point2::new(depth_point.x - s.start.x, depth_point.y - s.start.y);
        let tangent = w.path.tangent(0.);
        let mut polygon = Vec::new();
        for (station, sign) in [(lo, -1.), (hi, -1.), (hi, 1.), (lo, 1.)] {
            let p = w.path.point(station);
            let dx = p.x - tangent.y * w.thickness * 0.5 * sign - s.start.x;
            let dy = p.y + tangent.x * w.thickness * 0.5 * sign - s.start.y;
            polygon.push(Point2::new(
                dx * axis.x + dy * axis.y,
                dx * normal.x + dy * normal.y,
            ));
        }
        // Clip a convex analytic footprint to near/far depth half-planes.
        // CutOnly uses the plane itself, including crossing segment endpoints.
        let far = if s.mode == SectionDisplayMode::CutOnly {
            0.
        } else {
            s.far_depth
        };
        for (bound, sign) in [(0., 1.), (far, -1.)] {
            let input = std::mem::take(&mut polygon);
            if input.is_empty() {
                return Err(D::OutsideSection);
            }
            for i in 0..input.len() {
                let a = input[i];
                let b = input[(i + 1) % input.len()];
                let da = (a.y - bound) * sign;
                let db = (b.y - bound) * sign;
                if da >= 0. {
                    polygon.push(a);
                }
                if (da >= 0.) != (db >= 0.) {
                    let t = da / (da - db);
                    polygon.push(Point2::new(a.x + t * (b.x - a.x), bound));
                }
            }
        }
        if polygon.is_empty() {
            return Err(D::OutsideSection);
        }
        let left = polygon.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let right = polygon
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max);
        if [left, right, bottom, top]
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 1e6)
            || right - left <= 1e-6
            || top - bottom <= 1e-6
        {
            return Err(D::InvalidGeometry);
        }
        if right < s.min_station
            || left > s.max_station
            || top < s.bottom_elevation
            || bottom > s.top_elevation
        {
            return Err(D::OutsideSection);
        }
        Ok([left, right, bottom, top])
    }

    pub fn resolve_section(self, model: &Model, view: Id) -> Result<Point2, D> {
        let [left, right, bottom, top] = self.section_bounds(model, view)?;
        let feature = match self {
            R::SectionWall { feature, .. } | R::SectionOpening { feature, .. } => feature,
            _ => return Err(D::InvalidGeometry),
        };
        Ok(match feature {
            F::Start => Point2::new(left, (bottom + top) * 0.5),
            F::End => Point2::new(right, (bottom + top) * 0.5),
            F::Bottom => Point2::new((left + right) * 0.5, bottom),
            F::Top => Point2::new((left + right) * 0.5, top),
        })
    }
}

impl DimensionParams {
    pub fn resolve_section(&self, model: &Model) -> Result<ResolvedDimension, D> {
        if !self.layout.is_section() || self.validate().is_err() {
            return Err(D::InvalidGeometry);
        }
        let first = self.first.resolve_section(model, self.view)?;
        let second = self.second.resolve_section(model, self.view)?;
        let length_metres = if self.layout == L::SectionHorizontal {
            (second.x - first.x).abs()
        } else {
            (second.y - first.y).abs()
        };
        if length_metres <= 1e-6 {
            return Err(D::CoincidentAnchors);
        }
        Ok(ResolvedDimension {
            first,
            second,
            length_metres,
        })
    }

    pub fn section_line(&self, r: ResolvedDimension) -> (Point2, Point2) {
        let (mut a, mut b) = (r.first, r.second);
        if self.layout == L::SectionHorizontal {
            a.y = (a.y + b.y) * 0.5 + self.offset_m;
            b.y = a.y;
        } else {
            a.x = (a.x + b.x) * 0.5 + self.offset_m;
            b.x = a.x;
        }
        (a, b)
    }
}
