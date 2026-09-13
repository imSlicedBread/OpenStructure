use os_core::Point2;
use os_document::Command;
use os_model::{Building, BuildingParams, Grid, GridParams, PlanViewBasis};
use os_render::{
    plan::PlanCamera,
    snapping::{SnapKind, SnapQuery},
};
use os_ui::Editor;

#[test]
fn rotated_building_grid_drawings_snap_and_track_edit_undo_save() {
    let mut editor = Editor::new().unwrap();
    let level = *editor.document.model().levels.keys().next().unwrap();
    let building = editor.document.model().levels[&level].parameters.building;
    let view = editor.create_floor_plan("Grid plan", level).unwrap();
    let grid = Grid::new(
        "core.grid",
        GridParams {
            name: "A".into(),
            building,
            start: Point2::new(1_000_000.0, 2_000_000.0),
            end: Point2::new(1_000_006.0, 2_000_008.0),
        },
    );
    editor
        .command("Grid", Command::AddGrid(grid.clone()))
        .unwrap();
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.basis = PlanViewBasis {
        origin: grid.parameters.start,
        rotation: 0.3,
    };
    settings.visibility.walls = false;
    editor
        .update_floor_plan(view, "Grid plan", level, settings)
        .unwrap();
    let before = editor.document.model().clone();
    let context = editor.native_plan_context(view).unwrap();
    let drawing = editor.native_wall_plan(view).unwrap();
    let g = &drawing.grids(context).unwrap()[0];
    assert_eq!(g.entity, grid.id());
    assert_eq!(g.start, Point2::new(0.0, 0.0));
    let expected = Point2::new(
        6.0 * 0.3_f64.cos() + 8.0 * 0.3_f64.sin(),
        -6.0 * 0.3_f64.sin() + 8.0 * 0.3_f64.cos(),
    );
    assert!(g.end.distance(expected) < 1e-9);
    let camera = PlanCamera::default();
    let q = SnapQuery {
        camera,
        viewport: [800.0, 600.0],
        pointer: camera.project(g.end, [800.0, 600.0]).unwrap(),
        radius_pixels: 12.0,
        endpoints: true,
        midpoints: true,
        intersections: false,
        perpendicular_from: None,
        nearest: true,
        axis_extensions: false,
        exclude_entity: None,
    };
    let hit = drawing
        .snap(context, q)
        .unwrap()
        .candidate(context, q)
        .unwrap()
        .unwrap();
    assert_eq!(hit.kind, SnapKind::Endpoint);
    assert!(
        context
            .basis
            .plane_to_world(hit.point)
            .unwrap()
            .distance(grid.parameters.end)
            < 1e-8
    );
    assert_eq!(editor.document.model(), &before);
    assert!(!editor.scene.contains_key(&grid.id())); // Datums are not fabricated solids.
    let mut params = grid.parameters.clone();
    params.end.x += 1.0;
    editor
        .command(
            "Move grid",
            Command::UpdateGrid {
                id: grid.id(),
                parameters: params,
            },
        )
        .unwrap();
    assert!(
        drawing
            .grids(editor.native_plan_context(view).unwrap())
            .is_err()
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.model(), &before);
    assert!(
        drawing
            .grids(editor.native_plan_context(view).unwrap())
            .is_err()
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grids.osb");
    editor.save(&path).unwrap();
    editor.open(&path).unwrap();
    let current = editor.native_plan_context(view).unwrap();
    assert_eq!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .grids(current)
            .unwrap()[0]
            .entity,
        grid.id()
    );

    // Same XY datum in another building must not leak into this level's plan.
    let mut model = editor.document.model().clone();
    let other = Building::new(
        "core.building",
        BuildingParams {
            name: "Other".into(),
            site: *model.sites.keys().next().unwrap(),
        },
    );
    let mut other_grid = grid.clone();
    other_grid.header.id = os_core::Id::new();
    other_grid.parameters.building = other.id();
    model.buildings.insert(other.id(), other);
    model.grids.insert(other_grid.id(), other_grid);
    editor.document = os_document::Document::from_model(model).unwrap();
    let c = editor.native_plan_context(view).unwrap();
    assert_eq!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .grids(c)
            .unwrap()
            .len(),
        1
    );
}
