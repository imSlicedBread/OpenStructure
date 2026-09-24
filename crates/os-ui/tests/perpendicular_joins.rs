use os_core::{Id, Point2};
use os_document::Command;
use os_geometry::{Mesh, walls::NativeWall};
use os_model::*;
use os_ui::Editor;

fn add_wall(e: &mut Editor, n: u8, start: Point2, end: Point2, reverse: bool) -> WallAnchor {
    let mut wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: format!("Wall {n}"),
            start: if reverse { end } else { start },
            end: if reverse { start } else { end },
            thickness: 0.2,
            height: 3.,
            level: *e.document.model().levels.keys().next().unwrap(),
            material: None,
        },
    );
    wall.header.id = serde_json::from_value(serde_json::json!(format!(
        "00000000-0000-4000-8000-{n:012}"
    )))
    .unwrap();
    let id = wall.id();
    e.command("Wall", Command::AddWall(wall)).unwrap();
    WallAnchor {
        wall: id,
        endpoint: if reverse {
            WallEndpoint::Start
        } else {
            WallEndpoint::End
        },
    }
}

fn fixture(
    tee: bool,
    reverse_a: bool,
    reverse_b: bool,
    owner_b: bool,
    swap_ids: bool,
) -> (Editor, Id, WallAnchor, WallAnchor, WallJoinParams) {
    let mut e = Editor::new().unwrap();
    let a = add_wall(
        &mut e,
        if swap_ids { 2 } else { 1 },
        Point2::new(0., 0.),
        Point2::new(4., 0.),
        reverse_a,
    );
    let x = if tee { 2. } else { 4. };
    let mut b = add_wall(
        &mut e,
        if swap_ids { 1 } else { 2 },
        Point2::new(x, 0.),
        Point2::new(x, 3.),
        reverse_b,
    );
    b.endpoint = b.endpoint.opposite();
    let level = e.document.model().walls[&a.wall].parameters.level;
    let view = e.create_floor_plan("Joined plan", level).unwrap();
    let join = if tee {
        WallJoinParams::Tee {
            host: a.wall,
            station: 2.,
            branch: b,
        }
    } else {
        WallJoinParams::Corner {
            a,
            b,
            owner: if owner_b { b.wall } else { a.wall },
        }
    };
    (e, view, a, b, join)
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}
fn mesh_area(mesh: &Mesh) -> f64 {
    mesh.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.vertices[i as usize]);
            let (u, v) = (
                [b.x - a.x, b.y - a.y, b.z - a.z],
                [c.x - a.x, c.y - a.y, c.z - a.z],
            );
            (u[1] * v[2] - u[2] * v[1])
                .hypot(u[2] * v[0] - u[0] * v[2])
                .hypot(u[0] * v[1] - u[1] * v[0])
                / 2.
        })
        .sum()
}
fn assert_disjoint(a: &NativeWall, b: &NativeWall) {
    let bounds = |w: &NativeWall| {
        let mesh = Mesh::from_prisms(&w.cells().unwrap()).unwrap();
        let xs: Vec<_> = mesh.vertices.iter().map(|v| v.x).collect();
        let ys: Vec<_> = mesh.vertices.iter().map(|v| v.y).collect();
        (
            xs.iter().copied().fold(f64::INFINITY, f64::min),
            xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            ys.iter().copied().fold(f64::INFINITY, f64::min),
            ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (a, b) = (bounds(a), bounds(b));
    close(
        (a.1.min(b.1) - a.0.max(b.0)).max(0.) * (a.3.min(b.3) - a.2.max(b.2)).max(0.),
        0.,
    );
}

#[test]
fn corner_tee_directions_owners_ids_share_cells_meshes_plan_and_quantities() {
    for tee in [false, true] {
        for ra in [false, true] {
            for rb in [false, true] {
                for owner_b in [false, true] {
                    for swap in [false, true] {
                        let (mut e, view, a, b, join) = fixture(tee, ra, rb, owner_b, swap);
                        let original = e.document.model().clone();
                        assert!(original.wall_joins.is_empty());
                        let jid = e.join_walls(join.clone()).unwrap();
                        assert_eq!(e.document.model().walls, original.walls);
                        let ga = NativeWall::from_model(e.document.model(), a.wall).unwrap();
                        let gb = NativeWall::from_model(e.document.model(), b.wall).unwrap();
                        assert_disjoint(&ga, &gb);
                        let area = if tee { 1.38 } else { 1.4 };
                        let perimeter = if tee { 14.2 } else { 14.4 };
                        close(
                            ga.net_volume().unwrap() + gb.net_volume().unwrap(),
                            area * 3.,
                        );
                        close(e.scene.values().map(|m| m.signed_volume()).sum(), area * 3.);
                        close(
                            e.scene.values().map(mesh_area).sum(),
                            perimeter * 3. + 2. * area,
                        );
                        assert_eq!(ga.interfaces, gb.interfaces);
                        let ctx = e.native_plan_context(view).unwrap();
                        let plan = e.native_wall_plan(view).unwrap();
                        close(
                            plan.items(ctx)
                                .unwrap()
                                .iter()
                                .map(|i| i.footprint.area())
                                .sum(),
                            area,
                        );
                        close(
                            plan.items(ctx)
                                .unwrap()
                                .iter()
                                .flat_map(|i| i.outline())
                                .map(|(a, b)| a.distance(b))
                                .sum(),
                            perimeter,
                        );
                        assert_eq!(plan.pick(ctx, Point2::new(1., 0.)).unwrap(), Some(a.wall));
                        assert_eq!(
                            plan.pick(ctx, Point2::new(if tee { 2. } else { 4. }, 2.))
                                .unwrap(),
                            Some(b.wall)
                        );
                        assert_eq!(
                            e.scene.keys().copied().collect::<Vec<_>>(),
                            e.document.model().walls.keys().copied().collect::<Vec<_>>()
                        );
                        let camera = os_render::Camera {
                            pitch: std::f64::consts::FRAC_PI_2,
                            yaw: 0.,
                            ..Default::default()
                        };
                        let triangles = os_render::project_scene(&e.scene, &camera, 800., 600.);
                        for (id, x, y) in
                            [(a.wall, 1., 0.), (b.wall, if tee { 2. } else { 4. }, 2.)]
                        {
                            let p = camera.project(os_geometry::Vec3::new(x, y, 3.), 800., 600.);
                            assert_eq!(os_render::pick(&triangles, p.x, p.y), Some(id));
                        }
                        assert!(os_ifc::WallIfc.export_report(e.document.model()).is_err());
                        e.undo().unwrap();
                        assert_eq!(e.document.model(), &original);
                        e.redo().unwrap();
                        assert_eq!(e.document.model().wall_joins[&jid].parameters, join);
                    }
                }
            }
        }
    }
}

#[test]
fn tee_partial_host_strokes_crop_sections_and_coincident_cut() {
    let (mut e, view, a, b, join) = fixture(true, false, false, false, false);
    e.join_walls(join).unwrap();
    let level = e.document.model().walls[&a.wall].parameters.level;
    let ctx = e.native_plan_context(view).unwrap();
    let plan = e.native_wall_plan(view).unwrap();
    let host = plan
        .items(ctx)
        .unwrap()
        .iter()
        .find(|i| i.entity == a.wall)
        .unwrap();
    assert_eq!(host.split_edges.len(), 2);
    close(
        host.split_edges.iter().map(|(a, b)| a.distance(*b)).sum(),
        3.8,
    );
    let settings = PlanSettings {
        crop: Some(PlanViewCrop {
            min: Point2::new(1.95, 0.),
            max: Point2::new(2.5, 0.5),
        }),
        ..Default::default()
    };
    e.update_floor_plan(view, "Crop", level, settings).unwrap();
    let ctx = e.native_plan_context(view).unwrap();
    let plan = e.native_wall_plan(view).unwrap();
    let edges: Vec<_> = plan
        .items(ctx)
        .unwrap()
        .iter()
        .flat_map(|i| i.outline())
        .collect();
    let upper_host: Vec<_> = edges
        .iter()
        .filter(|(a, b)| (a.y - 0.1).abs() < 1e-9 && (b.y - 0.1).abs() < 1e-9)
        .collect();
    close(upper_host.iter().map(|(a, b)| a.distance(*b)).sum(), 0.4);
    assert_eq!(plan.pick(ctx, Point2::new(2., 0.25)).unwrap(), Some(b.wall));
    for (name, start, end, width) in [
        (
            "Through tee",
            Point2::new(2., -2.),
            Point2::new(2., 5.),
            3.1,
        ),
        (
            "Coincident",
            Point2::new(-2., 0.1),
            Point2::new(6., 0.1),
            4.,
        ),
    ] {
        let section = e
            .create_section_view(name, level, SectionViewSettings::new(start, end, -1., 4.))
            .unwrap();
        let ctx = e.native_section_context(section).unwrap();
        let drawing = e.native_drawing(section).unwrap();
        let lines = drawing.provider_lines(ctx).unwrap();
        close(
            lines.iter().map(|l| l.start.distance(l.end)).sum(),
            2. * (width + 3.),
        );
        if name == "Coincident" {
            assert!(lines.iter().all(|l| l.entity == a.wall));
        }
    }
}

#[test]
fn four_corner_cycle_atomic_edit_room_identity_and_reopen() {
    let mut e = Editor::new().unwrap();
    let points = [
        Point2::new(0., 0.),
        Point2::new(4., 0.),
        Point2::new(4., 3.),
        Point2::new(0., 3.),
    ];
    let walls: Vec<_> = (0..4)
        .map(|i| add_wall(&mut e, i as u8 + 1, points[i], points[(i + 1) % 4], false))
        .collect();
    let level = e.document.model().walls[&walls[0].wall].parameters.level;
    let view = e.create_floor_plan("Loop", level).unwrap();
    let before = e.document.model().clone();
    let commands: Vec<_> = (0..4)
        .map(|i| {
            let mut b = walls[(i + 1) % 4];
            b.endpoint = WallEndpoint::Start;
            Command::AddWallJoin(WallJoin::new(
                "core.wall_join",
                WallJoinParams::Corner {
                    a: walls[i],
                    b,
                    owner: walls[i].wall,
                },
            ))
        })
        .collect();
    e.document.execute("Four corners", commands).unwrap();
    e.regenerate().unwrap();
    let joined = e.document.model().clone();
    assert_eq!(joined.walls, before.walls);
    assert_eq!(
        joined.room_boundary_segments(level).unwrap(),
        before.room_boundary_segments(level).unwrap()
    );
    close(e.scene.values().map(|m| m.signed_volume()).sum(), 8.4);
    let ctx = e.native_plan_context(view).unwrap();
    close(
        e.native_wall_plan(view)
            .unwrap()
            .items(ctx)
            .unwrap()
            .iter()
            .map(|i| i.footprint.area())
            .sum(),
        2.8,
    );
    let mut moved = joined.walls[&walls[0].wall].parameters.clone();
    moved.end.x += 1.;
    let revision = e.document.revision();
    assert!(
        e.document
            .execute(
                "Detached edit",
                vec![Command::UpdateWall {
                    id: walls[0].wall,
                    parameters: moved
                }]
            )
            .is_err()
    );
    assert_eq!(e.document.revision(), revision);
    assert_eq!(e.document.model(), &joined);
    let commands = joined
        .walls
        .values()
        .map(|w| {
            let mut p = w.parameters.clone();
            p.start.x += 1.;
            p.end.x += 1.;
            Command::UpdateWall {
                id: w.id(),
                parameters: p,
            }
        })
        .collect();
    e.document.execute("Move group", commands).unwrap();
    e.regenerate().unwrap();
    e.undo().unwrap();
    assert_eq!(e.document.model(), &joined);
    e.redo().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("corners.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), e.document.model());
    assert_eq!(reopened.scene, e.scene);
    #[cfg(feature = "external-plugins")]
    {
        let mut staged = Editor::new().unwrap();
        staged
            .start_plugin_open(&path, std::time::Duration::from_secs(5))
            .unwrap();
        assert!(staged.poll_plugin_open().unwrap().is_some());
        assert_eq!(staged.scene, e.scene);
        assert!(!staged.plugin_work_pending());
    }
}

#[test]
fn invalid_profiles_nodes_ownership_competitors_openings_and_tiny_members_are_atomic() {
    for tee in [false, true] {
        for case in 0..10 {
            let (mut e, _, a, b, mut join) = fixture(tee, false, false, false, false);
            let mut model = e.document.model().clone();
            match case {
                0 => model.walls.get_mut(&b.wall).unwrap().parameters.height += 0.1,
                1 => model.walls.get_mut(&b.wall).unwrap().parameters.thickness += 0.1,
                2 => model.walls.get_mut(&b.wall).unwrap().parameters.start.x += 1e-8,
                3 => model.walls.get_mut(&b.wall).unwrap().parameters.end.x += 0.1,
                4 => model.walls.get_mut(&b.wall).unwrap().parameters.end.y = 0.1005,
                5 => {
                    if let WallJoinParams::Tee { station, .. } = &mut join {
                        *station = f64::INFINITY;
                    } else if let WallJoinParams::Corner { owner, .. } = &mut join {
                        *owner = Id::new();
                    }
                }
                6 => {
                    let mut third = model.walls[&b.wall].clone();
                    third.header.id = Id::new();
                    third.parameters.end.y = -3.;
                    model.walls.insert(third.id(), third);
                }
                7 => {
                    let duplicate = WallJoin::new("core.wall_join", join.clone());
                    model.wall_joins.insert(duplicate.id(), duplicate);
                }
                8 | 9 => {
                    let opening = Opening::new(
                        "core.opening",
                        OpeningParams {
                            name: "Conflict".into(),
                            host: if case == 8 { a.wall } else { b.wall },
                            offset: if case == 8 {
                                if tee { 1.5 } else { 3.5 }
                            } else {
                                0.05
                            },
                            definition: OpeningDefinition::Legacy {
                                kind: OpeningKind::Door,
                                width: if case == 8 {
                                    if tee { 1. } else { 0.45 }
                                } else {
                                    1.
                                },
                                height: 2.,
                                sill: 0.,
                            },
                            hinge: Default::default(),
                            swing: Default::default(),
                        },
                    );
                    model.openings.insert(opening.id(), opening);
                }
                _ => unreachable!(),
            }
            let j = WallJoin::new("core.wall_join", join);
            model.wall_joins.insert(j.id(), j);
            assert!(model.validate().is_err(), "tee {tee} case {case}");
            // Validate the same malformed graph through the atomic document gate.
            let before = e.document.model().clone();
            let mut commands: Vec<_> = model
                .walls
                .values()
                .map(|w| {
                    if before.walls.contains_key(&w.id()) {
                        Command::UpdateWall {
                            id: w.id(),
                            parameters: w.parameters.clone(),
                        }
                    } else {
                        Command::AddWall(w.clone())
                    }
                })
                .collect();
            commands.extend(model.openings.values().cloned().map(Command::AddOpening));
            commands.extend(model.wall_joins.values().cloned().map(Command::AddWallJoin));
            assert!(e.document.execute("Invalid join graph", commands).is_err());
            assert_eq!(e.document.model(), &before);
        }
    }
}
