use os_core::{Id, Point2};
use os_document::Command;
use os_geometry::{GeometryKernel, PrismKernel};
use os_model::{DoorHinge, DoorSwing, OpeningType, OpeningTypeParams, WindowPanePosition};
use os_model::{Opening, OpeningDefinition, OpeningKind, OpeningParams, Wall, WallParams};
use os_ui::Editor;

fn setup() -> (Editor, Id, Id, Opening, Opening) {
    let mut editor = Editor::new().unwrap();
    let level = *editor.document.model().levels.keys().next().unwrap();
    let view = editor.create_floor_plan("Openings", level).unwrap();
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Host".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(6.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let id = wall.id();
    editor.command("Host", Command::AddWall(wall)).unwrap();
    let opening = |kind, offset, sill| {
        Opening::new(
            "core.opening",
            OpeningParams {
                hinge: Default::default(),
                swing: Default::default(),
                name: format!("{kind:?}"),
                host: id,
                offset,
                definition: OpeningDefinition::Legacy {
                    kind,
                    width: 1.0,
                    height: 1.8,
                    sill,
                },
            },
        )
    };
    (
        editor,
        id,
        view,
        opening(OpeningKind::Door, 1.0, 0.0),
        opening(OpeningKind::Window, 3.0, 0.8),
    )
}

#[test]
fn opening_validation_history_invalidation_and_roundtrip() {
    let (mut e, host, view, door, window) = setup();
    let before = e.document.model().clone();
    e.document.drain_events();
    e.document
        .execute(
            "Openings",
            vec![
                Command::AddOpening(door.clone()),
                Command::AddOpening(window.clone()),
            ],
        )
        .unwrap();
    let event = e.document.drain_events().pop().unwrap();
    for id in [host, view, door.id(), window.id()] {
        assert!(event.invalidated.contains(&id));
    }
    let created = e.document.model().clone();
    assert!(e.document.undo());
    assert_eq!(e.document.model(), &before);
    assert!(e.document.redo());
    assert_eq!(e.document.model(), &created);
    for case in 0..9 {
        let mut p = door.parameters.clone();
        match case {
            0 => p.host = Id::new(),
            1 => p.offset = -1.0,
            2 => {
                if let OpeningDefinition::Legacy { width, .. } = &mut p.definition {
                    *width = f64::NAN;
                }
            }
            3 => {
                if let OpeningDefinition::Legacy { height, .. } = &mut p.definition {
                    *height = 9.0;
                }
            }
            4 => {
                if let OpeningDefinition::Legacy { sill, .. } = &mut p.definition {
                    *sill = 0.5;
                }
            }
            5 => p.offset = 3.0,
            6 => {
                if let OpeningDefinition::Legacy { width, .. } = &mut p.definition {
                    *width = 0.0;
                }
            }
            7 => p.offset = 5.5,
            _ => p.name = "Bad\nname".into(),
        }
        let history = e.document.history_stats();
        let revision = e.document.revision();
        assert!(
            e.document
                .execute(
                    "Bad",
                    vec![Command::UpdateOpening {
                        id: door.id(),
                        parameters: p
                    }]
                )
                .is_err()
        );
        assert_eq!(e.document.model(), &created);
        assert_eq!(e.document.history_stats(), history);
        assert_eq!(e.document.revision(), revision);
    }
    assert!(e.command("Remove host", Command::RemoveWall(host)).is_err());
    let mut short = created.walls[&host].parameters.clone();
    short.end.x = 2.0;
    assert!(
        e.command(
            "Short host",
            Command::UpdateWall {
                id: host,
                parameters: short
            }
        )
        .is_err()
    );
    let mut thin = created.walls[&host].parameters.clone();
    thin.thickness = 0.000005;
    assert!(
        e.command(
            "Thin host",
            Command::UpdateWall {
                id: host,
                parameters: thin,
            },
        )
        .is_err()
    );
    let mut moved = created.walls[&host].parameters.clone();
    moved.start.y = 2.0;
    moved.end.y = 2.0;
    e.document
        .execute(
            "Move host",
            vec![Command::UpdateWall {
                id: host,
                parameters: moved,
            }],
        )
        .unwrap();
    let event = e.document.drain_events().pop().unwrap();
    for id in [host, view, door.id(), window.id()] {
        assert!(event.invalidated.contains(&id));
    }
    e.undo().unwrap();
    e.redo().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("openings.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), e.document.model());
    assert_eq!(reopened.scene, e.scene);
    let saved = e.document.model().clone();
    e.document
        .execute(
            "Delete host and fixtures",
            vec![
                Command::RemoveOpening(door.id()),
                Command::RemoveOpening(window.id()),
                Command::RemoveWall(host),
            ],
        )
        .unwrap();
    e.regenerate().unwrap();
    assert!(e.scene.is_empty());
    e.undo().unwrap();
    assert_eq!(e.document.model(), &saved);
    assert_eq!(e.scene, reopened.scene);
}

#[test]
fn actual_missing_volume_plan_cut_gaps_symbols_and_independent_3d_picking() {
    let (mut e, host, view, door, window) = setup();
    e.command("Door", Command::AddOpening(door.clone()))
        .unwrap();
    e.command("Window", Command::AddOpening(window.clone()))
        .unwrap();
    let expected = 6.0 * 3.0 * 0.2 - 2.0 * 1.0 * 1.8 * 0.2;
    assert!((e.scene[&host].signed_volume() - expected).abs() < 1e-9);
    let model = e.document.model();
    let wall = &model.walls[&host].parameters;
    let openings = [
        model.resolve_opening(&door.parameters).unwrap(),
        model.resolve_opening(&window.parameters).unwrap(),
    ];
    let parts = os_walls::wall_prisms(wall, 0.0, &openings).unwrap();
    // No rectangular cell occupies the interior of either opening, including its front/back surfaces.
    for (x, z) in [(1.5, 0.5), (1.5, 1.7), (3.5, 1.0), (3.5, 2.5)] {
        assert!(parts.iter().all(|s| !(s.profile.vertices[0].x < x
            && x < s.profile.vertices[1].x
            && s.transform.translation.z < z
            && z < s.transform.translation.z + s.height)));
    }
    let context = e.native_plan_context(view).unwrap();
    let drawing = e.native_wall_plan(view).unwrap();
    assert_eq!(drawing.pick(context, Point2::new(1.5, 0.0)).unwrap(), None);
    assert!(
        !drawing
            .items(context)
            .unwrap()
            .iter()
            .any(
                |item| item.footprint.role == os_geometry::plan::PlanRole::Cut
                    && item.footprint.contains(Point2::new(3.5, 0.0))
            )
    );
    let camera = os_render::plan::PlanCamera::default();
    let size = [800.0, 600.0];
    for (id, point) in [
        (door.id(), Point2::new(1.0, 0.4)),
        (window.id(), Point2::new(3.5, 0.0)),
    ] {
        assert_eq!(
            drawing
                .pick_screen(
                    context,
                    camera,
                    size,
                    camera.project(point, size).unwrap(),
                    6.0
                )
                .unwrap(),
            Some(id)
        );
    }
    let camera = os_render::Camera::default();
    let triangles = os_render::project_scene(&e.scene, &camera, 800.0, 600.0);
    for id in [door.id(), window.id()] {
        assert!(
            triangles.iter().filter(|t| t.entity == id).any(|t| {
                let x = t.points.iter().map(|p| p.x).sum::<f32>() / 3.0;
                let y = t.points.iter().map(|p| p.y).sum::<f32>() / 3.0;
                os_render::pick(&triangles, x, y) == Some(id)
            }),
            "fixture must be independently pickable: {id}"
        );
    }
    let mut settings = model.views[&view].parameters.plan.unwrap();
    settings.visibility.walls = false;
    let level = wall.level;
    e.update_floor_plan(view, "Openings", level, settings)
        .unwrap();
    assert!(drawing.items(e.native_plan_context(view).unwrap()).is_err());
    let hidden = e.native_wall_plan(view).unwrap();
    assert!(
        hidden
            .items(e.native_plan_context(view).unwrap())
            .unwrap()
            .is_empty()
    );
    assert!(
        hidden
            .provider_lines(e.native_plan_context(view).unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn door_hinge_swing_all_combinations_both_host_directions_match_arc_and_3d_leaf() {
    for reverse in [false, true] {
        for typed in [false, true] {
            let (mut e, host, view, mut door, window) = setup();
            // Diagonal hosts catch accidental world-axis interpretation.
            let mut wall = e.document.model().walls[&host].parameters.clone();
            wall.start = Point2::new(1.0, 2.0);
            wall.end = Point2::new(4.6, 6.8);
            if reverse {
                std::mem::swap(&mut wall.start, &mut wall.end);
            }
            e.command(
                "Direction",
                Command::UpdateWall {
                    id: host,
                    parameters: wall.clone(),
                },
            )
            .unwrap();
            let world = |x: f64, y: f64| {
                let dx = (wall.end.x - wall.start.x) / wall.length();
                let dy = (wall.end.y - wall.start.y) / wall.length();
                Point2::new(
                    wall.start.x + dx * x - dy * y,
                    wall.start.y + dy * x + dx * y,
                )
            };
            if typed {
                let ty = OpeningType::new(
                    "core.opening_type",
                    OpeningTypeParams {
                        family: Default::default(),
                        name: "Shared".into(),
                        pane_position: Default::default(),
                        kind: OpeningKind::Door,
                        width: 1.0,
                        height: 1.8,
                        sill: 0.0,
                    },
                );
                door.parameters.definition = OpeningDefinition::Typed { type_id: ty.id() };
                e.command("Type", Command::AddOpeningType(ty)).unwrap();
            }
            e.command("Door", Command::AddOpening(door.clone()))
                .unwrap();
            e.command("Window", Command::AddOpening(window.clone()))
                .unwrap();
            let original_host = e.scene[&host].clone();
            let original_window = e.scene[&window.id()].clone();
            let types = e.document.model().opening_types.clone();
            let mut expected_window = wall.clone();
            expected_window.start = world(3.0, 0.0);
            expected_window.end = world(4.0, 0.0);
            expected_window.height = 1.8;
            expected_window.thickness = 0.025;
            assert_eq!(
                original_window,
                PrismKernel
                    .tessellate(&os_walls::wall_solid(&expected_window, 0.8).unwrap())
                    .unwrap()
            );
            for hinge in [DoorHinge::Start, DoorHinge::End] {
                for swing in [DoorSwing::Left, DoorSwing::Right] {
                    let mut p = door.parameters.clone();
                    p.hinge = hinge;
                    p.swing = swing;
                    e.command(
                        "Orientation",
                        Command::UpdateOpening {
                            id: door.id(),
                            parameters: p,
                        },
                    )
                    .unwrap();
                    let x = if hinge == DoorHinge::Start { 1.0 } else { 2.0 };
                    let side = if swing == DoorSwing::Left { 1.0 } else { -1.0 };
                    let closed = if hinge == DoorHinge::Start { 1.0 } else { -1.0 };
                    let h = world(x, -side * 0.1);
                    let tip = world(x, side * 0.9);
                    let mut expected_panel = wall.clone();
                    expected_panel.start = h;
                    expected_panel.end = tip;
                    expected_panel.height = 1.8;
                    expected_panel.thickness = 0.025;
                    assert_eq!(
                        e.scene[&door.id()],
                        PrismKernel
                            .tessellate(&os_walls::wall_solid(&expected_panel, 0.0).unwrap())
                            .unwrap()
                    );
                    assert_eq!(e.scene[&host], original_host);
                    assert_eq!(e.scene[&window.id()], original_window);
                    assert_eq!(e.document.model().opening_types, types);
                    let context = e.native_plan_context(view).unwrap();
                    let drawing = e.native_wall_plan(view).unwrap();
                    let lines: Vec<_> = drawing
                        .provider_lines(context)
                        .unwrap()
                        .iter()
                        .filter(|line| line.entity == door.id())
                        .collect();
                    assert_eq!(lines.len(), 19);
                    for (i, line) in lines.iter().enumerate() {
                        assert_eq!(line.feature, i as u32);
                    }
                    assert!(lines[2].start.distance(h) < 1e-12);
                    assert!(lines[2].end.distance(tip) < 1e-12);
                    assert!(lines[3].start.distance(world(x + closed, -side * 0.1)) < 1e-12);
                    assert!(lines[18].end.distance(tip) < 1e-12);
                    for line in &lines[3..] {
                        for point in [line.start, line.end] {
                            assert!((point.distance(h) - 1.0).abs() < 1e-12);
                        }
                    }
                    for pair in lines[3..].windows(2) {
                        assert_eq!(pair[0].end, pair[1].start);
                    }
                    let camera = os_render::plan::PlanCamera::default();
                    for line in [lines[2], lines[11]] {
                        let midpoint = Point2::new(
                            (line.start.x + line.end.x) * 0.5,
                            (line.start.y + line.end.y) * 0.5,
                        );
                        assert_eq!(
                            drawing
                                .pick_screen(
                                    context,
                                    camera,
                                    [800.0, 600.0],
                                    camera.project(midpoint, [800.0, 600.0]).unwrap(),
                                    1.0
                                )
                                .unwrap(),
                            Some(door.id())
                        );
                    }
                    let window_lines: Vec<_> = drawing
                        .provider_lines(context)
                        .unwrap()
                        .iter()
                        .filter(|line| line.entity == window.id())
                        .collect();
                    assert_eq!(window_lines.len(), 5);
                    for (i, y) in [-0.05, 0.0, 0.05].into_iter().enumerate() {
                        assert!(window_lines[i + 2].start.distance(world(3.0, y)) < 1e-12);
                        assert!(window_lines[i + 2].end.distance(world(4.0, y)) < 1e-12);
                    }
                }
            }
        }
    }
}

#[test]
fn typed_window_pane_faces_match_plan_and_3d_for_both_wall_directions() {
    for reverse in [false, true] {
        let (mut e, host, view, _, _) = setup();
        let original_model = e.document.model().clone();
        let invalid_door = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                family: Default::default(),
                name: "Invalid offset door".into(),
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
                pane_position: WindowPanePosition::LeftFace,
            },
        );
        assert!(
            e.command("Invalid door type", Command::AddOpeningType(invalid_door))
                .is_err()
        );
        assert_eq!(e.document.model(), &original_model);
        let mut wall = e.document.model().walls[&host].parameters.clone();
        if reverse {
            std::mem::swap(&mut wall.start, &mut wall.end);
            e.command(
                "Reverse host",
                Command::UpdateWall {
                    id: host,
                    parameters: wall.clone(),
                },
            )
            .unwrap();
        }
        let world = |x: f64, y: f64| {
            let dx = (wall.end.x - wall.start.x) / wall.length();
            let dy = (wall.end.y - wall.start.y) / wall.length();
            Point2::new(
                wall.start.x + dx * x - dy * y,
                wall.start.y + dy * x + dx * y,
            )
        };
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                family: Default::default(),
                name: "Face-aligned window".into(),
                kind: OpeningKind::Window,
                width: 1.0,
                height: 1.2,
                sill: 0.8,
                pane_position: WindowPanePosition::Center,
            },
        );
        let type_id = ty.id();
        let windows = [1.0, 3.5].map(|offset| {
            Opening::new(
                "core.opening",
                OpeningParams {
                    name: "Shared window".into(),
                    host,
                    offset,
                    definition: OpeningDefinition::Typed { type_id },
                    hinge: DoorHinge::Start,
                    swing: DoorSwing::Left,
                },
            )
        });
        let window_ids = windows.each_ref().map(|window| window.id());
        e.document
            .execute(
                "Create shared windows",
                std::iter::once(Command::AddOpeningType(ty))
                    .chain(windows.into_iter().map(Command::AddOpening))
                    .collect(),
            )
            .unwrap();
        e.regenerate().unwrap();
        let host_mesh = e.scene[&host].clone();
        let level_elevation = e.document.model().levels[&wall.level].parameters.elevation;

        for position in [
            WindowPanePosition::Center,
            WindowPanePosition::LeftFace,
            WindowPanePosition::RightFace,
        ] {
            if position != WindowPanePosition::Center {
                let history_before = e.document.history_stats().undo_entries;
                let mut parameters = e.document.model().opening_types[&type_id]
                    .parameters
                    .clone();
                parameters.pane_position = position;
                e.command(
                    "Move shared pane",
                    Command::UpdateOpeningType {
                        id: type_id,
                        parameters,
                    },
                )
                .unwrap();
                assert_eq!(e.document.history_stats().undo_entries, history_before + 1);
            }
            assert_eq!(e.scene[&host], host_mesh);
            let context = e.native_plan_context(view).unwrap();
            let drawing = e.native_wall_plan(view).unwrap();
            let lines = drawing.provider_lines(context).unwrap();
            for id in window_ids {
                let opening = &e.document.model().openings[&id];
                let resolved = e
                    .document
                    .model()
                    .resolve_opening(&opening.parameters)
                    .unwrap();
                assert_eq!(resolved.type_id, Some(type_id));
                assert_eq!(resolved.pane_position, position);
                let panel_thickness = 0.025_f64
                    .min(wall.thickness * 0.2)
                    .min(resolved.width * 0.2);
                let face_offset = (wall.thickness - panel_thickness) / 2.0;
                let pane_y = match position {
                    WindowPanePosition::Center => 0.0,
                    WindowPanePosition::LeftFace => face_offset,
                    WindowPanePosition::RightFace => -face_offset,
                };
                let mut panel = wall.clone();
                panel.start = world(opening.parameters.offset, pane_y);
                panel.end = world(opening.parameters.offset + resolved.width, pane_y);
                panel.height = resolved.height;
                panel.thickness = panel_thickness;
                let expected = PrismKernel
                    .tessellate(
                        &os_walls::wall_solid(&panel, level_elevation + resolved.sill).unwrap(),
                    )
                    .unwrap();
                // Family extrusion may triangulate/order the same rectangular
                // solid differently; migration promises geometric equivalence.
                let mut actual_vertices = e.scene[&id].vertices.clone();
                let mut expected_vertices = expected.vertices.clone();
                let order = |a: &os_geometry::Vec3, b: &os_geometry::Vec3| {
                    a.x.total_cmp(&b.x)
                        .then(a.y.total_cmp(&b.y))
                        .then(a.z.total_cmp(&b.z))
                };
                actual_vertices.sort_by(order);
                expected_vertices.sort_by(order);
                for (a, b) in actual_vertices.iter().zip(&expected_vertices) {
                    assert!(
                        (a.x - b.x).abs() < 1e-12
                            && (a.y - b.y).abs() < 1e-12
                            && (a.z - b.z).abs() < 1e-12
                    );
                }
                assert_eq!(actual_vertices.len(), expected_vertices.len());
                assert!((e.scene[&id].signed_volume() - expected.signed_volume()).abs() < 1e-12);

                let pane_line = lines
                    .iter()
                    .find(|line| line.entity == id && line.feature == 3)
                    .expect("window pane line");
                let expected_start = context
                    .basis
                    .world_to_plane(world(opening.parameters.offset, pane_y))
                    .unwrap();
                let expected_end = context
                    .basis
                    .world_to_plane(world(opening.parameters.offset + resolved.width, pane_y))
                    .unwrap();
                assert!(pane_line.start.distance(expected_start) < 1e-10);
                assert!(pane_line.end.distance(expected_end) < 1e-10);
            }
        }

        let changed = e.document.model().clone();
        assert!(e.document.undo());
        assert_eq!(
            e.document.model().opening_types[&type_id]
                .parameters
                .pane_position,
            WindowPanePosition::LeftFace
        );
        assert!(e.document.redo());
        assert_eq!(e.document.model(), &changed);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pane-position.osb");
        e.save(&path).unwrap();
        let mut reopened = Editor::new().unwrap();
        reopened.open(&path).unwrap();
        assert_eq!(reopened.document.model(), &changed);
        for id in window_ids {
            assert_eq!(
                reopened
                    .document
                    .model()
                    .resolve_opening(&reopened.document.model().openings[&id].parameters)
                    .unwrap()
                    .pane_position,
                WindowPanePosition::RightFace
            );
        }
    }
}

#[test]
fn opening_arc_crop_draw_and_pick_share_clipped_segments_even_without_aperture_in_crop() {
    let (mut e, _, view, door, _) = setup();
    e.command("Door", Command::AddOpening(door.clone()))
        .unwrap();
    let full_context = e.native_plan_context(view).unwrap();
    let full = e.native_wall_plan(view).unwrap();
    let originals = full.provider_lines(full_context).unwrap().to_vec();
    let mut settings = e.document.model().views[&view].parameters.plan.unwrap();
    settings.crop = Some(os_model::PlanViewCrop {
        min: Point2::new(1.45, 0.45),
        max: Point2::new(1.85, 0.75),
    });
    let level = e.document.model().walls[&door.parameters.host]
        .parameters
        .level;
    e.update_floor_plan(view, "Arc crop", level, settings)
        .unwrap();
    let context = e.native_plan_context(view).unwrap();
    let drawing = e.native_wall_plan(view).unwrap();
    let lines = drawing.provider_lines(context).unwrap();
    assert!(!lines.is_empty());
    assert!(drawing.items(context).unwrap().is_empty());
    let camera = os_render::plan::PlanCamera::default();
    let size = [800.0, 600.0];
    for line in lines {
        assert_eq!(line.entity, door.id());
        assert!(line.feature >= 3);
        let raw = originals
            .iter()
            .find(|raw| raw.feature == line.feature)
            .unwrap();
        for point in [line.start, line.end] {
            assert!(
                point.x >= 1.45 - 1e-12
                    && point.x <= 1.85 + 1e-12
                    && point.y >= 0.45 - 1e-12
                    && point.y <= 0.75 + 1e-12
            );
            assert!(
                (raw.start.distance(point) + point.distance(raw.end) - raw.start.distance(raw.end))
                    .abs()
                    < 1e-12
            );
        }
        let midpoint = Point2::new(
            (line.start.x + line.end.x) * 0.5,
            (line.start.y + line.end.y) * 0.5,
        );
        assert_eq!(
            drawing
                .pick_screen(
                    context,
                    camera,
                    size,
                    camera.project(midpoint, size).unwrap(),
                    0.1
                )
                .unwrap(),
            Some(door.id())
        );
    }
    assert_eq!(
        drawing
            .pick_screen(
                context,
                camera,
                size,
                camera.project(Point2::new(1.0, 0.4), size).unwrap(),
                1.0
            )
            .unwrap(),
        None
    );
    assert!(originals.iter().any(|raw| lines.iter().any(
        |line| raw.feature == line.feature && (raw.start != line.start || raw.end != line.end)
    )));
}
