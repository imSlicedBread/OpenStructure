use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_geometry::plan::PlanRole;
use os_model::*;
use os_ui::Editor;

fn fixture() -> (Editor, Id) {
    let mut e = Editor::new().unwrap();
    e.document = Document::new("Section projection").unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    (e, level)
}
fn wall(e: &mut Editor, level: Id, a: Point2, b: Point2, height: f64) -> Id {
    let w = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Wall".into(),
            level,
            path: WallPath::Straight { start: a, end: b },
            height,
            thickness: 0.2,
            material: None,
        },
    );
    let id = w.id();
    e.command("Wall", Command::AddWall(w)).unwrap();
    id
}
fn settings() -> SectionViewSettings {
    let mut s = SectionViewSettings::new(Point2::new(0., 0.), Point2::new(8., 0.), -1., 6.);
    s.mode = SectionDisplayMode::CutAndProjection;
    s.far_depth = 4.;
    s
}

#[test]
fn section_projection_both_sides_foreground_depth_crop_and_rotated_reversed_axes() {
    for (dx, dy) in [(1., 0.), (0.6, 0.8), (-0.6, -0.8)] {
        let (mut e, level) = fixture();
        let p = |x: f64, y: f64| Point2::new(17. + dx * x - dy * y, -11. + dy * x + dx * y);
        let back = wall(&mut e, level, p(1., -2.), p(7., -2.), 3.);
        let front = wall(&mut e, level, p(0., 2.), p(8., 2.), 5.);
        let mut s = settings();
        s.start = p(0., 0.);
        s.end = p(8., 0.);
        for side in [ElevationSide::Left, ElevationSide::Right] {
            s.side = side;
            let view = e.create_section_view("Analytic", level, s).unwrap();
            let c = e.native_section_context(view).unwrap();
            let d = e.native_drawing(view).unwrap();
            let lines = d.provider_lines(c).unwrap();
            let owner = if side == ElevationSide::Left {
                back
            } else {
                front
            };
            assert_eq!(lines.len(), 4, "{lines:?}");
            assert!(
                lines
                    .iter()
                    .all(|l| l.entity == owner && l.role == PlanRole::Projected)
            );
            let expected = if side == ElevationSide::Left {
                [1., 7., 3.]
            } else {
                [0., 8., 5.]
            };
            for l in lines {
                for point in [l.start, l.end] {
                    assert!(
                        (point.x - expected[0]).abs() < 1e-7
                            || (point.x - expected[1]).abs() < 1e-7
                    );
                    assert!(point.y.abs() < 1e-7 || (point.y - expected[2]).abs() < 1e-7);
                }
            }
        }
        s.side = ElevationSide::Left;
        s.far_depth = 1.8;
        let view = e.create_section_view("Far excluded", level, s).unwrap();
        let c = e.native_section_context(view).unwrap();
        assert!(
            e.native_drawing(view)
                .unwrap()
                .provider_lines(c)
                .unwrap()
                .is_empty()
        );
        s.far_depth = 1.9;
        let view = e.create_section_view("Far tangent", level, s).unwrap();
        let c = e.native_section_context(view).unwrap();
        assert_eq!(
            e.native_drawing(view)
                .unwrap()
                .provider_lines(c)
                .unwrap()
                .len(),
            4
        );
        s.far_depth = 4.;
        s.min_station = 2.;
        s.max_station = 6.;
        s.bottom_elevation = 1.;
        s.top_elevation = 4.;
        let view = e.create_section_view("Independent crop", level, s).unwrap();
        let c = e.native_section_context(view).unwrap();
        let d = e.native_drawing(view).unwrap();
        let lines = d.provider_lines(c).unwrap();
        assert_eq!(lines.len(), 1, "crop must not invent vertical outlines");
        assert!((lines[0].start.x - 2.).abs() < 1e-7 && (lines[0].end.x - 6.).abs() < 1e-7);
        assert_eq!(lines[0].start.y, 3.);
    }
}

fn column(level: Id, x: f64, y: f64) -> Column {
    Column::new(
        "core.column",
        ColumnParams {
            name: "Column".into(),
            level,
            center: Point2::new(x, y),
            width: 0.4,
            depth: 0.4,
            height: 3.,
            base_offset: 0.,
            material: None,
        },
    )
}
#[test]
fn section_projection_columns_casework_cuts_and_off_volume_budget() {
    let (mut e, level) = fixture();
    let ty = CaseworkType::new(
        "core.casework_type",
        CaseworkTypeParams {
            name: "Cabinet".into(),
            width: 1.,
            depth: 0.6,
            height: 1.2,
            material: None,
        },
    );
    let type_id = ty.id();
    e.command("Type", Command::AddCaseworkType(ty)).unwrap();
    let mut ids = Vec::new();
    let mut commands = Vec::new();
    for (x, y) in [(1., 0.), (3., -2.)] {
        let c = column(level, x, y);
        ids.push(c.id());
        commands.push(Command::AddColumn(c));
    }
    for (x, y) in [(5., 0.), (7., -2.)] {
        let c = Casework::new(
            "core.casework",
            CaseworkParams {
                name: "Cabinet".into(),
                level,
                type_id,
                center: Point2::new(x, y),
                yaw: 0.,
                base_offset: 0.,
            },
        );
        ids.push(c.id());
        commands.push(Command::AddCasework(c));
    }
    for i in 0..342 {
        commands.push(Command::AddColumn(column(level, 100. + i as f64, 100.)));
    }
    e.commands("Components", commands).unwrap();
    let view = e
        .create_section_view("Components", level, settings())
        .unwrap();
    let c = e.native_section_context(view).unwrap();
    let d = e.native_drawing(view).unwrap();
    let lines = d.provider_lines(c).unwrap();
    for (id, role) in ids.iter().zip([
        PlanRole::Cut,
        PlanRole::Projected,
        PlanRole::Cut,
        PlanRole::Projected,
    ]) {
        assert!(lines.iter().any(|l| l.entity == *id && l.role == role));
    }
    assert!(lines.iter().all(|l| ids.contains(&l.entity)));
    assert!(
        d.section_fills(c)
            .unwrap()
            .iter()
            .all(|f| [ids[0], ids[2]].contains(&f.entity))
    );
    let commands = (0..342)
        .map(|_| Command::AddColumn(column(level, 4., -1.)))
        .collect();
    e.commands("Exceed projection budget", commands).unwrap();
    assert!(
        e.native_drawing(view)
            .err()
            .unwrap()
            .to_string()
            .contains("projection triangle budget")
    );
}

#[test]
fn section_cut_mask_keeps_aperture_background_visible_and_coplanar_cut_only_boundaries() {
    let (mut e, level) = fixture();
    let host = wall(&mut e, level, Point2::new(0., 0.), Point2::new(8., 0.), 3.);
    let background = wall(
        &mut e,
        level,
        Point2::new(1.5, -2.),
        Point2::new(2.5, -2.),
        1.5,
    );
    let hidden = wall(
        &mut e,
        level,
        Point2::new(4., -2.),
        Point2::new(5., -2.),
        1.5,
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Door".into(),
            host,
            offset: 1.,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 2.,
                height: 2.1,
                sill: 0.,
            },
            hinge: Default::default(),
            swing: Default::default(),
            open_state: Default::default(),
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
        },
    );
    e.command("Aperture", Command::AddOpening(opening)).unwrap();
    let view = e
        .create_section_view("Aperture", level, settings())
        .unwrap();
    let c = e.native_section_context(view).unwrap();
    let d = e.native_drawing(view).unwrap();
    let lines = d.provider_lines(c).unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l.entity == background && l.role == PlanRole::Projected)
    );
    assert!(!lines.iter().any(|l| l.entity == hidden));
    let mut s = settings();
    s.mode = SectionDisplayMode::CutOnly;
    s.start.y = 0.1;
    s.end.y = 0.1;
    let view = e.create_section_view("Supporting face", level, s).unwrap();
    let c = e.native_section_context(view).unwrap();
    let d = e.native_drawing(view).unwrap();
    assert!(
        d.provider_lines(c)
            .unwrap()
            .iter()
            .any(|l| l.entity == host)
    );
    assert!(d.section_fills(c).unwrap().iter().all(|f| f.entity != host));
}
