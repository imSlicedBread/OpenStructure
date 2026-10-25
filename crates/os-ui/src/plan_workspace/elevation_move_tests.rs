use super::*;

#[test]
fn elevation_move_stale_contexts_and_horizontal_pins() {
    for profile in 0..2 {
        for mode in [
            "pins", "revision", "session", "view", "drawing", "provider", "canvas", "modal",
            "settings",
        ] {
            let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
            let mut parameters = h.app.editor.document.model().openings[&id]
                .parameters
                .clone();
            parameters.width_override = Some(1.2);
            parameters.height_override = Some(1.5);
            parameters.sill_override = Some(1.);
            h.app
                .editor
                .command("Pins", Command::UpdateOpening { id, parameters })
                .unwrap();
            h.settle();
            h.app.select(Some(id));
            let p = start(&mut h);
            let to = p + egui::vec2(25., 0.);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            match mode {
                "revision" => {
                    h.app
                        .editor
                        .command("Revision", Command::RenameProject("Changed".into()))
                        .unwrap();
                }
                "session" => {
                    h.app.editor.document =
                        os_document::Document::from_model(h.app.editor.document.model().clone())
                            .unwrap();
                }
                "view" => {
                    h.app.focus_plan(None);
                    h.app.focus_plan(Some(view));
                }
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_drawing(view).unwrap());
                }
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "canvas" => {
                    h.size.x -= 25.;
                }
                "modal" => h.app.cancel_plan_authoring_for_modal(),
                "settings" => {
                    let mut parameters = h.app.editor.document.model().views[&view]
                        .parameters
                        .clone();
                    parameters.elevation.as_mut().unwrap().max_station = 9.;
                    h.app
                        .editor
                        .command(
                            "Crop",
                            Command::UpdateView {
                                id: view,
                                parameters,
                            },
                        )
                        .unwrap();
                }
                _ => {}
            }
            let before = h.app.editor.document.model().clone();
            let stats = h.app.editor.document.history_stats();
            h.frame(vec![button(to, false)]);
            if mode == "pins" {
                h.settle();
                let mut expected = before.openings[&id].parameters.clone();
                expected.offset = h.app.editor.document.model().openings[&id]
                    .parameters
                    .offset;
                assert_ne!(expected.offset, 3.);
                assert_eq!(
                    h.app.editor.document.model().openings[&id].parameters,
                    expected
                );
            } else {
                assert!(h.app.plans.elevation.moving.is_none(), "{mode}");
                assert_eq!(h.app.editor.document.model(), &before, "{mode}");
                assert_eq!(h.app.editor.document.history_stats(), stats, "{mode}");
            }
        }
    }
}

fn start(h: &mut Harness) -> egui::Pos2 {
    h.click("Move opening");
    let p = h.point(Point2::new(3.35, 4.4));
    h.frame(vec![egui::Event::PointerMoved(p), button(p, true)]);
    assert!(h.app.plans.elevation.move_claimed, "{}", h.app.status);
    p
}

#[test]
fn elevation_move_lifecycle_both_dpis_sides_kinds_definitions() {
    for profile in 0..2 {
        for side in [ElevationSide::Left, ElevationSide::Right] {
            for typed in [false, true] {
                for kind in [OpeningKind::Door, OpeningKind::Window] {
                    let (mut h, id, view) = fixture(profile, kind, typed);
                    let mut vp = h.app.editor.document.model().views[&view]
                        .parameters
                        .clone();
                    vp.elevation.as_mut().unwrap().side = side;
                    h.app
                        .editor
                        .command(
                            "Side",
                            Command::UpdateView {
                                id: view,
                                parameters: vp,
                            },
                        )
                        .unwrap();
                    h.settle();
                    let before = h.app.editor.document.model().clone();
                    let stats = h.app.editor.document.history_stats();
                    let scene = h.app.editor.scene.clone();
                    let p = start(&mut h);
                    let camera = h.app.plans.cameras[&view];
                    let to = p + egui::vec2(
                        (0.7 * camera.pixels_per_metre) as f32,
                        (-0.2 * camera.pixels_per_metre) as f32,
                    );
                    h.frame(vec![egui::Event::PointerMoved(to)]);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats(), stats);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.plans.cameras[&view], camera);
                    let context = h.app.editor.native_elevation_context(view).unwrap();
                    let preview = h
                        .app
                        .elevation_move_input(
                            &h.ctx,
                            camera,
                            h.app.plans.canvas_rect.unwrap(),
                            context,
                        )
                        .unwrap();
                    h.frame(vec![button(to, false)]);
                    h.settle();
                    let after = h.app.editor.document.model().clone();
                    let r = after
                        .resolve_opening(&after.openings[&id].parameters)
                        .unwrap();
                    assert!((r.offset - 3.7).abs() < 1e-5, "{}", h.app.status);
                    assert!(
                        (r.sill - if kind == OpeningKind::Door { 0. } else { 1.2 }).abs() < 1e-5
                    );
                    assert_eq!((r.width, r.height), (1.2, 1.5));
                    let mut expected = before.openings[&id].clone();
                    expected.parameters.offset = after.openings[&id].parameters.offset;
                    if kind == OpeningKind::Window {
                        if typed {
                            expected.parameters.sill_override = Some(r.sill);
                        } else if let OpeningDefinition::Legacy { sill, .. } =
                            &mut expected.parameters.definition
                        {
                            *sill = r.sill;
                        }
                    }
                    assert_eq!(expected, after.openings[&id]);
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        stats.undo_entries + 1
                    );
                    assert_eq!(
                        preview.provider_lines(context).unwrap(),
                        h.app
                            .editor
                            .native_drawing(view)
                            .unwrap()
                            .provider_lines(h.app.editor.native_elevation_context(view).unwrap())
                            .unwrap()
                    );
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &after);
                    assert_ne!(h.app.editor.scene, scene);
                    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
                    let path = dir.path().join("elevation-move.osb");
                    h.app.editor.save(&path).unwrap();
                    h.app.editor.open(&path).unwrap();
                    assert_eq!(h.app.editor.document.model(), &after);
                }
            }
        }
    }
}

#[test]
fn elevation_move_cancel_noop_invalid_and_precedence() {
    for profile in 0..2 {
        for case in [
            "noop", "escape", "focus", "pointer", "overflow", "sill", "stale", "camera",
        ] {
            let (mut h, _, view) = fixture(profile, OpeningKind::Window, true);
            let before = h.app.editor.document.model().clone();
            let stats = h.app.editor.document.history_stats();
            let p = start(&mut h);
            let camera = h.app.plans.cameras[&view];
            let to = match case {
                "overflow" => p + egui::vec2((7. * camera.pixels_per_metre) as f32, 0.),
                "sill" => p + egui::vec2(0., (1.2 * camera.pixels_per_metre) as f32),
                _ => p + egui::vec2(25., -15.),
            };
            h.frame(vec![egui::Event::PointerMoved(to)]);
            match case {
                "noop" => {
                    h.frame(vec![egui::Event::PointerMoved(p)]);
                }
                "escape" => h.escape(),
                "focus" => {
                    h.frame(vec![egui::Event::WindowFocused(false)]);
                }
                "pointer" => {
                    h.frame(vec![egui::Event::PointerGone]);
                }
                "stale" => {
                    h.app.select(None);
                }
                "camera" => {
                    h.app.plans.cameras.get_mut(&view).unwrap().center.x += 0.1;
                }
                _ => {}
            }
            h.frame(vec![button(if case == "noop" { p } else { to }, false)]);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before, "{case}");
            assert_eq!(h.app.editor.document.history_stats(), stats, "{case}");
        }
        let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
        h.click("Move opening");
        let p = begin(&mut h, id, true);
        assert!(h.app.plans.elevation.resize.is_some());
        assert!(!h.app.plans.elevation.move_claimed);
        h.frame(vec![button(p, false)]);
        h.frame(vec![]);
        let camera = h.app.plans.cameras[&view];
        let p = h.point(Point2::new(7., 4.));
        h.frame(vec![egui::Event::PointerMoved(p), button(p, true)]);
        h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(25., 15.))]);
        h.frame(vec![button(p + egui::vec2(25., 15.), false)]);
        assert_ne!(h.app.plans.cameras[&view], camera);
    }
}

#[test]
fn elevation_move_snaps_crop_self_exclusion_and_rejections() {
    for profile in 0..2 {
        for mode in [
            "snap",
            "disabled",
            "modes_off",
            "midpoints",
            "cropped",
            "self",
            "tolerance",
            "overlap",
            "lock",
            "frame",
        ] {
            let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
            if mode == "frame" {
                let OpeningDefinition::Typed { type_id } = h.app.editor.document.model().openings
                    [&id]
                    .parameters
                    .definition
                else {
                    unreachable!()
                };
                let mut parameters = h.app.editor.document.model().opening_types[&type_id]
                    .parameters
                    .clone();
                parameters.family.frame_width = 0.2;
                h.app
                    .editor
                    .command(
                        "Frame",
                        Command::UpdateOpeningType {
                            id: type_id,
                            parameters,
                        },
                    )
                    .unwrap();
            }
            if mode == "lock" {
                h.app
                    .editor
                    .command(
                        "Lock",
                        Command::SetOpeningClearance {
                            id,
                            clearance: Some(os_model::OpeningClearance {
                                end: os_model::ClearanceEnd::Start,
                                distance: 3.,
                            }),
                        },
                    )
                    .unwrap();
            }
            if mode != "self" && mode != "lock" && mode != "frame" {
                let mut other = h.app.editor.document.model().openings[&id].clone();
                other.header.id = Id::new();
                other.parameters.offset = 7.;
                other.parameters.sill_override = Some(1.4);
                h.app
                    .editor
                    .command("Reference", Command::AddOpening(other))
                    .unwrap();
            }
            if mode == "cropped" {
                let mut parameters = h.app.editor.document.model().views[&view]
                    .parameters
                    .clone();
                parameters.elevation.as_mut().unwrap().max_station = 6.;
                h.app
                    .editor
                    .command(
                        "Crop",
                        Command::UpdateView {
                            id: view,
                            parameters,
                        },
                    )
                    .unwrap();
            }
            h.settle();
            h.app.select(Some(id));
            h.app.plans.snaps.enabled = mode != "disabled" && mode != "overlap" && mode != "lock";
            if mode == "modes_off" || mode == "midpoints" {
                h.app.plans.snaps.endpoints = false;
                h.app.plans.snaps.nearest = false;
                h.app.plans.snaps.axis_extensions = false;
                h.app.plans.snaps.midpoints = mode == "midpoints";
            }
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let p = start(&mut h);
            let camera = h.app.plans.cameras[&view];
            let dx = if mode == "frame" {
                -3.1
            } else if mode == "overlap" {
                4.
            } else if mode == "self" {
                4. / camera.pixels_per_metre
            } else {
                0.3
            };
            let dy = if mode == "self" || mode == "lock" || mode == "overlap" {
                0.
            } else {
                0.4 - if mode == "tolerance" { 18. } else { 4. } / camera.pixels_per_metre
            };
            let to = p + egui::vec2(
                (dx * camera.pixels_per_metre) as f32,
                (-dy * camera.pixels_per_metre) as f32,
            );
            h.frame(vec![egui::Event::PointerMoved(to)]);
            assert_eq!(h.app.editor.document.model(), &before);
            h.frame(vec![button(to, false)]);
            h.settle();
            if mode == "overlap" || mode == "lock" || mode == "frame" {
                assert_eq!(h.app.editor.document.model(), &before, "{mode}");
                assert_eq!(h.app.editor.document.history_stats(), history, "{mode}");
            } else {
                let model = h.app.editor.document.model();
                let r = model
                    .resolve_opening(&model.openings[&id].parameters)
                    .unwrap();
                let expected = if mode == "snap" || mode == "midpoints" {
                    1.4
                } else {
                    1. + dy
                };
                assert!(
                    (r.sill - expected).abs() < 1e-5,
                    "{mode}: {} != {expected}, {}",
                    r.sill,
                    h.app.status
                );
                assert!((r.offset - 3. - dx).abs() < 1e-5, "{mode}");
                if dy == 0. {
                    assert_eq!(
                        model.openings[&id].parameters.sill_override,
                        before.openings[&id].parameters.sill_override
                    );
                }
            }
        }
    }
}
