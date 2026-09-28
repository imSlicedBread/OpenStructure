use os_core::Point2;
use os_document::Command;
use os_model::*;
use os_ui::Editor;
use std::f64::consts::PI;

fn fixture() -> (Editor, os_core::Id, os_core::Id, os_core::Id) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let view = e.create_floor_plan("Arcs", level).unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Arc".into(),
            path: WallPath::CircularArc {
                center: Point2::default(),
                radius: 4.0,
                start_angle_rad: 0.0,
                signed_sweep_rad: PI,
            },
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let id = wall.id();
    e.command("Arc", Command::AddWall(wall)).unwrap();
    (e, level, view, id)
}

#[test]
fn arc_plan_pick_analytic_snaps_crop_range_visibility_and_section_sheet() {
    let (mut e, level, view, id) = fixture();
    let context = e.native_plan_context(view).unwrap();
    let drawing = e.native_wall_plan(view).unwrap();
    let camera = os_render::plan::PlanCamera {
        center: Point2::new(0.0, 2.0),
        pixels_per_metre: 50.0,
    };
    let viewport = [800.0, 600.0];
    let top = Point2::new(0.0, 4.0);
    assert_eq!(
        drawing
            .pick_screen(
                context,
                camera,
                viewport,
                camera.project(top, viewport).unwrap(),
                0.1
            )
            .unwrap(),
        Some(id)
    );
    assert_ne!(
        drawing
            .pick_screen(
                context,
                camera,
                viewport,
                camera.project(Point2::default(), viewport).unwrap(),
                0.1
            )
            .unwrap(),
        Some(id)
    );
    for (p, kind) in [
        (
            Point2::new(4.0, 0.0),
            os_render::snapping::SnapKind::Endpoint,
        ),
        (top, os_render::snapping::SnapKind::Midpoint),
        (
            Point2::new(2.0, 12.0f64.sqrt()),
            os_render::snapping::SnapKind::Nearest,
        ),
    ] {
        let q = os_render::snapping::SnapQuery {
            camera,
            viewport,
            pointer: camera.project(p, viewport).unwrap(),
            radius_pixels: 12.0,
            endpoints: kind == os_render::snapping::SnapKind::Endpoint,
            midpoints: kind == os_render::snapping::SnapKind::Midpoint,
            intersections: false,
            perpendicular_from: None,
            nearest: true,
            axis_extensions: false,
            exclude_entity: None,
        };
        let snap = drawing
            .snap(context, q)
            .unwrap()
            .candidate(context, q)
            .unwrap()
            .unwrap();
        assert_eq!(snap.kind, kind);
        assert!(snap.point.distance(p) < 1e-10);
    }
    let section = e
        .create_section_view(
            "Arc cut",
            level,
            SectionViewSettings::new(Point2::new(1.0, -1.0), Point2::new(1.0, 5.0), -0.5, 4.0),
        )
        .unwrap();
    let section_context = e.native_section_context(section).unwrap();
    let cut = e.native_drawing(section).unwrap();
    assert!(
        cut.provider_lines(section_context)
            .unwrap()
            .iter()
            .any(|l| l.entity == id)
    );
    // Regenerate plan context after the section-view transaction.
    let context = e.native_plan_context(view).unwrap();
    let drawing = e.native_wall_plan(view).unwrap();
    let page = os_render::sheet::compose_view_sheet(
        os_render::sheet::PaperSheetInfo {
            width_mm: 420.0,
            height_mm: 297.0,
            number: "A1",
            name: "Arc",
        },
        "Plan",
        os_render::sheet::PaperViewport {
            center_mm: Point2::new(210.0, 130.0),
            width_mm: 360.0,
            height_mm: 220.0,
            model_center_m: Point2::new(0.0, 2.0),
            scale_denominator: 100.0,
        },
        context,
        &drawing,
    )
    .unwrap();
    assert!(page.marks().len() > 20);
    assert!(page.to_pdf().unwrap().starts_with(b"%PDF"));
    let initial = e.document.model().views[&view].parameters.plan.unwrap();
    for mode in 0..3 {
        let mut settings = initial;
        match mode {
            0 => settings.visibility.walls = false,
            1 => {
                settings.crop = Some(PlanViewCrop {
                    min: Point2::new(-1.0, -1.0),
                    max: Point2::new(1.0, 1.0),
                })
            }
            _ => {
                settings.range.top = 8.0;
                settings.range.cut = 7.0;
                settings.range.bottom = 6.0;
                settings.range.depth = 5.0;
            }
        }
        e.update_floor_plan(view, "Arcs", level, settings).unwrap();
        let context = e.native_plan_context(view).unwrap();
        assert!(
            !e.native_wall_plan(view)
                .unwrap()
                .items(context)
                .unwrap()
                .iter()
                .any(|i| i.entity == id)
        );
    }
}

#[test]
fn arc_hosting_joins_and_invalid_properties_fail_without_mutation() {
    let (mut e, level, _view, id) = fixture();
    let second = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Straight".into(),
            path: WallPath::Straight {
                start: Point2::new(4.0, 0.0),
                end: Point2::new(6.0, 0.0),
            },
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let second_id = second.id();
    e.command("Straight", Command::AddWall(second)).unwrap();
    let before = e.document.model().clone();
    let history = e.document.history_stats();
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Door".into(),
            host: id,
            offset: 1.0,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            },
            hinge: Default::default(),
            swing: Default::default(),
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
        },
    );
    assert!(
        e.command("Invalid host", Command::AddOpening(opening))
            .unwrap_err()
            .to_string()
            .contains("straight")
    );
    let join = WallJoin::new(
        "core.wall_join",
        WallJoinParams::Butt {
            a: WallAnchor {
                wall: id,
                endpoint: WallEndpoint::Start,
            },
            b: WallAnchor {
                wall: second_id,
                endpoint: WallEndpoint::Start,
            },
        },
    );
    assert!(
        e.command("Invalid join", Command::AddWallJoin(join))
            .unwrap_err()
            .to_string()
            .contains("straight")
    );
    let mut parameters = before.walls[&id].parameters.clone();
    if let WallPath::CircularArc { radius, .. } = &mut parameters.path {
        *radius = 0.05;
    }
    assert!(
        e.command("Invalid radius", Command::UpdateWall { id, parameters })
            .is_err()
    );
    assert_eq!(e.document.model(), &before);
    assert_eq!(e.document.history_stats(), history);
}
