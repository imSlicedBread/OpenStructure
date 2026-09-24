use super::{paint_plan_line, paint_sheet_page};
use eframe::egui;
use os_core::Point2;
use os_render::plan::PlanStroke;
use os_render::sheet::{PaperColor, PaperMark, PaperMarkKind, PaperStroke, SheetPage};

fn line_shapes(
    start: egui::Pos2,
    end: egui::Pos2,
    pixels_per_paper_mm: f32,
) -> (Vec<([egui::Pos2; 2], egui::Stroke)>, f32) {
    let context = egui::Context::default();
    let mut phase_mm = 0.0;
    let output = context.run(egui::RawInput::default(), |context| {
        let painter = context.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("plan-graphics-test"),
        ));
        paint_plan_line(
            &painter,
            start,
            end,
            Some(PlanStroke {
                color: [20, 120, 220],
                weight_mm: 0.5,
                dashed: true,
            }),
            egui::Stroke::new(1.0, egui::Color32::BLACK),
            pixels_per_paper_mm,
            &mut phase_mm,
        )
        .unwrap();
    });
    let segments = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, stroke } => Some((*points, *stroke)),
            _ => None,
        })
        .collect();
    (segments, phase_mm)
}

#[test]
fn dashed_canvas_strokes_keep_paper_pattern_and_phase_at_two_screen_scales() {
    for scale in [1.0, 2.0] {
        let (first, phase) =
            line_shapes(egui::pos2(0.0, 0.0), egui::pos2(12.0 * scale, 0.0), scale);
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].0[0], egui::pos2(0.0, 0.0));
        assert_eq!(first[0].0[1].x, 3.0 * scale);
        assert_eq!(first[1].0[0].x, 4.5 * scale);
        assert_eq!(first[2].0[1].x, 12.0 * scale);
        assert_eq!(first[0].1.width, 0.5 * scale);
        assert_eq!(first[0].1.color, egui::Color32::from_rgb(20, 120, 220));
        assert_eq!(phase, 3.0);

        let (continued, _) = line_shapes_with_phase(12.0 * scale, 6.0 * scale, scale, phase);
        assert_eq!(continued.len(), 1);
        assert_eq!(continued[0].0[0].x, 13.5 * scale);
        assert_eq!(continued[0].0[1].x, 16.5 * scale);
    }
}

#[test]
fn dashed_paper_strokes_remain_dashed_in_the_sheet_preview() {
    let page = SheetPage::new(
        100.0,
        100.0,
        vec![PaperMark {
            clip: None,
            kind: PaperMarkKind::Path {
                points_mm: vec![Point2::new(10.0, 10.0), Point2::new(22.0, 10.0)],
                closed: false,
                stroke: Some(PaperStroke {
                    color: PaperColor {
                        red: 20,
                        green: 120,
                        blue: 220,
                    },
                    width_mm: 0.5,
                    dashed: true,
                }),
                fill: None,
            },
        }],
    )
    .unwrap();
    let context = egui::Context::default();
    let output = context.run(egui::RawInput::default(), |context| {
        let painter = context.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("sheet-preview-plan-style-test"),
        ));
        paint_sheet_page(
            &painter,
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0)),
            &page,
        )
        .unwrap();
    });
    let segments: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, stroke } => Some((*points, *stroke)),
            _ => None,
        })
        .collect();
    assert_eq!(segments.len(), 3);
    assert!((segments[0].0[0].x - 39.2).abs() < 1e-4);
    assert!((segments[0].0[1].x - 43.76).abs() < 1e-4);
    assert_eq!(segments[0].1.color, egui::Color32::from_rgb(20, 120, 220));
    assert!((segments[0].1.width - 0.76).abs() < 1e-4);
}

fn line_shapes_with_phase(
    start_x: f32,
    length: f32,
    pixels_per_paper_mm: f32,
    mut phase_mm: f32,
) -> (Vec<([egui::Pos2; 2], egui::Stroke)>, f32) {
    let context = egui::Context::default();
    let output = context.run(egui::RawInput::default(), |context| {
        let painter = context.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("plan-graphics-phase-test"),
        ));
        paint_plan_line(
            &painter,
            egui::pos2(start_x, 0.0),
            egui::pos2(start_x + length, 0.0),
            Some(PlanStroke {
                color: [20, 120, 220],
                weight_mm: 0.5,
                dashed: true,
            }),
            egui::Stroke::new(1.0, egui::Color32::BLACK),
            pixels_per_paper_mm,
            &mut phase_mm,
        )
        .unwrap();
    });
    let segments = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, stroke } => Some((*points, *stroke)),
            _ => None,
        })
        .collect();
    (segments, phase_mm)
}
