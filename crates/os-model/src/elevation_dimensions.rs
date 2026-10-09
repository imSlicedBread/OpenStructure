//! Elevation coordinates never pass through the floor-plan XY resolver.
use crate::{
    DimensionDiagnostic as D, DimensionLayout, DimensionParams, DimensionReference,
    ElevationDimensionFeature as F, Model, ResolvedDimension, ViewKind,
};
use os_core::{Id, Point2};

impl DimensionReference {
    /// Representative point on a nominal reference edge (not a profile contour).
    pub fn resolve_elevation(self, model: &Model, host: Id) -> Result<Point2, D> {
        let wall = model.walls.get(&host).ok_or(D::MissingWall)?;
        if !wall.parameters.path.is_straight() {
            return Err(D::InvalidGeometry);
        }
        let w = model
            .resolve_wall(host)
            .map_err(|_| D::InvalidGeometry)?
            .parameters;
        let z = model
            .levels
            .get(&w.level)
            .ok_or(D::WrongLevel)?
            .parameters
            .elevation;
        let (left, right, bottom, top, feature) = match self {
            Self::ElevationWall { wall, feature } => {
                if wall != host {
                    return Err(D::WrongHost);
                }
                (0.0, w.length(), z, z + w.height, feature)
            }
            Self::ElevationOpening { opening, feature } => {
                let p = &model
                    .openings
                    .get(&opening)
                    .ok_or(D::MissingOpening)?
                    .parameters;
                if p.host != host {
                    return Err(D::WrongHost);
                }
                let o = model.resolve_opening(p).map_err(|_| D::InvalidGeometry)?;
                o.validate_host(&w).map_err(|_| D::InvalidGeometry)?;
                (
                    o.offset,
                    o.offset + o.width,
                    z + o.sill,
                    z + o.sill + o.height,
                    feature,
                )
            }
            _ => return Err(D::InvalidGeometry),
        };
        let p = match feature {
            F::Start => Point2::new(left, (bottom + top) * 0.5),
            F::End => Point2::new(right, (bottom + top) * 0.5),
            F::Bottom => Point2::new((left + right) * 0.5, bottom),
            F::Top => Point2::new((left + right) * 0.5, top),
        };
        if !p.is_finite() || p.x.abs() > 1e6 || p.y.abs() > 1e6 {
            return Err(D::InvalidGeometry);
        }
        Ok(p)
    }
}

impl DimensionParams {
    pub fn resolve_elevation(&self, model: &Model) -> Result<ResolvedDimension, D> {
        if !self.layout.is_elevation() || self.validate().is_err() {
            return Err(D::InvalidGeometry);
        }
        let settings = model
            .views
            .get(&self.view)
            .filter(|v| v.parameters.kind == ViewKind::Elevation)
            .and_then(|v| v.parameters.elevation)
            .ok_or(D::MissingElevationView)?;
        let first = self.first.resolve_elevation(model, settings.host)?;
        let second = self.second.resolve_elevation(model, settings.host)?;
        let length_metres = if self.layout == DimensionLayout::ElevationHorizontal {
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

    /// Dimension-line endpoints, retaining the original points for witnesses.
    pub fn elevation_line(&self, resolved: ResolvedDimension) -> (Point2, Point2) {
        let (mut a, mut b) = (resolved.first, resolved.second);
        if self.layout == DimensionLayout::ElevationHorizontal {
            a.y = (a.y + b.y) * 0.5 + self.offset_m;
            b.y = a.y;
        } else {
            a.x = (a.x + b.x) * 0.5 + self.offset_m;
            b.x = a.x;
        }
        (a, b)
    }
}
