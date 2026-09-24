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
    pub thickness: f64,
    pub top_offset: f64,
}
pub type Floor = Entity<FloorParams>;

fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}
impl FloorParams {
    pub fn area(&self) -> f64 {
        let Some(&a) = self.boundary.first() else {
            return 0.0;
        };
        self.boundary[1..]
            .windows(2)
            .map(|p| cross(a, p[0], p[1]))
            .sum::<f64>()
            .abs()
            * 0.5
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
        let p = &self.boundary;
        ensure((3..=256).contains(&p.len()), "floor needs 3-256 vertices")?;
        ensure(
            p.iter()
                .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
            "floor coordinates must be finite and within +/- 1000000 metres",
        )?;
        for i in 0..p.len() {
            for j in i + 1..p.len() {
                ensure(
                    p[i].distance(p[j]) > 1e-6,
                    "floor has duplicate or near-zero edges",
                )?;
                let (a, b, c, d) = (p[i], p[(i + 1) % p.len()], p[j], p[(j + 1) % p.len()]);
                if j == i + 1 || (i == 0 && j == p.len() - 1) {
                    continue;
                }
                let (ab_c, ab_d, cd_a, cd_b) = (
                    cross(a, b, c),
                    cross(a, b, d),
                    cross(c, d, a),
                    cross(c, d, b),
                );
                let overlap = a.x.min(b.x) <= c.x.max(d.x) + 1e-9
                    && c.x.min(d.x) <= a.x.max(b.x) + 1e-9
                    && a.y.min(b.y) <= c.y.max(d.y) + 1e-9
                    && c.y.min(d.y) <= a.y.max(b.y) + 1e-9;
                ensure(
                    !(overlap && ab_c * ab_d <= 0.0 && cd_a * cd_b <= 0.0),
                    "floor boundary self-intersects",
                )?;
            }
            let (a, b, c) = (p[(i + p.len() - 1) % p.len()], p[i], p[(i + 1) % p.len()]);
            ensure(
                cross(a, b, c).abs() > 1e-12
                    || (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y) > 0.0,
                "floor boundary doubles back",
            )?;
        }
        ensure(self.area() > 1e-8, "floor area is negligible")
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
}
