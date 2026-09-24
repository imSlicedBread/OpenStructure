use os_core::{Id, Point2};
use os_document::Command;
use os_geometry::walls::NativeWall;
use os_model::*;
use os_ui::Editor;

fn pair(reverse_a: bool, reverse_b: bool) -> (Editor, Id, WallAnchor, WallAnchor) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let view = e.create_floor_plan("Butt joins", level).unwrap();
    let mut make = |name: &str, x: f64, y: f64, reverse: bool, end: WallEndpoint| {
        let (start, finish) = if reverse { (y, x) } else { (x, y) };
        let w = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: name.into(),
                start: Point2::new(start, 0.),
                end: Point2::new(finish, 0.),
                thickness: 0.2,
                height: 3.,
                level,
                material: None,
            },
        );
        let wall = w.id();
        e.command("Wall", Command::AddWall(w)).unwrap();
        WallAnchor {
            wall,
            endpoint: if reverse { end.opposite() } else { end },
        }
    };
    let a = make("A", 0., 3., reverse_a, WallEndpoint::End);
    let b = make("B", 3., 7., reverse_b, WallEndpoint::Start);
    (e, view, a, b)
}

#[test]
fn butt_join_directions_preserve_solids_ids_picking_snaps_and_remove_only_explicit_seams() {
    for reverse_a in [false, true] {
        for reverse_b in [false, true] {
            let (mut e, view, a, b) = pair(reverse_a, reverse_b);
            let original = e.document.model().clone();
            let meshes = e.scene.clone();
            let before = e.native_wall_plan(view).unwrap();
            let old_context = e.native_plan_context(view).unwrap();
            assert!(
                before
                    .items(old_context)
                    .unwrap()
                    .iter()
                    .all(|i| i.hidden_edges.is_empty())
            );
            let join = e.join_wall_ends(a, b).unwrap();
            assert_eq!(e.document.model().walls, original.walls);
            for id in [a.wall, b.wall] {
                assert_eq!(
                    e.scene[&id].triangles.len(),
                    meshes[&id].triangles.len() - 2
                );
            }
            assert!((e.scene.values().map(|m| m.signed_volume()).sum::<f64>() - 4.2).abs() < 1e-9);
            let context = e.native_plan_context(view).unwrap();
            assert!(before.items(context).is_err());
            let plan = e.native_wall_plan(view).unwrap();
            let items = plan.items(context).unwrap();
            assert_eq!(items.len(), 2);
            assert_eq!(items.iter().map(|i| i.hidden_edges.len()).sum::<usize>(), 2);
            assert_eq!(items.iter().map(|i| i.outline().count()).sum::<usize>(), 6);
            assert_eq!(
                plan.pick(context, Point2::new(1., 0.)).unwrap(),
                Some(a.wall)
            );
            assert_eq!(
                plan.pick(context, Point2::new(5., 0.)).unwrap(),
                Some(b.wall)
            );
            let camera = os_render::plan::PlanCamera::default();
            let query = os_render::snapping::SnapQuery {
                camera,
                viewport: [800., 600.],
                pointer: camera.project(Point2::new(3., 0.), [800., 600.]).unwrap(),
                radius_pixels: 10.,
                endpoints: true,
                midpoints: false,
                intersections: false,
                perpendicular_from: None,
                nearest: false,
                axis_extensions: false,
                exclude_entity: Some(a.wall),
            };
            let candidate = plan
                .snap(context, query)
                .unwrap()
                .candidate(context, query)
                .unwrap()
                .unwrap();
            assert_eq!(candidate.entity, b.wall);
            assert_eq!(candidate.point, Point2::new(3., 0.));
            let ga = NativeWall::from_model(e.document.model(), a.wall).unwrap();
            let gb = NativeWall::from_model(e.document.model(), b.wall).unwrap();
            assert!((ga.net_volume().unwrap() + gb.net_volume().unwrap() - 4.2).abs() < 1e-10);
            let bounds = |id| {
                let mesh = &e.scene[&id];
                (
                    mesh.vertices
                        .iter()
                        .map(|v| v.x)
                        .fold(f64::INFINITY, f64::min),
                    mesh.vertices
                        .iter()
                        .map(|v| v.x)
                        .fold(f64::NEG_INFINITY, f64::max),
                )
            };
            assert!((bounds(a.wall).1 - bounds(b.wall).0).abs() < 1e-12);
            assert!(
                os_ifc::WallIfc
                    .export_report(e.document.model())
                    .unwrap_err()
                    .to_string()
                    .contains("butt joins")
            );
            e.undo().unwrap();
            assert_eq!(e.document.model(), &original);
            e.redo().unwrap();
            assert!(e.document.model().wall_joins.contains_key(&join));
            e.unjoin_wall_ends(join).unwrap();
            assert_eq!(e.document.model(), &original);
            assert!(os_ifc::WallIfc.export_report(e.document.model()).is_ok());
        }
    }
}

#[test]
fn butt_join_rejects_incompatible_topology_and_atomic_edits_never_detach() {
    let (mut e, view, a, b) = pair(false, false);
    for case in 0..7 {
        let mut model = e.document.model().clone();
        let mut p = model.walls[&b.wall].parameters.clone();
        match case {
            0 => p.start.x += 1e-8,
            1 => p.end = Point2::new(3., 4.),
            2 => p.end = Point2::new(1., 0.),
            3 => p.thickness += 0.01,
            4 => p.height += 0.01,
            5 => p.end = Point2::new(7., 1.),
            _ => {
                let mut level = model.levels[&p.level].clone();
                level.header.id = Id::new();
                p.level = level.id();
                model.levels.insert(level.id(), level);
            }
        }
        model.walls.get_mut(&b.wall).unwrap().parameters = p;
        assert!(
            ButtJoinParams { a, b }.validate(&model).is_err(),
            "case {case}"
        );
    }
    let join = e.join_wall_ends(a, b).unwrap();
    let original = e.document.model().clone();
    let revision = e.document.revision();
    let mut p = original.walls[&a.wall].parameters.clone();
    p.end.x = 4.;
    for command in [
        Command::UpdateWall {
            id: a.wall,
            parameters: p.clone(),
        },
        Command::RemoveWall(a.wall),
        Command::AddButtJoin(ButtJoin::new(
            "core.wall_butt_join",
            ButtJoinParams { a: b, b: a },
        )),
    ] {
        assert!(e.document.execute("Invalid", vec![command]).is_err());
        assert_eq!(e.document.model(), &original);
        assert_eq!(e.document.revision(), revision);
    }
    let third = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Tee".into(),
            start: Point2::new(3., 0.),
            end: Point2::new(3., 4.),
            ..p.clone()
        },
    );
    assert!(
        e.document
            .execute("Tee", vec![Command::AddWall(third)])
            .is_err()
    );
    let mut q = original.walls[&b.wall].parameters.clone();
    q.start.x = 4.;
    e.document.drain_events();
    e.document
        .execute(
            "Move connected node",
            vec![
                Command::UpdateWall {
                    id: a.wall,
                    parameters: p,
                },
                Command::UpdateWall {
                    id: b.wall,
                    parameters: q,
                },
            ],
        )
        .unwrap();
    let event = e.document.drain_events().pop().unwrap();
    for id in [a.wall, b.wall, join, view] {
        assert!(event.invalidated.contains(&id));
    }
    assert_eq!(e.document.model().wall_joins, original.wall_joins);
    e.undo().unwrap();
    assert_eq!(e.document.model(), &original);
    e.redo().unwrap();
    assert_eq!(e.document.model().walls[&a.wall].parameters.end.x, 4.);
}

#[test]
fn butt_join_openings_quantities_storage_and_section_interfaces() {
    let (mut e, view, a, b) = pair(false, true);
    let level = e.document.model().walls[&a.wall].parameters.level;
    let section = e
        .create_section_view(
            "Longitudinal",
            level,
            SectionViewSettings::new(Point2::new(0., 0.), Point2::new(7., 0.), -1., 4.),
        )
        .unwrap();
    let cross = e
        .create_section_view(
            "At join",
            level,
            SectionViewSettings::new(Point2::new(3., -1.), Point2::new(3., 1.), -1., 4.),
        )
        .unwrap();
    let original_section = e.native_drawing(section).unwrap();
    let context = e.native_section_context(section).unwrap();
    assert!(
        original_section
            .provider_lines(context)
            .unwrap()
            .iter()
            .any(|l| l.start.x == 3. && l.end.x == 3.)
    );
    let join = e.join_wall_ends(a, b).unwrap();
    let ctx = e.native_section_context(section).unwrap();
    let drawing = e.native_drawing(section).unwrap();
    assert!(
        drawing
            .provider_lines(ctx)
            .unwrap()
            .iter()
            .all(|l| (l.start.x - 3.).abs() > 1e-8 || (l.end.x - 3.).abs() > 1e-8)
    );
    let cross_context = e.native_section_context(cross).unwrap();
    assert_eq!(
        e.native_drawing(cross)
            .unwrap()
            .provider_lines(cross_context)
            .unwrap()
            .len(),
        4
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Door".into(),
            host: a.wall,
            offset: 0.5,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 1.,
                height: 2.,
                sill: 0.,
            },
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let opening_id = opening.id();
    e.command("Door", Command::AddOpening(opening)).unwrap();
    let expected = e.document.model().clone();
    assert!(
        (NativeWall::from_model(&expected, a.wall)
            .unwrap()
            .net_volume()
            .unwrap()
            - 1.4)
            .abs()
            < 1e-10
    );
    // Culling leaves each member open at its contact; only the assembled shell
    // has a closed-mesh volume. Per-member quantities come from material cells.
    assert!(
        (e.scene.values().map(|m| m.signed_volume()).sum::<f64>()
            - 3.8
            - e.scene[&opening_id].signed_volume())
        .abs()
            < 1e-10
    );
    let mut invalid = expected.walls[&a.wall].parameters.clone();
    invalid.end.x = 1.;
    let mut other = expected.walls[&b.wall].parameters.clone();
    other.end.x = 1.;
    assert!(
        e.document
            .execute(
                "Collapse host clearance",
                vec![
                    Command::UpdateWall {
                        id: a.wall,
                        parameters: invalid
                    },
                    Command::UpdateWall {
                        id: b.wall,
                        parameters: other
                    }
                ]
            )
            .is_err()
    );
    assert_eq!(e.document.model(), &expected);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("joined.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), &expected);
    assert_eq!(reopened.scene, e.scene);
    assert!(reopened.document.model().openings.contains_key(&opening_id));
    assert!(reopened.document.model().wall_joins.contains_key(&join));
    assert!(reopened.native_wall_plan(view).is_ok());
    assert!(reopened.native_drawing(section).is_ok());
}

#[test]
fn butt_join_plan_crop_and_visibility_preserve_member_ownership() {
    let (mut e, view, a, b) = pair(false, false);
    e.join_wall_ends(a, b).unwrap();
    let level = e.document.model().walls[&a.wall].parameters.level;
    let mut settings = PlanSettings {
        crop: Some(PlanViewCrop {
            min: Point2::new(2., -0.05),
            max: Point2::new(4., 0.05),
        }),
        ..PlanSettings::default()
    };
    e.update_floor_plan(view, "Cropped", level, settings)
        .unwrap();
    let ctx = e.native_plan_context(view).unwrap();
    let plan = e.native_wall_plan(view).unwrap();
    assert_eq!(
        plan.items(ctx)
            .unwrap()
            .iter()
            .map(|i| i.hidden_edges.len())
            .sum::<usize>(),
        2
    );
    assert_eq!(plan.pick(ctx, Point2::new(2.5, 0.)).unwrap(), Some(a.wall));
    settings.visibility.walls = false;
    e.update_floor_plan(view, "Hidden", level, settings)
        .unwrap();
    let ctx = e.native_plan_context(view).unwrap();
    assert!(
        e.native_wall_plan(view)
            .unwrap()
            .items(ctx)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn corner_tee_and_closed_corner_loop_have_gap_free_non_overlapping_cells() {
    fn make_wall(e: &mut Editor, name: &str, start: Point2, end: Point2, level: Id) -> Id {
        let wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: name.into(),
                start,
                end,
                thickness: 0.2,
                height: 3.0,
                level,
                material: None,
            },
        );
        let id = wall.id();
        e.command("Add wall", Command::AddWall(wall)).unwrap();
        id
    }

    let mut corner = Editor::new().unwrap();
    let level = *corner.document.model().levels.keys().next().unwrap();
    let horizontal = make_wall(
        &mut corner,
        "Owner",
        Point2::new(0.0, 0.0),
        Point2::new(3.0, 0.0),
        level,
    );
    let vertical = make_wall(
        &mut corner,
        "Trimmed",
        Point2::new(3.0, 0.0),
        Point2::new(3.0, 4.0),
        level,
    );
    corner
        .join_walls(WallJoinParams::Corner {
            a: WallAnchor {
                wall: horizontal,
                endpoint: WallEndpoint::End,
            },
            b: WallAnchor {
                wall: vertical,
                endpoint: WallEndpoint::Start,
            },
            owner: horizontal,
        })
        .unwrap();
    assert_eq!(
        os_model::wall_station_limits(corner.document.model(), horizontal).unwrap(),
        (0.0, 3.1)
    );
    assert_eq!(
        os_model::wall_station_limits(corner.document.model(), vertical).unwrap(),
        (0.1, 4.0)
    );
    let sum_volume = [horizontal, vertical]
        .into_iter()
        .map(|id| {
            NativeWall::from_model(corner.document.model(), id)
                .unwrap()
                .net_volume()
                .unwrap()
        })
        .sum::<f64>();
    assert!((sum_volume - 4.2).abs() < 1e-9);
    let view = corner.create_floor_plan("Corner", level).unwrap();
    let context = corner.native_plan_context(view).unwrap();
    let drawing = corner.native_wall_plan(view).unwrap();
    assert_eq!(
        drawing
            .items(context)
            .unwrap()
            .iter()
            .map(|i| i.hidden_edges.len())
            .sum::<usize>(),
        2
    );

    let mut tee = Editor::new().unwrap();
    let level = *tee.document.model().levels.keys().next().unwrap();
    let host = make_wall(
        &mut tee,
        "Host",
        Point2::new(0.0, 0.0),
        Point2::new(4.0, 0.0),
        level,
    );
    let branch = make_wall(
        &mut tee,
        "Branch",
        Point2::new(2.0, 0.0),
        Point2::new(2.0, 3.0),
        level,
    );
    tee.join_walls(WallJoinParams::Tee {
        host,
        station: 2.0,
        branch: WallAnchor {
            wall: branch,
            endpoint: WallEndpoint::Start,
        },
    })
    .unwrap();
    assert_eq!(
        os_model::wall_station_limits(tee.document.model(), host).unwrap(),
        (0.0, 4.0)
    );
    assert_eq!(
        os_model::wall_station_limits(tee.document.model(), branch).unwrap(),
        (0.1, 3.0)
    );
    let sum_volume = [host, branch]
        .into_iter()
        .map(|id| {
            NativeWall::from_model(tee.document.model(), id)
                .unwrap()
                .net_volume()
                .unwrap()
        })
        .sum::<f64>();
    assert!((sum_volume - 4.14).abs() < 1e-9);
    let view = tee.create_floor_plan("Tee", level).unwrap();
    let context = tee.native_plan_context(view).unwrap();
    let drawing = tee.native_wall_plan(view).unwrap();
    let items = drawing.items(context).unwrap();
    assert_eq!(items.iter().map(|i| i.hidden_edges.len()).sum::<usize>(), 2);
    assert!(
        items.iter().any(|i| !i.split_edges.is_empty()),
        "host contact intervals must be split, not hide the full edge"
    );

    let mut looped = Editor::new().unwrap();
    let level = *looped.document.model().levels.keys().next().unwrap();
    let points = [
        (Point2::new(0.0, 0.0), Point2::new(4.0, 0.0)),
        (Point2::new(4.0, 0.0), Point2::new(4.0, 3.0)),
        (Point2::new(4.0, 3.0), Point2::new(0.0, 3.0)),
        (Point2::new(0.0, 3.0), Point2::new(0.0, 0.0)),
    ];
    let ids: Vec<_> = points
        .iter()
        .enumerate()
        .map(|(i, (a, b))| make_wall(&mut looped, &format!("Loop {i}"), *a, *b, level))
        .collect();
    for i in 0..4 {
        looped
            .join_walls(WallJoinParams::Corner {
                a: WallAnchor {
                    wall: ids[i],
                    endpoint: WallEndpoint::End,
                },
                b: WallAnchor {
                    wall: ids[(i + 1) % 4],
                    endpoint: WallEndpoint::Start,
                },
                owner: ids[i],
            })
            .unwrap_or_else(|error| panic!("closed-loop corner {i} rejected: {error}"));
    }
    looped.document.model().validate().unwrap();
    let total = ids
        .iter()
        .map(|id| {
            NativeWall::from_model(looped.document.model(), *id)
                .unwrap()
                .net_volume()
                .unwrap()
        })
        .sum::<f64>();
    assert!((total - 8.4).abs() < 1e-9);
    let view = looped.create_floor_plan("Closed loop", level).unwrap();
    let context = looped.native_plan_context(view).unwrap();
    let drawing = looped.native_wall_plan(view).unwrap();
    assert_eq!(
        drawing
            .items(context)
            .unwrap()
            .iter()
            .map(|i| i.hidden_edges.len())
            .sum::<usize>(),
        8
    );
}
