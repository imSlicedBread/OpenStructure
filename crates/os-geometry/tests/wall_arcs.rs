use os_core::Point2;
use os_model::*;
use std::f64::consts::PI;

#[test]
fn annular_mesh_is_closed_bounded_and_quantities_are_analytic() {
    for sweep in [PI / 2.0, -PI / 2.0, 1.5 * PI, -1.5 * PI] {
        let mut model = Model::new("Arc geometry");
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Arc".into(),
                path: WallPath::CircularArc {
                    center: Point2::default(),
                    radius: 4.0,
                    start_angle_rad: 0.0,
                    signed_sweep_rad: sweep,
                },
                thickness: 0.2,
                height: 3.0,
                level: *model.levels.keys().next().unwrap(),
                material: None,
            },
        );
        let id = wall.id();
        model.walls.insert(id, wall);
        let native = os_geometry::walls::NativeWall::from_model(&model, id).unwrap();
        let mesh = native.mesh().unwrap();
        assert!(mesh.vertices.len() <= 4 * (MAX_WALL_ARC_SEGMENTS + 1));
        let mut edges = std::collections::BTreeMap::new();
        for t in &mesh.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        assert!(
            edges
                .iter()
                .all(|(&(a, b), &n)| n == 1 && edges.get(&(b, a)) == Some(&1))
        );
        let exact = 4.0 * sweep.abs() * 0.2 * 3.0;
        assert!((native.net_volume().unwrap() - exact).abs() < 1e-12);
        assert!((mesh.signed_volume() - exact).abs() / exact < 0.001);
        assert!(native.cells().is_err());
        assert!(os_geometry::walls::wall_solid(&native.parameters, 0.0).is_err());
        let parts = native
            .layer_plan_footprints(Default::default(), Default::default(), None)
            .unwrap();
        assert!(parts[0].1.len() > 8);
        let plane = os_geometry::section::VerticalSectionPlane {
            origin: Point2::new(1.0, 0.0),
            direction: Point2::new(0.0, 1.0),
        };
        // The public section extractor validates watertight topology too.
        assert!(
            !os_geometry::section::vertical_section(&mesh, plane)
                .unwrap()
                .is_empty()
        );
    }
}
