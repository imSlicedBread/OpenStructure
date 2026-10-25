//! Real-egui acceptance for atomic native wall assembly Rotate/Mirror.
use super::*;
use crate::plan_workspace::transforms::{Draft, Mode};
use os_geometry::rooms::derive_faces;
use os_model::*;

fn add_wall(model: &mut Model, template: Id, a: Point2, b: Point2) -> Id {
    let mut parameters = model.walls[&template].parameters.clone();
    parameters.path = WallPath::Straight { start: a, end: b };
    let wall = Wall::new(os_walls::WALL_TYPE, parameters);
    let id = wall.id();
    model.walls.insert(id, wall);
    id
}

fn load(h: &mut Harness, model: Model, ids: &[Id]) {
    h.app
        .editor
        .pending_geometry
        .extend(model.walls.keys().chain(model.openings.keys()).copied());
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(ids[0]));
    h.app.selected_ids = ids.iter().copied().collect();
    h.settle();
}

fn fixture(h: &mut Harness) -> (Model, Vec<Id>, Id, Id) {
    let (door, window, _) = install_transform_openings(h, WindowPanePosition::LeftFace, true);
    let mut m = h.app.editor.document.model().clone();
    m.openings
        .get_mut(&window)
        .unwrap()
        .parameters
        .width_override = Some(0.45);
    m.opening_clearances.insert(
        door,
        OpeningClearance {
            end: ClearanceEnd::Start,
            distance: 0.2,
        },
    );
    let window_type = m.openings[&window].parameters.type_id().unwrap();
    m.opening_types
        .get_mut(&window_type)
        .unwrap()
        .parameters
        .family
        .side_lite = Some(SideLite {
        side: LiteSide::Start,
        width_fraction: 0.25,
        mullion_width: 0.04,
        material: None,
    });
    m.openings
        .get_mut(&window)
        .unwrap()
        .parameters
        .lite_side_override = Some(LiteSide::End);
    let right = add_wall(&mut m, h.wall, Point2::new(1., 0.), Point2::new(1., 2.));
    let top = add_wall(&mut m, h.wall, Point2::new(1., 2.), Point2::new(-1., 2.));
    let left = add_wall(&mut m, h.wall, Point2::new(-1., 2.), Point2::new(-1., 0.));
    let ids = vec![h.wall, right, top, left];
    for pair in ids.iter().zip(ids.iter().cycle().skip(1)).take(4) {
        let join = WallJoin::new(
            "core.wall_join",
            WallJoinParams::Corner {
                a: WallAnchor {
                    wall: *pair.0,
                    endpoint: WallEndpoint::End,
                },
                b: WallAnchor {
                    wall: *pair.1,
                    endpoint: WallEndpoint::Start,
                },
                owner: *pair.0,
            },
        );
        m.wall_joins.insert(join.id(), join);
    }
    let level = m.walls[&h.wall].parameters.level;
    let seed = Point2::new(0., 1.);
    let faces = derive_faces(&m.room_boundary_segments(level).unwrap()).unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Assembly room".into(),
            floor_finish: Some("Keep finish".into()),
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            level,
            seed,
            boundary_signature: faces.assign_seed(seed).unwrap().key.as_signature().to_vec(),
        },
    );
    let room_id = room.id();
    m.rooms.insert(room_id, room);
    let second = View::new("core.view", ViewParams::floor_plan("Other plan", level));
    let second_id = second.id();
    m.views.insert(second_id, second);
    for view in [h.view, second_id] {
        let tag = OpeningTag::new(
            "core.opening_tag",
            OpeningTagParams {
                view,
                opening: door,
                position: Point2::new(-0.5, -0.4),
                label_preset: Default::default(),
            },
        );
        m.opening_tags.insert(tag.id(), tag);
        let tag = RoomTag::new(
            "core.room_tag",
            RoomTagParams {
                view,
                room: room_id,
                position: Point2::new(0.2, 1.2),
            },
        );
        m.room_tags.insert(tag.id(), tag);
        let d = Dimension::new(
            "core.dimension",
            DimensionParams {
                layout: DimensionLayout::Aligned,
                view,
                first: DimensionReference::WallEndpoint {
                    wall: h.wall,
                    endpoint: DimensionEndpoint::Start,
                },
                second: DimensionReference::WallEndpoint {
                    wall: h.wall,
                    endpoint: DimensionEndpoint::End,
                },
                additional: Vec::new(),
                baseline_spacing_m: 0.25,
                offset_m: 0.3,
                orphan_hint: Point2::new(0., 0.3),
            },
        );
        d.parameters.validate_creation(&m).unwrap();
        for layout in [
            DimensionLayout::Chain,
            DimensionLayout::Baseline,
            DimensionLayout::Angular,
        ] {
            let mut p = d.parameters.clone();
            p.layout = layout;
            if layout == DimensionLayout::Angular {
                p.first = DimensionReference::WallEndpoint {
                    wall: h.wall,
                    endpoint: DimensionEndpoint::End,
                };
                p.second = DimensionReference::WallEndpoint {
                    wall: right,
                    endpoint: DimensionEndpoint::Start,
                };
            } else {
                p.second = DimensionReference::OpeningJamb {
                    opening: door,
                    jamb: DimensionJamb::Start,
                };
                p.additional.push(DimensionReference::OpeningJamb {
                    opening: door,
                    jamb: DimensionJamb::End,
                });
            }
            p.validate_creation(&m).unwrap();
            let d = Dimension::new("core.dimension", p);
            m.dimensions.insert(d.id(), d);
        }
        let mut p = d.parameters.clone();
        p.first = DimensionReference::WallFace {
            wall: h.wall,
            side: DimensionWallSide::Left,
            station_m: 0.,
        };
        p.second = DimensionReference::WallFace {
            wall: h.wall,
            side: DimensionWallSide::Left,
            station_m: 1.8,
        };
        p.validate_creation(&m).unwrap();
        let face = Dimension::new("core.dimension", p);
        m.dimensions.insert(face.id(), face);
        m.dimensions.insert(d.id(), d);
    }
    let mut vp = ViewParams::floor_plan("Linked elevation", level);
    vp.kind = ViewKind::Elevation;
    vp.level = None;
    vp.plan = None;
    vp.elevation = Some(ElevationViewSettings::new(
        h.wall,
        ElevationSide::Right,
        -1.,
        3.,
        -1.,
        4.,
    ));
    let view = View::new("core.view", vp);
    let view_id = view.id();
    m.views.insert(view_id, view);
    for layout in [
        DimensionLayout::ElevationHorizontal,
        DimensionLayout::ElevationVertical,
    ] {
        let (first, second) = if layout == DimensionLayout::ElevationHorizontal {
            (
                ElevationDimensionFeature::Start,
                ElevationDimensionFeature::End,
            )
        } else {
            (
                ElevationDimensionFeature::Bottom,
                ElevationDimensionFeature::Top,
            )
        };
        let p = DimensionParams {
            layout,
            view: view_id,
            first: DimensionReference::ElevationOpening {
                opening: window,
                feature: first,
            },
            second: DimensionReference::ElevationOpening {
                opening: window,
                feature: second,
            },
            additional: vec![],
            baseline_spacing_m: 0.25,
            offset_m: 0.3,
            orphan_hint: Point2::new(0., 1.),
        };
        p.validate_creation(&m).unwrap();
        let d = Dimension::new("core.dimension", p);
        m.dimensions.insert(d.id(), d);
    }
    (m, ids, door, window)
}

fn near(a: Point2, b: Point2) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

#[test]
fn wall_assembly_transform_exact_pivot_mirror_rooms_tags_history_reopen_both_dpis() {
    for (size, scale) in PROFILES {
        for mode in [Mode::Rotate, Mode::Mirror] {
            let mut h = Harness::new(size, scale);
            let (model, ids, door, window) = fixture(&mut h);
            load(&mut h, model, &ids);
            h.app.plans.snaps.enabled = false;
            h.app.plans.split = true;
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            let camera = h.app.plans.cameras[&h.view];
            let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
            h.click_text_in_band("Snaps off", 0., h.size.y);
            h.frame(vec![]);
            h.click_text_in_band("Wall transforms", 0., h.size.y);
            h.frame(vec![]);
            h.click_text_in_band(
                if mode == Mode::Rotate {
                    "Rotate wall"
                } else {
                    "Mirror wall"
                },
                0.,
                h.size.y,
            );
            assert!(h.app.plans.transform.is_some(), "{}", h.app.status);
            h.frame(vec![]);
            let pivot = if mode == Mode::Rotate {
                Point2::new(0.5, -0.5)
            } else {
                Point2::new(0., -1.)
            };
            h.click(h.point(pivot));
            assert!(h.app.plans.transform.as_ref().unwrap().axis.is_some());
            if mode == Mode::Rotate {
                h.app.plans.transform.as_mut().unwrap().degrees = "90".into();
            }
            let target = h.point(Point2::new(0., 1.));
            h.frame(vec![egui::Event::PointerMoved(target)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.plans.cameras[&h.view], camera);
            assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
            let candidate = h
                .app
                .plans
                .transform
                .as_ref()
                .unwrap()
                .candidate(Point2::new(0., 1.))
                .unwrap();
            let map = |p: Point2| {
                if mode == Mode::Rotate {
                    Point2::new(pivot.x - (p.y - pivot.y), pivot.y + (p.x - pivot.x))
                } else {
                    Point2::new(-p.x, p.y)
                }
            };
            let mut expected = before.clone();
            for &id in &ids {
                let w = expected.walls.get_mut(&id).unwrap();
                let a = map(w.parameters.start());
                let b = map(w.parameters.end());
                *w.parameters.path.straight_start_mut().unwrap() = a;
                *w.parameters.path.straight_end_mut().unwrap() = b;
                near(candidate.model.walls[&id].parameters.start(), a);
                near(candidate.model.walls[&id].parameters.end(), b);
                // Compare all parameters/metadata except floating-point endpoint roundoff.
                w.parameters.path = candidate.model.walls[&id].parameters.path.clone();
            }
            for t in expected.opening_tags.values_mut() {
                t.parameters.position = map(t.parameters.position);
            }
            for t in expected.room_tags.values_mut() {
                t.parameters.position = map(t.parameters.position);
            }
            for r in expected.rooms.values_mut() {
                near(
                    candidate.model.rooms[&r.id()].parameters.seed,
                    map(r.parameters.seed),
                );
                r.parameters.seed = candidate.model.rooms[&r.id()].parameters.seed;
                r.parameters.boundary_signature = candidate.model.rooms[&r.id()]
                    .parameters
                    .boundary_signature
                    .clone();
                let faces = derive_faces(
                    &candidate
                        .model
                        .room_boundary_segments(r.parameters.level)
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    faces
                        .assign_seed(r.parameters.seed)
                        .unwrap()
                        .key
                        .as_signature(),
                    r.parameters.boundary_signature
                );
                assert!((faces.assign_seed(r.parameters.seed).unwrap().area_m2 - 4.).abs() < 1e-9);
            }
            for (id, t) in &mut expected.opening_tags {
                near(
                    candidate.model.opening_tags[id].parameters.position,
                    t.parameters.position,
                );
                t.parameters.position = candidate.model.opening_tags[id].parameters.position;
            }
            for (id, t) in &mut expected.room_tags {
                near(
                    candidate.model.room_tags[id].parameters.position,
                    t.parameters.position,
                );
                t.parameters.position = candidate.model.room_tags[id].parameters.position;
            }
            if mode == Mode::Mirror {
                expected
                    .wall_type_assignments
                    .get_mut(&h.wall)
                    .unwrap()
                    .flipped = true;
                expected.openings.get_mut(&door).unwrap().parameters.swing = DoorSwing::Right;
                expected
                    .openings
                    .get_mut(&window)
                    .unwrap()
                    .parameters
                    .pane_position_override = Some(WindowPanePosition::RightFace);
            }
            assert_eq!(
                candidate.model, expected,
                "candidate must preserve every other model field"
            );
            for opening in [door, window] {
                let old = crate::opening_tools::panel_mesh(&before, opening).unwrap();
                let new = crate::opening_tools::panel_mesh(&candidate.model, opening).unwrap();
                for vertex in &old.vertices {
                    let p = map(Point2::new(vertex.x, vertex.y));
                    assert!(
                        new.vertices.iter().any(|v| (v.x - p.x).abs() < 1e-9
                            && (v.y - p.y).abs() < 1e-9
                            && (v.z - vertex.z).abs() < 1e-9),
                        "hosted panel must undergo the same rigid transform"
                    );
                }
            }
            let preview = h
                .app
                .plans
                .transform
                .as_ref()
                .unwrap()
                .assembly_preview(Point2::new(0., 1.))
                .unwrap();
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            assert_eq!(preview.opening_tags(context).unwrap().len(), 1);
            assert_eq!(preview.room_tags(context).unwrap().len(), 1);
            assert_eq!(preview.rooms(context).unwrap().len(), 1);
            assert!(
                preview
                    .dimensions(context)
                    .unwrap()
                    .iter()
                    .all(|d| d.diagnostic.is_none())
            );
            assert_eq!(preview.angular_dimensions(context).unwrap().len(), 1);
            for d in candidate.model.dimensions.values() {
                d.parameters.validate_creation(&candidate.model).unwrap();
            }
            h.click(target);
            h.settle();
            assert!(!h.app.status_error, "{}", h.app.status);
            assert_eq!(h.app.editor.document.model(), &candidate.model);
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            assert_ne!(h.app.editor.scene, scene);
            assert_ne!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
            let committed = h.app.editor.document.model().clone();
            let committed_scene = h.app.editor.scene.clone();
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &committed);
            assert_eq!(h.app.editor.scene, committed_scene);
            let temp = tempfile::tempdir_in(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"),
            )
            .unwrap();
            let path = temp.path().join("assembly-transform.osb");
            h.app.editor.save(&path).unwrap();
            h.app.editor.open(&path).unwrap();
            assert_eq!(h.app.editor.document.model(), &committed);
            assert_eq!(h.app.editor.scene, committed_scene);
        }
    }
}

#[test]
fn wall_assembly_transform_mirror_axis_order_and_rotated_plan_snaps() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (mut model, ids, _, _) = fixture(&mut h);
        model
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .basis = PlanViewBasis {
            origin: Point2::new(0.2, -0.3),
            rotation: 0.63,
        };
        add_wall(&mut model, h.wall, Point2::new(4., 4.), Point2::new(5., 4.));
        load(&mut h, model, &ids);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let plane = |p| context.basis.world_to_plane(p).unwrap();
        for (a, b) in [
            (Point2::new(0., -1.), Point2::new(0., 1.)),
            (Point2::new(-1., 0.), Point2::new(1., 0.)),
            (Point2::new(-1., -1.), Point2::new(1., 1.)),
        ] {
            let mut draft = Draft::begin(&h.app, Mode::Mirror).unwrap();
            draft.axis = Some(plane(a));
            let candidate = draft.candidate(plane(b)).unwrap();
            for &id in &ids {
                let old = &h.app.editor.document.model().walls[&id].parameters;
                let new = &candidate.model.walls[&id].parameters;
                for (p, q) in [(old.start(), new.start()), (old.end(), new.end())] {
                    let expected = if a.x == b.x {
                        Point2::new(-p.x, p.y)
                    } else if a.y == b.y {
                        Point2::new(p.x, -p.y)
                    } else {
                        Point2::new(p.y, p.x)
                    };
                    near(q, expected);
                }
            }
        }
        let mut draft = Draft::begin(&h.app, Mode::Rotate).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        let camera = h.app.plans.cameras[&h.view];
        let rect = h.app.plans.canvas_rect.unwrap();
        let viewport = [rect.width() as f64, rect.height() as f64];
        let snaps = SnapOptions {
            enabled: true,
            endpoints: true,
            midpoints: false,
            intersections: false,
            perpendicular: false,
            nearest: false,
            axis_extensions: false,
        };
        let external = camera
            .project(plane(Point2::new(4.05, 4.)), viewport)
            .unwrap();
        near(
            draft
                .target(drawing, camera, viewport, external, snaps)
                .unwrap(),
            plane(Point2::new(4., 4.)),
        );
        let selected = camera
            .project(plane(Point2::new(1.05, 0.)), viewport)
            .unwrap();
        near(
            draft
                .target(drawing, camera, viewport, selected, snaps)
                .unwrap(),
            camera.unproject(selected, viewport).unwrap(),
        );
        draft.axis = Some(plane(Point2::new(0., 0.)));
        draft.degrees = "37".into();
        near(
            draft
                .target(drawing, camera, viewport, external, snaps)
                .unwrap(),
            camera.unproject(external, viewport).unwrap(),
        );
        assert!(
            draft.candidate(plane(Point2::new(0., 1.))).is_ok(),
            "non-cardinal rotation of joined loop"
        );
    }
}

#[test]
fn wall_assembly_transform_section_references_resolve_or_reject_whole_candidate() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let (mut model, ids, _, window) = fixture(&mut h);
    let level = model.walls[&h.wall].parameters.level;
    let mut vp = ViewParams::floor_plan("Section", level);
    vp.kind = ViewKind::Section;
    vp.plan = None;
    vp.section = Some(SectionViewSettings::new(
        Point2::new(-3., 0.),
        Point2::new(3., 0.),
        -1.,
        4.,
    ));
    let view = View::new("core.view", vp);
    let view_id = view.id();
    model.views.insert(view_id, view);
    let p = DimensionParams {
        view: view_id,
        layout: DimensionLayout::SectionHorizontal,
        first: DimensionReference::SectionOpening {
            opening: window,
            feature: ElevationDimensionFeature::Start,
        },
        second: DimensionReference::SectionOpening {
            opening: window,
            feature: ElevationDimensionFeature::End,
        },
        additional: vec![],
        baseline_spacing_m: 0.25,
        offset_m: 0.4,
        orphan_hint: Point2::new(3., 1.),
    };
    p.validate_creation(&model).unwrap();
    let d = Dimension::new("core.dimension", p);
    model.dimensions.insert(d.id(), d);
    load(&mut h, model.clone(), &ids);
    let mut mirror = Draft::begin(&h.app, Mode::Mirror).unwrap();
    mirror.axis = Some(Point2::new(0., -1.));
    let accepted = mirror.candidate(Point2::new(0., 1.)).unwrap();
    for d in accepted.model.dimensions.values() {
        d.parameters.validate_creation(&accepted.model).unwrap();
    }
    mirror.commit(&mut h.app, Point2::new(0., 1.)).unwrap();
    load(&mut h, model.clone(), &ids);
    let mut rotate = Draft::begin(&h.app, Mode::Rotate).unwrap();
    rotate.axis = Some(Point2::new(0., -2.));
    rotate.degrees = "180".into();
    assert!(
        rotate.candidate(Point2::new(0., 1.)).is_err(),
        "opening leaves the section cut plane"
    );
    assert!(rotate.commit(&mut h.app, Point2::new(0., 1.)).is_err());
    assert_eq!(h.app.editor.document.model(), &model);
    assert!(!h.app.editor.document.can_undo());
}

#[cfg(feature = "external-plugins")]
#[test]
#[ignore = "requires explicitly installed independent Wall guest via OPENSTRUCTURE_WALL_TEST_PLUGIN"]
fn wall_assembly_transform_installed_batch_refusal_and_single_wall_worker() {
    let directory =
        std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN").expect("Wall guest directory");
    for mode in [Mode::Rotate, Mode::Mirror] {
        let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
        let mut model = h.app.editor.document.model().clone();
        let other = add_wall(
            &mut model,
            h.wall,
            Point2::new(-1., 2.),
            Point2::new(1., 2.),
        );
        let ids = [h.wall, other];
        load(&mut h, model.clone(), &ids);
        h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
        h.app
            .editor
            .host
            .load_wasm_directory(
                Path::new(&directory),
                [
                    Permission::ModelRead,
                    Permission::ModelWrite,
                    Permission::UiTool,
                ]
                .into(),
            )
            .unwrap();
        h.settle();
        assert!(h.app.editor.host.worker_supported(os_walls::PLUGIN_ID));
        let error = Draft::begin(&h.app, mode)
            .err()
            .expect("installed multi-wall batch must reject");
        assert!(
            error
                .to_string()
                .contains("installed Wall provider cannot submit this batch"),
            "{error}"
        );
        assert!(!h.app.editor.plugin_work_pending());
        assert_eq!(h.app.editor.document.model(), &model);
        assert!(!h.app.editor.document.can_undo());
        h.app.selected_ids = [h.wall].into_iter().collect();
        let mut d = Draft::begin(&h.app, mode).unwrap();
        d.degrees = "90".into();
        d.axis = Some(Point2::new(0., -1.));
        let mut expected = d.candidate(Point2::new(0., 1.)).unwrap().model;
        d.commit(&mut h.app, Point2::new(0., 1.)).unwrap();
        assert!(h.app.editor.plugin_work_pending());
        h.settle();
        assert!(!h.app.status_error, "{}", h.app.status);
        // Installed commits also record the provider requirement, unlike a native preview.
        expected.plugin_requirements = h.app.editor.document.model().plugin_requirements.clone();
        assert_eq!(h.app.editor.document.model(), &expected);
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &model);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &expected);
    }
}

#[test]
fn wall_assembly_transform_complete_butt_and_tee_components() {
    for tee in [false, true] {
        for mode in [Mode::Rotate, Mode::Mirror] {
            let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
            let mut m = h.app.editor.document.model().clone();
            let (a, b) = if tee {
                (Point2::new(0., 0.), Point2::new(0., 2.))
            } else {
                (Point2::new(1., 0.), Point2::new(3., 0.))
            };
            let other = add_wall(&mut m, h.wall, a, b);
            let parameters = if tee {
                WallJoinParams::Tee {
                    host: h.wall,
                    station: 1.,
                    branch: WallAnchor {
                        wall: other,
                        endpoint: WallEndpoint::Start,
                    },
                }
            } else {
                WallJoinParams::Butt {
                    a: WallAnchor {
                        wall: h.wall,
                        endpoint: WallEndpoint::End,
                    },
                    b: WallAnchor {
                        wall: other,
                        endpoint: WallEndpoint::Start,
                    },
                }
            };
            let join = WallJoin::new("core.wall_join", parameters);
            m.wall_joins.insert(join.id(), join);
            let ids = [h.wall, other];
            load(&mut h, m.clone(), &ids);
            let mut d = Draft::begin(&h.app, mode).unwrap();
            d.axis = Some(Point2::new(0.2, -0.3));
            d.degrees = "37".into();
            let candidate = d.candidate(Point2::new(0.2, 1.)).unwrap();
            assert_eq!(candidate.model.wall_joins, m.wall_joins);
            for join in candidate.model.wall_joins.values() {
                join.parameters.validate(&candidate.model).unwrap();
            }
            d.commit(&mut h.app, Point2::new(0.2, 1.)).unwrap();
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        }
    }
}

#[test]
fn wall_assembly_transform_partial_loops_invalid_geometry_and_dimensions_reject_atomically() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let (model, ids, _, _) = fixture(&mut h);
    load(&mut h, model.clone(), &ids[..2]);
    assert!(Draft::begin(&h.app, Mode::Rotate).is_err(), "partial join");
    let mut unjoined = model.clone();
    unjoined.wall_joins.clear();
    load(&mut h, unjoined.clone(), &ids[..2]);
    let mut d = Draft::begin(&h.app, Mode::Rotate).unwrap();
    d.axis = Some(Point2::default());
    d.degrees = "90".into();
    assert!(d.candidate(Point2::new(0., 1.)).is_err(), "partial room");
    // A separation-line side cannot be silently left behind.
    unjoined.rooms.clear();
    unjoined.room_tags.clear();
    let removed = unjoined.walls.remove(&ids[3]).unwrap();
    let line = RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level: removed.parameters.level,
            start: removed.parameters.start(),
            end: removed.parameters.end(),
        },
    );
    unjoined.room_separation_lines.insert(line.id(), line);
    load(&mut h, unjoined, &ids[..3]);
    let mut d = Draft::begin(&h.app, Mode::Rotate).unwrap();
    d.axis = Some(Point2::default());
    d.degrees = "90".into();
    assert!(
        d.candidate(Point2::new(0., 1.)).is_err(),
        "separation-line loop"
    );
    load(&mut h, model.clone(), &ids);
    let before = h.app.editor.document.model().clone();
    let scene = h.app.editor.scene.clone();
    let history = h.app.editor.document.history_stats();
    let mut d = Draft::begin(&h.app, Mode::Rotate).unwrap();
    d.axis = Some(Point2::default());
    for angle in ["NaN", "inf", "bad"] {
        d.degrees = angle.into();
        assert!(d.candidate(Point2::new(0., 1.)).is_err());
    }
    d.degrees = "90".into();
    assert!(d.candidate(Point2::new(f64::NAN, 0.)).is_err());
    d.axis = Some(Point2::new(1e7, 0.));
    assert!(d.candidate(Point2::new(0., 1.)).is_err());
    let mut d = Draft::begin(&h.app, Mode::Mirror).unwrap();
    d.axis = Some(Point2::default());
    assert!(d.candidate(Point2::default()).is_err());
    assert!(d.commit(&mut h.app, Point2::default()).is_err());
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);
    // A valid source and valid finite axis produce overlapping room boundaries.
    // This is a geometric rejection, independent of angle syntax or references.
    let mut obstacle = model.clone();
    add_wall(
        &mut obstacle,
        h.wall,
        Point2::new(-1., -2.),
        Point2::new(1., -2.),
    );
    load(&mut h, obstacle.clone(), &ids);
    let mut d = Draft::begin(&h.app, Mode::Mirror).unwrap();
    d.axis = Some(Point2::new(-1., -1.));
    let scene = h.app.editor.scene.clone();
    let history = h.app.editor.document.history_stats();
    let error = d
        .candidate(Point2::new(1., -1.))
        .err()
        .expect("overlapping reflected boundaries must reject");
    assert!(
        error.to_string().contains("ambiguous third wall"),
        "{error}"
    );
    assert!(d.commit(&mut h.app, Point2::new(1., -1.)).is_err());
    assert_eq!(h.app.editor.document.model(), &obstacle);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);
    let mut bad = model.clone();
    bad.dimensions
        .values_mut()
        .find(|dimension| dimension.parameters.layout == DimensionLayout::Aligned)
        .unwrap()
        .parameters
        .second = DimensionReference::WallEndpoint {
        wall: Id::new(),
        endpoint: DimensionEndpoint::End,
    };
    load(&mut h, bad, &ids);
    let mut d = Draft::begin(&h.app, Mode::Rotate).unwrap();
    d.axis = Some(Point2::default());
    d.degrees = "90".into();
    assert!(
        d.candidate(Point2::new(0., 1.)).is_err(),
        "already orphaned affected reference"
    );
}

#[test]
fn wall_assembly_transform_escape_stale_pan_bounds_both_dpis() {
    for (size, scale) in PROFILES {
        for mode in [Mode::Rotate, Mode::Mirror] {
            let mut h = Harness::new(size, scale);
            let (model, ids, _, _) = fixture(&mut h);
            load(&mut h, model.clone(), &ids);
            h.app.selected_ids = (0..65).map(|_| Id::new()).collect();
            assert!(Draft::begin(&h.app, mode).is_err());
            h.app.selected_ids = ids.iter().copied().collect();
            for cause in ["escape", "selection", "drawing", "document", "pointer"] {
                h.app.begin_wall_transform(mode);
                h.frame(vec![]);
                h.click(h.point(Point2::new(0., -1.)));
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                match cause {
                    "escape" => {
                        h.frame(vec![escape()]);
                    }
                    "selection" => {
                        h.app.selected_ids.remove(&ids[1]);
                        h.frame(vec![]);
                    }
                    "drawing" => {
                        h.app.plans.drawing = Some(h.app.editor.native_drawing(h.view).unwrap());
                        h.frame(vec![]);
                    }
                    "document" => {
                        h.app.editor.document = Document::from_model(before.clone()).unwrap();
                        h.frame(vec![]);
                    }
                    _ => {
                        h.frame(vec![egui::Event::PointerGone]);
                    }
                }
                assert!(h.app.plans.transform.is_none(), "{cause}");
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
                load(&mut h, model.clone(), &ids);
            }
            let camera = h.app.plans.cameras[&h.view];
            h.app.begin_wall_transform(mode);
            h.frame(vec![]);
            let from = h.point(Point2::new(0., -1.));
            let to = from + egui::vec2(25., 18.);
            h.press(from);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            h.release(to);
            assert_eq!(
                h.app.plans.cameras[&h.view], camera,
                "armed transform owns pointer"
            );
            h.frame(vec![escape()]);
            h.frame(vec![]);
            let from = h.app.plans.canvas_rect.unwrap().center() + egui::vec2(80., 100.);
            let to = from + egui::vec2(25., 18.);
            h.press(from);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            h.release(to);
            assert_ne!(h.app.plans.cameras[&h.view], camera, "ordinary drag pans");
            assert_eq!(h.app.editor.document.model(), &model);
        }
    }
}
