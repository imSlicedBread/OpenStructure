//! Decorative native cut fills, independent of semantic acquisition geometry.
use super::*;
use os_geometry::SurfaceIdentity;

const MAX_TRIANGLES: usize = 50_000;
const MAX_RAW_VERTICES: usize = 150_000;
const MAX_STORED_VERTICES: usize = 350_000;
const MAX_COORDINATE: f64 = os_geometry::section::MAX_SECTION_COORDINATE;

/// Checked CCW triangles in view-plane metres (X ±4e6, Y ±1e6).
/// Section X includes projection relative to the plane origin; Y is elevation.
/// Input order is compositing order.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanSectionFill {
    pub entity: Id,
    pub component: u32,
    pub surface: SurfaceIdentity,
    pub triangles: Vec<[Point2; 3]>,
}

/// A cropped convex fill polygon; triangle edges are never contour strokes.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanSectionFillPolygon {
    pub entity: Id,
    pub component: u32,
    pub surface: SurfaceIdentity,
    pub points: Vec<Point2>,
}

impl PlanDrawing {
    /// Replace decorative fills atomically after checking all raw input, even
    /// outside the crop. Does not create source, pick, or snap identities.
    pub fn with_section_fills(mut self, fills: Vec<PlanSectionFill>) -> Result<Self> {
        let triangle_count = fills.iter().fold(0usize, |count, fill| {
            count.saturating_add(fill.triangles.len())
        });
        ensure(
            fills.len() <= MAX_TRIANGLES
                && triangle_count <= MAX_TRIANGLES
                && triangle_count.saturating_mul(3) <= MAX_RAW_VERTICES,
            "section fills exceed raw geometry budget",
        )?;
        let mut identities = BTreeSet::new();
        let mut polygons = Vec::new();
        let mut vertex_count = 0usize;
        for fill in fills {
            ensure(
                !fill.entity.0.is_nil() && identities.insert((fill.entity, fill.component)),
                "invalid or duplicate section fill identity",
            )?;
            ensure(
                [fill.surface.layer, fill.surface.material]
                    .into_iter()
                    .flatten()
                    .all(|id| !id.0.is_nil()),
                "invalid section fill surface identity",
            )?;
            for triangle in fill.triangles {
                ensure(
                    triangle.iter().all(|point| valid_point(*point)),
                    "section fill needs finite, bounded, nondegenerate CCW triangles",
                )?;
                ensure(
                    orientation(triangle[0], triangle[1], triangle[2]) > 0,
                    "section fill needs nondegenerate CCW triangles",
                )?;
                let points = clip_section_polygon(&triangle, self.context.crop)?;
                if points.is_empty() {
                    continue;
                }
                ensure(points.len() <= 7, "invalid clipped section triangle")?;
                vertex_count = vertex_count.saturating_add(points.len());
                ensure(
                    polygons.len() < MAX_TRIANGLES && vertex_count <= MAX_STORED_VERTICES,
                    "section fills exceed clipped geometry budget",
                )?;
                polygons.push(PlanSectionFillPolygon {
                    entity: fill.entity,
                    component: fill.component,
                    surface: fill.surface,
                    points,
                });
            }
        }
        self.section_fills = polygons;
        self.section_hatches.clear();
        Ok(self)
    }

    pub fn section_fills(&self, current: PlanContext) -> Result<&[PlanSectionFillPolygon]> {
        self.items(current)?;
        Ok(&self.section_fills)
    }
}

fn valid_point(point: Point2) -> bool {
    point.is_finite()
        && point.x.abs() <= 4.0 * os_geometry::section::MAX_SECTION_COORDINATE
        && point.y.abs() <= MAX_COORDINATE
}

/// Adaptive exact sign for finite binary coordinates. The usual determinant
/// handles well-conditioned inputs. For cancellation, accumulate the six exact
/// signed mantissa products on the binary f64 exponent lattice. This also avoids
/// treating a product underflow as zero area. 68 limbs cover all finite f64
/// products, including crop corners outside the section coordinate envelope.
fn orientation(a: Point2, b: Point2, c: Point2) -> i8 {
    let left = (b.x - a.x) * (c.y - a.y);
    let right = (b.y - a.y) * (c.x - a.x);
    let det = left - right;
    if det.is_finite()
        && det.abs() >= f64::MIN_POSITIVE
        && det.abs() > 4.0 * f64::EPSILON * (left.abs() + right.abs())
    {
        return if det > 0.0 { 1 } else { -1 };
    }
    let mut positive = [0u64; 68];
    let mut negative = [0u64; 68];
    for (x, y, subtract) in [
        (a.x, b.y, false),
        (b.x, c.y, false),
        (c.x, a.y, false),
        (a.y, b.x, true),
        (b.y, c.x, true),
        (c.y, a.x, true),
    ] {
        let (xm, xe) = binary_parts(x);
        let (ym, ye) = binary_parts(y);
        let product = u128::from(xm) * u128::from(ym);
        let sum = if x.is_sign_negative() ^ y.is_sign_negative() ^ subtract {
            &mut negative
        } else {
            &mut positive
        };
        let shift = (xe + ye + 2148) as usize;
        let offset = shift % 64;
        for i in 0..2 {
            let word = (product >> (i * 64)) as u64;
            add_word(sum, shift / 64 + i, word << offset);
            if offset != 0 {
                add_word(sum, shift / 64 + i + 1, word >> (64 - offset));
            }
        }
    }
    match positive.iter().rev().cmp(negative.iter().rev()) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn binary_parts(value: f64) -> (u64, i32) {
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & ((1u64 << 52) - 1);
    if exponent == 0 {
        (mantissa, -1074)
    } else {
        (mantissa | (1u64 << 52), exponent - 1075)
    }
}

fn add_word(sum: &mut [u64; 68], mut index: usize, mut word: u64) {
    while word != 0 {
        let (value, carry) = sum[index].overflowing_add(word);
        sum[index] = value;
        word = u64::from(carry);
        index += 1;
    }
}

/// Separating axes distinguish exact tangency from a positive intersection
/// before rounded intersection coordinates can create or erase material.
fn intersects_area(points: &[Point2], crop: PlanCrop) -> bool {
    if points.iter().all(|p| p.x <= crop.min.x)
        || points.iter().all(|p| p.x >= crop.max.x)
        || points.iter().all(|p| p.y <= crop.min.y)
        || points.iter().all(|p| p.y >= crop.max.y)
    {
        return false;
    }
    let corners = [
        crop.min,
        Point2::new(crop.max.x, crop.min.y),
        crop.max,
        Point2::new(crop.min.x, crop.max.y),
    ];
    for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
        let mut any_inside = false;
        for corner in corners {
            any_inside |= orientation(a, b, corner) > 0;
        }
        if !any_inside {
            return false;
        }
    }
    true
}

/// Half-plane clipping with exact intersection/tangency classification. No
/// distance/area epsilon removes thin material; precision collapse is an error.
/// Also used for sheet viewport clipping of the checked convex polygons.
pub(crate) fn clip_section_polygon(
    points: &[Point2],
    crop: Option<PlanCrop>,
) -> Result<Vec<Point2>> {
    let mut output = points.to_vec();
    if let Some(crop) = crop {
        crop.validate()?;
        if !intersects_area(points, crop) {
            return Ok(Vec::new());
        }
        for edge in 0..4 {
            let input = std::mem::take(&mut output);
            let Some(&last) = input.last() else {
                break;
            };
            let inside = |p: Point2| match edge {
                0 => p.x >= crop.min.x,
                1 => p.x <= crop.max.x,
                2 => p.y >= crop.min.y,
                _ => p.y <= crop.max.y,
            };
            let intersect = |a: Point2, b: Point2| {
                if edge < 2 {
                    let x = if edge == 0 { crop.min.x } else { crop.max.x };
                    if x == a.x {
                        return a;
                    }
                    if x == b.x {
                        return b;
                    }
                    let t = ((x - a.x) / (b.x - a.x)).clamp(0.0, 1.0);
                    Point2::new(x, (a.y + t * (b.y - a.y)).clamp(a.y.min(b.y), a.y.max(b.y)))
                } else {
                    let y = if edge == 2 { crop.min.y } else { crop.max.y };
                    if y == a.y {
                        return a;
                    }
                    if y == b.y {
                        return b;
                    }
                    let t = ((y - a.y) / (b.y - a.y)).clamp(0.0, 1.0);
                    Point2::new((a.x + t * (b.x - a.x)).clamp(a.x.min(b.x), a.x.max(b.x)), y)
                }
            };
            let mut previous = last;
            for current in input {
                if inside(previous) != inside(current) {
                    output.push(intersect(previous, current));
                }
                if inside(current) {
                    output.push(current);
                }
                previous = current;
            }
            output.dedup();
            if output.len() > 1 && output.first() == output.last() {
                output.pop();
            }
        }
    }
    ensure(
        output.iter().all(|point| valid_point(*point)),
        "invalid clipped section fill coordinates",
    )?;
    // Remove exactly redundant vertices only, then verify the rounded result
    // remains a distinct convex polygon with positive material area.
    let mut index = 0;
    while output.len() >= 3 && index < output.len() {
        let n = output.len();
        let turn = orientation(
            output[(index + n - 1) % n],
            output[index],
            output[(index + 1) % n],
        );
        ensure(
            turn >= 0,
            "section fill clipping lost convexity through roundoff",
        )?;
        if turn == 0 {
            output.remove(index);
            index = 0;
        } else {
            index += 1;
        }
    }
    ensure(
        output.len() >= 3,
        "section fill clipping collapsed positive material through roundoff",
    )?;
    ensure(
        output
            .iter()
            .enumerate()
            .all(|(i, p)| !output[..i].contains(p)),
        "section fill clipping repeated a vertex",
    )?;
    Ok(output)
}
