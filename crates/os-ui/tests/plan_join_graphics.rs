use os_core::{Id, Point2};
use os_document::Command;
use os_geometry::{plan::PlanRole, walls::NativeWall};
use os_model::*;
use os_render::plan::{PlanContext, PlanDrawing};
use os_render::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};
use os_ui::Editor;

fn fixture(kind: u8, reverse: [bool; 2], swap: bool, side: f64) -> (Editor, Id, WallJoinParams) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let view = e.create_floor_plan("Join graphics", level).unwrap();
    let x = if kind == 3 { 2. } else { 4. };
    let mut anchors = Vec::new();
    for (i, (start, end)) in [
        (Point2::new(0., 0.), Point2::new(4., 0.)),
        (
            Point2::new(x, 0.),
            if kind == 0 {
                Point2::new(7., 0.)
            } else {
                Point2::new(x, side * 3.)
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: format!("Wall {i}"),
                level,
                thickness: 0.2,
                height: 3.,
                material: None,
                path: WallPath::Straight {
                    start: if reverse[i] { end } else { start },
                    end: if reverse[i] { start } else { end },
                },
            },
        );
        let n = if swap { 2 - i } else { i + 1 };
        wall.header.id =
            serde_json::from_str(&format!("\"00000000-0000-4000-8000-{n:012}\"")).unwrap();
        let id = wall.id();
        e.command("Add wall", Command::AddWall(wall)).unwrap();
        let endpoint = if i == 0 {
            WallEndpoint::End
        } else {
            WallEndpoint::Start
        };
        anchors.push(WallAnchor {
            wall: id,
            endpoint: if reverse[i] {
                endpoint.opposite()
            } else {
                endpoint
            },
        });
    }
    let (a, b) = (anchors[0], anchors[1]);
    let join = match kind {
        0 => WallJoinParams::Butt { a, b },
        1 | 2 => WallJoinParams::Corner {
            a,
            b,
            owner: if kind == 1 { a.wall } else { b.wall },
        },
        _ => WallJoinParams::Tee {
            host: a.wall,
            station: 2.,
            branch: b,
        },
    };
    e.join_walls(join.clone()).unwrap();
    (e, view, join)
}
fn owner(join: &WallJoinParams) -> Id {
    match *join {
        WallJoinParams::Butt { a, b } => a.wall.min(b.wall),
        WallJoinParams::Corner { a, b, owner } => {
            if owner == a.wall {
                b.wall
            } else {
                a.wall
            }
        }
        WallJoinParams::Tee { branch, .. } => branch.wall,
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
fn seam_length(d: &PlanDrawing, ctx: PlanContext, id: Id, trace: (Point2, Point2)) -> f64 {
    d.items(ctx)
        .unwrap()
        .iter()
        .filter(|i| i.entity == id)
        .flat_map(|i| i.outline())
        .map(|edge| {
            edge.0.distance(edge.1)
                - os_geometry::walls::subtract_segment(edge, trace)
                    .iter()
                    .map(|(a, b)| a.distance(*b))
                    .sum::<f64>()
        })
        .sum()
}
fn assert_seam(e: &Editor, view: Id, join: &WallJoinParams, discontinuity: bool) {
    let before = e.document.model().clone();
    let ctx = e.native_plan_context(view).unwrap();
    let d = e.native_wall_plan(view).unwrap();
    let (a, b) = wall_join_trace(&before, join).unwrap();
    let trace = (
        ctx.basis.world_to_plane(a).unwrap(),
        ctx.basis.world_to_plane(b).unwrap(),
    );
    for id in join.members() {
        close(
            seam_length(&d, ctx, id, trace),
            if discontinuity && id == owner(join) {
                0.2
            } else {
                0.
            },
        );
        let native = NativeWall::from_model(&before, id).unwrap();
        close(
            d.items(ctx)
                .unwrap()
                .iter()
                .filter(|i| i.entity == id)
                .map(|i| i.footprint.area())
                .sum(),
            native.net_volume().unwrap() / 3.,
        );
    }
    assert_eq!(before.walls, e.document.model().walls);
    assert_eq!(before.wall_joins, e.document.model().wall_joins);
    assert_eq!(before.openings, e.document.model().openings);
    assert_sheet(&d, ctx);
}
fn assert_sheet(d: &PlanDrawing, ctx: PlanContext) {
    let viewport = PaperViewport {
        center_mm: Point2::new(210., 125.),
        width_mm: 390.,
        height_mm: 230.,
        model_center_m: Point2::new(2., 0.),
        scale_denominator: 100.,
    };
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.,
            height_mm: 297.,
            number: "A101",
            name: "Join graphics",
        },
        "Plan",
        viewport,
        ctx,
        d,
    )
    .unwrap();
    let key = |a: Point2, b: Point2, color: [u8; 3], weight: f64, dashed: bool| {
        (
            [a.x, a.y, b.x, b.y].map(|v| (v * 1e7).round() as i64),
            color,
            weight.to_bits(),
            dashed,
        )
    };
    let mut expected = Vec::new();
    let mut colors = Vec::new();
    for item in d.items(ctx).unwrap() {
        let style = d
            .appearance(ctx, item.entity, item.footprint.role)
            .unwrap()
            .unwrap();
        colors.push(style.color);
        for (a, b) in item.outline() {
            expected.push(key(
                viewport.model_to_paper(a).unwrap(),
                viewport.model_to_paper(b).unwrap(),
                style.color,
                style.weight_mm,
                style.dashed,
            ));
        }
    }
    let mut actual = Vec::new();
    for mark in page.marks() {
        if let PaperMarkKind::Path {
            points_mm,
            closed: false,
            stroke: Some(s),
            ..
        } = &mark.kind
        {
            let color = [s.color.red, s.color.green, s.color.blue];
            if colors.contains(&color) {
                assert_eq!(points_mm.len(), 2);
                actual.push(key(points_mm[0], points_mm[1], color, s.width_mm, s.dashed));
            }
        }
    }
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected);
    assert!(page.to_pdf().unwrap().starts_with(b"%PDF-"));
}
fn overrides(e: &mut Editor, view: Id, members: [Id; 2], different: bool) {
    let values = members
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            let style = PlanStrokeStyle {
                color: if different && i == 1 {
                    [195, 40, 80]
                } else {
                    [20, 80, 140]
                },
                weight_mm: 0.6,
                pattern: PlanLinePattern::Dashed,
            };
            (
                id,
                PlanElementGraphicsOverride {
                    cut: Some(style),
                    projected: Some(style),
                },
            )
        })
        .collect();
    e.command(
        "Appearance",
        Command::SetPlanElementGraphicsOverrides {
            view,
            overrides: values,
        },
    )
    .unwrap();
}

#[test]
fn butt_corner_tee_owners_directions_and_sides_flow_to_sheet_after_appearance_resolution() {
    for kind in 0..4 {
        for a in [false, true] {
            for b in [false, true] {
                for swap in [false, true] {
                    for side in [-1., 1.] {
                        let (mut e, view, join) = fixture(kind, [a, b], swap, side);
                        assert_seam(&e, view, &join, false);
                        let old = e.native_wall_plan(view).unwrap();
                        overrides(&mut e, view, join.members(), true);
                        assert!(old.items(e.native_plan_context(view).unwrap()).is_err());
                        assert_seam(&e, view, &join, true);
                        overrides(&mut e, view, join.members(), false);
                        assert_seam(&e, view, &join, false);
                    }
                }
            }
        }
    }
}

#[test]
fn resolved_material_colors_control_join_cleanup_without_changing_join_geometry() {
    for kind in 0..4 {
        let (mut e, view, join) = fixture(kind, [false, true], true, 1.);
        let members = join.members();
        let mut materials = Vec::new();
        for id in members {
            let material = Material::new(
                "core.material",
                MaterialParams {
                    name: format!("Material {id}"),
                    density_kg_m3: 1000.,
                    color: [210, 180, 140],
                    cut_pattern: None,
                },
            );
            let mid = material.id();
            e.command("Material", Command::AddMaterial(material))
                .unwrap();
            let mut parameters = e.document.model().walls[&id].parameters.clone();
            parameters.material = Some(mid);
            e.command("Wall material", Command::UpdateWall { id, parameters })
                .unwrap();
            materials.push(mid);
        }
        assert_seam(&e, view, &join, false);
        let id = materials[1];
        let mut parameters = e.document.model().materials[&id].parameters.clone();
        parameters.color = [120, 140, 210];
        e.command("Material color", Command::UpdateMaterial { id, parameters })
            .unwrap();
        assert_seam(&e, view, &join, true);
    }
}

#[test]
fn rotated_plan_preserves_roles_pick_overlap_and_authored_endpoint_snap_ownership() {
    let (mut e, view, join) = fixture(0, [true, false], true, 1.);
    let members = join.members();
    let level = e.document.model().walls[&members[0]].parameters.level;
    let settings = PlanSettings {
        basis: PlanViewBasis {
            origin: Point2::new(1., -2.),
            rotation: 0.37,
        },
        ..Default::default()
    };
    e.update_floor_plan(view, "Rotated", level, settings)
        .unwrap();
    assert_seam(&e, view, &join, false);
    let ctx = e.native_plan_context(view).unwrap();
    let d = e.native_wall_plan(view).unwrap();
    assert!(
        d.items(ctx)
            .unwrap()
            .iter()
            .all(|i| i.footprint.role == PlanRole::Cut)
    );
    let point = ctx.basis.world_to_plane(Point2::new(4., 0.)).unwrap();
    let camera = os_render::plan::PlanCamera::default();
    for viewport in [[1280., 800.], [1000. / 1.5, 650. / 1.5]] {
        let pointer = camera.project(point, viewport).unwrap();
        let hits = d.hits_screen(ctx, camera, viewport, pointer, 1.).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(members.iter().all(|id| hits.contains(id)));
        let query = os_render::snapping::SnapQuery {
            camera,
            viewport,
            pointer,
            radius_pixels: 10.,
            endpoints: true,
            midpoints: false,
            intersections: false,
            perpendicular_from: None,
            nearest: false,
            axis_extensions: false,
            exclude_entity: Some(members[0]),
        };
        let result = d.snap(ctx, query).unwrap();
        let candidate = result.candidate(ctx, query).unwrap().unwrap();
        assert_eq!(candidate.entity, members[1]);
        close(candidate.point.distance(point), 0.);
    }
}

#[test]
fn phase_exclusion_removes_view_only_join_and_crop_at_contact_retains_visible_edge() {
    let (mut e, view, join) = fixture(0, [false, false], false, 1.);
    let members = join.members();
    let level = e.document.model().walls[&members[0]].parameters.level;
    let settings = PlanSettings {
        crop: Some(PlanViewCrop {
            min: Point2::new(0., -0.05),
            max: Point2::new(4., 0.05),
        }),
        ..Default::default()
    };
    e.update_floor_plan(view, "Crop contact", level, settings)
        .unwrap();
    let ctx = e.native_plan_context(view).unwrap();
    let d = e.native_wall_plan(view).unwrap();
    let trace = wall_join_trace(e.document.model(), &join).unwrap();
    close(seam_length(&d, ctx, members[0], trace), 0.1);
    assert_sheet(&d, ctx);
    let phase = new_phase("Future", e.document.model().phases.len() as u32);
    let future = phase.id();
    let target = e.document.model().latest_phase().unwrap();
    e.command("Future phase", Command::AddPhase(phase)).unwrap();
    e.command(
        "Future wall",
        Command::SetElementLifecycle {
            element: members[1],
            lifecycle: ElementLifecycle {
                created_in: future,
                demolished_in: None,
            },
        },
    )
    .unwrap();
    e.update_floor_plan(
        view,
        "Phase",
        level,
        PlanSettings {
            target_phase: Some(target),
            ..Default::default()
        },
    )
    .unwrap();
    let ctx = e.native_plan_context(view).unwrap();
    let d = e.native_wall_plan(view).unwrap();
    assert_eq!(d.items(ctx).unwrap().len(), 1);
    close(seam_length(&d, ctx, members[0], trace), 0.2);
    assert_eq!(e.document.model().wall_joins.len(), 1);
    assert_sheet(&d, ctx);
}
