use super::*;
use os_model::{DimensionLayout, WallPath};
use std::f64::consts::PI;

fn fixture(size: egui::Vec2, scale: f32, sweep: f64) -> Harness {
    let mut h = Harness::new(size, scale);
    let mut m = h.app.editor.document.model().clone();
    m.walls.get_mut(&h.wall).unwrap().parameters.path = WallPath::CircularArc {
        center: Point2::new(0.0, 0.0),
        radius: 1.0,
        start_angle_rad: 0.0,
        signed_sweep_rad: sweep,
    };
    reload(&mut h, m);
    h
}
fn reload(h: &mut Harness, model: os_model::Model) {
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(None);
    h.settle();
}
fn pick(h: &Harness, p: egui::Pos2) -> Result<DimensionAnchorDraft> {
    h.app.arc_length_anchor_at(
        h.app.plans.drawing.as_ref().unwrap(),
        h.app.editor.native_plan_context(h.view).unwrap(),
        h.app.plans.cameras[&h.view],
        h.app.plans.canvas_rect.unwrap(),
        p,
    )
}
fn choose(h: &mut Harness) {
    h.app.begin_dimension(h.view, DimensionLayout::ArcLength);
    h.frame(vec![]);
    let path = h.app.editor.document.model().walls[&h.wall].parameters.path;
    h.click(h.point(path.point(path.length() * 0.3)));
    assert!(
        h.app.plans.dimension_draft.as_ref().unwrap().placing,
        "{}",
        h.app.status
    );
}

#[test]
fn arc_length_authoring_is_available_from_the_compact_dimension_menu() {
    let (size, scale) = PROFILES[0];
    let mut h = Harness::new(size, scale);
    h.frame(vec![]);
    h.click_text_in_band("Architecture", 28.0, 60.0);
    h.click_text_in_band("Dimensions…", 60.0, 150.0);
    h.frame(vec![]);
    h.click_text_in_band("Arc length", 0.0, h.size.y);

    assert_eq!(
        h.app.plans.dimension_draft.as_ref().unwrap().layout,
        DimensionLayout::ArcLength
    );
}

fn create(h: &mut Harness) -> Id {
    choose(h);
    h.frame(vec![]);
    h.click(h.point(Point2::new(1.45, 0.0)));
    h.settle();
    assert!(!h.app.status_error, "{}", h.app.status);
    h.app.selected.unwrap()
}

#[test]
fn arc_length_real_egui_authoring_preview_history_properties_and_split_refresh() {
    for (size, scale) in PROFILES {
        for sweep in [PI / 2.0, -PI / 2.0, 1.5 * PI, -1.5 * PI] {
            let mut h = fixture(size, scale, sweep);
            h.app.plans.split = true;
            h.settle();
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let revision = h.app.editor.document.revision();
            let history = h.app.editor.document.history_stats();
            choose(&mut h);
            h.frame(vec![egui::Event::PointerMoved(
                h.point(Point2::new(1.45, 0.0)),
            )]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.revision(), revision);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert!(h.output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text==format!("{:.3} m",sweep.abs()))));
            h.click(h.point(Point2::new(1.45, 0.0)));
            h.settle();
            assert!(!h.app.status_error, "{}", h.app.status);
            let id = h.app.selected.unwrap();
            let committed = h.app.editor.document.model().clone();
            let p = &committed.dimensions[&id].parameters;
            assert_eq!(
                p.first.wall_endpoint(),
                Some((h.wall, DimensionEndpoint::Start))
            );
            assert_eq!(
                p.second.wall_endpoint(),
                Some((h.wall, DimensionEndpoint::End))
            );
            assert!((p.offset_m - 0.45).abs() < 1e-5);
            assert_eq!(p.resolve(&committed).unwrap().length_metres, sweep.abs());
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            assert_eq!(h.app.editor.scene, scene);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &committed);
            h.settle();
            h.app.select(Some(id));
            h.app.dimension_offset_draft = -1.0;
            h.app.apply_dimension_properties();
            assert!(h.app.status_error);
            assert_eq!(h.app.editor.document.model(), &committed);
            h.app.dimension_offset_draft = -0.3;
            h.app.apply_dimension_properties();
            assert!(!h.app.status_error);
            let mut wall = committed.walls[&h.wall].parameters.clone();
            if let WallPath::CircularArc { radius, .. } = &mut wall.path {
                *radius = 1.2;
            }
            h.app
                .editor
                .command(
                    "Radius",
                    Command::UpdateWall {
                        id: h.wall,
                        parameters: wall,
                    },
                )
                .unwrap();
            h.settle();
            assert_ne!(h.app.editor.scene[&h.wall], scene[&h.wall]);
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            let graphics = h
                .app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .arc_length_dimensions(context)
                .unwrap();
            assert!((graphics[0].value_m - 1.2 * sweep.abs()).abs() < 1e-12);
            assert!((graphics[0].radius - 0.9).abs() < 1e-12);
        }
    }
}

#[test]
fn arc_length_real_egui_cancel_stale_claim_and_ordinary_pan() {
    for (size, scale) in PROFILES {
        for cause in [
            "escape", "pointer", "revision", "session", "view", "drawing", "provider",
        ] {
            let mut h = fixture(size, scale, 1.5 * PI);
            choose(&mut h);
            let original = h.app.editor.document.model().clone();
            let p = h.point(Point2::new(1.4, 0.0));
            h.press(p);
            let camera = h.app.plans.cameras[&h.view];
            let selection = h.app.selected;
            match cause {
                "escape" => h.frame(vec![escape()]),
                "pointer" => h.frame(vec![egui::Event::PointerGone]),
                "revision" => {
                    h.app
                        .editor
                        .command("Edit", Command::RenameProject("Edited".into()))
                        .unwrap();
                }
                "session" => {
                    h.app.editor.document = Document::from_model(original.clone()).unwrap()
                }
                "view" => h.app.focus_plan(None),
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                _ => h
                    .app
                    .plans
                    .dimension_draft
                    .as_mut()
                    .unwrap()
                    .provider_signature
                    .push(("changed".into(), Id::new())),
            }
            h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(40.0, 30.0))]);
            h.release(p + egui::vec2(40.0, 30.0));
            h.frame(vec![]);
            assert!(h.app.plans.dimension_draft.is_none(), "{cause}");
            assert!(
                h.app.editor.document.model().dimensions.is_empty(),
                "{cause}"
            );
            if cause != "session" && cause != "view" {
                assert_eq!(h.app.plans.cameras[&h.view], camera, "{cause}");
            }
            assert_eq!(h.app.selected, selection, "{cause}");
        }
        let mut h = fixture(size, scale, PI);
        choose(&mut h);
        let before = h.app.editor.document.model().clone();
        h.click(h.point(Point2::new(0.0, 0.0)));
        assert!(h.app.status_error);
        assert_eq!(h.app.editor.document.model(), &before);
        h.frame(vec![key_pressed(egui::Key::Backspace)]);
        let d = h.app.plans.dimension_draft.as_ref().unwrap();
        assert!(!d.placing && d.first.is_none() && d.second.is_none());
        h.frame(vec![escape()]);
        h.frame(vec![]);
        h.app.select(None);
        let p = h.point(Point2::new(-0.5, -0.7));
        let camera = h.app.plans.cameras[&h.view];
        h.press(p);
        h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(35.0, 20.0))]);
        h.release(p + egui::vec2(35.0, 20.0));
        assert_ne!(h.app.plans.cameras[&h.view], camera);
    }
}

#[test]
fn arc_length_real_egui_paired_repair_preserves_identity_offset_and_history() {
    for (size, scale) in PROFILES {
        let mut h = fixture(size, scale, 1.5 * PI);
        let id = create(&mut h);
        let mut m = h.app.editor.document.model().clone();
        let mut replacement = m.walls[&h.wall].clone();
        replacement.header.id = Id::new();
        replacement.parameters.path = WallPath::CircularArc {
            center: Point2::new(0.0, 0.0),
            radius: 0.55,
            start_angle_rad: 0.0,
            signed_sweep_rad: -1.5 * PI,
        };
        let target = replacement.id();
        m.walls.insert(target, replacement);
        m.walls.remove(&h.wall);
        m.element_lifecycles.remove(&h.wall);
        reload(&mut h, m);
        h.app.select(Some(id));
        h.settle();
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        h.app.begin_dimension_repair(0);
        h.frame(vec![]);
        let p = h.point(Point2::new(0.55, 0.0));
        h.frame(vec![egui::Event::PointerMoved(p)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        h.click(p);
        h.settle();
        assert!(!h.app.status_error, "{}", h.app.status);
        let repaired = h.app.editor.document.model().clone();
        let d = &repaired.dimensions[&id];
        assert_eq!(d.header, before.dimensions[&id].header);
        let mut expected = before.dimensions[&id].parameters.clone();
        expected.first = DimensionReference::WallEndpoint {
            wall: target,
            endpoint: DimensionEndpoint::Start,
        };
        expected.second = DimensionReference::WallEndpoint {
            wall: target,
            endpoint: DimensionEndpoint::End,
        };
        assert_eq!(d.parameters, expected);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &repaired);
    }
}

#[test]
fn arc_length_pick_radius_visibility_crop_level_occlusion_and_ties() {
    for (size, scale) in PROFILES {
        let mut h = fixture(size, scale, PI);
        // Outer endpoint is in the actual clipped footprint; tolerance uses logical pixels.
        let edge = h.point(Point2::new(1.135, 0.0));
        assert!(pick(&h, edge + egui::vec2(11.9, 0.0)).is_ok());
        assert!(pick(&h, edge + egui::vec2(12.1, 0.0)).is_err());
        let base = h.app.editor.document.model().clone();
        for case in ["hidden", "crop", "level", "straight"] {
            let mut m = base.clone();
            let plan = m
                .views
                .get_mut(&h.view)
                .unwrap()
                .parameters
                .plan
                .as_mut()
                .unwrap();
            match case {
                "hidden" => plan.visibility.walls = false,
                "crop" => {
                    plan.crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(-1.0, -0.5),
                        max: Point2::new(-0.5, 1.5),
                    })
                }
                "level" => {
                    let mut l = m.levels[&m.walls[&h.wall].parameters.level].clone();
                    l.header.id = Id::new();
                    m.walls.get_mut(&h.wall).unwrap().parameters.level = l.id();
                    m.levels.insert(l.id(), l);
                }
                _ => {
                    m.walls.get_mut(&h.wall).unwrap().parameters.path = WallPath::Straight {
                        start: Point2::new(-1.0, 0.0),
                        end: Point2::new(1.0, 0.0),
                    }
                }
            }
            reload(&mut h, m);
            assert!(pick(&h, h.point(Point2::new(1.0, 0.0))).is_err(), "{case}");
        }
        let mut m = base.clone();
        let mut second = m.walls[&h.wall].clone();
        second.header.id = Id::new();
        let other = second.id();
        m.walls.insert(other, second);
        reload(&mut h, m);
        assert_eq!(
            pick(&h, h.point(Point2::new(1.0, 0.0)))
                .unwrap()
                .reference
                .entity(),
            h.wall.min(other)
        );
        let mut m = base;
        let line = os_model::DetailLine::new(
            "core.detail_line",
            os_model::DetailLineParams {
                view: h.view,
                start: Point2::new(1.0, -0.3),
                end: Point2::new(1.0, 0.3),
            },
        );
        m.detail_lines.insert(line.id(), line);
        reload(&mut h, m);
        assert!(
            pick(&h, h.point(Point2::new(1.0, 0.0))).is_err(),
            "foreground drafting line blocks curve"
        );
    }
}

#[test]
fn arc_length_shared_sheet_pdf_rotated_basis_crop_and_large_coordinates() {
    let mut h = fixture(PROFILES[0].0, PROFILES[0].1, -1.5 * PI);
    let id = create(&mut h);
    let mut m = h.app.editor.document.model().clone();
    let shift = Point2::new(100000.0, -100000.0);
    if let WallPath::CircularArc { center, .. } =
        &mut m.walls.get_mut(&h.wall).unwrap().parameters.path
    {
        *center = shift;
    }
    let plan = m
        .views
        .get_mut(&h.view)
        .unwrap()
        .parameters
        .plan
        .as_mut()
        .unwrap();
    plan.basis = os_model::PlanViewBasis {
        origin: shift,
        rotation: 0.73,
    };
    plan.crop = Some(os_model::PlanViewCrop {
        min: Point2::new(-2.0, -2.0),
        max: Point2::new(0.5, 2.0),
    });
    reload(&mut h, m);
    let ctx = h.app.editor.native_plan_context(h.view).unwrap();
    let drawing = h.app.plans.drawing.as_ref().unwrap();
    let arc = &drawing.arc_length_dimensions(ctx).unwrap()[0];
    assert_eq!(arc.entity, id);
    assert!((arc.value_m - 1.5 * PI).abs() < 1e-12);
    assert!((arc.start_radians + 0.73).abs() < 1e-10);
    assert_eq!(arc.sweep_radians, -1.5 * PI);
    let page = compose_sheet_views(
        PaperSheetInfo {
            width_mm: 420.0,
            height_mm: 297.0,
            number: "A1",
            name: "Arc",
        },
        &[(
            "Plan",
            PaperViewport {
                center_mm: Point2::new(210.0, 130.0),
                width_mm: 360.0,
                height_mm: 220.0,
                model_center_m: Point2::new(0.0, 0.0),
                scale_denominator: 20.0,
            },
            ctx,
            drawing,
        )],
    )
    .unwrap();
    assert!(
        page.marks()
            .iter()
            .any(|m| matches!(&m.kind,PaperMarkKind::Text{text,..} if text==&arc.label()))
    );
    let pdf = page.to_pdf().unwrap();
    let hex: String = arc.label().bytes().map(|b| format!("{b:02X}")).collect();
    assert!(String::from_utf8_lossy(&pdf).contains(&format!("<{hex}> Tj")));
}
