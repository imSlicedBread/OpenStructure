use os_core::{Id, Point2};
use os_geometry::plan::{HorizontalBasis, PlanFootprint, PlanRange, PlanRole};
use os_render::plan::{
    PlanCamera, PlanContext, PlanDrawing, PlanElementAppearance, PlanFloorItem, PlanStroke,
};
use os_render::sheet::{
    PaperColor, PaperMarkKind, PaperSheetInfo, PaperStroke, PaperViewport, compose_view_sheet,
};
use std::collections::BTreeMap;

fn context() -> PlanContext {
    PlanContext {
        annotation_crop: None,
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}

fn styled_wall(context: PlanContext, wall: Id, style: PlanStroke) -> PlanDrawing {
    let footprint = PlanFootprint::from_convex(
        PlanRole::Cut,
        vec![
            Point2::new(-1.0, -0.1),
            Point2::new(1.0, -0.1),
            Point2::new(1.0, 0.1),
            Point2::new(-1.0, 0.1),
        ],
        None,
    )
    .unwrap()
    .unwrap();
    let footprints = BTreeMap::from([(
        wall,
        vec![(os_geometry::SurfaceIdentity::default(), footprint)],
    )]);
    PlanDrawing::from_layered_footprints(context, &footprints, vec![])
        .unwrap()
        .with_appearances(BTreeMap::from([(
            wall,
            PlanElementAppearance {
                cut: Some(style),
                projected: None,
                overhead: None,
            },
        )]))
        .unwrap()
}

#[test]
fn native_appearance_is_checked_and_flows_to_sheet_and_dashed_vector_pdf() {
    let context = context();
    let wall = Id::new();
    let style = PlanStroke {
        color: [20, 120, 220],
        weight_mm: 0.8,
        dashed: true,
    };
    let drawing = styled_wall(context, wall, style);
    assert_eq!(
        drawing.appearance(context, wall, PlanRole::Cut).unwrap(),
        Some(style)
    );
    assert_eq!(
        drawing
            .appearance(context, wall, PlanRole::Projected)
            .unwrap(),
        None
    );
    assert!(
        drawing
            .appearance(
                PlanContext {
                    model_revision: 1,
                    ..context
                },
                wall,
                PlanRole::Cut,
            )
            .is_err()
    );

    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.0,
            height_mm: 297.0,
            number: "A101",
            name: "Plan graphics",
        },
        "Ground floor",
        PaperViewport {
            center_mm: Point2::new(210.0, 125.0),
            width_mm: 390.0,
            height_mm: 230.0,
            model_center_m: Point2::new(0.0, 0.0),
            scale_denominator: 100.0,
        },
        context,
        &drawing,
    )
    .unwrap();
    assert!(page.marks().iter().any(|mark| matches!(
        &mark.kind,
        PaperMarkKind::Path {
            closed: false,
            stroke: Some(PaperStroke {
                color: PaperColor {
                    red: 20,
                    green: 120,
                    blue: 220
                },
                width_mm: 0.8,
                dashed: true,
            }),
            ..
        }
    )));

    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    assert!(pdf.contains("[8.503937007874017 4.251968503937008] 0 d"));
    assert!(pdf.contains("2.267717 w"));
    assert!(pdf.contains("0.078431 0.470588 0.862745 RG"));
}

#[test]
fn overhead_geometry_uses_default_dash_and_boundary_only_selection() {
    let context = context();
    let wall = Id::new();
    let footprint = PlanFootprint::from_convex(
        PlanRole::Overhead,
        vec![
            Point2::new(-1.0, -0.1),
            Point2::new(1.0, -0.1),
            Point2::new(1.0, 0.1),
            Point2::new(-1.0, 0.1),
        ],
        None,
    )
    .unwrap()
    .unwrap();
    let drawing = PlanDrawing::from_layered_footprints(
        context,
        &BTreeMap::from([(
            wall,
            vec![(os_geometry::SurfaceIdentity::default(), footprint)],
        )]),
        vec![],
    )
    .unwrap();
    let appearance = drawing
        .appearance(context, wall, PlanRole::Overhead)
        .unwrap()
        .unwrap();
    assert!(appearance.dashed);
    assert_eq!(appearance.weight_mm, 0.18);
    assert_eq!(drawing.pick(context, Point2::new(0.0, 0.0)).unwrap(), None);

    let camera = PlanCamera {
        center: Point2::new(0.0, 0.0),
        pixels_per_metre: 100.0,
    };
    let viewport = [800.0, 600.0];
    let interior = camera.project(Point2::new(0.0, 0.0), viewport).unwrap();
    let edge = camera.project(Point2::new(0.0, 0.1), viewport).unwrap();
    assert_eq!(
        drawing
            .pick_screen(context, camera, viewport, interior, 3.0)
            .unwrap(),
        None
    );
    assert_eq!(
        drawing
            .pick_screen(context, camera, viewport, edge, 3.0)
            .unwrap(),
        Some(wall)
    );

    let solid_style = PlanStroke {
        dashed: false,
        ..appearance
    };
    let styled = drawing
        .with_appearances(BTreeMap::from([(
            wall,
            PlanElementAppearance {
                cut: None,
                projected: None,
                overhead: Some(solid_style),
            },
        )]))
        .unwrap();
    assert_eq!(
        styled
            .appearance(context, wall, PlanRole::Overhead)
            .unwrap(),
        Some(solid_style)
    );
}

#[test]
fn overhead_floor_has_outline_hits_and_no_paper_fill() {
    let context = context();
    let boundary = vec![
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(2.0, 2.0),
        Point2::new(0.0, 2.0),
    ];
    let floor = PlanFloorItem {
        entity: Id::new(),
        role: PlanRole::Overhead,
        area_m2: 4.0,
        triangles: os_geometry::floors::triangulate_floor(&boundary).unwrap(),
        vertices: boundary.clone(),
        boundary,
        holes: vec![],
    };
    let id = floor.entity;
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_floors(vec![floor])
        .unwrap();
    let camera = PlanCamera {
        center: Point2::new(1.0, 1.0),
        pixels_per_metre: 100.0,
    };
    let viewport = [800.0, 600.0];
    assert_eq!(
        drawing.pick_floor(context, Point2::new(1.0, 1.0)).unwrap(),
        None
    );
    let interior = camera.project(Point2::new(1.0, 1.0), viewport).unwrap();
    let edge = camera.project(Point2::new(1.0, 0.0), viewport).unwrap();
    assert_eq!(
        drawing
            .pick_screen(context, camera, viewport, interior, 3.0)
            .unwrap(),
        None
    );
    assert_eq!(
        drawing
            .pick_screen(context, camera, viewport, edge, 3.0)
            .unwrap(),
        Some(id)
    );
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.0,
            height_mm: 297.0,
            number: "A101",
            name: "Overhead floor",
        },
        "Plan",
        PaperViewport {
            center_mm: Point2::new(210.0, 125.0),
            width_mm: 390.0,
            height_mm: 230.0,
            model_center_m: Point2::new(1.0, 1.0),
            scale_denominator: 100.0,
        },
        context,
        &drawing,
    )
    .unwrap();
    assert!(
        !page
            .marks()
            .iter()
            .any(|mark| matches!(&mark.kind, PaperMarkKind::Path { fill: Some(_), .. }))
    );
}
