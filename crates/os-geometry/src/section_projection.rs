//! Remove cut material from projected vectors before drawing and hit testing.
use super::SECTION_TOLERANCE as T;
use crate::elevation::{ElevationSegment, MAX_ELEVATION_SEGMENTS};
use os_core::{Point2, Result, ensure};

/// Subtract the union of opaque cut triangles. Triangle boundaries introduce
/// endpoints only, never edges. Holes remain visible because they have no mask.
pub fn mask_projected_segments(
    source: Vec<ElevationSegment>,
    masks: &[[Point2; 3]],
) -> Result<Vec<ElevationSegment>> {
    ensure(
        masks.len() <= super::MAX_SECTION_FILL_TRIANGLES && source.len() <= MAX_ELEVATION_SEGMENTS,
        "section projection mask input budget exceeded",
    )?;
    ensure(
        masks.iter().all(|t| {
            t.iter().all(|p| p.is_finite())
                && ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[1].y - t[0].y) * (t[2].x - t[0].x))
                    .abs()
                    > 0.
        }),
        "invalid section projection mask triangle",
    )?;
    ensure(
        source.iter().all(|s| {
            s.start.is_finite()
                && s.end.is_finite()
                && s.start.distance(s.end).is_finite()
                && s.start.distance(s.end) > T
        }),
        "invalid section projection source segment",
    )?;
    let mut result = Vec::new();
    let mut work = 0usize;
    for segment in source {
        let a = segment.start;
        let b = segment.end;
        let length = a.distance(b);
        let mut hidden = Vec::new();
        for triangle in masks {
            work += 1;
            ensure(
                work <= 40_000_000,
                "section projection mask work budget exceeded",
            )?;
            let cross = |a: Point2, b: Point2, c: Point2| {
                (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
            };
            let sign = cross(triangle[0], triangle[1], triangle[2]).signum();
            let mut lo = 0.0_f64;
            let mut hi = 1.0_f64;
            for i in 0..3 {
                let p = triangle[i];
                let q = triangle[(i + 1) % 3];
                let da = sign * cross(p, q, a);
                let db = sign * cross(p, q, b);
                if da < 0.0 && db < 0.0 {
                    hi = -1.0;
                    break;
                }
                if da < 0.0 {
                    lo = lo.max(da / (da - db));
                }
                if db < 0.0 {
                    hi = hi.min(da / (da - db));
                }
            }
            if hi > lo {
                hidden.push((lo, hi));
            }
        }
        hidden.sort_by(|a, b| a.0.total_cmp(&b.0));
        let at = |t| Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        let mut cursor = 0.0_f64;
        for (lo, hi) in hidden.into_iter().chain(std::iter::once((1.0, 1.0))) {
            if (lo - cursor) * length > T {
                ensure(
                    result.len() < MAX_ELEVATION_SEGMENTS,
                    "section projected output budget exceeded",
                )?;
                result.push(ElevationSegment {
                    start: at(cursor),
                    end: at(lo),
                    ..segment
                });
            }
            cursor = cursor.max(hi);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_union_without_erasing_holes_or_adding_edges() {
        let p = Point2::new;
        let source = ElevationSegment {
            entity: os_core::Id::new(),
            start: p(-1., 0.5),
            end: p(4., 0.5),
            surface: Default::default(),
        };
        let square = |x| {
            [
                [p(x, 0.), p(x + 1., 0.), p(x + 1., 1.)],
                [p(x, 0.), p(x + 1., 1.), p(x, 1.)],
            ]
        };
        let masks = [square(0.), square(2.)].concat();
        let result = mask_projected_segments(vec![source], &masks).unwrap();
        assert_eq!(
            result.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>(),
            vec![
                (p(-1., 0.5), p(0., 0.5)),
                (p(1., 0.5), p(2., 0.5)),
                (p(3., 0.5), p(4., 0.5))
            ]
        );
        assert!(
            result
                .iter()
                .all(|s| s.entity == source.entity && s.surface == source.surface)
        );
    }
}
