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

fn annotation_fixture(size: egui::Vec2, scale: f32) -> Harness {
    let mut h = fixture(size, scale, 0.63);
    let v = h.app.editor.document.model().views[&h.view]
        .parameters
        .clone();
    let mut s = v.plan.unwrap();
    s.annotation_crop = Some(PlanViewCrop {
        min: Point2::new(-2., -2.),
        max: Point2::new(2., 2.),
    });
    h.app
        .editor
        .update_floor_plan(h.view, &v.name, v.level.unwrap(), s)
        .unwrap();
    h.settle();
    h.app.toggle_annotation_crop_edit();
    h.frame(vec![]);
    h
}

#[test]
fn annotation_crop_handles_preview_history_and_pan_both_dpis() {
    for (size, scale) in PROFILES {
        for index in 0..8 {
            let mut h = annotation_fixture(size, scale);
            let annotation = h.app.editor.document.model().views[&h.view]
                .parameters
                .plan
                .unwrap()
                .annotation_crop
                .unwrap();
            let p = at(&h, crop::handles(annotation)[index]);
            for (radius, hit) in [(9.9, true), (10.1, false)] {
                h.press(p + egui::vec2(radius, 0.));
                assert_eq!(h.app.plans.crop.claimed, hit);
                h.release(p + egui::vec2(radius, 0.));
            }
            h = annotation_fixture(size, scale);
            let p = at(&h, crop::handles(annotation)[index]);
            let before = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            let camera = h.app.plans.cameras[&h.view];
            h.press(p);
            h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(12., -12.))]);
            assert!(h.app.plans.crop.claimed);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.revision(), revision);
            assert_eq!(h.app.plans.cameras[&h.view], camera);
            h.release(p + egui::vec2(12., -12.));
            assert_eq!(h.app.editor.document.revision(), revision + 1);
            let after = h.app.editor.document.model().clone();
            assert_eq!(
                saved(&h),
                before.views[&h.view].parameters.plan.unwrap().crop.unwrap()
            );
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &after);
            h.settle();
            let p = at(&h, Point2::new(0., 0.5));
            let camera = h.app.plans.cameras[&h.view];
            h.press(p);
            h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(20., 10.))]);
            h.release(p + egui::vec2(20., 10.));
            assert_ne!(h.app.plans.cameras[&h.view], camera);
            assert_eq!(h.app.editor.document.model(), &after);
        }
    }
}

#[test]
fn annotation_crop_invalid_escape_stale_camera_canvas_cancel() {
    for (size, scale) in PROFILES {
        for reason in [
            "containment",
            "collapse",
            "escape",
            "revision",
            "session",
            "view",
            "camera",
            "canvas",
        ] {
            let mut h = annotation_fixture(size, scale);
            let p = at(&h, Point2::new(2., 0.));
            h.press(p);
            assert!(h.app.plans.crop.claimed);
            let mut target = p + egui::vec2(24., 0.);
            h.frame(vec![egui::Event::PointerMoved(target)]);
            match reason {
                "containment" => target = at(&h, Point2::new(0.8, 0.)),
                "collapse" => target = at(&h, Point2::new(-2., 0.)),
                "escape" => h.frame(vec![escape()]),
                "revision" => {
                    h.app
                        .editor
                        .command("Other edit", Command::RenameProject("Other".into()))
                        .unwrap();
                }
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "view" => h.app.focus_plan(None),
                "camera" => {
                    h.app
                        .plans
                        .cameras
                        .get_mut(&h.view)
                        .unwrap()
                        .pixels_per_metre *= 1.1
                }
                "canvas" => h.size.x -= 25.,
                _ => unreachable!(),
            }
            let expected = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.release(target);
            assert_eq!(h.app.editor.document.model(), &expected, "{reason}");
            assert_eq!(h.app.editor.document.revision(), revision, "{reason}");
        }
    }
}

#[test]
fn annotation_crop_rotated_enlarged_plan_sources_and_native_vectors() {
    use os_model::*;
    use os_render::sheet::PaperMarkKind;
    for (size, dpi) in PROFILES {
        for enlarged in [false, true] {
            let mut h = annotation_fixture(size, dpi);
            h.app.plans.crop.mode = None;
            if enlarged {
                let (_, target, commands) = h
                    .app
                    .editor
                    .document
                    .enlarged_plan_commands(
                        h.view,
                        "Enlarged annotation plan".into(),
                        saved(&h),
                        25.,
                    )
                    .unwrap();
                h.app.editor.document.execute("Enlarge", commands).unwrap();
                h.view = target;
                h.app.focus_plan(Some(target));
                h.app.plans.cameras.insert(
                    target,
                    PlanCamera {
                        center: Point2::default(),
                        pixels_per_metre: 60.,
                    },
                );
                h.settle();
            }
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            let world = |p| context.basis.plane_to_world(p).unwrap();
            let mut wall = h.app.editor.document.model().walls[&h.wall]
                .parameters
                .clone();
            wall.path = WallPath::Straight {
                start: world(Point2::new(-0.8, 0.)),
                end: world(Point2::new(0.8, 0.)),
            };
            h.app
                .editor
                .command(
                    "Rotated wall",
                    Command::UpdateWall {
                        id: h.wall,
                        parameters: wall,
                    },
                )
                .unwrap();
            let dimension = Dimension::new(
                "core.dimension",
                DimensionParams {
                    layout: DimensionLayout::Aligned,
                    additional: vec![],
                    baseline_spacing_m: 0.25,
                    view: h.view,
                    first: DimensionReference::WallEndpoint {
                        wall: h.wall,
                        endpoint: DimensionEndpoint::Start,
                    },
                    second: DimensionReference::WallEndpoint {
                        wall: h.wall,
                        endpoint: DimensionEndpoint::End,
                    },
                    offset_m: 1.5,
                    orphan_hint: world(Point2::new(0., 1.5)),
                },
            );
            let dim = dimension.id();
            let opening = Opening::new(
                "core.opening",
                OpeningParams {
                    name: "D1".into(),
                    host: h.wall,
                    offset: 0.5,
                    definition: OpeningDefinition::Legacy {
                        kind: OpeningKind::Door,
                        width: 0.5,
                        height: 2.1,
                        sill: 0.,
                    },
                    open_state: Default::default(),
                    width_override: None,
                    height_override: None,
                    sill_override: None,
                    pane_position_override: None,
                    lite_side_override: None,
                    hinge: Default::default(),
                    swing: Default::default(),
                },
            );
            let tag = OpeningTag::new(
                "core.opening_tag",
                OpeningTagParams {
                    view: h.view,
                    opening: opening.id(),
                    position: world(Point2::new(-1.6, -0.5)),
                    label_preset: Default::default(),
                },
            );
            let tag_id = tag.id();
            let line = DetailLine::new(
                "core.detail_line",
                DetailLineParams {
                    view: h.view,
                    start: world(Point2::new(-3., 1.8)),
                    end: world(Point2::new(3., 1.8)),
                },
            );
            let line_id = line.id();
            let mut p = TextNoteParams::new(h.view);
            p.text = "AB".into();
            p.position = world(Point2::new(1.6, 0.5));
            p.text_height_mm = 2.;
            let note = TextNote::new("core.text_note", p);
            let note_id = note.id();
            h.app
                .editor
                .document
                .execute(
                    "Annotations",
                    vec![
                        Command::AddDimension(dimension),
                        Command::AddOpening(opening),
                        Command::AddOpeningTag(tag),
                        Command::AddDetailLine(line),
                        Command::AddTextNote(note),
                    ],
                )
                .unwrap();
            h.app.editor.regenerate().unwrap();
            h.app.select(None);
            h.settle();
            let c = h.app.editor.native_plan_context(h.view).unwrap();
            let d = h.app.plans.drawing.as_ref().unwrap();
            let camera = h.app.plans.cameras[&h.view];
            let rect = h.app.plans.canvas_rect.unwrap();
            assert_eq!(d.dimensions(c).unwrap().len(), 1);
            assert_eq!(d.opening_tags(c).unwrap().len(), 1);
            assert_eq!(d.pick(c, Point2::new(1.6, 0.5)).unwrap(), None);
            let native = d.native_annotation_graphics(c, camera.center).unwrap();
            for id in [dim, tag_id, line_id, note_id] {
                assert!(native.iter().any(|g| g.entity == id && !g.marks.is_empty()));
            }
            let px_per_mm = (c.scale_denominator * camera.pixels_per_metre / 1000.) as f32;
            for mark in native.iter().flat_map(|g| &g.marks) {
                if let PaperMarkKind::Text {
                    baseline_mm, text, ..
                } = &mark.kind
                {
                    let expected = rect.center()
                        + egui::vec2(
                            (baseline_mm.x - 2500.) as f32 * px_per_mm,
                            (baseline_mm.y - 2500.) as f32 * px_per_mm,
                        );
                    assert!(h.output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==text && (t.pos.x-expected.x).abs()<0.02)),"missing native text {text}");
                }
            }
            // Shrinking only the model boundary hides ineligible sources while
            // independent drafting stays visible in the annotation boundary.
            let v = h.app.editor.document.model().views[&h.view]
                .parameters
                .clone();
            let mut s = v.plan.unwrap();
            s.crop = Some(PlanViewCrop {
                min: Point2::new(0.5, 0.5),
                max: Point2::new(0.8, 0.8),
            });
            h.app
                .editor
                .update_floor_plan(h.view, &v.name, v.level.unwrap(), s)
                .unwrap();
            h.settle();
            let c = h.app.editor.native_plan_context(h.view).unwrap();
            let d = h.app.plans.drawing.as_ref().unwrap();
            assert!(d.items(c).unwrap().is_empty());
            assert!(d.dimensions(c).unwrap().is_empty());
            assert!(d.opening_tags(c).unwrap().is_empty());
            assert_eq!(d.detail_lines(c).unwrap().len(), 1);
            assert_eq!(d.text_notes(c).unwrap().len(), 1);
        }
    }
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
        let reasons: Vec<_> = [
            "collapse", "outside", "escape", "mode", "view", "revision", "settings", "session",
            "drawing", "context", "missing",
        ]
        .into_iter()
        .chain(std::iter::once("provider").filter(|_| cfg!(feature = "external-plugins")))
        .collect();
        for reason in reasons {
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
        *wall.path.straight_end_mut().unwrap() = Point2::new(2.0, 0.0);
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
