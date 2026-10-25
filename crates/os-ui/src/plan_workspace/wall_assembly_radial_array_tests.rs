//! Radial array acceptance through the existing real-egui harness.
use super::*;

fn rotate(p: Point2, pivot: Point2, degrees: f64) -> Point2 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (x, y) = (p.x - pivot.x, p.y - pivot.y);
    Point2::new(pivot.x + cos * x - sin * y, pivot.y + sin * x + cos * y)
}

#[test]
fn wall_assembly_radial_array_signed_preview_commit_both_dpis() {
    for (size, scale) in PROFILES {
        for step in [30.0, -30.0] {
            let mut h = Harness::new(size, scale);
            let (mut model, bay) = copy_tests::fixture(&mut h);
            model
                .views
                .get_mut(&h.view)
                .unwrap()
                .parameters
                .plan
                .as_mut()
                .unwrap()
                .basis
                .rotation = 0.37;
            // A second view's opening tag stays exclusively attached to the source.
            let mut other_view = model.views[&h.view].clone();
            other_view.header.id = Id::new();
            let mut other_tag = model.opening_tags[&bay.tags[0]].clone();
            other_tag.header.id = Id::new();
            other_tag.parameters.view = other_view.id();
            model.views.insert(other_view.id(), other_view);
            model.opening_tags.insert(other_tag.id(), other_tag.clone());
            copy_tests::load(&mut h, model, &bay.walls);
            h.app.plans.split = true;
            let pivot = Point2::new(10.0, 3.0);
            let basis = h.app.editor.native_plan_context(h.view).unwrap().basis;
            let camera = h.app.plans.cameras.get_mut(&h.view).unwrap();
            camera.center = basis.world_to_plane(Point2::new(6.0, 2.0)).unwrap();
            camera.pixels_per_metre = 14.0;
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            h.app.begin_wall_assembly_array();
            h.frame(vec![]);
            h.click_text_in_band("Radial", 0.0, size.y);
            let draft = h.app.plans.wall_set_move.as_mut().unwrap();
            assert!(
                draft.radial,
                "the visible mode control selects radial placement"
            );
            draft.set_array_count(3).unwrap();
            draft.radial_degrees = step.to_string();
            // Exact angle must not disable snapping of the initial pivot.
            draft.gesture.length = "99".into();
            draft.gesture.angle_degrees = "45".into();
            h.app.plans.snaps = SnapOptions {
                enabled: true,
                endpoints: true,
                intersections: false,
                perpendicular: false,
                midpoints: false,
                nearest: false,
                axis_extensions: false,
            };
            let screen = h.point(pivot);
            h.click(screen + egui::vec2(3.0, 2.0));
            let draft = h.app.plans.wall_set_move.as_ref().unwrap();
            near(
                basis.plane_to_world(draft.gesture.start.unwrap()).unwrap(),
                pivot,
            );
            let point = basis.world_to_plane(pivot).unwrap();
            let preview = draft.candidate(point).unwrap();
            assert_eq!(preview.model, draft.candidate(point).unwrap().model);
            let copied = draft.copied_ids();
            let camera = h.app.plans.cameras[&h.view];
            h.press(screen);
            h.frame(vec![egui::Event::PointerMoved(
                screen + egui::vec2(5.0, 4.0),
            )]);
            assert_eq!(h.app.plans.cameras[&h.view], camera, "array owns the drag");
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.editor.scene, scene);
            h.release(screen);
            h.settle();
            assert!(!h.app.status_error, "{}", h.app.status);
            let after = h.app.editor.document.model().clone();
            assert_eq!(after, preview.model);
            assert_eq!(h.app.selected_ids, copied);
            assert_eq!(after.walls.len(), before.walls.len() + 8);
            assert_eq!(after.openings.len(), before.openings.len() + 4);
            assert_eq!(after.wall_joins.len(), before.wall_joins.len() + 8);
            assert_eq!(after.opening_tags.len(), before.opening_tags.len() + 4);
            assert_eq!(after.opening_tags[&other_tag.id()], other_tag);
            assert_eq!(after.rooms, before.rooms);
            assert_eq!(after.room_tags, before.room_tags);
            assert_eq!(after.dimensions, before.dimensions);
            for (id, wall) in &before.walls {
                assert_eq!(&after.walls[id], wall);
            }
            for (id, opening) in &before.openings {
                assert_eq!(&after.openings[id], opening);
            }
            for source in &bay.walls {
                let old = &before.walls[source];
                for repeat in 1..=2 {
                    let start = rotate(old.parameters.start(), pivot, step * repeat as f64);
                    let end = rotate(old.parameters.end(), pivot, step * repeat as f64);
                    let id = *copied
                        .iter()
                        .find(|id| {
                            let wall = &after.walls[id].parameters;
                            wall.start().distance(start) < 1e-8 && wall.end().distance(end) < 1e-8
                        })
                        .unwrap();
                    let clone = &after.walls[&id];
                    near(clone.parameters.end(), end);
                    let mut expected = old.parameters.clone();
                    expected.path = clone.parameters.path;
                    assert_eq!(clone.parameters, expected);
                    assert_eq!(clone.header.properties, old.header.properties);
                    assert_eq!(
                        after.wall_type_assignments[&id],
                        before.wall_type_assignments[source]
                    );
                    assert_eq!(
                        after.element_lifecycle(id),
                        before.element_lifecycle(*source)
                    );
                    assert!(h.app.editor.scene.contains_key(&id));
                    for opening in before
                        .openings
                        .values()
                        .filter(|o| o.parameters.host == *source)
                    {
                        let clone = after
                            .openings
                            .values()
                            .find(|o| o.parameters.host == id)
                            .unwrap();
                        let mut expected = opening.parameters.clone();
                        expected.host = id;
                        assert!((expected.offset - clone.parameters.offset).abs() < 1e-9);
                        expected.offset = clone.parameters.offset;
                        assert_eq!(clone.parameters, expected);
                        assert_eq!(
                            after.opening_clearances[&clone.id()],
                            before.opening_clearances[&opening.id()]
                        );
                        assert_eq!(
                            after.element_lifecycle(clone.id()),
                            before.element_lifecycle(opening.id())
                        );
                        assert!(h.app.editor.scene.contains_key(&clone.id()));
                        let tag = after
                            .opening_tags
                            .values()
                            .find(|t| t.parameters.opening == clone.id())
                            .unwrap();
                        let source_tag = before
                            .opening_tags
                            .values()
                            .find(|t| {
                                t.parameters.opening == opening.id() && t.parameters.view == h.view
                            })
                            .unwrap();
                        near(
                            tag.parameters.position,
                            rotate(source_tag.parameters.position, pivot, step * repeat as f64),
                        );
                        assert_eq!(
                            tag.parameters.label_preset,
                            source_tag.parameters.label_preset
                        );
                    }
                }
            }
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            let committed_scene = h.app.editor.scene.clone();
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &after);
            assert_eq!(h.app.editor.scene, committed_scene);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("radial-wall-array.osb");
            h.app.editor.save(&path).unwrap();
            h.app.editor.open(&path).unwrap();
            assert_eq!(h.app.editor.document.model(), &after);
            assert_eq!(h.app.editor.scene, committed_scene);
        }
    }
}

#[test]
fn wall_assembly_radial_array_bounds_collisions_and_large_coordinates() {
    let h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    model.walls.get_mut(&h.wall).unwrap().parameters.path = WallPath::Straight {
        start: Point2::new(4.0, 0.0),
        end: Point2::new(6.0, 0.0),
    };
    let walls = BTreeSet::from([h.wall]);
    for count in [0, 1, 33, usize::MAX] {
        assert!(Array::new(&model, &walls, h.view, count).is_err());
    }
    let array = Array::new(&model, &walls, h.view, 3).unwrap();
    for step in [0.0, -0.0, 180.0, -180.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(
            array
                .radial_candidate(&model, Point2::default(), step)
                .is_err(),
            "{step}"
        );
    }
    assert!(
        array
            .radial_candidate(&model, Point2::new(f64::NAN, 0.0), 30.0)
            .is_err()
    );
    assert!(
        array
            .radial_candidate(&model, Point2::default(), 0.1)
            .is_err(),
        "overlapping repeated footprints reject"
    );
    // Only the last repeat encounters this obstacle.
    copy_tests::add_wall(
        &mut model,
        h.wall,
        Point2::new(0.0, -4.0),
        Point2::new(0.0, -6.0),
    );
    assert!(
        array
            .radial_candidate(&model, Point2::default(), 90.0)
            .is_ok()
    );
    let four = Array::new(&model, &walls, h.view, 4).unwrap();
    assert!(
        four.radial_candidate(&model, Point2::default(), 90.0)
            .is_err()
    );
    // Every copy uses the original geometry, even near the coordinate bound.
    let pivot = Point2::new(900_000.0, -900_000.0);
    model.walls.retain(|id, _| *id == h.wall);
    model.walls.get_mut(&h.wall).unwrap().parameters.path = WallPath::Straight {
        start: Point2::new(pivot.x + 4.0, pivot.y),
        end: Point2::new(pivot.x + 6.0, pivot.y),
    };
    for step in [37.123, -37.123] {
        let candidate = array.radial_candidate(&model, pivot, step).unwrap();
        for repeat in 1..=2 {
            let expected = rotate(
                model.walls[&h.wall].parameters.start(),
                pivot,
                step * repeat as f64,
            );
            assert!(
                candidate
                    .model
                    .walls
                    .values()
                    .any(|w| w.parameters.start().distance(expected) < 1e-8)
            );
        }
        assert_eq!(candidate.model.walls[&h.wall], model.walls[&h.wall]);
    }
}

#[test]
fn wall_assembly_radial_array_cancel_stale_invalid_release_and_pan_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (model, bay) = copy_tests::fixture(&mut h);
        copy_tests::load(&mut h, model, &bay.walls);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let scene = h.app.editor.scene.clone();
        for reason in [
            "escape",
            "selection",
            "view",
            "drawing",
            "revision",
            "invalid",
        ] {
            h.app.begin_wall_assembly_array();
            let draft = h.app.plans.wall_set_move.as_mut().unwrap();
            draft.radial = true;
            draft.gesture.start = Some(Point2::default());
            draft.radial_degrees = "0".into();
            match reason {
                "escape" => h.frame(vec![escape()]),
                "selection" => {
                    h.app.selected_ids.clear();
                    h.frame(vec![]);
                }
                "view" => {
                    h.app.plans.active = None;
                    h.frame(vec![]);
                }
                "drawing" => {
                    h.app.plans.drawing = None;
                    h.frame(vec![]);
                }
                "revision" => {
                    // Reload the same model into a new document session.
                    h.app.editor.document =
                        os_document::Document::from_model(before.clone()).unwrap();
                    h.frame(vec![]);
                }
                _ => h.click(h.point(Point2::default())),
            }
            assert!(h.app.plans.wall_set_move.is_none(), "{reason}");
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.editor.scene, scene);
            copy_tests::load(&mut h, before.clone(), &bay.walls);
        }
        let camera = h.app.plans.cameras[&h.view];
        let from = h.app.plans.canvas_rect.unwrap().center();
        let to = from + egui::vec2(22.0, 16.0);
        h.press(from);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        h.release(to);
        assert_ne!(h.app.plans.cameras[&h.view], camera);
    }
}

#[test]
fn wall_assembly_radial_array_exact_tee_contacts_and_room_protection() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    let host = h.wall;
    model.walls.get_mut(&host).unwrap().parameters.path = WallPath::Straight {
        start: Point2::new(4.0, 0.0),
        end: Point2::new(8.0, 0.0),
    };
    let branch = copy_tests::add_wall(
        &mut model,
        host,
        Point2::new(6.0, 0.0),
        Point2::new(6.0, -2.0),
    );
    let join = WallJoin::new(
        "core.wall_join",
        WallJoinParams::Tee {
            host,
            station: 2.0,
            branch: WallAnchor {
                wall: branch,
                endpoint: WallEndpoint::Start,
            },
        },
    );
    model.wall_joins.insert(join.id(), join);
    let array = Array::new(&model, &BTreeSet::from([host, branch]), h.view, 3).unwrap();
    for step in [63.713, -63.713] {
        let candidate = array
            .radial_candidate(&model, Point2::new(-3.137, 1.519), step)
            .unwrap();
        for join in candidate.model.wall_joins.values() {
            let WallJoinParams::Tee {
                host,
                station,
                branch,
            } = join.parameters
            else {
                unreachable!()
            };
            assert_eq!(
                candidate.model.walls[&host].parameters.path.point(station),
                branch
                    .endpoint
                    .point(&candidate.model.walls[&branch.wall].parameters)
            );
        }
    }
    // A 180-degree copy would split this existing separator-bounded room.
    let level = model.walls[&host].parameters.level;
    for (start, end) in [
        (Point2::new(-8.0, -2.0), Point2::new(-4.0, -2.0)),
        (Point2::new(-4.0, -2.0), Point2::new(-4.0, 2.0)),
        (Point2::new(-4.0, 2.0), Point2::new(-8.0, 2.0)),
        (Point2::new(-8.0, 2.0), Point2::new(-8.0, -2.0)),
    ] {
        let line = RoomSeparationLine::new(
            "core.room_separation_line",
            RoomSeparationLineParams { level, start, end },
        );
        model.room_separation_lines.insert(line.id(), line);
    }
    let seed = Point2::new(-5.0, 1.0);
    let faces = derive_faces(&model.room_boundary_segments(level).unwrap()).unwrap();
    let (room_model, _) = copy_tests::fixture(&mut h);
    let mut room = room_model.rooms.values().next().unwrap().clone();
    room.parameters.seed = seed;
    room.parameters.boundary_signature =
        faces.assign_seed(seed).unwrap().key.as_signature().to_vec();
    model.rooms.insert(room.id(), room);
    let array = Array::new(&model, &BTreeSet::from([host, branch]), h.view, 2).unwrap();
    let error = array
        .radial_candidate(&model, Point2::default(), 180.0)
        .err()
        .unwrap();
    assert!(error.to_string().contains("room"), "{error}");
}

#[cfg(feature = "external-plugins")]
#[test]
#[ignore = "requires OPENSTRUCTURE_WALL_TEST_PLUGIN installed guest directory"]
fn wall_assembly_radial_array_installed_provider_refusal_and_revocation() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let (model, bay) = copy_tests::fixture(&mut h);
    copy_tests::load(&mut h, model, &bay.walls);
    let before = h.app.editor.document.model().clone();
    let scene = h.app.editor.scene.clone();
    let history = h.app.editor.document.history_stats();
    let mut draft = Draft::begin_array(&h.app).unwrap();
    draft.radial = true;
    draft.gesture.start = Some(Point2::new(10.0, 3.0));
    assert!(draft.candidate(Point2::default()).is_ok());
    let directory =
        std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN").expect("installed guest directory");
    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
    h.app
        .editor
        .host
        .load_wasm_directory(
            std::path::Path::new(&directory),
            [
                os_plugin_api::Permission::ModelRead,
                os_plugin_api::Permission::ModelWrite,
                os_plugin_api::Permission::UiTool,
            ]
            .into(),
        )
        .unwrap();
    assert!(h.app.editor.host.worker_supported(os_walls::PLUGIN_ID));
    assert!(Draft::begin_array(&h.app).is_err());
    assert!(draft.commit(&mut h.app, Point2::default()).is_err());
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.document.history_stats(), history);
    assert_eq!(h.app.editor.scene, scene);
}
