use super::*;
use os_model::{Opening, OpeningDefinition, OpeningParams};
#[path = "elevation_move_tests.rs"]
mod move_tests;

fn fixture(profile: usize, kind: OpeningKind, typed: bool) -> (Harness, Id, Id) {
    let mut app = DesktopApp::new().unwrap();
    let mut model = app.editor.document.model().clone();
    model
        .levels
        .get_mut(&app.active_level)
        .unwrap()
        .parameters
        .elevation = 3.;
    app.editor.document = os_document::Document::from_model(model).unwrap();
    let mut wp = default_wall(app.active_level);
    wp.path = WallPath::Straight {
        start: Point2::new(8., 11.),
        end: Point2::new(2., 3.),
    };
    wp.height = 4.;
    let wall = Wall::new(os_walls::WALL_TYPE, wp);
    let host = wall.id();
    app.editor.command("Host", Command::AddWall(wall)).unwrap();
    let definition = if typed {
        let ty = os_model::OpeningType::new(
            "core.opening_type",
            os_model::OpeningTypeParams {
                name: "Resize type".into(),
                kind,
                width: 1.2,
                height: 1.5,
                sill: if kind == OpeningKind::Door { 0. } else { 1. },
                window_operation: Default::default(),
                family: Default::default(),
                pane_position: Default::default(),
            },
        );
        let type_id = ty.id();
        app.editor
            .command("Type", Command::AddOpeningType(ty))
            .unwrap();
        OpeningDefinition::Typed { type_id }
    } else {
        OpeningDefinition::Legacy {
            kind,
            width: 1.2,
            height: 1.5,
            sill: if kind == OpeningKind::Door { 0. } else { 1. },
        }
    };
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Resize fixture".into(),
            host,
            offset: 3.,
            definition,
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
            open_state: Default::default(),
        },
    );
    let id = opening.id();
    app.editor
        .command("Opening", Command::AddOpening(opening))
        .unwrap();
    let view = app
        .editor
        .create_wall_elevation(
            "Resize",
            host,
            if profile == 0 {
                ElevationSide::Left
            } else {
                ElevationSide::Right
            },
        )
        .unwrap();
    app.plans.poll(&app.editor);
    app.focus_plan(Some(view));
    app.select(Some(id));
    app.plans.snaps.enabled = false;
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let (size, scale) = if profile == 0 {
        (egui::vec2(1280., 800.), 1.)
    } else {
        (egui::vec2(1000., 650.), 1.5)
    };
    let mut h = Harness {
        app,
        ctx,
        size,
        scale,
        time: 0.,
    };
    h.settle();
    (h, id, view)
}

fn point(h: &Harness, id: Id, head: bool) -> Point2 {
    let m = h.app.editor.document.model();
    let p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
    Point2::new(
        p.offset + p.width * 0.5,
        3. + p.sill + if head { p.height } else { 0. },
    )
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn begin(h: &mut Harness, id: Id, head: bool) -> egui::Pos2 {
    let pos = h.point(point(h, id, head)) + egui::vec2(3., -2.);
    h.frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
    assert!(
        h.app.plans.elevation.resize.is_some(),
        "grip missing: {}",
        h.app.status
    );
    pos
}

fn jamb_point(h: &Harness, id: Id, start: bool) -> Point2 {
    let m = h.app.editor.document.model();
    let p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
    Point2::new(
        p.offset + if start { 0. } else { p.width },
        3. + p.sill + p.height * 0.5,
    )
}

fn begin_jamb(h: &mut Harness, id: Id, start: bool) -> egui::Pos2 {
    let pos = h.point(jamb_point(h, id, start)) + egui::vec2(3., -2.);
    h.frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
    assert!(
        h.app.plans.elevation.resize.is_some(),
        "jamb grip missing: {} start={start} pos={pos:?} opening={:?} elevation={:?}",
        h.app.status,
        h.app.editor.document.model().openings[&id].parameters,
        h.app.active_elevation()
    );
    pos
}

#[test]
fn elevation_resize_jamb_both_dpis_fixed_edge_preview_history_reopen() {
    for profile in 0..2 {
        for side in [ElevationSide::Left, ElevationSide::Right] {
            for typed in [false, true] {
                for kind in [OpeningKind::Door, OpeningKind::Window] {
                    for start in [true, false] {
                        let (mut h, id, view) = fixture(profile, kind, typed);
                        let mut parameters = h.app.editor.document.model().views[&view]
                            .parameters
                            .clone();
                        parameters.elevation.as_mut().unwrap().side = side;
                        h.app
                            .editor
                            .command(
                                "Side",
                                Command::UpdateView {
                                    id: view,
                                    parameters,
                                },
                            )
                            .unwrap();
                        h.settle();
                        h.app.select(Some(id));
                        let before = h.app.editor.document.model().clone();
                        let stats = h.app.editor.document.history_stats();
                        let scene = h.app.editor.scene.clone();
                        let camera = h.app.plans.cameras[&view];
                        let original = before
                            .resolve_opening(&before.openings[&id].parameters)
                            .unwrap();
                        // A complete outward/back gesture must preserve legacy
                        // values and typed inheritance byte-for-byte.
                        let origin = begin_jamb(&mut h, id, start);
                        h.frame(vec![egui::Event::PointerMoved(
                            origin + egui::vec2(12., 0.),
                        )]);
                        h.frame(vec![egui::Event::PointerMoved(origin)]);
                        h.frame(vec![button(origin, false)]);
                        h.frame(vec![]);
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.document.history_stats(), stats);
                        let from = begin_jamb(&mut h, id, start);
                        let delta = if start { -0.35 } else { 0.35 };
                        let to = from + egui::vec2((delta * camera.pixels_per_metre) as f32, 19.);
                        h.frame(vec![egui::Event::PointerMoved(to)]);
                        assert_eq!(h.app.editor.document.model(), &before);
                        assert_eq!(h.app.editor.document.history_stats(), stats);
                        assert_eq!(h.app.editor.scene, scene);
                        assert_eq!(h.app.plans.cameras[&view], camera);
                        let context = h.app.editor.native_elevation_context(view).unwrap();
                        let canvas = h.app.plans.canvas_rect.unwrap();
                        let preview = h
                            .app
                            .elevation_resize_input(&h.ctx, camera, canvas, context)
                            .drawing
                            .unwrap()
                            .provider_lines(context)
                            .unwrap()
                            .to_vec();
                        h.frame(vec![button(to, false)]);
                        h.settle();
                        let after = h.app.editor.document.model().clone();
                        let updated = after
                            .resolve_opening(&after.openings[&id].parameters)
                            .unwrap();
                        assert!(
                            (updated.width - original.width - 0.35).abs() < 1e-5,
                            "{}",
                            h.app.status
                        );
                        assert!(
                            (if start {
                                updated.offset + updated.width - original.offset - original.width
                            } else {
                                updated.offset - original.offset
                            })
                            .abs()
                                < 1e-10
                        );
                        assert_eq!(
                            (updated.height, updated.sill),
                            (original.height, original.sill)
                        );
                        let mut expected = before.openings[&id].clone();
                        expected.parameters.offset = after.openings[&id].parameters.offset;
                        if typed {
                            assert!(after.openings[&id].parameters.width_override.is_some());
                            expected.parameters.width_override =
                                after.openings[&id].parameters.width_override;
                        } else if let OpeningDefinition::Legacy { width, .. } =
                            &mut expected.parameters.definition
                        {
                            *width = updated.width;
                        }
                        assert_eq!(expected, after.openings[&id]);
                        assert_eq!(before.opening_types, after.opening_types);
                        assert_eq!(
                            preview,
                            h.app
                                .editor
                                .native_drawing(view)
                                .unwrap()
                                .provider_lines(
                                    h.app.editor.native_elevation_context(view).unwrap()
                                )
                                .unwrap()
                        );
                        assert_eq!(
                            h.app.editor.document.history_stats().undo_entries,
                            stats.undo_entries + 1
                        );
                        assert_ne!(h.app.editor.scene, scene);
                        h.app.history(false);
                        h.settle();
                        assert_eq!(h.app.editor.document.model(), &before);
                        h.app.history(true);
                        h.settle();
                        assert_eq!(h.app.editor.document.model(), &after);
                        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
                        let path = dir.path().join("jamb-resize.osb");
                        h.app.editor.save(&path).unwrap();
                        h.app.editor.open(&path).unwrap();
                        assert_eq!(h.app.editor.document.model(), &after);
                    }
                }
            }
        }
    }
}

#[test]
fn elevation_resize_jamb_cancel_invalid_noop_stale_and_pan() {
    for profile in 0..2 {
        for start in [true, false] {
            for mode in [
                "escape",
                "collapse",
                "overflow",
                "noop",
                "pointer",
                "focus",
                "revision",
                "selection",
                "camera",
                "canvas",
                "view",
                "drawing",
                "provider",
                "modal",
                "session",
                "settings",
            ] {
                let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
                let from = begin_jamb(&mut h, id, start);
                let camera = h.app.plans.cameras[&view];
                let to = from + egui::vec2(15., 0.);
                h.frame(vec![egui::Event::PointerMoved(to)]);
                let mut release = to;
                match mode {
                    "escape" => h.escape(),
                    "collapse" | "overflow" => {
                        let station = if mode == "collapse" {
                            if start { 4.3 } else { 2.9 }
                        } else if start {
                            -0.1
                        } else {
                            10.1
                        };
                        release = from
                            + egui::vec2(
                                ((station - if start { 3. } else { 4.2 }) * camera.pixels_per_metre)
                                    as f32,
                                0.,
                            );
                        h.frame(vec![egui::Event::PointerMoved(release)]);
                    }
                    "noop" => {
                        release = from;
                        h.frame(vec![egui::Event::PointerMoved(from)]);
                    }
                    "pointer" => {
                        h.frame(vec![egui::Event::PointerGone]);
                    }
                    "focus" => {
                        h.frame(vec![egui::Event::WindowFocused(false)]);
                    }
                    "revision" => h
                        .app
                        .editor
                        .command("Rename", Command::RenameProject("Changed".into()))
                        .unwrap(),
                    "selection" => h.app.select(None),
                    "camera" => h.app.plans.cameras.get_mut(&view).unwrap().center.x += 0.2,
                    "canvas" => h.size.x -= 25.,
                    "view" => {
                        h.app.focus_plan(None);
                        h.app.focus_plan(Some(view));
                    }
                    "drawing" => {
                        h.app.plans.drawing = Some(h.app.editor.native_drawing(view).unwrap())
                    }
                    "provider" => {
                        h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                    }
                    "modal" => h.app.cancel_plan_authoring_for_modal(),
                    "session" => {
                        h.app.editor.document =
                            os_document::Document::from_model(h.app.editor.document.model().clone())
                                .unwrap()
                    }
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
                    _ => unreachable!(),
                }
                let before = h.app.editor.document.model().clone();
                let stats = h.app.editor.document.history_stats();
                let scene = h.app.editor.scene.clone();
                let camera = h.app.plans.cameras.get(&view).copied();
                h.frame(vec![egui::Event::PointerMoved(release)]);
                h.frame(vec![button(release, false)]);
                assert_eq!(h.app.editor.document.model(), &before, "{mode}");
                assert_eq!(h.app.editor.document.history_stats(), stats, "{mode}");
                assert_eq!(h.app.editor.scene, scene, "{mode}");
                if mode != "session" {
                    assert_eq!(h.app.plans.cameras.get(&view).copied(), camera, "{mode}");
                }
                assert!(h.app.plans.elevation.resize.is_none(), "{mode}");
            }
        }
        let (mut h, _, view) = fixture(profile, OpeningKind::Window, true);
        let camera = h.app.plans.cameras[&view];
        let from = h.point(Point2::new(7., 4.5));
        let to = from + egui::vec2(25., 15.);
        h.frame(vec![egui::Event::PointerMoved(from), button(from, true)]);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        h.frame(vec![button(to, false)]);
        assert_ne!(h.app.plans.cameras[&view], camera);
    }
}

#[test]
fn elevation_resize_jamb_snaps_visibility_radius_and_ambiguity() {
    for profile in 0..2 {
        for mode in [
            "snap",
            "midpoints",
            "disabled",
            "modes_off",
            "outside_tolerance",
            "cropped",
            "self",
            "overlap",
        ] {
            let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
            let mut other = h.app.editor.document.model().openings[&id].clone();
            other.header.id = Id::new();
            other.parameters.offset = 5.;
            // Crop can hide this reference vertically. The model still rejects
            // touching/stacked apertures, even when their heights differ.
            other.parameters.sill_override = Some(if mode == "overlap" { 1. } else { 2.7 });
            other.parameters.height_override = Some(0.8);
            h.app
                .editor
                .command("Reference", Command::AddOpening(other))
                .unwrap();
            if mode == "cropped" {
                let mut parameters = h.app.editor.document.model().views[&view]
                    .parameters
                    .clone();
                parameters.elevation.as_mut().unwrap().top_elevation = 5.6;
                h.app
                    .editor
                    .command(
                        "Crop reference",
                        Command::UpdateView {
                            id: view,
                            parameters,
                        },
                    )
                    .unwrap();
            }
            h.settle();
            h.app.select(Some(id));
            h.app.plans.snaps.enabled = mode != "disabled";
            if mode == "modes_off" || mode == "midpoints" {
                h.app.plans.snaps.endpoints = false;
                h.app.plans.snaps.nearest = false;
                h.app.plans.snaps.axis_extensions = false;
                h.app.plans.snaps.midpoints = mode == "midpoints";
            }
            let before = h.app.editor.document.model().clone();
            let stats = h.app.editor.document.history_stats();
            let camera = h.app.plans.cameras[&view];
            let from = begin_jamb(&mut h, id, false);
            let desired = if mode == "self" {
                4.2 + 4. / camera.pixels_per_metre
            } else if mode == "overlap" {
                5. + 4. / camera.pixels_per_metre
            } else {
                5. - if mode == "outside_tolerance" { 13. } else { 4. } / camera.pixels_per_metre
            };
            let to = from + egui::vec2(((desired - 4.2) * camera.pixels_per_metre) as f32, 0.);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            h.frame(vec![button(to, false)]);
            h.settle();
            if matches!(mode, "overlap" | "snap" | "midpoints") {
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), stats);
            } else {
                let m = h.app.editor.document.model();
                let p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
                assert!(
                    (p.offset + p.width - desired).abs() < 1e-5,
                    "{mode}: {}",
                    h.app.status
                );
            }
        }
        for mode in ["inside_radius", "outside_radius", "cropped", "ambiguous"] {
            let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
            if mode == "cropped" {
                let mut parameters = h.app.editor.document.model().views[&view]
                    .parameters
                    .clone();
                parameters.elevation.as_mut().unwrap().min_station = 3.1;
                h.app
                    .editor
                    .command(
                        "Crop jamb",
                        Command::UpdateView {
                            id: view,
                            parameters,
                        },
                    )
                    .unwrap();
                h.settle();
            }
            if mode == "ambiguous" {
                h.app.plans.cameras.get_mut(&view).unwrap().pixels_per_metre = 12.;
                h.frame(vec![]);
            }
            let pos = h.point(jamb_point(&h, id, true))
                + egui::vec2(
                    if mode == "inside_radius" {
                        9.5
                    } else if mode == "outside_radius" {
                        10.5
                    } else {
                        0.
                    },
                    0.,
                );
            h.frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
            assert_eq!(
                h.app.plans.elevation.resize.is_some(),
                mode == "inside_radius",
                "{mode}"
            );
            h.frame(vec![button(pos, false)]);
        }
    }
}

#[test]
fn elevation_resize_jamb_rejects_clearance_locks_frame_fit_and_wall_end_snaps() {
    for profile in 0..2 {
        for start in [true, false] {
            for mode in ["lock", "frame", "wall_snap", "wall_unsnapped"] {
                let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
                if mode == "lock" {
                    h.app
                        .editor
                        .command(
                            "Lock jamb",
                            Command::SetOpeningClearance {
                                id,
                                clearance: Some(os_model::OpeningClearance {
                                    end: if start {
                                        os_model::ClearanceEnd::Start
                                    } else {
                                        os_model::ClearanceEnd::End
                                    },
                                    distance: if start { 3. } else { 5.8 },
                                }),
                            },
                        )
                        .unwrap();
                }
                if mode == "frame" {
                    let OpeningDefinition::Typed { type_id } =
                        &h.app.editor.document.model().openings[&id]
                            .parameters
                            .definition
                    else {
                        unreachable!()
                    };
                    let type_id = *type_id;
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
                h.settle();
                h.app.select(Some(id));
                h.app.plans.snaps.enabled = mode == "wall_snap";
                let before = h.app.editor.document.model().clone();
                let stats = h.app.editor.document.history_stats();
                let scene = h.app.editor.scene.clone();
                let camera = h.app.plans.cameras[&view];
                let from = begin_jamb(&mut h, id, start);
                let station = match mode {
                    "lock" => {
                        if start {
                            2.8
                        } else {
                            4.4
                        }
                    }
                    "frame" => {
                        if start {
                            3.9
                        } else {
                            3.3
                        }
                    }
                    _ => {
                        if start {
                            4. / camera.pixels_per_metre
                        } else {
                            10. - 4. / camera.pixels_per_metre
                        }
                    }
                };
                let to = from
                    + egui::vec2(
                        ((station - if start { 3. } else { 4.2 }) * camera.pixels_per_metre) as f32,
                        0.,
                    );
                h.frame(vec![egui::Event::PointerMoved(to)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), stats);
                assert_eq!(h.app.editor.scene, scene);
                h.frame(vec![button(to, false)]);
                h.settle();
                if mode == "wall_unsnapped" {
                    let m = h.app.editor.document.model();
                    let p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
                    assert!(
                        (if start { p.offset } else { p.offset + p.width } - station).abs() < 1e-5,
                        "{}",
                        h.app.status
                    );
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        stats.undo_entries + 1
                    );
                } else {
                    assert_eq!(h.app.editor.document.model(), &before, "{mode}");
                    assert_eq!(h.app.editor.document.history_stats(), stats, "{mode}");
                    assert_eq!(h.app.editor.scene, scene, "{mode}");
                }
            }
        }
    }
}

#[test]
fn elevation_resize_both_dpis_head_sill_preview_commit_history_reopen() {
    for profile in 0..2 {
        for typed in [false, true] {
            for kind in [OpeningKind::Door, OpeningKind::Window] {
                let (mut h, id, view) = fixture(profile, kind, typed);
                for head in if kind == OpeningKind::Window {
                    vec![true, false]
                } else {
                    vec![true]
                } {
                    let before = h.app.editor.document.model().clone();
                    let stats = h.app.editor.document.history_stats();
                    let scene = h.app.editor.scene.clone();
                    let camera = h.app.plans.cameras[&view];
                    let original = before
                        .resolve_opening(&before.openings[&id].parameters)
                        .unwrap();
                    let from = begin(&mut h, id, head);
                    let delta = if head { 0.35 } else { 0.2 };
                    let to = from + egui::vec2(19., -(delta * camera.pixels_per_metre) as f32);
                    h.frame(vec![egui::Event::PointerMoved(to)]);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert_eq!(h.app.editor.document.history_stats(), stats);
                    assert_eq!(h.app.editor.scene, scene);
                    assert_eq!(h.app.plans.cameras[&view], camera);
                    let context = h.app.editor.native_elevation_context(view).unwrap();
                    let canvas = h.app.plans.canvas_rect.unwrap();
                    let preview = h
                        .app
                        .elevation_resize_input(&h.ctx, camera, canvas, context)
                        .drawing
                        .unwrap()
                        .provider_lines(context)
                        .unwrap()
                        .to_vec();
                    h.frame(vec![button(to, false)]);
                    h.settle();
                    let committed_context = h.app.editor.native_elevation_context(view).unwrap();
                    assert_eq!(
                        preview,
                        h.app
                            .editor
                            .native_drawing(view)
                            .unwrap()
                            .provider_lines(committed_context)
                            .unwrap()
                    );
                    let after = h.app.editor.document.model().clone();
                    let updated = after
                        .resolve_opening(&after.openings[&id].parameters)
                        .unwrap();
                    assert!(
                        (updated.height - original.height - if head { delta } else { -delta })
                            .abs()
                            < 1e-5,
                        "{}",
                        h.app.status
                    );
                    assert!(
                        (updated.sill - original.sill - if head { 0. } else { delta }).abs() < 1e-5
                    );
                    let mut expected = before.openings[&id].clone();
                    expected.parameters.height_override =
                        after.openings[&id].parameters.height_override;
                    expected.parameters.sill_override =
                        after.openings[&id].parameters.sill_override;
                    if !typed {
                        expected.parameters.definition =
                            after.openings[&id].parameters.definition.clone();
                    }
                    assert_eq!(expected, after.openings[&id]);
                    assert_eq!(before.opening_types, after.opening_types);
                    assert_eq!(
                        h.app.editor.document.history_stats().undo_entries,
                        stats.undo_entries + 1
                    );
                    assert_ne!(h.app.editor.scene, scene);
                    h.app.history(false);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &before);
                    h.app.history(true);
                    h.settle();
                    assert_eq!(h.app.editor.document.model(), &after);
                    h.app.select(Some(id));
                    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
                    let path = dir.path().join("resize.osb");
                    h.app.editor.save(&path).unwrap();
                    h.app.editor.open(&path).unwrap();
                    assert_eq!(h.app.editor.document.model(), &after);
                    h.app.plans.poll(&h.app.editor);
                    h.app.focus_plan(Some(view));
                    h.settle();
                    h.app.select(Some(id));
                }
            }
        }
    }
}

#[test]
fn elevation_resize_snap_controls_crop_tolerance_and_self_exclusion() {
    for profile in 0..2 {
        for mode in [
            "snap",
            "midpoints",
            "disabled",
            "modes_off",
            "outside_tolerance",
            "cropped",
            "self",
        ] {
            let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
            let mut other = h.app.editor.document.model().openings[&id].clone();
            other.header.id = Id::new();
            other.parameters.offset = 7.;
            other.parameters.height_override = Some(1.85);
            h.app
                .editor
                .command("Snap reference", Command::AddOpening(other))
                .unwrap();
            if mode == "cropped" {
                let mut parameters = h.app.editor.document.model().views[&view]
                    .parameters
                    .clone();
                parameters.elevation.as_mut().unwrap().max_station = 6.;
                h.app
                    .editor
                    .command(
                        "Crop reference",
                        Command::UpdateView {
                            id: view,
                            parameters,
                        },
                    )
                    .unwrap();
            }
            h.settle();
            h.app.select(Some(id));
            h.app.plans.snaps.enabled = mode != "disabled";
            if mode == "modes_off" || mode == "midpoints" {
                h.app.plans.snaps.endpoints = false;
                h.app.plans.snaps.nearest = false;
                h.app.plans.snaps.axis_extensions = false;
                h.app.plans.snaps.midpoints = mode == "midpoints";
            }
            let camera = h.app.plans.cameras[&view];
            let from = begin(&mut h, id, true);
            let desired = if mode == "self" {
                5.5 + 4. / camera.pixels_per_metre
            } else {
                5.85 + if mode == "outside_tolerance" { 13. } else { 4. } / camera.pixels_per_metre
            };
            let to = from + egui::vec2(0., -((desired - 5.5) * camera.pixels_per_metre) as f32);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            h.frame(vec![button(to, false)]);
            h.settle();
            let m = h.app.editor.document.model();
            let p = m.resolve_opening(&m.openings[&id].parameters).unwrap();
            let expected = if mode == "snap" || mode == "midpoints" {
                5.85
            } else {
                desired
            };
            assert!(
                (3. + p.sill + p.height - expected).abs() < 1e-5,
                "mode {mode}: {}",
                3. + p.sill + p.height
            );
        }
    }
}

#[test]
fn elevation_resize_stale_guards_and_visibility() {
    for reason in [
        "revision", "session", "view", "camera", "drawing", "provider", "modal", "settings",
    ] {
        let (mut h, id, view) = fixture(0, OpeningKind::Window, true);
        let from = begin(&mut h, id, true);
        let to = from + egui::vec2(0., -20.);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        match reason {
            "revision" => h
                .app
                .editor
                .command("Rename", Command::RenameProject("Changed".into()))
                .unwrap(),
            "session" => {
                h.app.editor.document =
                    os_document::Document::from_model(h.app.editor.document.model().clone())
                        .unwrap()
            }
            "view" => {
                h.app.focus_plan(None);
                h.app.focus_plan(Some(view));
            }
            "camera" => h.app.plans.cameras.get_mut(&view).unwrap().center.x += 0.2,
            "drawing" => h.app.plans.drawing = Some(h.app.editor.native_drawing(view).unwrap()),
            "provider" => {
                h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
            }
            "modal" => h.app.cancel_plan_authoring_for_modal(),
            "settings" => {
                let mut parameters = h.app.editor.document.model().views[&view]
                    .parameters
                    .clone();
                parameters.elevation.as_mut().unwrap().side = ElevationSide::Right;
                h.app
                    .editor
                    .command(
                        "Side",
                        Command::UpdateView {
                            id: view,
                            parameters,
                        },
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let before = h.app.editor.document.model().clone();
        let stats = h.app.editor.document.history_stats();
        let camera = h.app.plans.cameras.get(&view).copied();
        h.frame(vec![egui::Event::PointerMoved(to)]);
        h.frame(vec![button(to, false)]);
        assert_eq!(h.app.editor.document.model(), &before, "{reason}");
        assert_eq!(h.app.editor.document.history_stats(), stats, "{reason}");
        if reason == "session" {
            // A new document session deliberately discards cached view cameras.
            assert!(!h.app.plans.cameras.contains_key(&view));
        } else {
            assert_eq!(h.app.plans.cameras.get(&view).copied(), camera, "{reason}");
        }
        assert!(h.app.plans.elevation.resize.is_none(), "{reason}");
    }
    // A cropped nominal head and a door bottom must never acquire a grip.
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut h, id, view) = fixture(0, kind, true);
        if kind == OpeningKind::Window {
            let mut parameters = h.app.editor.document.model().views[&view]
                .parameters
                .clone();
            parameters.elevation.as_mut().unwrap().top_elevation = 5.;
            h.app
                .editor
                .command(
                    "Crop head",
                    Command::UpdateView {
                        id: view,
                        parameters,
                    },
                )
                .unwrap();
            h.settle();
        }
        let pos = h.point(point(&h, id, kind == OpeningKind::Window));
        h.frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
        assert!(h.app.plans.elevation.resize.is_none());
        h.frame(vec![button(pos, false)]);
    }
}

#[test]
fn elevation_resize_cancel_invalid_noop_and_pan_precedence() {
    for profile in 0..2 {
        let (mut h, id, view) = fixture(profile, OpeningKind::Window, true);
        for mode in 0..8 {
            let before = h.app.editor.document.model().clone();
            let stats = h.app.editor.document.history_stats();
            let camera = h.app.plans.cameras[&view];
            let from = begin(&mut h, id, true);
            let to = from + egui::vec2(0., -20.);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            let release = match mode {
                0 => {
                    h.escape();
                    to
                }
                1 => {
                    let p = h.point(Point2::new(3.6, 3.2));
                    h.frame(vec![egui::Event::PointerMoved(p)]);
                    p
                }
                2 => {
                    h.frame(vec![egui::Event::PointerMoved(from)]);
                    from
                }
                3 => {
                    h.frame(vec![egui::Event::PointerGone]);
                    to
                }
                4 => {
                    h.frame(vec![egui::Event::WindowFocused(false)]);
                    to
                }
                5 => {
                    h.app.select(None);
                    to
                }
                6 => {
                    h.app
                        .begin_elevation_dimension(os_model::DimensionLayout::ElevationVertical);
                    to
                }
                _ => {
                    h.size.x -= 25.;
                    h.frame(vec![]);
                    to
                }
            };
            h.frame(vec![button(release, false)]);
            h.frame(vec![]);
            assert!(h.app.plans.elevation.resize.is_none());
            assert_eq!(h.app.editor.document.model(), &before, "mode {mode}");
            assert_eq!(h.app.editor.document.history_stats(), stats, "mode {mode}");
            assert_eq!(h.app.plans.cameras[&view], camera, "mode {mode}");
            h.app.plans.elevation.dimension = None;
            h.app.select(Some(id));
            h.frame(vec![egui::Event::WindowFocused(true)]);
        }
        let camera = h.app.plans.cameras[&view];
        let pos = h.point(Point2::new(7., 4.5));
        h.frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
        h.frame(vec![egui::Event::PointerMoved(pos + egui::vec2(25., 15.))]);
        h.frame(vec![button(pos + egui::vec2(25., 15.), false)]);
        assert_ne!(h.app.plans.cameras[&view], camera);
    }
}
