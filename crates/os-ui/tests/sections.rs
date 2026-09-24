use os_core::Point2;
use os_document::Command;
use os_model::{
    Floor, FloorParams, Opening, OpeningDefinition, OpeningKind, OpeningParams,
    SectionViewSettings, ViewKind, Wall, WallParams,
};
use os_ui::Editor;

#[test]
fn linked_section_draws_native_wall_openings_floor_and_plan_marker() {
    let mut editor = Editor::new().unwrap();
    let level = *editor.document.model().levels.keys().next().unwrap();
    let plan = editor.create_floor_plan("Ground", level).unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Section wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(8.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    let door = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Door".into(),
            host: wall_id,
            offset: 1.0,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            },
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let window = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Window".into(),
            host: wall_id,
            offset: 4.0,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Window,
                width: 1.2,
                height: 1.2,
                sill: 1.0,
            },
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Section slab".into(),
            level,
            material: None,
            boundary: vec![
                Point2::new(0.0, -2.0),
                Point2::new(8.0, -2.0),
                Point2::new(8.0, 2.0),
                Point2::new(0.0, 2.0),
            ],
            thickness: 0.2,
            top_offset: 0.0,
        },
    );
    let floor_id = floor.id();
    editor
        .document
        .execute(
            "Author sectioned building elements",
            vec![
                Command::AddWall(wall),
                Command::AddOpening(door),
                Command::AddOpening(window),
                Command::AddFloor(floor),
            ],
        )
        .unwrap();

    let settings =
        SectionViewSettings::new(Point2::new(0.0, 0.0), Point2::new(8.0, 0.0), -0.5, 4.0);
    let section = editor.create_section_view("A", level, settings).unwrap();
    let model_before = editor.document.model().clone();
    let context = editor.native_section_context(section).unwrap();
    let drawing = editor.native_drawing(section).unwrap();
    let cut_lines = drawing.provider_lines(context).unwrap();
    assert!(cut_lines.iter().any(|line| line.entity == wall_id));
    assert!(cut_lines.iter().any(|line| line.entity == floor_id));
    assert!(
        cut_lines
            .iter()
            .all(|line| line.role == os_geometry::plan::PlanRole::Cut)
    );

    let wall_lines: Vec<_> = cut_lines
        .iter()
        .filter(|line| line.entity == wall_id)
        .collect();
    let has_vertical = |station: f64, bottom: f64, top: f64| {
        wall_lines.iter().any(|line| {
            (line.start.x - station).abs() < 1e-7
                && (line.end.x - station).abs() < 1e-7
                && (line.start.y.min(line.end.y) - bottom).abs() < 1e-7
                && (line.start.y.max(line.end.y) - top).abs() < 1e-7
        })
    };
    assert!(has_vertical(1.0, 0.0, 2.1), "door jamb contour missing");
    assert!(has_vertical(1.9, 0.0, 2.1), "door jamb contour missing");
    assert!(has_vertical(4.0, 1.0, 2.2), "window reveal contour missing");
    assert!(has_vertical(5.2, 1.0, 2.2), "window reveal contour missing");
    assert!(
        wall_lines
            .iter()
            .any(|line| { (line.start.y - 1.0).abs() < 1e-7 && (line.end.y - 1.0).abs() < 1e-7 })
    );
    assert!(
        wall_lines
            .iter()
            .any(|line| { (line.start.y - 2.2).abs() < 1e-7 && (line.end.y - 2.2).abs() < 1e-7 })
    );
    assert_eq!(editor.document.model(), &model_before);

    let plan_context = editor.native_plan_context(plan).unwrap();
    let plan_drawing = editor.native_drawing(plan).unwrap();
    let marker: Vec<_> = plan_drawing
        .provider_lines(plan_context)
        .unwrap()
        .iter()
        .filter(|line| line.entity == section)
        .collect();
    assert_eq!(marker.len(), 3);
    assert!(editor.document.model().views[&section].parameters.kind == ViewKind::Section);
}
