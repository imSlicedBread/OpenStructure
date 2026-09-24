//! Crop acceptance through real desktop egui frames at both supported DPI profiles.
use super::*;
use os_model::PlanViewCrop;

fn fixture(size: egui::Vec2, scale: f32, rotation: f64) -> Harness {
    let mut h = Harness::new(size, scale);
    let v = h.app.editor.document.model().views[&h.view]
        .parameters
        .clone();
    let mut s = v.plan.unwrap();
    s.crop = Some(PlanViewCrop {
        min: Point2::new(-1.0, -1.0),
        max: Point2::new(1.0, 1.0),
    });
    s.basis.rotation = rotation;
    s.scale_denominator = 75.0;
    s.range.top = 4.0;
    h.app
        .editor
        .update_floor_plan(h.view, &v.name, v.level.unwrap(), s)
        .unwrap();
    h.app.plans.cameras.insert(
        h.view,
        PlanCamera {
            center: Point2::default(),
            pixels_per_metre: 60.0,
        },
    );
    h.settle();
    h.app.toggle_crop_edit();
    h.frame(vec![]);
    h
}
fn at(h: &Harness, p: Point2) -> egui::Pos2 {
    let c = h.app.editor.native_plan_context(h.view).unwrap();
    h.point(c.basis.plane_to_world(p).unwrap())
}
fn saved(h: &Harness) -> PlanViewCrop {
    h.app.editor.document.model().views[&h.view]
        .parameters
        .plan
        .unwrap()
        .crop
        .unwrap()
}

#[test]
fn crop_eight_handles_hit_radius_rotated_edges_and_corners() {
    for (size, scale) in PROFILES {
        for rotation in [0.0, 0.63] {
            for index in 0..8 {
                let mut h = fixture(size, scale, rotation);
                let original = saved(&h);
                let p = at(&h, crop::handles(original)[index]);
                assert!(h.output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Circle(c) if c.radius==6.0 && c.center.distance(p)<0.01)));
                for (offset, hit) in [(9.9, true), (10.1, false)] {
                    h.press(p + egui::vec2(offset, 0.0));
                    assert_eq!(
                        h.app.plans.crop.claimed, hit,
                        "handle {index} radius {offset}"
                    );
                    h.release(p + egui::vec2(offset, 0.0));
                    assert!(
                        !h.app.plans.crop.claimed,
                        "claim survived button-up for handle {index} radius {offset}"
                    );
                }
                // A miss is deliberately handled by ordinary canvas selection and may
                // change layout, so start the drag from a fresh, current canvas frame.
                h = fixture(size, scale, rotation);
                let before = h.app.editor.document.model().clone();
                let revision = h.app.editor.document.revision();
                let original = saved(&h);
                let p = at(&h, crop::handles(original)[index]);
                let camera = h.app.plans.cameras[&h.view];
                h.press(p);
                assert!(
                    h.app.plans.crop.claimed,
                    "drag press index={index} rotation={rotation} scale={scale} mode={:?}",
                    h.app.plans.crop.mode
                );
                let target = p + egui::vec2(12.0, -18.0);
                h.frame(vec![egui::Event::PointerMoved(target)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.revision(), revision);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
                assert!(h.app.wall_gesture.is_none());
                assert!(h.app.plans.room_tag_draft.is_none());
                h.release(target);
                let c = saved(&h);
                let close = |a: f64, b: f64| assert!((a - b).abs() < 1e-6, "{a} != {b}");
                close(
                    c.min.x,
                    original.min.x + if [0, 6, 7].contains(&index) { 0.2 } else { 0.0 },
                );
                close(
                    c.max.x,
                    original.max.x + if [2, 3, 4].contains(&index) { 0.2 } else { 0.0 },
                );
                close(
                    c.min.y,
                    original.min.y + if [0, 1, 2].contains(&index) { 0.3 } else { 0.0 },
                );
                close(
                    c.max.y,
                    original.max.y + if [4, 5, 6].contains(&index) { 0.3 } else { 0.0 },
                );
                assert_eq!(h.app.editor.document.revision(), revision + 1);
                let committed = h.app.editor.document.model().clone();
                let mut normalized = committed.clone();
                normalized
                    .views
                    .insert(h.view, before.views[&h.view].clone());
                assert_eq!(normalized, before);
                let mut v = committed.views[&h.view].parameters.clone();
                v.plan.as_mut().unwrap().crop = Some(original);
                v.settings_revision = before.views[&h.view].parameters.settings_revision;
                assert_eq!(v, before.views[&h.view].parameters);
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &before);
                h.app.history(true);
                assert_eq!(h.app.editor.document.model(), &committed);
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("crop.osb");
                h.app.editor.save(&path).unwrap();
                h.app.editor.open(&path).unwrap();
                assert_eq!(h.app.editor.document.model(), &committed);
            }
        }
    }
}

#[test]
fn crop_cancel_keeps_pointer_until_release_and_never_edits() {
    for (size, scale) in PROFILES {
        for reason in [
            "collapse", "outside", "escape", "mode", "view", "revision", "settings", "session",
            "provider", "drawing", "context", "missing",
        ] {
            let mut h = fixture(size, scale, 0.0);
            let camera = h.app.plans.cameras[&h.view];
            let p = at(&h, Point2::new(1.0, 0.0));
            h.press(p);
            assert!(h.app.plans.crop.claimed);
            let mut target = p + egui::vec2(24.0, 0.0);
            h.frame(vec![egui::Event::PointerMoved(target)]);
            match reason {
                "collapse" => target = at(&h, Point2::new(-1.0, 0.0)),
                "outside" => {
                    target =
                        h.app.plans.canvas_rect.unwrap().right_bottom() + egui::vec2(30.0, 30.0)
                }
                "escape" => h.frame(vec![escape()]),
                "mode" => h.app.toggle_crop_edit(),
                "view" => h.app.focus_plan(None),
                "revision" => h
                    .app
                    .editor
                    .command("change", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "settings" => {
                    let v = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .clone();
                    let mut s = v.plan.unwrap();
                    s.visibility.walls = false;
                    h.app
                        .editor
                        .update_floor_plan(h.view, &v.name, v.level.unwrap(), s)
                        .unwrap();
                }
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "missing" => h.app.plans.drawing = None,
                "drawing" | "context" => {
                    let mut c = h.app.plans.desired.unwrap();
                    if reason == "context" {
                        c.model_revision += 1;
                    }
                    h.app.plans.drawing =
                        Some(PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![]).unwrap());
                }
                _ => unreachable!(),
            }
            let expected = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.frame(vec![egui::Event::PointerMoved(target)]);
            assert!(h.app.plans.crop.claimed, "{reason}");
            if reason != "session" {
                assert_eq!(h.app.plans.cameras[&h.view], camera, "{reason}");
            }
            h.release(target);
            assert_eq!(h.app.editor.document.model(), &expected, "{reason}");
            assert_eq!(h.app.editor.document.revision(), revision, "{reason}");
            assert!(!h.app.plans.crop.claimed, "{reason}");
            assert!(h.app.wall_gesture.is_none());
        }
    }
}

#[test]
fn crop_expansion_regenerates_clipping_and_picking_and_empty_drag_pans() {
    for (size, scale) in PROFILES {
        let mut h = fixture(size, scale, 0.0);
        let mut wall = h.app.editor.document.model().walls[&h.wall]
            .parameters
            .clone();
        wall.end = Point2::new(2.0, 0.0);
        h.app
            .editor
            .command(
                "long wall",
                Command::UpdateWall {
                    id: h.wall,
                    parameters: wall,
                },
            )
            .unwrap();
        h.settle();
        let pick = |h: &Harness, p: Point2| {
            let c = h.app.editor.native_plan_context(h.view).unwrap();
            h.app.plans.drawing.as_ref().unwrap().pick(c, p).unwrap()
        };
        assert_eq!(pick(&h, Point2::new(1.5, 0.0)), None);
        let p = at(&h, Point2::new(1.0, 0.0));
        let target = at(&h, Point2::new(1.8, 0.0));
        h.press(p);
        h.frame(vec![egui::Event::PointerMoved(target)]);
        assert_eq!(pick(&h, Point2::new(1.5, 0.0)), None);
        h.release(target);
        h.settle();
        assert_eq!(pick(&h, Point2::new(1.5, 0.0)), Some(h.wall));
        h.press(at(&h, Point2::new(1.8, 0.0)));
        h.release(at(&h, Point2::new(0.5, 0.0)));
        h.settle();
        assert_eq!(pick(&h, Point2::new(1.5, 0.0)), None);
        assert_eq!(pick(&h, Point2::new(0.3, 0.0)), Some(h.wall));
        let original = saved(&h);
        let camera = h.app.plans.cameras[&h.view];
        let p = at(&h, Point2::new(0.0, 0.5));
        h.press(p);
        h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(30.0, 20.0))]);
        h.release(p + egui::vec2(30.0, 20.0));
        assert_ne!(h.app.plans.cameras[&h.view], camera);
        assert_eq!(saved(&h), original);
    }
}
