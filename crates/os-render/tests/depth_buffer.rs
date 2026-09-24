use os_core::Id;
use os_render::{MAX_PIXELS, ProjectedPoint, RasterFrame, RasterStyle, Triangle, pick};

fn triangle(id: Id, depths: [f64; 3]) -> Triangle {
    Triangle {
        surface: Default::default(),
        entity: id,
        points: [(0., 0.), (100., 0.), (0., 100.)]
            .into_iter()
            .zip(depths)
            .map(|((x, y), depth)| ProjectedPoint { x, y, depth })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        shade: 1.,
    }
}
fn render(ts: &[Triangle], selected: Option<Id>) -> RasterFrame {
    RasterFrame::render(ts, [100., 100.], 1., selected, RasterStyle::default()).unwrap()
}

#[test]
fn crossing_planes_change_visible_owner_within_the_same_triangles() {
    let a = Id::new();
    let b = Id::new();
    // A is in front at the left but behind at the right. No triangle ordering
    // can paint both regions correctly: average-depth sorting necessarily fails.
    let triangles = [triangle(a, [10., -10., 10.]), triangle(b, [0., 0., 0.])];
    let frame = render(&triangles, Some(a));
    assert_eq!(frame.pick(10.5, 10.5), Some(a));
    assert_eq!(frame.pick(70.5, 10.5), Some(b));
    assert_eq!(frame.rgba[10 * 100 + 10], [75, 132, 237, 255]);
    assert_eq!(frame.rgba[10 * 100 + 70], [171, 182, 197, 255]);
    assert_eq!(pick(&triangles, 10.5, 10.5), Some(a));
    assert_eq!(pick(&triangles, 70.5, 10.5), Some(b));
    assert_eq!(
        frame.rgba,
        render(&[triangles[1].clone(), triangles[0].clone()], Some(a)).rgba
    );
    assert!((frame.depth_at(10, 10).unwrap() - 7.9).abs() < 1e-10);
    assert_eq!(frame.depth_at(70, 10), Some(0.));
}

#[test]
fn coplanar_ties_and_occluded_selection_are_order_independent() {
    let a = Id::new();
    let b = Id::new();
    let low = a.min(b);
    let ts = [triangle(a, [3.; 3]), triangle(b, [3.; 3])];
    for input in [
        vec![ts[0].clone(), ts[1].clone()],
        vec![ts[1].clone(), ts[0].clone()],
    ] {
        assert_eq!(render(&input, None).pick(20., 20.), Some(low));
        assert_eq!(pick(&input, 20., 20.), Some(low));
    }
    let mut ts = ts;
    ts[0].points.iter_mut().for_each(|p| p.depth = 4.);
    let frame = render(&ts, Some(b));
    assert_eq!(frame.pick(20., 20.), Some(a));
    assert_eq!(frame.rgba[20 * 100 + 20], [171, 182, 197, 255]);
}

#[test]
fn shared_edges_clipping_and_invalid_samples_are_safe() {
    let id = Id::new();
    let mut a = triangle(id, [1.; 3]);
    let mut b = a.clone();
    b.points =
        [(100., 100.), (0., 100.), (100., 0.)].map(|(x, y)| ProjectedPoint { x, y, depth: 1. });
    let frame = render(&[a.clone(), b], None);
    assert!(
        frame.rgba.iter().all(|p| p[3] == 255),
        "crack along shared diagonal"
    );
    for (x, y) in [(100., 10.), (-1., 0.), (0., 100.), (f32::NAN, 0.)] {
        assert_eq!(frame.pick(x, y), None);
    }
    a.points.iter_mut().for_each(|p| p.x -= 1000.);
    assert!(render(&[a.clone()], None).rgba.iter().all(|p| p[3] == 0));
    a.points[0].x = f32::INFINITY;
    assert!(render(&[a], None).rgba.iter().all(|p| p[3] == 0));
    let mut zero = triangle(id, [1.; 3]);
    zero.points[2] = zero.points[1];
    assert_eq!(render(&[zero], None).pick(0., 0.), None);
}

#[test]
fn dpi_scaled_pick_matches_display_and_memory_is_bounded() {
    let id = Id::new();
    let ts = [triangle(id, [1.; 3])];
    for scale in [1., 1.25, 1.5, 2.] {
        let frame =
            RasterFrame::render(&ts, [100., 100.], scale, None, RasterStyle::default()).unwrap();
        assert_eq!(frame.width, (100. * scale) as usize);
        assert_eq!(frame.pick(20., 20.), Some(id));
        assert_eq!(frame.pick(90., 90.), None);
    }
    for size in [[100000., 100000.], [f32::MAX, 1.], [1., f32::MAX]] {
        let frame = RasterFrame::render(&[], size, 10., None, RasterStyle::default()).unwrap();
        assert!(frame.width * frame.height <= MAX_PIXELS);
        assert!(frame.width <= 4096 && frame.height <= 4096);
    }
    assert!(RasterFrame::render(&[], [0., 100.], 1., None, RasterStyle::default()).is_err());
    assert!(
        RasterFrame::render(&[], [100., 100.], f32::NAN, None, RasterStyle::default()).is_err()
    );
}

#[test]
fn actual_intersecting_prisms_agree_with_independent_ray_box_intersections() {
    use os_core::Point2;
    use os_geometry::{GeometryKernel, PrismKernel, Profile, Solid, Transform, Vec3};
    use os_render::{Camera, Scene, project_scene};
    let a = Id::new();
    let b = Id::new();
    let boxes = [
        (a, [-4., -0.25, 0.], [4., 0.25, 3.]),
        (b, [-0.25, -4., 0.], [0.25, 4., 4.]),
    ];
    let mut scene = Scene::new();
    for (id, lo, hi) in boxes {
        let solid = Solid {
            profile: Profile {
                vertices: vec![
                    Point2::new(lo[0], lo[1]),
                    Point2::new(hi[0], lo[1]),
                    Point2::new(hi[0], hi[1]),
                    Point2::new(lo[0], hi[1]),
                ],
            },
            height: hi[2],
            transform: Transform::default(),
        };
        scene.insert(id, PrismKernel.tessellate(&solid).unwrap());
    }
    for yaw in [-2.5, -0.65, 0.5, 2.] {
        let camera = Camera {
            yaw,
            pitch: 0.55,
            target: Vec3::new(0., 0., 2.),
            pixels_per_metre: 18.,
        };
        let triangles = project_scene(&scene, &camera, 200., 160.);
        let frame = RasterFrame::render(&triangles, [200., 160.], 1., None, RasterStyle::default())
            .unwrap();
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = camera.pitch.sin_cos();
        let direction = [-sy * cp, cy * cp, sp];
        let right = [cy, sy, 0.];
        let up = [sy * sp, -cy * sp, cp];
        let mut samples = 0;
        for py in (3..157).step_by(3) {
            for px in (3..197).step_by(3) {
                let x = (px as f64 + 0.5 - 100.) / 18.;
                let y = (80. - py as f64 - 0.5) / 18.;
                let origin = [
                    right[0] * x + up[0] * y,
                    right[1] * x + up[1] * y,
                    2. + up[2] * y,
                ];
                let mut hits = Vec::new();
                for (id, lo, hi) in boxes {
                    let mut near = f64::NEG_INFINITY;
                    let mut far = f64::INFINITY;
                    for i in 0..3 {
                        let t0 = (lo[i] - origin[i]) / direction[i];
                        let t1 = (hi[i] - origin[i]) / direction[i];
                        near = near.max(t0.min(t1));
                        far = far.min(t0.max(t1));
                    }
                    if near <= far {
                        hits.push((far, id));
                    }
                }
                let expected = hits
                    .into_iter()
                    .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
                    .map(|(_, id)| id);
                assert_eq!(
                    frame.pick(px as f32 + 0.5, py as f32 + 0.5),
                    expected,
                    "yaw {yaw}, pixel {px},{py}"
                );
                if expected.is_some() {
                    samples += 1;
                }
            }
        }
        assert!(samples > 100);
    }
}
