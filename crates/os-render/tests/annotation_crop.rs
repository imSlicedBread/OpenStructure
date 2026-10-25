use os_core::{Id, Point2};
use os_geometry::plan::*;
use os_render::{plan::*, sheet::*, snapping::*};
use std::collections::BTreeMap;

fn rect(r: f64) -> PlanCrop {
    PlanCrop {
        min: Point2::new(-r, -r),
        max: Point2::new(r, r),
    }
}
fn fixture() -> (PlanContext, PlanDrawing, [Id; 4]) {
    let c = PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: Some(rect(1.)),
        annotation_crop: Some(rect(2.)),
        scale_denominator: 50.,
        show_walls: true,
        show_extensions: false,
    };
    let ids = [Id::new(), Id::new(), Id::new(), Id::new()];
    let mut note = os_model::TextNoteParams::new(c.view_id);
    note.position = Point2::new(1.9, 1.5);
    note.text = "ABCD".into();
    note.text_height_mm = 2.;
    note.leader = Some(Point2::new(3., 0.));
    let d = PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![])
        .unwrap()
        .with_detail_lines(vec![PlanDetailLine {
            entity: ids[0],
            view: c.view_id,
            start: Point2::new(-3., 1.7),
            end: Point2::new(3., 1.7),
        }])
        .unwrap()
        .with_text_notes(vec![NoteGraphic {
            entity: ids[1],
            parameters: note,
        }])
        .unwrap()
        .with_dimensions(vec![PlanDimensionItem {
            entity: ids[2],
            spans: vec![],
            shared_start_witness: false,
            witness_start: Point2::new(-0.5, 0.),
            witness_end: Point2::new(0.5, 0.),
            line_start: Point2::new(-0.5, 1.5),
            line_end: Point2::new(0.5, 1.5),
            value_m: Some(1.),
            orphan_hint: Point2::default(),
            diagnostic: None,
        }])
        .unwrap()
        .with_opening_tags(vec![PlanOpeningTag {
            entity: ids[3],
            view: c.view_id,
            opening: Id::new(),
            anchor: Point2::new(-1.7, 0.),
            leader_source: Some(Point2::default()),
            label: "D1".into(),
            diagnostic: None,
        }])
        .unwrap()
        .with_snap_segments(vec![])
        .unwrap();
    (c, d, ids)
}

#[test]
fn annotation_vectors_native_sheet_pdf_and_viewport_scales_are_identical() {
    let (c, d, ids) = fixture();
    assert_eq!(d.detail_lines(c).unwrap()[0].start, Point2::new(-2., 1.7));
    let camera = PlanCamera {
        center: Point2::default(),
        pixels_per_metre: 100.,
    };
    let size = [800., 600.];
    for (id, p) in [
        (ids[0], Point2::new(1.5, 1.7)),
        (ids[1], Point2::new(1.95, 1.45)),
        (ids[2], Point2::new(0., 1.5)),
        (ids[3], Point2::new(-1.7, -0.0625)),
    ] {
        assert!(
            d.annotation_hits(c, camera, size, camera.project(p, size).unwrap(), 4., |i| i
                == id)
                .unwrap()
                .contains(&id),
            "{p:?}"
        );
    }
    assert!(
        d.annotation_hits(
            c,
            camera,
            size,
            camera.project(Point2::new(2.01, 1.7), size).unwrap(),
            6.,
            |_| true
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(d.pick(c, Point2::new(1.5, 1.7)).unwrap(), None);
    let mut q = SnapQuery {
        camera,
        viewport: size,
        pointer: camera.project(Point2::new(1.5, 1.7), size).unwrap(),
        radius_pixels: 5.,
        endpoints: false,
        midpoints: false,
        intersections: false,
        perpendicular_from: None,
        nearest: true,
        axis_extensions: false,
        exclude_entity: None,
    };
    assert_eq!(
        d.snap(c, q)
            .unwrap()
            .candidate(c, q)
            .unwrap()
            .unwrap()
            .entity,
        ids[0]
    );
    q.pointer = camera.project(Point2::new(3., 1.7), size).unwrap();
    q.nearest = false;
    q.endpoints = true;
    assert!(d.snap(c, q).unwrap().candidate(c, q).unwrap().is_none());
    for scale in [25., 100.] {
        let vp = PaperViewport {
            center_mm: Point2::new(210., 148.5),
            width_mm: 400.,
            height_mm: 280.,
            model_center_m: Point2::default(),
            scale_denominator: scale,
        };
        let expected = annotation_graphics(c, &d, vp, c.annotation_crop.unwrap()).unwrap();
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 420.,
                height_mm: 297.,
                number: "A1",
                name: "Annotations",
            },
            "Plan",
            vp,
            c,
            &d,
        )
        .unwrap();
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        for g in &expected {
            for mark in &g.marks {
                assert!(page.marks().contains(mark));
                let clip = mark.clip.unwrap();
                match &mark.kind {
                    PaperMarkKind::Path { points_mm, .. } => {
                        assert!(points_mm.iter().all(|p| clip.contains(*p)))
                    }
                    PaperMarkKind::Text {
                        baseline_mm, text, ..
                    } => assert!(pdf.contains(&format!(
                        "1 0 0 1 {:.6} {:.6} Tm <{:02X}>",
                        baseline_mm.x * 72. / 25.4,
                        (297. - baseline_mm.y) * 72. / 25.4,
                        text.as_bytes()[0]
                    ))),
                }
            }
        }
        // Native and viewport override use exactly the same vectors after translation.
        let cc = PlanContext {
            scale_denominator: scale,
            ..c
        };
        let native_vp = PlanDrawing::annotation_viewport(cc, Point2::default());
        let native = annotation_graphics(c, &d, native_vp, c.annotation_crop.unwrap()).unwrap();
        for (a, b) in native.iter().zip(&expected) {
            assert_eq!(a.entity, b.entity);
            assert_eq!(a.marks.len(), b.marks.len());
            for (a, b) in a.marks.iter().zip(&b.marks) {
                let delta = Point2::new(
                    native_vp.center_mm.x - vp.center_mm.x,
                    native_vp.center_mm.y - vp.center_mm.y,
                );
                let same = |a: Point2, b: Point2| {
                    assert!(
                        (a.x - b.x - delta.x).abs() < 1e-9 && (a.y - b.y - delta.y).abs() < 1e-9
                    )
                };
                same(a.clip.unwrap().min_mm, b.clip.unwrap().min_mm);
                same(a.clip.unwrap().max_mm, b.clip.unwrap().max_mm);
                match (&a.kind, &b.kind) {
                    (
                        PaperMarkKind::Path {
                            points_mm: a,
                            stroke: sa,
                            ..
                        },
                        PaperMarkKind::Path {
                            points_mm: b,
                            stroke: sb,
                            ..
                        },
                    ) => {
                        assert_eq!(sa, sb);
                        for (a, b) in a.iter().zip(b) {
                            same(*a, *b);
                        }
                    }
                    (
                        PaperMarkKind::Text {
                            baseline_mm: a,
                            size_mm: sa,
                            text: ta,
                            ..
                        },
                        PaperMarkKind::Text {
                            baseline_mm: b,
                            size_mm: sb,
                            text: tb,
                            ..
                        },
                    ) => {
                        same(*a, *b);
                        assert_eq!(sa, sb);
                        assert_eq!(ta, tb);
                    }
                    _ => panic!("different mark kinds"),
                }
            }
        }
    }
}
