//! Authored horizontal slab boundaries; triangulation and meshes are derived.
use crate::Entity;
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorParams {
    pub name: String,
    pub level: Id,
    pub material: Option<Id>,
    pub boundary: Vec<Point2>,
    /// Authored void loops cut from the slab. Each loop is a simple ring and
    /// winding has no semantic meaning.
    pub holes: Vec<Vec<Point2>>,
    pub thickness: f64,
    pub top_offset: f64,
}
pub type Floor = Entity<FloorParams>;

fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}
fn ring_area(ring: &[Point2]) -> f64 {
    let Some(&a) = ring.first() else {
        return 0.0;
    };
    ring[1..]
        .windows(2)
        .map(|p| cross(a, p[0], p[1]))
        .sum::<f64>()
        .abs()
        * 0.5
}
fn point_in_ring(point: Point2, ring: &[Point2]) -> bool {
    let mut inside = false;
    for index in 0..ring.len() {
        let a = ring[index];
        let b = ring[(index + 1) % ring.len()];
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
    }
    inside
}
fn segments_intersect(a: Point2, b: Point2, c: Point2, d: Point2) -> bool {
    fn on_segment(a: Point2, b: Point2, p: Point2) -> bool {
        cross(a, b, p).abs() <= 1e-9
            && p.x >= a.x.min(b.x) - 1e-9
            && p.x <= a.x.max(b.x) + 1e-9
            && p.y >= a.y.min(b.y) - 1e-9
            && p.y <= a.y.max(b.y) + 1e-9
    }
    let (ab_c, ab_d, cd_a, cd_b) = (
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    );
    (ab_c > 1e-9 && ab_d < -1e-9 || ab_c < -1e-9 && ab_d > 1e-9)
        && (cd_a > 1e-9 && cd_b < -1e-9 || cd_a < -1e-9 && cd_b > 1e-9)
        || on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
}
fn rings_intersect(a: &[Point2], b: &[Point2]) -> bool {
    (0..a.len()).any(|i| {
        (0..b.len())
            .any(|j| segments_intersect(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()]))
    })
}
fn point_segment_distance(point: Point2, a: Point2, b: Point2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (point.x - a.x - t * dx).hypot(point.y - a.y - t * dy)
}
fn rings_clear(a: &[Point2], b: &[Point2]) -> bool {
    if rings_intersect(a, b) {
        return false;
    }
    (0..a.len()).all(|i| {
        (0..b.len()).all(|j| {
            let (start_a, end_a) = (a[i], a[(i + 1) % a.len()]);
            let (start_b, end_b) = (b[j], b[(j + 1) % b.len()]);
            point_segment_distance(start_a, start_b, end_b)
                .min(point_segment_distance(end_a, start_b, end_b))
                .min(point_segment_distance(start_b, start_a, end_a))
                .min(point_segment_distance(end_b, start_a, end_a))
                > 1e-6
        })
    })
}
fn validate_ring(ring: &[Point2], description: &str) -> Result<()> {
    ensure(
        ring.len() >= 3,
        format!("{description} needs at least 3 vertices"),
    )?;
    ensure(
        ring.iter()
            .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
        format!("{description} coordinates must be finite and within +/- 1000000 metres"),
    )?;
    for i in 0..ring.len() {
        for j in i + 1..ring.len() {
            ensure(
                ring[i].distance(ring[j]) > 1e-6,
                format!("{description} has duplicate or near-zero edges"),
            )?;
            if j == i + 1 || (i == 0 && j == ring.len() - 1) {
                continue;
            }
            ensure(
                !segments_intersect(
                    ring[i],
                    ring[(i + 1) % ring.len()],
                    ring[j],
                    ring[(j + 1) % ring.len()],
                ),
                format!("{description} self-intersects"),
            )?;
        }
        let (a, b, c) = (
            ring[(i + ring.len() - 1) % ring.len()],
            ring[i],
            ring[(i + 1) % ring.len()],
        );
        ensure(
            cross(a, b, c).abs() > 1e-12
                || (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y) > 0.0,
            format!("{description} doubles back"),
        )?;
    }
    ensure(
        ring_area(ring) > 1e-8,
        format!("{description} area is negligible"),
    )
}
impl FloorParams {
    pub fn area(&self) -> f64 {
        (ring_area(&self.boundary) - self.holes.iter().map(|ring| ring_area(ring)).sum::<f64>())
            .max(0.0)
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "floor name must be 1-256 bytes without control characters",
        )?;
        ensure(
            self.thickness.is_finite()
                && self.thickness > 1e-6
                && self.thickness <= 1e6
                && self.top_offset.is_finite()
                && self.top_offset.abs() <= 1e6,
            "invalid floor thickness or top offset",
        )?;
        ensure(
            (3..=256).contains(&self.boundary.len()),
            "floor needs 3-256 vertices",
        )?;
        validate_ring(&self.boundary, "floor boundary")?;
        ensure(self.holes.len() <= 16, "floor supports at most 16 openings")?;
        let total_vertices = self.holes.iter().map(Vec::len).sum::<usize>() + self.boundary.len();
        ensure(
            total_vertices <= 1024,
            "floor boundary and openings support at most 1024 vertices",
        )?;
        for (index, hole) in self.holes.iter().enumerate() {
            ensure(
                (3..=256).contains(&hole.len()),
                "floor opening needs 3-256 vertices",
            )?;
            validate_ring(hole, &format!("floor opening {index}"))?;
            ensure(
                point_in_ring(hole[0], &self.boundary) && rings_clear(&self.boundary, hole),
                format!("floor opening {index} must be inside and clear of its boundary"),
            )?;
            for previous in &self.holes[..index] {
                ensure(
                    rings_clear(previous, hole)
                        && !point_in_ring(hole[0], previous)
                        && !point_in_ring(previous[0], hole),
                    "floor openings must not overlap, touch, or nest",
                )?;
            }
        }
        ensure(self.area() > 1e-8, "floor net area is negligible")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn floor_bounds_identity_references_and_owned_memory_are_checked() {
        let mut model = crate::Model::new("Floors");
        let p = FloorParams {
            name: "Slab".into(),
            level: *model.levels.keys().next().unwrap(),
            material: None,
            boundary: vec![
                Point2::new(0., 0.),
                Point2::new(4., 0.),
                Point2::new(0., 4.),
            ],
            holes: Vec::new(),
            thickness: 0.2,
            top_offset: 0.0,
        };
        for case in 0..11 {
            let mut bad = p.clone();
            match case {
                0 => bad.name.clear(),
                1 => bad.name = "x".repeat(257),
                2 => bad.name = "bad\nname".into(),
                3 => bad.thickness = 0.,
                4 => bad.thickness = f64::INFINITY,
                5 => bad.top_offset = f64::NAN,
                6 => bad.top_offset = 1e7,
                7 => bad.boundary = vec![Point2::new(0., 0.); 257],
                8 => bad.boundary[1] = Point2::new(1e-7, 0.),
                9 => {
                    bad.boundary = vec![
                        Point2::new(0., 0.),
                        Point2::new(2., 0.),
                        Point2::new(1., 0.),
                        Point2::new(0., 1.),
                    ]
                }
                _ => {
                    bad.boundary = vec![
                        Point2::new(0., 0.),
                        Point2::new(1e-5, 0.),
                        Point2::new(0., 1e-5),
                    ]
                }
            }
            assert!(bad.validate().is_err(), "case {case}");
        }
        let baseline = model.estimated_memory_bytes();
        let floor = Floor::new("core.floor", p);
        let id = floor.id();
        model.floors.insert(id, floor.clone());
        model.validate().unwrap();
        assert!(model.estimated_memory_bytes() > baseline + std::mem::size_of::<Floor>());
        for case in 0..5 {
            let mut bad = model.clone();
            let f = bad.floors.get_mut(&id).unwrap();
            match case {
                0 => f.header.id = Id::new(),
                1 => f.header.type_id = "core.wall".into(),
                2 => f.parameters.level = Id::new(),
                3 => f.parameters.material = Some(Id::new()),
                _ => f.header.schema_version = 8,
            }
            assert!(bad.validate().is_err());
        }
        let json = serde_json::to_value(floor).unwrap();
        assert!(json["parameters"].get("mesh").is_none());
        assert!(json["parameters"].get("area").is_none());
        let mut missing_openings = json.clone();
        missing_openings["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("holes");
        assert!(serde_json::from_value::<Floor>(missing_openings).is_err());
        model
            .levels
            .values_mut()
            .next()
            .unwrap()
            .parameters
            .elevation = f64::MAX;
        assert!(model.validate().is_err());
    }
    #[test]
    fn validates_concavity_and_rejects_invalid_rings() {
        let mut f = FloorParams {
            name: "Slab".into(),
            level: Id::new(),
            material: None,
            boundary: vec![
                Point2::new(0., 0.),
                Point2::new(4., 0.),
                Point2::new(4., 1.),
                Point2::new(1., 1.),
                Point2::new(1., 4.),
                Point2::new(0., 4.),
            ],
            holes: Vec::new(),
            thickness: 0.2,
            top_offset: 0.0,
        };
        f.validate().unwrap();
        assert_eq!(f.area(), 7.0);
        f.boundary.reverse();
        f.validate().unwrap();
        for ring in [
            vec![Point2::new(0., 0.); 3],
            vec![
                Point2::new(0., 0.),
                Point2::new(2., 2.),
                Point2::new(0., 2.),
                Point2::new(2., 0.),
            ],
            vec![
                Point2::new(f64::NAN, 0.),
                Point2::new(1., 0.),
                Point2::new(0., 1.),
            ],
            vec![
                Point2::new(0., 0.),
                Point2::new(1e7, 0.),
                Point2::new(0., 1.),
            ],
        ] {
            f.boundary = ring;
            assert!(f.validate().is_err());
        }
    }

    #[test]
    fn floor_openings_are_strictly_contained_disjoint_and_reduce_net_area() {
        let mut floor = FloorParams {
            name: "Slab with stair void".into(),
            level: Id::new(),
            material: None,
            boundary: vec![
                Point2::new(0., 0.),
                Point2::new(10., 0.),
                Point2::new(10., 8.),
                Point2::new(0., 8.),
            ],
            holes: vec![
                vec![
                    Point2::new(2., 2.),
                    Point2::new(4., 2.),
                    Point2::new(4., 4.),
                    Point2::new(2., 4.),
                ],
                vec![
                    Point2::new(6., 2.),
                    Point2::new(8., 2.),
                    Point2::new(8., 4.),
                    Point2::new(6., 4.),
                ],
            ],
            thickness: 0.2,
            top_offset: 0.0,
        };
        floor.holes[0].reverse();
        floor.validate().unwrap();
        assert_eq!(floor.area(), 72.0);

        let valid_holes = floor.holes.clone();
        let invalid_against_boundary = [
            vec![
                Point2::new(9., 2.),
                Point2::new(11., 2.),
                Point2::new(11., 4.),
                Point2::new(9., 4.),
            ],
            vec![
                Point2::new(0., 2.),
                Point2::new(1., 2.),
                Point2::new(1., 4.),
                Point2::new(0., 4.),
            ],
        ];
        for opening in invalid_against_boundary {
            floor.holes = vec![opening];
            assert!(floor.validate().is_err());
        }
        let nested = vec![
            Point2::new(2.5, 2.5),
            Point2::new(3.5, 2.5),
            Point2::new(3.5, 3.5),
            Point2::new(2.5, 3.5),
        ];
        floor.holes = vec![valid_holes[0].clone(), nested];
        assert!(floor.validate().is_err());
        let overlapping = vec![
            Point2::new(2., 2.),
            Point2::new(5., 2.),
            Point2::new(5., 3.),
            Point2::new(2., 3.),
        ];
        floor.holes = vec![valid_holes[0].clone(), overlapping];
        assert!(floor.validate().is_err());
        floor.holes = vec![valid_holes[0].clone(), valid_holes[0].clone()];
        assert!(floor.validate().is_err());
    }
}
