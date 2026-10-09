//! Deterministic, bounded even-odd fill of a single solid's section contours.
use super::{
    CutContour, CutContourKind, MAX_SECTION_CONTOURS, MAX_SECTION_COORDINATE, MAX_SECTION_SEGMENTS,
    SECTION_TOLERANCE, compare_points, contains, cross, invalid, segments_touch,
};
use os_core::{Point2, Result, ensure};

/// Hard output cap, including all disconnected regions, holes, and islands.
pub const MAX_SECTION_FILL_TRIANGLES: usize = 50_000;

#[derive(Clone, Copy)]
struct Edge {
    a: Point2,
    b: Point2,
    ring: usize,
    vertex: usize,
}

#[derive(Clone, Copy)]
struct Crossing {
    left: f64,
    right: f64,
    edge: Edge,
}

// Sum both halves of each product: ordinary shoelace sums can erase a thin
// ring completely at large coordinates, even after compensated summation.
#[derive(Clone, Copy, Default)]
struct AreaSum {
    hi: f64,
    lo: f64,
}

impl AreaSum {
    fn add(&mut self, value: f64) {
        let sum = self.hi + value;
        let tail = sum - self.hi;
        let error = (self.hi - (sum - tail)) + (value - tail);
        let low = self.lo + error;
        self.hi = sum + low;
        self.lo = low - (self.hi - sum);
    }

    fn product(&mut self, a: f64, b: f64) {
        let product = a * b;
        self.add(product);
        self.add(a.mul_add(b, -product));
    }

    fn edge(&mut self, a: Point2, b: Point2) {
        self.product(a.x, b.y);
        self.product(-a.y, b.x);
    }

    fn area(self) -> f64 {
        (self.hi + self.lo) * 0.5
    }
}

fn buffer<T>(count: usize) -> Result<Vec<T>> {
    ensure(
        count
            .checked_mul(std::mem::size_of::<T>())
            .is_some_and(|bytes| bytes <= isize::MAX as usize),
        "section fill allocation overflow",
    )?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| invalid("section fill allocation failed"))?;
    Ok(result)
}

/// Fill the contours of one solid with CCW triangles using even-odd material
/// semantics. Call separately for independent solids: nesting across solids
/// must not turn one solid into another solid's hole.
///
/// Validates finite coordinates, ring/vertex bounds, winding, declared nesting,
/// and the kernel's `SECTION_TOLERANCE` topology rules before producing output.
/// X may reach four times `MAX_SECTION_COORDINATE` because section distances
/// include a translated and rotated world coordinate; Y retains the world
/// envelope. Contour order and cyclic choice of first vertex do not matter.
///
/// Every distinct vertex X defines a slab; no narrow slab or positive triangle
/// is discarded by an area tolerance. Output is checked against the contour
/// area with a floating-point interpolation error allowance. Invalid topology,
/// numerically collapsed material, allocation failure, or exceeding the smaller
/// of `triangle_budget` and `MAX_SECTION_FILL_TRIANGLES` returns `Error::Invalid`
/// atomically. Empty input succeeds even with a zero budget.
///
/// With V capped by `MAX_SECTION_SEGMENTS`, work is O(V² log V), and working
/// storage is O(V + output). Output can have T-junctions at slab boundaries;
/// it is intended for planar rendering, not a welded manifold surface.
pub fn tessellate_contours(
    contours: &[CutContour],
    triangle_budget: usize,
) -> Result<Vec<[Point2; 3]>> {
    ensure(
        contours.len() <= MAX_SECTION_CONTOURS,
        "section fill contour limit exceeded",
    )?;
    let count = contours.iter().try_fold(0usize, |count, contour| {
        ensure(
            contour.points.len() >= 3,
            "section fill ring needs three points",
        )?;
        let count = count
            .checked_add(contour.points.len())
            .ok_or_else(|| invalid("section fill vertex count overflow"))?;
        ensure(
            count <= MAX_SECTION_SEGMENTS,
            "section fill vertex limit exceeded",
        )?;
        Ok(count)
    })?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut edges = buffer::<Edge>(count)?;
    let mut xs = buffer::<f64>(count)?;
    let mut expected_area = AreaSum::default();
    let mut max_y = 0.0_f64;
    for (ring, contour) in contours.iter().enumerate() {
        let outer = contour.nesting_depth & 1 == 0;
        ensure(
            contour.nesting_depth < contours.len()
                && (contour.kind == CutContourKind::Outer) == outer,
            "section fill kind/depth mismatch",
        )?;
        ensure(
            contour.points.iter().all(|p| {
                p.is_finite()
                    && p.x.abs() <= 4.0 * MAX_SECTION_COORDINATE
                    && p.y.abs() <= MAX_SECTION_COORDINATE
            }),
            "section fill outside finite coordinate envelope",
        )?;
        let mut ring_area = AreaSum::default();
        for (vertex, &a) in contour.points.iter().enumerate() {
            let b = contour.points[(vertex + 1) % contour.points.len()];
            ensure(a.distance(b) > SECTION_TOLERANCE, "section fill short edge")?;
            ring_area.edge(a, b);
            expected_area.edge(a, b);
            max_y = max_y.max(a.y.abs());
            // Normalize signed zero and edge direction for order-independent
            // interpolation (including after a cyclic ring rotation).
            xs.push(if a.x == 0.0 { 0.0 } else { a.x });
            let (a, b) = if compare_points(a, b).is_gt() {
                (b, a)
            } else {
                (a, b)
            };
            edges.push(Edge { a, b, ring, vertex });
        }
        let area = ring_area.area();
        ensure(
            area.abs() > SECTION_TOLERANCE.powi(2) && (area > 0.0) == outer,
            "section fill negligible area or invalid winding",
        )?;
    }
    validate_topology(contours, &edges)?;
    ensure(
        expected_area.area() > 0.0,
        "section fill has no material area",
    )?;

    xs.sort_unstable_by(f64::total_cmp);
    xs.dedup();
    let budget = triangle_budget.min(MAX_SECTION_FILL_TRIANGLES);
    // Check the number of output vertices as well as the array allocation.
    budget
        .checked_mul(3)
        .ok_or_else(|| invalid("section fill output vertex overflow"))?;
    // Small, independent components are common (frames, mullions, railing
    // members). Reserve only as this particular cut produces triangles.
    let mut triangles = Vec::new();
    let mut crossings = buffer::<Crossing>(count)?;
    let mut actual_area = AreaSum::default();
    let mut interpolation_width = 0.0;
    for slab in xs.windows(2) {
        let (left, right) = (slab[0], slab[1]);
        crossings.clear();
        for edge in &edges {
            if edge.a.x <= left && edge.b.x >= right {
                crossings.push(Crossing {
                    left: edge_y(*edge, left),
                    right: edge_y(*edge, right),
                    edge: *edge,
                });
            }
        }
        // The left endpoint orders non-intersecting edges; the right endpoint
        // breaks ties where two edges meet at a slab boundary. No rounded
        // midpoint is needed, even when consecutive X values are adjacent f64s.
        crossings
            .sort_unstable_by(|a, b| a.left.total_cmp(&b.left).then(a.right.total_cmp(&b.right)));
        ensure(
            crossings.len() & 1 == 0,
            "section fill odd slab crossing count",
        )?;
        for pair in crossings.chunks_exact(2) {
            let (bottom, top) = (pair[0], pair[1]);
            ensure(
                top.left >= bottom.left && top.right >= bottom.right,
                "section fill slab edges cross",
            )?;
            ensure(
                top.left > bottom.left || top.right > bottom.right,
                "section fill material collapsed at floating-point precision",
            )?;
            let bl = Point2::new(left, bottom.left);
            let br = Point2::new(right, bottom.right);
            let tl = Point2::new(left, top.left);
            let tr = Point2::new(right, top.right);
            // Each triangle starts with a vertical edge. Its cross product
            // cannot cancel two large oblique products for thin material.
            for (x, height, triangle) in [
                (right, top.right - bottom.right, [br, tr, bl]),
                (left, top.left - bottom.left, [tl, bl, tr]),
            ] {
                if height == 0.0 {
                    // Only a shared source vertex is an exact triangular tip.
                    // Interpolation must not silently erase a positive gap.
                    ensure(
                        [top.edge.a, top.edge.b]
                            .iter()
                            .any(|p| p.x == x && (*p == bottom.edge.a || *p == bottom.edge.b)),
                        "section fill material collapsed at floating-point precision",
                    )?;
                    continue;
                }
                ensure(
                    triangles.len() < budget,
                    "section fill triangle budget exceeded",
                )?;
                let area = cross(triangle[0], triangle[1], triangle[2]) * 0.5;
                ensure(
                    area > 0.0 && area.is_finite(),
                    "section fill triangle collapsed",
                )?;
                actual_area.product(right - left, height);
                if triangles.len() == triangles.capacity() {
                    let next_capacity = triangles
                        .len()
                        .checked_mul(2)
                        .ok_or_else(|| invalid("section fill capacity overflow"))?
                        .max(1)
                        .min(budget);
                    ensure(
                        next_capacity
                            .checked_mul(3)
                            .and_then(|vertices| {
                                vertices.checked_mul(std::mem::size_of::<Point2>())
                            })
                            .is_some_and(|bytes| bytes <= isize::MAX as usize),
                        "section fill output allocation overflow",
                    )?;
                    let additional = next_capacity - triangles.len();
                    triangles
                        .try_reserve_exact(additional)
                        .map_err(|_| invalid("section fill allocation failed"))?;
                }
                triangles.push(triangle);
            }
            interpolation_width += right - left;
        }
    }
    // Includes interpolation/subtraction rounding at each trapezoid endpoint.
    // The compensated contour sum retains thin outer-minus-hole material even
    // when that difference is far smaller than the two individual ring areas.
    let expected = expected_area.area();
    let actual = actual_area.area();
    let allowance =
        32.0 * f64::EPSILON * (max_y * interpolation_width + expected.abs() + actual.abs());
    ensure(
        actual > 0.0 && (actual - expected).abs() <= allowance,
        "section fill area mismatch",
    )?;
    Ok(triangles)
}

fn edge_y(edge: Edge, x: f64) -> f64 {
    // Endpoint identity matters both for tips and for precision near the
    // coordinate envelope. Interpolate from whichever endpoint is closer.
    if x == edge.a.x {
        edge.a.y
    } else if x == edge.b.x {
        edge.b.y
    } else if x - edge.a.x <= edge.b.x - x {
        ((x - edge.a.x) / (edge.b.x - edge.a.x)).mul_add(edge.b.y - edge.a.y, edge.a.y)
    } else {
        ((edge.b.x - x) / (edge.b.x - edge.a.x)).mul_add(edge.a.y - edge.b.y, edge.b.y)
    }
}

fn validate_topology(contours: &[CutContour], edges: &[Edge]) -> Result<()> {
    for (i, a) in edges.iter().enumerate() {
        for b in &edges[i + 1..] {
            let ring_len = contours[a.ring].points.len();
            let adjacent = a.ring == b.ring
                && ((a.vertex + 1) % ring_len == b.vertex || (b.vertex + 1) % ring_len == a.vertex);
            if adjacent {
                let (p, q, r) = if a.a == b.a {
                    (a.a, a.b, b.b)
                } else if a.a == b.b {
                    (a.a, a.b, b.a)
                } else if a.b == b.a {
                    (a.b, a.a, b.b)
                } else {
                    (a.b, a.a, b.a)
                };
                let dot = (q.x - p.x) * (r.x - p.x) + (q.y - p.y) * (r.y - p.y);
                ensure(
                    dot <= 0.0
                        || cross(p, q, r).abs()
                            > SECTION_TOLERANCE * p.distance(q).max(p.distance(r)),
                    "overlapping adjacent section segments",
                )?;
            } else {
                ensure(
                    !segments_touch(a.a, a.b, b.a, b.b),
                    "intersecting or touching section contours",
                )?;
            }
        }
    }
    for (i, contour) in contours.iter().enumerate() {
        let depth = contours
            .iter()
            .enumerate()
            .filter(|(j, other)| *j != i && contains(&other.points, contour.points[0]))
            .count();
        ensure(
            depth == contour.nesting_depth,
            "section fill incorrect nesting depth",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(points: &[(f64, f64)], depth: usize) -> CutContour {
        let mut points: Vec<_> = points.iter().map(|&(x, y)| Point2::new(x, y)).collect();
        if depth % 2 == 1 {
            points.reverse();
        }
        CutContour {
            points,
            kind: if depth & 1 == 0 {
                CutContourKind::Outer
            } else {
                CutContourKind::Hole
            },
            nesting_depth: depth,
        }
    }

    fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64, depth: usize) -> CutContour {
        ring(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], depth)
    }

    fn area(triangles: &[[Point2; 3]]) -> f64 {
        triangles
            .iter()
            .map(|t| {
                let area = cross(t[0], t[1], t[2]) * 0.5;
                assert!(area > 0.0 && area.is_finite());
                area
            })
            .sum()
    }

    fn covered(triangles: &[[Point2; 3]], point: Point2) -> bool {
        triangles.iter().any(|t| {
            cross(t[0], t[1], point) >= 0.0
                && cross(t[1], t[2], point) >= 0.0
                && cross(t[2], t[0], point) >= 0.0
        })
    }

    #[test]
    fn empty_and_exact_triangle_budgets() {
        assert!(tessellate_contours(&[], 0).unwrap().is_empty());
        let contours = [rectangle(0., 0., 2., 3., 0)];
        for budget in [0, 1] {
            assert!(
                tessellate_contours(&contours, budget)
                    .unwrap_err()
                    .to_string()
                    .contains("budget")
            );
        }
        let triangles = tessellate_contours(&contours, 2).unwrap();
        assert_eq!(triangles.len(), 2);
        assert!(triangles.capacity() <= 32, "small component over-reserved");
        assert_eq!(area(&triangles), 6.);
        assert_eq!(
            tessellate_contours(&contours, usize::MAX).unwrap(),
            triangles
        );
        let triangle = ring(&[(0., 0.), (2., 0.), (0., 3.)], 0);
        assert_eq!(area(&tessellate_contours(&[triangle], 1).unwrap()), 3.);
    }

    #[test]
    fn large_budgets_keep_tiny_component_capacity_proportional() {
        for budget in [MAX_SECTION_FILL_TRIANGLES, usize::MAX] {
            let empty = tessellate_contours(&[], budget).unwrap();
            assert_eq!(empty.capacity(), 0);
            for (contour, count) in [
                (ring(&[(0., 0.), (2., 0.), (0., 3.)], 0), 1),
                (rectangle(0., 0., 2., 3., 0), 2),
            ] {
                let triangles = tessellate_contours(&[contour], budget).unwrap();
                assert_eq!(triangles.len(), count);
                assert_eq!(triangles.capacity(), count);
            }
        }
    }

    #[test]
    fn holes_nested_islands_and_multiple_outer_rings_have_analytic_area() {
        let contours = [
            rectangle(0., 0., 10., 10., 0),
            rectangle(1., 1., 9., 9., 1),
            rectangle(2., 2., 8., 8., 2),
            rectangle(3., 3., 7., 7., 3),
            rectangle(20., 0., 23., 4., 0),
        ];
        let triangles = tessellate_contours(&contours, 100).unwrap();
        assert_eq!(area(&triangles), 100. - 64. + 36. - 16. + 12.);
        for (x, y) in [(1.5, 4.5), (8.5, 3.5), (4.5, 4.5), (15., 2.)] {
            assert!(!covered(&triangles, Point2::new(x, y)));
        }
        for (x, y) in [(0.5, 4.5), (2.5, 4.5), (7.5, 4.5), (21., 2.)] {
            assert!(covered(&triangles, Point2::new(x, y)));
        }
    }

    #[test]
    fn concave_rings_and_holes_leave_all_void_samples_empty() {
        let contours = [
            ring(
                &[
                    (0., 0.),
                    (6., 0.),
                    (6., 6.),
                    (4., 6.),
                    (4., 2.),
                    (2., 2.),
                    (2., 6.),
                    (0., 6.),
                ],
                0,
            ),
            rectangle(0.5, 0.5, 1.5, 1.5, 1),
        ];
        let triangles = tessellate_contours(&contours, 100).unwrap();
        assert_eq!(area(&triangles), 27.);
        assert!(!covered(&triangles, Point2::new(3., 4.)));
        assert!(!covered(&triangles, Point2::new(1., 1.)));
        assert!(covered(&triangles, Point2::new(3., 1.)));
    }

    #[test]
    fn contour_order_and_cyclic_vertex_order_are_deterministic() {
        let mut contours = vec![
            rectangle(0., 0., 10., 10., 0),
            ring(&[(1., 1.), (8., 2.), (7., 8.), (2., 7.)], 1),
            rectangle(3., 3., 5., 5., 2),
            ring(&[(15., 1.), (18., 4.), (16., 6.), (14., 4.)], 0),
        ];
        let expected = tessellate_contours(&contours, 100).unwrap();
        contours.reverse();
        for contour in &mut contours {
            contour.points.rotate_left(1);
        }
        assert_eq!(tessellate_contours(&contours, 100).unwrap(), expected);
    }

    #[test]
    fn thin_valid_material_and_narrow_slabs_are_never_tolerance_filtered() {
        let thin = rectangle(0., 0., 2., 2. * SECTION_TOLERANCE, 0);
        let triangles = tessellate_contours(&[thin], 2).unwrap();
        assert_eq!(area(&triangles), 4. * SECTION_TOLERANCE);
        // Unrelated rings can introduce slab widths far below the topology
        // tolerance. Their X coordinates must remain distinct.
        let contours = [
            rectangle(0., 0., 2., 1., 0),
            rectangle(1e-12, 2., 2. + 1e-12, 3., 0),
        ];
        let triangles = tessellate_contours(&contours, 8).unwrap();
        assert_eq!(triangles.len(), 8);
        assert_eq!(area(&triangles), 4.);
        assert!(triangles.iter().any(|t| cross(t[0], t[1], t[2]) < 1e-10));
    }

    #[test]
    fn precision_at_coordinate_envelope_retains_thin_sloped_regions() {
        let base = 999_990.;
        let height = 4. * SECTION_TOLERANCE;
        let contour = ring(
            &[
                (base, base),
                (base + 5., base + 5.),
                (base + 5., base + 5. + height),
                (base, base + height),
            ],
            0,
        );
        let actual_height = (base + height) - base;
        let triangles = tessellate_contours(&[contour], 2).unwrap();
        assert_eq!(area(&triangles), 5. * actual_height);
        // A rotated/translated kernel section X can exceed the world envelope.
        assert!(
            tessellate_contours(&[rectangle(2e6, base, 2e6 + 2., base + height, 0)], 2).is_ok()
        );
    }

    #[test]
    fn large_outer_minus_hole_area_does_not_cancel_thin_frame() {
        let low = -999_999.;
        let high = 999_999.;
        let inner_low = low + 4. * SECTION_TOLERANCE;
        let inner_high = high - 4. * SECTION_TOLERANCE;
        let contours = [
            rectangle(low, low, high, high, 0),
            rectangle(inner_low, inner_low, inner_high, inner_high, 1),
        ];
        let triangles = tessellate_contours(&contours, 8).unwrap();
        let width = high - low;
        let inset = inner_low - low;
        let expected = 4. * inset * (width - inset);
        assert!((area(&triangles) - expected).abs() < expected * 1e-14);
        assert!(!covered(&triangles, Point2::new(0., 0.)));
    }

    #[test]
    fn adjacent_floating_point_x_coordinates_remain_separate_slabs() {
        let x: f64 = 999_990.;
        let next = f64::from_bits(x.to_bits() + 1);
        let contours = [
            rectangle(x, 0., x + 2., 1., 0),
            rectangle(next, 2., x + 3., 3., 0),
        ];
        let triangles = tessellate_contours(&contours, 8).unwrap();
        assert_eq!(triangles.len(), 8);
        assert_eq!(area(&triangles), 2. + (x + 3. - next));
    }

    #[test]
    fn invalid_metadata_coordinates_and_ring_topology_fail_atomically() {
        let valid = rectangle(0., 0., 10., 10., 0);
        let mut bad_kind = valid.clone();
        bad_kind.kind = CutContourKind::Hole;
        let mut bad_depth = valid.clone();
        bad_depth.nesting_depth = usize::MAX;
        let mut reversed = valid.clone();
        reversed.points.reverse();
        let mut repeated = valid.clone();
        repeated.points.push(repeated.points[0]);
        let mut cases = vec![
            bad_kind,
            bad_depth,
            reversed,
            repeated,
            ring(&[(0., 0.), (1., 1.)], 0),
            ring(&[(0., 0.), (2., 2.), (0., 2.), (2., 0.)], 0),
            ring(&[(0., 0.), (3., 0.), (1., 0.), (1., 2.), (0., 2.)], 0),
            rectangle(0., 0., 2., SECTION_TOLERANCE, 0),
        ];
        for value in [f64::NAN, f64::INFINITY, 4. * MAX_SECTION_COORDINATE + 1.] {
            let mut contour = valid.clone();
            contour.points[0].x = value;
            cases.push(contour);
        }
        for contour in cases {
            assert!(tessellate_contours(&[contour], 100).is_err());
        }
        for second in [
            rectangle(1., 1., 3., 3., 0),   // incorrect geometric depth
            rectangle(10., 1., 12., 3., 0), // touching
            rectangle(9., 1., 12., 3., 0),  // overlapping
            valid.clone(),
        ] {
            assert!(tessellate_contours(&[valid.clone(), second], 100).is_err());
        }
        assert!(tessellate_contours(&[rectangle(0., 0., 3., 3., 1)], 100).is_err());
    }

    #[test]
    fn input_and_hard_output_caps_are_enforced() {
        let valid = rectangle(0., 0., 1., 1., 0);
        assert!(
            tessellate_contours(&vec![valid.clone(); MAX_SECTION_CONTOURS + 1], 100)
                .unwrap_err()
                .to_string()
                .contains("contour limit")
        );
        let mut oversized = valid;
        oversized
            .points
            .resize(MAX_SECTION_SEGMENTS + 1, Point2::new(0., 0.));
        assert!(
            tessellate_contours(&[oversized], 100)
                .unwrap_err()
                .to_string()
                .contains("vertex limit")
        );
        // Staggered X ranges produce 2*N² triangles from only 4*N vertices.
        let contours: Vec<_> = (0..160)
            .map(|i| {
                let x = f64::from(i) * 0.001;
                let y = f64::from(i) * 0.02;
                rectangle(x, y, 10. + x, y + 0.01, 0)
            })
            .collect();
        let triangles = tessellate_contours(&contours[..158], MAX_SECTION_FILL_TRIANGLES).unwrap();
        assert_eq!(triangles.len(), 2 * 158 * 158);
        assert!(
            tessellate_contours(&contours, usize::MAX)
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
    }

    #[test]
    fn kernel_contours_fill_directly() {
        use crate::{
            floors::extrude_floor,
            section::{VerticalSectionPlane, vertical_section},
        };
        let boundary = [
            Point2::new(0., 0.),
            Point2::new(4., 0.),
            Point2::new(4., 4.),
            Point2::new(0., 4.),
        ];
        let mesh = extrude_floor(&boundary, 2., 0.25).unwrap();
        let contours = vertical_section(
            &mesh,
            VerticalSectionPlane {
                origin: Point2::new(0., 1.),
                direction: Point2::new(1., 0.),
            },
        )
        .unwrap();
        assert_eq!(area(&tessellate_contours(&contours, 2).unwrap()), 1.);
    }
}
