use os_core::{Id, Point2};
use os_geometry::{SurfaceIdentity, plan::*};
use os_render::{plan::*, sheet::*, snapping::SnapQuery};
use std::collections::BTreeMap;

fn context() -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 3,
        view_id: Id::new(),
        settings_revision: 2,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}

fn drawing(ctx: PlanContext) -> PlanDrawing {
    PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![]).unwrap()
}

fn fill(points: [[f64; 2]; 3]) -> PlanSectionFill {
    PlanSectionFill {
        entity: Id::new(),
        component: 0,
        surface: SurfaceIdentity::default(),
        triangles: vec![points.map(|p| Point2::new(p[0], p[1]))],
    }
}

fn area(points: &[Point2]) -> f64 {
    let o = points[0];
    points[1..]
        .windows(2)
        .map(|p| (p[0].x - o.x) * (p[1].y - o.y) - (p[0].y - o.y) * (p[1].x - o.x))
        .sum::<f64>()
        * 0.5
}

#[test]
fn section_fills_crop_triangles_without_changing_compositing_order() {
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(0.0, 0.0),
            max: Point2::new(1.0, 1.0),
        }),
        ..context()
    };
    let first = fill([[-1.0, -1.0], [4.0, -1.0], [-1.0, 4.0]]);
    let mut second = first.clone();
    second.component = 1;
    let mut last = first.clone();
    last.entity = Id::new();
    let order = [first.entity, second.entity, last.entity];
    let d = drawing(ctx)
        .with_section_fills(vec![
            first,
            second,
            fill([[2.0, 2.0], [3.0, 2.0], [2.0, 3.0]]),
            last,
        ])
        .unwrap();
    let polygons = d.section_fills(ctx).unwrap();
    assert_eq!(polygons.iter().map(|p| p.entity).collect::<Vec<_>>(), order);
    assert_eq!(polygons[1].component, 1);
    for polygon in polygons {
        assert_eq!(polygon.points.len(), 4);
        assert_eq!(area(&polygon.points), 1.0);
        assert!(
            polygon
                .points
                .iter()
                .all(|p| (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y))
        );
    }
    assert!(
        d.with_section_fills(vec![])
            .unwrap()
            .section_fills(ctx)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn section_fills_keep_thin_clipped_regions_and_omit_only_zero_area_contacts() {
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(0.0, 0.0),
            max: Point2::new(1.0, 1.0),
        }),
        ..context()
    };
    let thin = fill([[-1.0, 0.0], [1.0, 0.0], [1.0, 1e-14]]);
    let edge_contact = fill([[-1.0, 0.0], [0.0, 0.0], [0.0, 1.0]]);
    let point_contact = fill([[-1.0, 0.0], [0.0, 0.0], [-1.0, 1.0]]);
    let d = drawing(ctx)
        .with_section_fills(vec![thin, edge_contact, point_contact])
        .unwrap();
    let polygons = d.section_fills(ctx).unwrap();
    assert_eq!(polygons.len(), 1);
    assert!(area(&polygons[0].points) > 0.0 && area(&polygons[0].points) < 1e-12);
}

#[test]
fn section_fill_ir_accepts_the_section_kernel_horizontal_coordinate_envelope() {
    let ctx = context();
    let input = fill([[2_000_000.0, 0.0], [2_000_001.0, 0.0], [2_000_000.0, 1.0]]);
    let drawing = drawing(ctx).with_section_fills(vec![input]).unwrap();
    assert_eq!(drawing.section_fills(ctx).unwrap().len(), 1);
}

#[test]
fn section_fill_access_uses_full_drawing_freshness() {
    let ctx = context();
    let d = drawing(ctx)
        .with_section_fills(vec![fill([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])])
        .unwrap();
    for stale in [
        PlanContext {
            session_id: Id::new(),
            ..ctx
        },
        PlanContext {
            view_id: Id::new(),
            ..ctx
        },
        PlanContext {
            model_revision: 4,
            ..ctx
        },
        PlanContext {
            settings_revision: 3,
            ..ctx
        },
        PlanContext {
            crop: Some(PlanCrop {
                min: Point2::default(),
                max: Point2::new(1.0, 1.0),
            }),
            ..ctx
        },
        PlanContext {
            scale_denominator: 50.0,
            ..ctx
        },
        PlanContext {
            show_walls: false,
            ..ctx
        },
        PlanContext {
            show_extensions: false,
            ..ctx
        },
        PlanContext {
            basis: HorizontalBasis {
                rotation: 0.5,
                ..ctx.basis
            },
            ..ctx
        },
        PlanContext {
            range: PlanRange {
                cut: 1.5,
                ..ctx.range
            },
            ..ctx
        },
    ] {
        assert!(d.section_fills(stale).is_err());
    }
}

#[test]
fn section_fills_reject_invalid_raw_geometry_and_duplicate_components_even_off_crop() {
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(10.0, 10.0),
            max: Point2::new(11.0, 11.0),
        }),
        ..context()
    };
    for points in [
        [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
        [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
        [[0.0, 0.0], [1.0, 0.0], [f64::NAN, 1.0]],
        [[0.0, 0.0], [f64::INFINITY, 0.0], [0.0, 1.0]],
        [[0.0, 0.0], [4_000_001.0, 0.0], [0.0, 1.0]],
        [[0.0, 0.0], [1.0, 0.0], [0.0, 1_000_001.0]],
    ] {
        assert!(drawing(ctx).with_section_fills(vec![fill(points)]).is_err());
    }
    let good = fill([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
    assert!(
        drawing(ctx)
            .with_section_fills(vec![good.clone(), good.clone()])
            .is_err()
    );
    let mut invalid = good.clone();
    invalid.entity = Id(Default::default());
    assert!(drawing(ctx).with_section_fills(vec![invalid]).is_err());
    let mut invalid = good.clone();
    invalid.surface.material = Some(Id(Default::default()));
    assert!(drawing(ctx).with_section_fills(vec![invalid]).is_err());
    // A late invalid triangle produces no successful partial drawing.
    let mut invalid = good.clone();
    invalid.triangles.push([Point2::default(); 3]);
    assert!(drawing(ctx).with_section_fills(vec![invalid]).is_err());
}

#[test]
fn section_fill_raw_budget_cannot_be_bypassed_by_crop_or_components() {
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(10.0, 10.0),
            max: Point2::new(11.0, 11.0),
        }),
        ..context()
    };
    let mut input = fill([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
    input.triangles = vec![input.triangles[0]; 50_000];
    assert!(
        drawing(ctx)
            .with_section_fills(vec![input.clone()])
            .unwrap()
            .section_fills(ctx)
            .unwrap()
            .is_empty()
    );
    let mut extra = input.clone();
    extra.component = 1;
    extra.triangles.truncate(1);
    assert!(
        drawing(ctx)
            .with_section_fills(vec![input.clone(), extra])
            .is_err()
    );
    input.triangles.push(input.triangles[0]);
    assert!(drawing(ctx).with_section_fills(vec![input]).is_err());
}

#[test]
fn section_fills_are_decorative_for_picking_and_snapping() {
    let ctx = context();
    let camera = PlanCamera::default();
    let viewport = [800.0, 600.0];
    let d = drawing(ctx)
        .with_snap_segments(vec![])
        .unwrap()
        .with_section_fills(vec![fill([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])])
        .unwrap();
    assert!(d.items(ctx).unwrap().is_empty());
    for point in [Point2::default(), Point2::new(0.2, 0.2)] {
        assert_eq!(d.pick(ctx, point).unwrap(), None);
        assert_eq!(
            d.pick_screen(
                ctx,
                camera,
                viewport,
                camera.project(point, viewport).unwrap(),
                8.0
            )
            .unwrap(),
            None
        );
    }
    let query = SnapQuery {
        camera,
        viewport,
        pointer: camera.project(Point2::default(), viewport).unwrap(),
        radius_pixels: 8.0,
        endpoints: true,
        midpoints: true,
        intersections: true,
        perpendicular_from: None,
        nearest: true,
        axis_extensions: true,
        exclude_entity: None,
    };
    assert_eq!(
        d.snap(ctx, query).unwrap().candidate(ctx, query).unwrap(),
        None
    );
    assert!(
        d.with_snap_segments(vec![os_render::snapping::SnapSegment {
            entity: Id::new(),
            feature: 0,
            start: Point2::default(),
            end: Point2::new(1.0, 0.0),
        }])
        .is_err()
    );
}

fn viewport() -> PaperViewport {
    PaperViewport {
        center_mm: Point2::new(100.0, 100.0),
        width_mm: 20.0,
        height_mm: 20.0,
        model_center_m: Point2::default(),
        scale_denominator: 100.0,
    }
}

fn paper() -> PaperSheetInfo<'static> {
    PaperSheetInfo {
        width_mm: 420.0,
        height_mm: 297.0,
        number: "A101",
        name: "Section",
    }
}

#[test]
fn section_projected_cut_styles_and_exact_vectors_match_multi_scale_sheet_pdf() {
    let ctx = context();
    let entity = Id::new();
    let material = Id::new();
    let cut = PlanLine {
        entity,
        feature: 0,
        start: Point2::new(-0.5, 0.),
        end: Point2::new(0.5, 0.),
        role: PlanRole::Cut,
    };
    let projected = PlanLine {
        entity,
        feature: 1,
        start: Point2::new(-0.5, 0.75),
        end: Point2::new(0.5, 0.75),
        role: PlanRole::Projected,
    };
    let surface = SurfaceIdentity {
        layer: None,
        material: Some(material),
    };
    let mut region = fill([[-0.5, 0.], [0.5, 0.], [0., 0.5]]);
    region.entity = entity;
    region.surface = surface;
    let d = PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![entity])
        .unwrap()
        .with_native_lines(BTreeMap::from([(
            entity,
            vec![cut.clone(), projected.clone()],
        )]))
        .unwrap()
        .with_line_surfaces(BTreeMap::from([
            ((entity, 0), surface),
            ((entity, 1), surface),
        ]))
        .with_section_fills(vec![region])
        .unwrap()
        .with_material_colors(BTreeMap::from([(material, [255, 255, 255])]))
        .with_section_strokes();
    assert_eq!(d.line_surface(entity, 0), surface);
    assert_eq!(d.line_surface(entity, 1), surface);
    let mut v1 = viewport();
    v1.width_mm = 40.;
    v1.height_mm = 40.;
    let mut v2 = v1;
    v2.center_mm.x = 200.;
    v2.scale_denominator = 50.;
    let page = compose_sheet_views(paper(), &[("100", v1, ctx, &d), ("50", v2, ctx, &d)]).unwrap();
    for view in [v1, v2] {
        let marks: Vec<_> = page
            .marks()
            .iter()
            .filter(|m| m.clip == Some(view.paper_rect().unwrap()))
            .collect();
        assert_eq!(marks.len(), 3);
        for (index, line) in [(0, &projected), (2, &cut)] {
            let PaperMarkKind::Path {
                points_mm,
                stroke: Some(stroke),
                fill: None,
                closed: false,
            } = &marks[index].kind
            else {
                panic!("stroke order")
            };
            assert_eq!(
                *points_mm,
                vec![
                    view.model_to_paper(line.start).unwrap(),
                    view.model_to_paper(line.end).unwrap()
                ]
            );
            let expected = d.line_appearance(ctx, line).unwrap().unwrap();
            assert_eq!(stroke.width_mm, expected.weight_mm);
            assert_eq!(
                [stroke.color.red, stroke.color.green, stroke.color.blue],
                expected.color
            );
            assert_ne!(expected.color, [255, 255, 255]);
        }
        assert!(matches!(
            marks[1].kind,
            PaperMarkKind::Path {
                fill: Some(PaperColor {
                    red: 255,
                    green: 255,
                    blue: 255
                }),
                stroke: None,
                ..
            }
        ));
    }
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    assert_eq!(pdf.matches("h\nf\n").count(), 2);
    assert!(pdf.contains("0.510236 w") && pdf.contains("0.992126 w"));
    assert!(pdf.contains("0.094118 0.094118 0.094118 RG"));
    assert!(pdf.contains("0.345098 0.345098 0.345098 RG"));
    let mut stale = ctx;
    stale.model_revision += 1;
    assert!(d.line_appearance(stale, &cut).is_err());
    assert!(compose_view_sheet(paper(), "stale", v1, stale, &d).is_err());
}

#[test]
fn section_sheet_and_pdf_fill_before_contours_without_triangle_strokes() {
    let ctx = context();
    let material = Id::new();
    let mut colored = fill([[-2.0, -2.0], [4.0, -2.0], [-2.0, 4.0]]);
    colored.surface = SurfaceIdentity {
        material: Some(material),
        layer: None,
    };
    let contour_entity = colored.entity;
    let fallback = fill([[0.0, 0.0], [1.0, 0.0], [0.0, 1e-14]]);
    let d = PlanDrawing::from_prisms(ctx, &BTreeMap::new(), vec![contour_entity])
        .unwrap()
        .with_native_lines(BTreeMap::from([(
            contour_entity,
            vec![PlanLine {
                entity: contour_entity,
                feature: 0,
                start: Point2::new(-2.0, 0.0),
                end: Point2::new(2.0, 0.0),
                role: PlanRole::Cut,
            }],
        )]))
        .unwrap()
        .with_section_fills(vec![colored, fallback])
        .unwrap()
        .with_material_colors(BTreeMap::from([(material, [10, 20, 30])]));
    let view = viewport();
    let page = compose_view_sheet(paper(), "Cut", view, ctx, &d).unwrap();
    let paths: Vec<_> = page
        .marks()
        .iter()
        .filter(|mark| mark.clip == Some(view.paper_rect().unwrap()))
        .collect();
    assert_eq!(paths.len(), 3);
    for (mark, expected) in paths[..2].iter().zip([[10, 20, 30], [232, 232, 232]]) {
        let PaperMarkKind::Path {
            points_mm,
            closed,
            stroke,
            fill,
        } = &mark.kind
        else {
            panic!("fill path")
        };
        assert!(*closed && stroke.is_none());
        assert_eq!(
            *fill,
            Some(PaperColor {
                red: expected[0],
                green: expected[1],
                blue: expected[2]
            })
        );
        assert!(
            points_mm
                .iter()
                .all(|p| view.paper_rect().unwrap().contains(*p))
        );
    }
    assert!(matches!(
        &paths[2].kind,
        PaperMarkKind::Path {
            closed: false,
            stroke: Some(_),
            fill: None,
            ..
        }
    ));
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    assert_eq!(pdf.matches("h\nf\n").count(), 2);
    assert!(!pdf.contains("h\nB\n"));
    assert!(pdf.find("0.039216 0.078431 0.117647 rg").unwrap() < pdf.find("h\nf\n").unwrap());
    assert!(pdf.contains("0.909804 0.909804 0.909804 rg"));
}

#[test]
fn section_fill_sheet_mark_budget_includes_all_viewports() {
    let ctx = context();
    let mut input = fill([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
    input.triangles = vec![input.triangles[0]; 50_000];
    let d = drawing(ctx).with_section_fills(vec![input]).unwrap();
    assert_eq!(d.section_fills(ctx).unwrap().len(), 50_000);
    assert_eq!(
        d.section_fills(ctx)
            .unwrap()
            .iter()
            .map(|p| p.points.len())
            .sum::<usize>(),
        150_000
    );
    assert!(compose_view_sheet(paper(), "Cut", viewport(), ctx, &d).is_ok());
    let other = PaperViewport {
        center_mm: Point2::new(200.0, 100.0),
        ..viewport()
    };
    assert!(
        compose_sheet_views(
            paper(),
            &[("Cut", viewport(), ctx, &d), ("Cut", other, ctx, &d)]
        )
        .unwrap_err()
        .to_string()
        .contains("budget")
    );
}

#[test]
fn section_fills_allow_the_full_seven_vertex_crop_budget() {
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(-1.0, -1.0),
            max: Point2::new(1.0, 1.0),
        }),
        ..context()
    };
    let mut input = fill([[0.5, 2.0], [-2.0, -0.5], [1.5, -1.5]]);
    input.triangles = vec![input.triangles[0]; 50_000];
    let d = drawing(ctx).with_section_fills(vec![input]).unwrap();
    let polygons = d.section_fills(ctx).unwrap();
    assert_eq!(polygons.len(), 50_000);
    assert!(
        polygons
            .iter()
            .all(|p| p.points.len() == 7 && area(&p.points) > 0.0)
    );
    assert_eq!(
        polygons.iter().map(|p| p.points.len()).sum::<usize>(),
        350_000
    );
}

#[test]
fn section_fills_preserve_exact_positive_orientation_after_fan_cancellation() {
    let n = 1_000_000.0f64;
    let u = n - f64::from_bits(n.to_bits() - 1);
    let input = fill([[0.0, 0.0], [n - u, n - 2.0 * u], [n, n - u]]);
    // The exact binary determinant is u², but ordinary f64 products cancel.
    assert_eq!(area(&input.triangles[0]), 0.0);
    for crop in [
        None,
        Some(PlanCrop {
            min: Point2::default(),
            max: Point2::new(n, n),
        }),
    ] {
        let ctx = PlanContext { crop, ..context() };
        let d = drawing(ctx)
            .with_section_fills(vec![input.clone()])
            .unwrap();
        let polygons = d.section_fills(ctx).unwrap();
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].points, input.triangles[0]);
    }
    let mut clockwise = input;
    clockwise.triangles[0].swap(1, 2);
    assert!(
        drawing(context())
            .with_section_fills(vec![clockwise])
            .is_err()
    );
}

#[test]
fn section_fills_keep_large_oblique_clips_distinct_and_fail_on_precision_collapse() {
    let n = 1_000_000.0f64;
    let u = n - f64::from_bits(n.to_bits() - 1);
    // A long, oblique strip: crop changes its triangles' first edge order.
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(n - 8.0, n - 8.0),
            max: Point2::new(n - 2.0, n - 2.0),
        }),
        ..context()
    };
    let d = drawing(ctx)
        .with_section_fills(vec![fill([[n - 10.0, n - 10.0], [n, n - 4.0 * u], [n, n]])])
        .unwrap();
    let polygons = d.section_fills(ctx).unwrap();
    assert_eq!(polygons.len(), 1);
    assert!((3..=7).contains(&polygons[0].points.len()));
    for (i, p) in polygons[0].points.iter().enumerate() {
        assert!(!polygons[0].points[..i].contains(p));
        assert!(p.x >= n - 8.0 && p.x <= n - 2.0 && p.y >= n - 8.0 && p.y <= n - 2.0);
    }
    // This exact positive sliver is thinner than representable output near N.
    // It must report precision failure instead of becoming an empty drawing.
    let collapsing = fill([[0.0, 0.0], [n - u, n - 2.0 * u], [n, n - u]]);
    let error = match drawing(ctx).with_section_fills(vec![collapsing]) {
        Ok(_) => panic!("unrepresentable clipped sliver must fail closed"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("roundoff"));
}

#[test]
fn section_fills_classify_oblique_corner_tangency_without_phantom_material() {
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(1.0, 1.0),
            max: Point2::new(2.0, 2.0),
        }),
        ..context()
    };
    let d = drawing(ctx)
        .with_section_fills(vec![fill([[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]])])
        .unwrap();
    assert!(d.section_fills(ctx).unwrap().is_empty());
}

#[test]
fn section_fill_envelope_survives_crop_and_sheet_viewport_projection() {
    for sign in [-1.0, 1.0] {
        let x = sign * 3_000_000.0;
        let ctx = PlanContext {
            crop: Some(PlanCrop {
                min: Point2::new(x - 1.0, -1.0),
                max: Point2::new(x + 1.0, 1.0),
            }),
            ..context()
        };
        let input = fill([[x - 2.0, -2.0], [x + 4.0, -2.0], [x - 2.0, 4.0]]);
        let d = drawing(ctx).with_section_fills(vec![input]).unwrap();
        assert_eq!(d.section_fills(ctx).unwrap().len(), 1);
        let view = PaperViewport {
            model_center_m: Point2::new(x, 0.0),
            ..viewport()
        };
        let page = compose_view_sheet(paper(), "Cut", view, ctx, &d).unwrap();
        let fills: Vec<_> = page
            .marks()
            .iter()
            .filter_map(|m| match &m.kind {
                PaperMarkKind::Path {
                    points_mm,
                    stroke: None,
                    fill: Some(_),
                    ..
                } => Some(points_mm),
                _ => None,
            })
            .collect();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].len(), 4);
        assert!(
            fills[0]
                .iter()
                .all(|p| view.paper_rect().unwrap().contains(*p))
        );
        assert_eq!(
            String::from_utf8(page.to_pdf().unwrap())
                .unwrap()
                .matches("h\nf\n")
                .count(),
            1
        );
    }
    for input in [
        fill([[-4_000_001.0, 0.0], [0.0, 0.0], [0.0, 1.0]]),
        fill([[0.0, -1_000_001.0], [1.0, 0.0], [0.0, 0.0]]),
    ] {
        assert!(drawing(context()).with_section_fills(vec![input]).is_err());
    }
}

#[test]
fn section_fill_exact_orientation_handles_product_underflow_and_extreme_crop() {
    let tiny = f64::from_bits(1);
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(-1e300, -1e300),
            max: Point2::new(1e300, 1e300),
        }),
        ..context()
    };
    let d = drawing(ctx)
        .with_section_fills(vec![fill([[0.0, 0.0], [tiny, 0.0], [0.0, tiny]])])
        .unwrap();
    assert_eq!(d.section_fills(ctx).unwrap().len(), 1);
    assert_eq!(d.section_fills(ctx).unwrap()[0].points.len(), 3);
}

#[test]
fn section_kernel_thin_sloped_triangles_survive_render_crop() {
    use os_geometry::section::{CutContour, CutContourKind, tessellate_contours};
    let base = 999_990.0;
    let height = 4.0 * os_geometry::section::SECTION_TOLERANCE;
    let triangles = tessellate_contours(
        &[CutContour {
            points: vec![
                Point2::new(base, base),
                Point2::new(base + 5.0, base + 5.0),
                Point2::new(base + 5.0, base + 5.0 + height),
                Point2::new(base, base + height),
            ],
            kind: CutContourKind::Outer,
            nesting_depth: 0,
        }],
        2,
    )
    .unwrap();
    assert_eq!(triangles.len(), 2);
    let ctx = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(base + 1.0, base),
            max: Point2::new(base + 4.0, base + 6.0),
        }),
        ..context()
    };
    let d = drawing(ctx)
        .with_section_fills(vec![PlanSectionFill {
            entity: Id::new(),
            component: 0,
            surface: SurfaceIdentity::default(),
            triangles,
        }])
        .unwrap();
    assert_eq!(d.section_fills(ctx).unwrap().len(), 2);
    assert!(
        d.section_fills(ctx)
            .unwrap()
            .iter()
            .all(|p| !p.points.is_empty())
    );
}
