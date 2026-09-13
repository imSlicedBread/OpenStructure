//! Synthetic, reproducible stage timings; not a production performance gate.
use os_core::{Id, Point2};
use os_geometry::plan::{HorizontalBasis, PlanRange};
use os_render::{
    plan::{PlanCamera, PlanContext},
    snapping::{SnapQuery, SnapScene, SnapSegment},
};
use std::{hint::black_box, time::Instant};

const SAMPLES: usize = 256;

fn main() {
    println!(
        "plan_snap_probe debug_assertions={} samples={SAMPLES} warmup=8 units=us",
        cfg!(debug_assertions)
    );
    println!("case,segments,source_us,scene_us,p50_us,p95_us,p99_us,max_us,hits,errors");
    for (name, count, dense, intersections) in [
        ("sparse_32_all", 32, false, true),
        ("sparse_1000_all", 1000, false, true),
        ("sparse_10000_all", 10000, false, true),
        ("dense_256_all", 256, true, true),
        ("dense_256_without_intersections", 256, true, false),
        ("dense_257_rejected", 257, true, true),
    ] {
        run(name, count, dense, intersections);
    }
}

fn run(name: &str, count: usize, dense: bool, intersections: bool) {
    let context = PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    };
    let started = Instant::now();
    let segments = (0..count)
        .map(|index| {
            let (start, end) = if dense {
                let angle = std::f64::consts::PI * index as f64 / count as f64;
                let (s, c) = angle.sin_cos();
                (
                    Point2::new(-10.0 * c, -10.0 * s),
                    Point2::new(10.0 * c, 10.0 * s),
                )
            } else {
                let x = (index % 100) as f64 * 20.0;
                let y = (index / 100) as f64 * 20.0;
                (Point2::new(x, y), Point2::new(x + 10.0, y))
            };
            SnapSegment {
                entity: Id::new(),
                feature: 0,
                start,
                end,
            }
        })
        .collect();
    let source_us = started.elapsed().as_secs_f64() * 1e6;
    let started = Instant::now();
    let scene = SnapScene::new(context, segments).unwrap();
    let scene_us = started.elapsed().as_secs_f64() * 1e6;
    let camera = PlanCamera {
        center: Point2::default(),
        pixels_per_metre: 65.0,
    };
    let base = if dense { 0.0 } else { 2.0 };
    let mut query = SnapQuery {
        camera,
        viewport: [800.0, 600.0],
        pointer: Point2::default(),
        radius_pixels: 12.0,
        endpoints: true,
        midpoints: true,
        intersections,
        nearest: true,
        axis_extensions: true,
        perpendicular_from: Some(Point2::new(2.0, 3.0)),
        exclude_entity: None,
    };
    let mut samples = Vec::with_capacity(SAMPLES);
    let (mut hits, mut errors) = (0, 0);
    for sample in 0..SAMPLES + 8 {
        let jitter = (sample % 7) as f64 * 0.001;
        query.pointer = camera
            .project(Point2::new(base + jitter, 0.04), query.viewport)
            .unwrap();
        let started = Instant::now();
        let result = black_box(&scene)
            .query(black_box(context), black_box(query))
            .and_then(|result| result.candidate(context, query));
        let elapsed = started.elapsed().as_secs_f64() * 1e6;
        if sample < 8 {
            continue;
        }
        samples.push(elapsed);
        match result {
            Ok(Some(candidate)) => {
                black_box(candidate);
                hits += 1;
            }
            Ok(None) => {}
            Err(error) => {
                assert!(
                    error.to_string().contains("256 nearby segments"),
                    "unexpected error: {error}"
                );
                errors += 1;
            }
        }
    }
    if dense && count > 256 && intersections {
        assert_eq!(errors, SAMPLES);
    } else {
        assert_eq!(hits, SAMPLES);
        assert_eq!(errors, 0);
    }
    samples.sort_by(f64::total_cmp);
    let percentile = |percent: usize| samples[(SAMPLES * percent).div_ceil(100) - 1];
    println!(
        "{name},{count},{source_us:.3},{scene_us:.3},{:.3},{:.3},{:.3},{:.3},{hits},{errors}",
        percentile(50),
        percentile(95),
        percentile(99),
        samples[SAMPLES - 1]
    );
}
