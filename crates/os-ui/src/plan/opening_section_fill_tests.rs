//! Independent area/void oracles for the complete native section fill adapter.
use super::*;
use os_render::plan::PlanSectionFillPolygon;

fn area(fill: &PlanSectionFillPolygon) -> f64 {
    let o = fill.points[0];
    fill.points[1..]
        .windows(2)
        .map(|p| ((p[0].x - o.x) * (p[1].y - o.y) - (p[0].y - o.y) * (p[1].x - o.x)).abs() / 2.)
        .sum()
}

#[test]
fn section_projection_curved_host_and_thin_framed_window_share_visibility() {
    let (mut e, level, _, host, window, _) = fixture();
    let mut wall = e.document.model().walls[&host].parameters.clone();
    wall.path = os_model::WallPath::CircularArc {
        center: Point2::new(4., 0.),
        radius: 4.,
        start_angle_rad: std::f64::consts::PI,
        signed_sweep_rad: -std::f64::consts::PI,
    };
    e.command(
        "Arc",
        Command::UpdateWall {
            id: host,
            parameters: wall,
        },
    )
    .unwrap();
    let mut s = SectionViewSettings::new(Point2::new(-1., -1.), Point2::new(9., -1.), -1., 5.);
    s.mode = os_model::SectionDisplayMode::CutAndProjection;
    s.side = os_model::ElevationSide::Right;
    s.far_depth = 10.;
    let view = e.create_section_view("Arc projection", level, s).unwrap();
    let c = e.native_section_context(view).unwrap();
    let d = e.native_drawing(view).unwrap();
    let lines = d.provider_lines(c).unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l.entity == host && l.role == PlanRole::Projected)
    );
    assert!(
        lines
            .iter()
            .any(|l| l.entity == window && l.role == PlanRole::Projected)
    );
    assert!(
        lines
            .iter()
            .filter(|l| l.entity == window)
            .all(|l| d.line_surface(window, l.feature).material.is_some())
    );
}

fn covers(fill: &PlanSectionFillPolygon, p: Point2) -> bool {
    fill.points
        .iter()
        .zip(fill.points.iter().cycle().skip(1))
        .take(fill.points.len())
        .all(|(a, b)| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x) >= -1e-12)
}

#[test]
fn section_fills_host_apertures_and_independent_openings_have_analytic_area() {
    let (mut e, level, _, host, window, door) = fixture();
    let view = section(&mut e, level, Point2::new(0., 0.), Point2::new(8., 0.));
    let context = e.native_section_context(view).unwrap();
    let baseline = e.document.model().clone();
    let history = e.document.history_stats().undo_entries;
    let drawing = e.native_drawing(view).unwrap();
    let fills = drawing.section_fills(context).unwrap();
    let owner_area = |id| {
        fills
            .iter()
            .filter(|f| f.entity == id)
            .map(area)
            .sum::<f64>()
    };
    assert!((owner_area(host) - (8. * 3.5 - 2. * 1.5 - 1. * 2.1)).abs() < 1e-10);
    assert!((owner_area(window) - 3.).abs() < 1e-10);
    // The legacy default is a 90-degree leaf: this plane cuts its 25 mm
    // thickness at the hinge, leaving the rest of the doorway visibly empty.
    assert!((owner_area(door) - 0.025 * 2.1).abs() < 1e-10);
    for point in [Point2::new(2.731, 1.8), Point2::new(5.43, 1.2)] {
        assert!(!fills.iter().any(|f| f.entity == host && covers(f, point)));
    }
    assert!(
        fills
            .iter()
            .any(|f| f.entity == window && covers(f, Point2::new(2.731, 1.8)))
    );
    assert!(
        fills
            .iter()
            .any(|f| f.entity == door && covers(f, Point2::new(5., 1.2)))
    );
    assert!(!fills.iter().any(|f| covers(f, Point2::new(5.43, 1.2))));
    let last_host = fills.iter().rposition(|f| f.entity == host).unwrap();
    assert!(fills.iter().position(|f| f.entity == window).unwrap() > last_host);
    assert!(fills.iter().position(|f| f.entity == door).unwrap() > last_host);
    assert_eq!(e.document.model(), &baseline);
    assert_eq!(e.document.history_stats().undo_entries, history);
}

#[test]
fn section_fills_layers_material_snapshot_crop_and_history_are_consistent() {
    use os_model::{LayerFunction, WallLayer, WallType, WallTypeAssignment, WallTypeParams};
    let (mut e, level, _, host, window, _) = fixture();
    let materials: Vec<_> = e.document.model().materials.keys().copied().collect();
    let layers = vec![
        WallLayer {
            id: Id::new(),
            name: "Core".into(),
            thickness: 0.12,
            function: LayerFunction::Structure,
            material: Some(materials[0]),
        },
        WallLayer {
            id: Id::new(),
            name: "Finish".into(),
            thickness: 0.08,
            function: LayerFunction::Insulation,
            material: Some(materials[1]),
        },
    ];
    let ty = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Layered host".into(),
            layers: layers.clone(),
        },
    );
    let type_id = ty.id();
    e.commands(
        "Layered host",
        vec![
            Command::AddWallType(ty),
            Command::AssignWallType {
                wall: host,
                assignment: Some(WallTypeAssignment {
                    type_id,
                    flipped: false,
                }),
            },
        ],
    )
    .unwrap();
    let view = section(
        &mut e,
        level,
        Point2::new(2.731, -1.),
        Point2::new(2.731, 1.),
    );
    let context = e.native_section_context(view).unwrap();
    let snapshot = e.section_snapshot(view).unwrap();
    let drawing = e.native_drawing(view).unwrap();
    let fills = drawing.section_fills(context).unwrap();
    for layer in &layers {
        let regions: Vec<_> = fills
            .iter()
            .filter(|f| f.entity == host && f.surface.layer == Some(layer.id))
            .collect();
        assert!(!regions.is_empty());
        assert!(regions.iter().all(|f| f.surface.material == layer.material));
        assert!(
            (regions.iter().map(|f| area(f)).sum::<f64>() - layer.thickness * (3.5 - 1.5)).abs()
                < 1e-10
        );
    }
    // A transverse window cut contains its narrow pane and frame, but the
    // unfilled aperture beside the pane must remain empty for every owner.
    assert!(
        fills
            .iter()
            .any(|f| f.entity == window && covers(f, Point2::new(1., 1.8)))
    );
    assert!(!fills.iter().any(|f| covers(f, Point2::new(0.95, 1.8))));
    let mut changed = e.document.model().materials[&materials[0]]
        .parameters
        .clone();
    let original_color = changed.color;
    changed.color = [31, 97, 153];
    e.command(
        "Section color",
        Command::UpdateMaterial {
            id: materials[0],
            parameters: changed,
        },
    )
    .unwrap();
    let current = e.native_section_context(view).unwrap();
    assert!(drawing.section_fills(current).is_err());
    let frozen = snapshot.derive().unwrap();
    assert!(frozen.section_fills(current).is_err());
    let surface = material_surface(Some(materials[0]));
    assert_eq!(frozen.surface_color(surface), Some(original_color));
    let recolored = e.native_drawing(view).unwrap();
    assert_eq!(recolored.surface_color(surface), Some([31, 97, 153]));
    e.undo().unwrap();
    assert_eq!(
        e.native_drawing(view).unwrap().surface_color(surface),
        Some(original_color)
    );
    e.redo().unwrap();
    let mut parameters = e.document.model().views[&view].parameters.clone();
    parameters.section.as_mut().unwrap().bottom_elevation = 1.3;
    parameters.section.as_mut().unwrap().top_elevation = 1.9;
    e.command(
        "Crop",
        Command::UpdateView {
            id: view,
            parameters,
        },
    )
    .unwrap();
    let context = e.native_section_context(view).unwrap();
    let cropped = e.native_drawing(view).unwrap();
    let fills = cropped.section_fills(context).unwrap();
    assert!(!fills.iter().any(|f| f.entity == host));
    assert!(
        fills
            .iter()
            .all(|f| f.points.iter().all(|p| p.y >= 1.3 && p.y <= 1.9))
    );
    assert!(
        (fills
            .iter()
            .filter(|f| f.entity == window)
            .map(area)
            .sum::<f64>()
            - 0.025 * 0.6)
            .abs()
            < 1e-10
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("section-fills.osproj");
    e.save(&path).unwrap();
    let mut other = Editor::new().unwrap();
    other.open(&path).unwrap();
    let other_context = other.native_section_context(view).unwrap();
    let other_drawing = other.native_drawing(view).unwrap();
    assert_eq!(other_drawing.section_fills(other_context).unwrap(), fills);
    assert_eq!(other_drawing.surface_color(surface), Some([31, 97, 153]));
}

#[test]
fn section_fills_supporting_face_retains_contours_without_filling() {
    let (mut e, level, _, host, _, _) = fixture();
    let view = section(&mut e, level, Point2::new(0., 0.1), Point2::new(8., 0.1));
    let context = e.native_section_context(view).unwrap();
    let drawing = e.native_drawing(view).unwrap();
    assert!(!drawing.provider_lines(context).unwrap().is_empty());
    assert!(
        !drawing
            .section_fills(context)
            .unwrap()
            .iter()
            .any(|f| f.entity == host)
    );
}
