use super::*;
#[path = "opening_section_fill_tests.rs"]
mod fill_tests;
use os_model::{
    Building, BuildingParams, Level, LevelParams, Material, MaterialParams, Opening,
    OpeningDefinition, OpeningFamily, OpeningKind, OpeningParams, OpeningState, OpeningType,
    OpeningTypeParams, SectionViewSettings, Wall, WallPath, WindowOperation,
};
use os_render::plan::PlanCamera;
use os_render::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};

fn fixture() -> (Editor, Id, Id, Id, Id, Id) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let mut lp = e.document.model().levels[&level].parameters.clone();
    lp.elevation = 0.3;
    e.command(
        "Elevated level",
        Command::UpdateLevel {
            id: level,
            parameters: lp,
        },
    )
    .unwrap();
    let plan = e.create_floor_plan("Opening plan", level).unwrap();
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Host".into(),
            level,
            height: 3.5,
            thickness: 0.2,
            material: None,
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
        },
    );
    let host = wall.id();
    e.command("Host", Command::AddWall(wall)).unwrap();
    let mut material_ids = Vec::new();
    for name in ["Panel", "Frame"] {
        let m = Material::new(
            "core.material",
            MaterialParams {
                cut_pattern: None,
                name: name.into(),
                density_kg_m3: 800.,
                color: [80, 100, 120],
            },
        );
        material_ids.push(m.id());
        e.command("Material", Command::AddMaterial(m)).unwrap();
    }
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Framed window".into(),
            kind: OpeningKind::Window,
            width: 2.,
            height: 1.5,
            sill: 0.8,
            pane_position: Default::default(),
            window_operation: WindowOperation::Fixed,
            family: OpeningFamily {
                frame_width: 0.06,
                panel_material: Some(material_ids[0]),
                frame_material: Some(material_ids[1]),
                ..Default::default()
            },
        },
    );
    let type_id = ty.id();
    let window = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Window".into(),
            host,
            offset: 2.,
            definition: OpeningDefinition::Typed { type_id },
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
    let window_id = window.id();
    let mut door = window.clone();
    door.header.id = Id::new();
    door.parameters.name = "Legacy door".into();
    door.parameters.offset = 5.;
    door.parameters.definition = OpeningDefinition::Legacy {
        kind: OpeningKind::Door,
        width: 1.,
        height: 2.1,
        sill: 0.,
    };
    let door_id = door.id();
    e.commands(
        "Openings",
        vec![
            Command::AddOpeningType(ty),
            Command::AddOpening(window),
            Command::AddOpening(door),
        ],
    )
    .unwrap();
    (e, level, plan, host, window_id, door_id)
}

fn section(e: &mut Editor, level: Id, start: Point2, end: Point2) -> Id {
    e.create_section_view(
        "Opening section",
        level,
        SectionViewSettings::new(start, end, -1., 5.),
    )
    .unwrap()
}

fn lines(e: &Editor, view: Id, id: Id) -> Vec<PlanLine> {
    let context = e.native_section_context(view).unwrap();
    e.native_drawing(view)
        .unwrap()
        .provider_lines(context)
        .unwrap()
        .iter()
        .filter(|l| l.entity == id)
        .cloned()
        .collect()
}

fn plan_lines(e: &Editor, view: Id, id: Id) -> Vec<PlanLine> {
    let context = e.native_plan_context(view).unwrap();
    e.native_drawing(view)
        .unwrap()
        .provider_lines(context)
        .unwrap()
        .iter()
        .filter(|l| l.entity == id)
        .cloned()
        .collect()
}

#[test]
fn opening_sections_share_scene_solids_materials_elevations_and_read_only_sources() {
    let (mut e, level, _, host, window, door) = fixture();
    for (start, end) in [
        (Point2::new(0., 0.), Point2::new(8., 0.)),
        (Point2::new(2.731, -1.), Point2::new(2.731, 1.)),
        (Point2::new(1.731, -1.), Point2::new(3.731, 1.)),
    ] {
        let view = section(&mut e, level, start, end);
        let model = e.document.model().clone();
        let revision = e.document.revision();
        let history = e.document.history_stats().undo_entries;
        let snapshot = e.section_snapshot(view).unwrap();
        let context = snapshot.context;
        let plane = snapshot.plane;
        let mut expected = Vec::new();
        for source in &snapshot.openings {
            let solids = os_geometry::openings::component_solids(
                &source.parameters,
                &source.host,
                source.elevation,
            )
            .unwrap();
            assert_eq!(
                e.scene[&source.entity],
                os_geometry::openings::component_mesh(
                    &source.parameters,
                    &source.host,
                    source.elevation
                )
                .unwrap()
            );
            let mut feature = 0;
            for solid in solids {
                if !section_plane_crosses_component(&solid, plane).unwrap() {
                    continue;
                }
                let surface = solid.surfaces.first().copied().unwrap_or_default();
                let mut edges = Vec::new();
                add_section_contours(
                    &mut edges,
                    os_geometry::section::vertical_section(&solid, plane).unwrap(),
                    &mut 0,
                )
                .unwrap();
                for segment in reduce_section_edges(edges).unwrap() {
                    expected.push((source.entity, feature, segment.start, segment.end, surface));
                    feature += 1;
                }
            }
        }
        let drawing = snapshot.derive().unwrap();
        let actual = drawing.provider_lines(context).unwrap();
        assert!(actual.iter().any(|l| l.entity == host));
        assert!(actual.iter().any(|l| l.entity == window));
        if start == Point2::new(0., 0.) {
            assert!(actual.iter().any(|l| l.entity == door));
        }
        for (id, feature, start, end, surface) in &expected {
            let line = actual
                .iter()
                .find(|l| l.entity == *id && l.feature == *feature)
                .unwrap();
            assert!(line.start.distance(*start) < 1e-8 && line.end.distance(*end) < 1e-8);
            assert_eq!(drawing.line_surface(*id, *feature), *surface);
            assert_eq!(line.role, PlanRole::Cut);
        }
        assert_eq!(
            actual
                .iter()
                .filter(|l| [window, door].contains(&l.entity))
                .count(),
            expected.len()
        );
        for l in actual.iter().filter(|l| l.entity == window) {
            assert!(l.start.y >= 1.1 - 1e-8 && l.start.y <= 2.6 + 1e-8);
        }
        assert_eq!(e.document.model(), &model);
        assert_eq!(e.document.revision(), revision);
        assert_eq!(e.document.history_stats().undo_entries, history);
    }
}

#[test]
fn opening_sections_cut_curved_hosts_in_both_directions() {
    for sweep in [-1.2, 1.2] {
        let (mut e, level, _, host, window, _) = fixture();
        let mut wall = e.document.model().walls[&host].parameters.clone();
        wall.path = WallPath::CircularArc {
            center: Point2::new(0., 0.),
            radius: 8.,
            start_angle_rad: 0.2,
            signed_sweep_rad: sweep,
        };
        e.command(
            "Curve opening host",
            Command::UpdateWall {
                id: host,
                parameters: wall,
            },
        )
        .unwrap();

        let path = e.document.model().walls[&host].parameters.path;
        let station = 3.;
        let origin = path.point(station);
        let radial = Point2::new(origin.x / 8., origin.y / 8.);
        let end = Point2::new(origin.x + radial.x * 2., origin.y + radial.y * 2.);
        let view = section(&mut e, level, origin, end);
        let context = e.native_section_context(view).unwrap();
        let drawing = e.native_drawing(view).unwrap();
        let opening_lines: Vec<_> = drawing
            .provider_lines(context)
            .unwrap()
            .iter()
            .filter(|line| line.entity == window)
            .cloned()
            .collect();
        assert!(
            !opening_lines.is_empty(),
            "missing curved-host filling at sweep {sweep}"
        );
        assert!(opening_lines.iter().all(|line| line.role == PlanRole::Cut));

        let resolved = e
            .document
            .model()
            .resolve_opening(&e.document.model().openings[&window].parameters)
            .unwrap();
        let materials: std::collections::BTreeSet<_> = opening_lines
            .iter()
            .map(|line| drawing.line_surface(window, line.feature).material)
            .collect();
        assert!(materials.contains(&resolved.family.panel_material));
        assert!(materials.contains(&resolved.family.frame_material));
    }
}

#[test]
fn opening_section_oblique_reversal_separates_host_reveals_pane_and_frame() {
    for reversed in [false, true] {
        let (mut e, level, _, host, window, _) = fixture();
        let mut wall = e.document.model().walls[&host].parameters.clone();
        wall.path = if reversed {
            WallPath::Straight {
                start: Point2::new(16.4, 0.8),
                end: Point2::new(10., -4.),
            }
        } else {
            WallPath::Straight {
                start: Point2::new(10., -4.),
                end: Point2::new(16.4, 0.8),
            }
        };
        // The symmetric fixed window occupies original stations 2..4 in both cases.
        let mut opening = e.document.model().openings[&window].parameters.clone();
        opening.offset = if reversed { 4. } else { 2. };
        // Relocate the other opening too, keeping the entire host equivalent.
        let door = e
            .document
            .model()
            .openings
            .values()
            .find(|o| o.id() != window)
            .unwrap();
        let door_id = door.id();
        let mut dp = door.parameters.clone();
        if reversed {
            dp.offset = 2.;
            dp.hinge = os_model::DoorHinge::End;
            dp.swing = os_model::DoorSwing::Right;
        }
        e.commands(
            "Oblique equivalent host",
            vec![
                Command::UpdateWall {
                    id: host,
                    parameters: wall,
                },
                Command::UpdateOpening {
                    id: window,
                    parameters: opening,
                },
                Command::UpdateOpening {
                    id: door_id,
                    parameters: dp,
                },
            ],
        )
        .unwrap();
        // World centre (12.4,-2.2), axis normal (-.6,.8), origin one metre back.
        let view = section(
            &mut e,
            level,
            Point2::new(13., -3.),
            Point2::new(11.8, -1.4),
        );
        let context = e.native_section_context(view).unwrap();
        let drawing = e.native_drawing(view).unwrap();
        let all = drawing.provider_lines(context).unwrap();
        let resolved = e
            .document
            .model()
            .resolve_opening(&e.document.model().openings[&window].parameters)
            .unwrap();
        let filling: Vec<_> = all.iter().filter(|l| l.entity == window).collect();
        assert_eq!(filling.len(), 12);
        // Explicit section-space boxes: 25 mm pane, 50 mm frame, 60 mm rails.
        for (x0, x1, z0, z1, material) in [
            (0.9875, 1.0125, 1.16, 2.54, resolved.family.panel_material),
            (0.975, 1.025, 1.1, 1.16, resolved.family.frame_material),
            (0.975, 1.025, 2.54, 2.6, resolved.family.frame_material),
        ] {
            let corners = [
                Point2::new(x0, z0),
                Point2::new(x1, z0),
                Point2::new(x1, z1),
                Point2::new(x0, z1),
            ];
            for i in 0..4 {
                let (a, b) = (corners[i], corners[(i + 1) % 4]);
                let edge = filling
                    .iter()
                    .find(|l| {
                        (l.start.distance(a) < 1e-8 && l.end.distance(b) < 1e-8)
                            || (l.start.distance(b) < 1e-8 && l.end.distance(a) < 1e-8)
                    })
                    .unwrap();
                assert_eq!(
                    drawing.line_surface(window, edge.feature).material,
                    material
                );
                assert_eq!(edge.role, PlanRole::Cut);
            }
        }
        let host_lines: Vec<_> = all.iter().filter(|l| l.entity == host).collect();
        assert!(!host_lines.is_empty());
        for z in [1.1, 2.6] {
            assert!(host_lines.iter().any(|l| (l.start.y - z).abs() < 1e-8
                && (l.end.y - z).abs() < 1e-8
                && (l.start.x.min(l.end.x) - 0.9).abs() < 1e-8
                && (l.start.x.max(l.end.x) - 1.1).abs() < 1e-8));
        }
        assert!(
            host_lines
                .iter()
                .all(|l| l.start.y.max(l.end.y) <= 1.1 + 1e-8
                    || l.start.y.min(l.end.y) >= 2.6 - 1e-8)
        );
    }
}

#[test]
fn opening_section_picking_crop_and_sheet_pdf_use_identical_contours() {
    let (mut e, level, _, _, window, _) = fixture();
    let type_id = e
        .document
        .model()
        .resolve_opening(&e.document.model().openings[&window].parameters)
        .unwrap()
        .type_id
        .unwrap();
    let mut parameters = e.document.model().opening_types[&type_id]
        .parameters
        .clone();
    let mut addition = os_model::OpeningExtrusion::new("Section identity witness");
    addition.attachment = os_model::OpeningAttachment::OpeningFixed;
    addition.normal_offset = 0.;
    addition.profile = vec![
        Point2::new(0.2, 0.2),
        Point2::new(0.4, 0.2),
        Point2::new(0.4, 0.9),
        Point2::new(0.2, 0.9),
    ];
    addition.material = parameters.family.frame_material;
    let child_surface = os_geometry::SurfaceIdentity {
        layer: Some(addition.id),
        material: addition.material,
    };
    parameters.family.extrusions.push(addition);
    e.command(
        "Authored section component",
        Command::UpdateOpeningType {
            id: type_id,
            parameters,
        },
    )
    .unwrap();
    let view = section(&mut e, level, Point2::new(0., 0.), Point2::new(8., 0.));
    let context = e.native_section_context(view).unwrap();
    let drawing = e.native_drawing(view).unwrap();
    let camera = PlanCamera {
        center: Point2::new(4., 2.),
        pixels_per_metre: 100.,
    };
    let size = [1000., 650.];
    let point = Point2::new(3., 1.16);
    assert_eq!(
        drawing
            .pick_screen(
                context,
                camera,
                size,
                camera.project(point, size).unwrap(),
                2.
            )
            .unwrap(),
        Some(window)
    );
    let viewport = PaperViewport {
        center_mm: Point2::new(148.5, 105.),
        width_mm: 200.,
        height_mm: 140.,
        model_center_m: Point2::new(4., 2.),
        scale_denominator: 100.,
    };
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 297.,
            height_mm: 210.,
            number: "A1",
            name: "Opening sections",
        },
        "Section",
        viewport,
        context,
        &drawing,
    )
    .unwrap();
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    for line in drawing
        .provider_lines(context)
        .unwrap()
        .iter()
        .filter(|l| l.entity == window)
    {
        let expected = [
            viewport.model_to_paper(line.start).unwrap(),
            viewport.model_to_paper(line.end).unwrap(),
        ];
        assert!(page.marks().iter().any(|mark| matches!(&mark.kind, PaperMarkKind::Path { points_mm, closed: false, .. } if points_mm.as_slice() == expected)));
        let pt = 72. / 25.4;
        assert!(pdf.contains(&format!(
            "{:.6} {:.6} m\n{:.6} {:.6} l",
            expected[0].x * pt,
            (210. - expected[0].y) * pt,
            expected[1].x * pt,
            (210. - expected[1].y) * pt
        )));
    }
    let mut parameters = e.document.model().views[&view].parameters.clone();
    parameters.section.as_mut().unwrap().bottom_elevation = 2.;
    parameters.section.as_mut().unwrap().top_elevation = 2.3;
    e.command(
        "Crop section",
        Command::UpdateView {
            id: view,
            parameters,
        },
    )
    .unwrap();
    let current = e.native_section_context(view).unwrap();
    assert!(drawing.provider_lines(current).is_err());
    assert!(
        compose_view_sheet(
            PaperSheetInfo {
                width_mm: 297.,
                height_mm: 210.,
                number: "A1",
                name: "Stale"
            },
            "Section",
            viewport,
            current,
            &drawing
        )
        .is_err()
    );
    let cropped = e.native_drawing(view).unwrap();
    assert!(!cropped.provider_lines(current).unwrap().is_empty());
    assert!(cropped.provider_lines(current).unwrap().iter().all(|l| {
        [l.start.y, l.end.y]
            .into_iter()
            .all(|y| (2. - 1e-8..=2.3 + 1e-8).contains(&y))
    }));
    assert_eq!(
        cropped
            .pick_screen(
                current,
                camera,
                size,
                camera.project(point, size).unwrap(),
                2.
            )
            .unwrap(),
        None
    );
    // Independent expected clipping: the opening-fixed addition spans
    // x=2.4..2.8, z=1.4..2.45. Cropping to z=2..2.3 leaves ONLY two vertical
    // strokes; it must neither keep the original top/bottom nor invent caps.
    let child_lines: Vec<_> = cropped
        .provider_lines(current)
        .unwrap()
        .iter()
        .filter(|l| {
            l.entity == window
                && cropped.line_surface(window, l.feature).layer == child_surface.layer
        })
        .collect();
    assert_eq!(child_lines.len(), 2);
    for x in [2.4, 2.8] {
        let line = child_lines
            .iter()
            .find(|l| (l.start.x - x).abs() < 1e-8)
            .unwrap();
        assert!((line.end.x - x).abs() < 1e-8);
        assert!((line.start.y.min(line.end.y) - 2.).abs() < 1e-8);
        assert!((line.start.y.max(line.end.y) - 2.3).abs() < 1e-8);
        assert_eq!(cropped.line_surface(window, line.feature), child_surface);
    }
    // Every surviving opening edge keeps its feature/material identity.
    for line in cropped
        .provider_lines(current)
        .unwrap()
        .iter()
        .filter(|l| l.entity == window)
    {
        assert_eq!(
            cropped.line_surface(window, line.feature),
            drawing.line_surface(window, line.feature)
        );
    }
    let cropped_page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 297.,
            height_mm: 210.,
            number: "A1",
            name: "Cropped",
        },
        "Section",
        viewport,
        current,
        &cropped,
    )
    .unwrap();
    let cropped_pdf = String::from_utf8(cropped_page.to_pdf().unwrap()).unwrap();
    let paper_paths: Vec<_> = cropped_page
        .marks()
        .iter()
        .filter_map(|mark| match &mark.kind {
            PaperMarkKind::Path {
                points_mm,
                closed: false,
                ..
            } if mark.clip.is_some() => Some(points_mm),
            _ => None,
        })
        .collect();
    let source_lines = cropped.provider_lines(current).unwrap();
    assert_eq!(paper_paths.len(), source_lines.len());
    let pdf_path = |points: [Point2; 2]| {
        let pt = 72. / 25.4;
        format!(
            "{:.6} {:.6} m\n{:.6} {:.6} l",
            points[0].x * pt,
            (210. - points[0].y) * pt,
            points[1].x * pt,
            (210. - points[1].y) * pt
        )
    };
    for line in source_lines {
        let expected = [
            viewport.model_to_paper(line.start).unwrap(),
            viewport.model_to_paper(line.end).unwrap(),
        ];
        assert!(paper_paths.iter().any(|p| p.as_slice() == expected));
        assert!(cropped_pdf.contains(&pdf_path(expected)));
    }
    // No off-crop original opening segment may leak into paper or PDF output.
    for line in
        drawing.provider_lines(context).unwrap().iter().filter(|l| {
            l.entity == window && [l.start.y, l.end.y].iter().any(|y| *y < 2. || *y > 2.3)
        })
    {
        let original = [
            viewport.model_to_paper(line.start).unwrap(),
            viewport.model_to_paper(line.end).unwrap(),
        ];
        assert!(!paper_paths.iter().any(|p| p.as_slice() == original));
        assert!(!cropped_pdf.contains(&pdf_path(original)));
    }
}

#[test]
fn opening_section_edits_types_poses_rehost_history_reopen_and_stale_sources() {
    let (mut e, level, plan, host, window, _) = fixture();
    let view = section(&mut e, level, Point2::new(0., 0.), Point2::new(8., 0.));
    let type_id = e
        .document
        .model()
        .resolve_opening(&e.document.model().openings[&window].parameters)
        .unwrap()
        .type_id
        .unwrap();
    for step in 0..4 {
        let snapshot = e.section_snapshot(view).unwrap();
        let old_context = snapshot.context;
        let old_lines = lines(&e, view, window);
        let old_host_lines = lines(&e, view, host);
        let old_plan = plan_lines(&e, plan, window);
        let old_scene = e.scene.clone();
        let history = e.document.history_stats().undo_entries;
        let command = if step == 1 {
            let mut parameters = e.document.model().opening_types[&type_id]
                .parameters
                .clone();
            parameters.height = 1.7;
            parameters.window_operation = WindowOperation::Casement;
            Command::UpdateOpeningType {
                id: type_id,
                parameters,
            }
        } else {
            let mut parameters = e.document.model().openings[&window].parameters.clone();
            match step {
                0 => {
                    parameters.width_override = Some(1.7);
                    parameters.sill_override = Some(0.9);
                }
                2 => parameters.open_state = OpeningState::CasementAngle(35.),
                _ => parameters.offset = 3.,
            }
            Command::UpdateOpening {
                id: window,
                parameters,
            }
        };
        e.command("Edit filling", command).unwrap();
        assert_eq!(e.document.history_stats().undo_entries, history + 1);
        let current = e.native_section_context(view).unwrap();
        let frozen = snapshot.derive().unwrap();
        assert_eq!(
            frozen
                .provider_lines(old_context)
                .unwrap()
                .iter()
                .filter(|l| l.entity == window)
                .cloned()
                .collect::<Vec<_>>(),
            old_lines
        );
        assert!(frozen.provider_lines(current).is_err());
        assert!(
            frozen
                .pick_screen(
                    current,
                    PlanCamera::default(),
                    [800., 600.],
                    Point2::new(400., 300.),
                    4.
                )
                .is_err()
        );
        let new_lines = lines(&e, view, window);
        assert_ne!(new_lines, old_lines);
        assert_ne!(e.scene[&window], old_scene[&window]);
        assert_ne!(plan_lines(&e, plan, window), old_plan);
        if step != 2 {
            assert_ne!(e.scene[&host], old_scene[&host]);
            assert_ne!(lines(&e, view, host), old_host_lines);
        } else {
            assert_eq!(e.scene[&host], old_scene[&host]);
            assert_eq!(lines(&e, view, host), old_host_lines);
        }
        e.undo().unwrap();
        assert_eq!(lines(&e, view, window), old_lines);
        assert_eq!(lines(&e, view, host), old_host_lines);
        assert_eq!(e.scene, old_scene);
        e.redo().unwrap();
        assert_eq!(lines(&e, view, window), new_lines);
    }
    let mut parameters = e.document.model().walls[&host].parameters.clone();
    parameters.path = WallPath::Straight {
        start: Point2::new(0., 2.),
        end: Point2::new(8., 2.),
    };
    let second = Wall::new(os_walls::WALL_TYPE, parameters);
    let second_id = second.id();
    e.command("Second host", Command::AddWall(second)).unwrap();
    let old_scene = e.scene.clone();
    let old_lines = lines(&e, view, window);
    let mut parameters = e.document.model().openings[&window].parameters.clone();
    parameters.host = second_id;
    e.command(
        "Rehost",
        Command::UpdateOpening {
            id: window,
            parameters,
        },
    )
    .unwrap();
    assert!(lines(&e, view, window).is_empty());
    for id in [host, second_id, window] {
        assert_ne!(e.scene[&id], old_scene[&id]);
    }
    e.undo().unwrap();
    assert_eq!(e.scene, old_scene);
    assert_eq!(lines(&e, view, window), old_lines);
    e.redo().unwrap();
    let second_view = section(&mut e, level, Point2::new(0., 2.), Point2::new(8., 2.));
    let expected = lines(&e, second_view, window);
    assert!(!expected.is_empty());
    let temp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = temp.path().join("opening-section.osp");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(lines(&reopened, second_view, window), expected);
    assert_eq!(reopened.scene, e.scene);
}

#[test]
fn opening_section_scope_and_plan_visibility_are_independent() {
    let (mut e, level, plan, _, window, _) = fixture();
    let view = section(&mut e, level, Point2::new(0., 0.), Point2::new(8., 0.));
    let before = lines(&e, view, window);
    let mut parameters = e.document.model().views[&plan].parameters.clone();
    parameters.plan.as_mut().unwrap().visibility.windows = false;
    e.command(
        "Hide plan windows",
        Command::UpdateView {
            id: plan,
            parameters,
        },
    )
    .unwrap();
    assert!(plan_lines(&e, plan, window).is_empty());
    assert_eq!(lines(&e, view, window), before);
    let building = e.document.model().levels[&level].parameters.building;
    let foreign = Building::new(
        "core.building",
        BuildingParams {
            name: "Other".into(),
            site: e.document.model().buildings[&building].parameters.site,
        },
    );
    let foreign_id = foreign.id();
    let foreign_level = Level::new(
        "core.level",
        LevelParams {
            name: "Other".into(),
            elevation: 0.,
            building: foreign_id,
        },
    );
    let foreign_level_id = foreign_level.id();
    let mut model = e.document.model().clone();
    model.buildings.insert(foreign_id, foreign);
    model.levels.insert(foreign_level_id, foreign_level);
    e.document = os_document::Document::from_model(model).unwrap();
    let foreign_view = section(
        &mut e,
        foreign_level_id,
        Point2::new(0., 0.),
        Point2::new(8., 0.),
    );
    assert!(
        e.section_snapshot(foreign_view)
            .unwrap()
            .openings
            .is_empty()
    );
    assert!(lines(&e, foreign_view, window).is_empty());
}

#[test]
fn opening_section_snapshots_regenerate_after_host_level_and_deletion_history() {
    let assert_frozen =
        |snapshot: PreparedSectionSnapshot, expected: &PlanDrawing, current: PlanContext| {
            let old = snapshot.context;
            let frozen = snapshot.derive().unwrap();
            assert_eq!(
                frozen.provider_lines(old).unwrap(),
                expected.provider_lines(old).unwrap()
            );
            for line in frozen.provider_lines(old).unwrap() {
                assert_eq!(
                    frozen.line_surface(line.entity, line.feature),
                    expected.line_surface(line.entity, line.feature)
                );
            }
            assert!(frozen.provider_lines(current).is_err());
        };
    for edit in ["translation", "reversal", "elevation", "deletion"] {
        let (mut e, level, _, host, window, door) = fixture();
        let view = section(&mut e, level, Point2::new(0., 0.), Point2::new(8., 0.));
        let before_model = e.document.model().clone();
        let before_scene = e.scene.clone();
        let before_context = e.native_section_context(view).unwrap();
        let before = e.native_drawing(view).unwrap();
        let snapshot = e.section_snapshot(view).unwrap();
        let history = e.document.history_stats().undo_entries;
        let mut commands = Vec::new();
        match edit {
            "translation" | "reversal" => {
                let mut wall = e.document.model().walls[&host].parameters.clone();
                wall.path = if edit == "translation" {
                    WallPath::Straight {
                        start: Point2::new(0.5, 0.),
                        end: Point2::new(8.5, 0.),
                    }
                } else {
                    WallPath::Straight {
                        start: Point2::new(8., 0.),
                        end: Point2::new(0., 0.),
                    }
                };
                commands.push(Command::UpdateWall {
                    id: host,
                    parameters: wall,
                });
                if edit == "reversal" {
                    for (id, offset) in [(window, 4.), (door, 2.)] {
                        let mut parameters = e.document.model().openings[&id].parameters.clone();
                        parameters.offset = offset;
                        if id == door {
                            parameters.hinge = os_model::DoorHinge::End;
                            parameters.swing = os_model::DoorSwing::Right;
                        }
                        commands.push(Command::UpdateOpening { id, parameters });
                    }
                }
            }
            "elevation" => {
                let mut parameters = e.document.model().levels[&level].parameters.clone();
                parameters.elevation += 0.4;
                commands.push(Command::UpdateLevel {
                    id: level,
                    parameters,
                });
            }
            "deletion" => commands.push(Command::RemoveOpening(window)),
            _ => unreachable!(),
        }
        e.commands(edit, commands).unwrap();
        assert_eq!(e.document.history_stats().undo_entries, history + 1);
        let current = e.native_section_context(view).unwrap();
        assert_frozen(snapshot, &before, current);
        let changed = e.native_drawing(view).unwrap();
        let changed_model = e.document.model().clone();
        let changed_scene = e.scene.clone();
        let changed_snapshot = e.section_snapshot(view).unwrap();
        let old_opening: Vec<_> = before
            .provider_lines(before_context)
            .unwrap()
            .iter()
            .filter(|l| l.entity == window)
            .collect();
        let new_opening: Vec<_> = changed
            .provider_lines(current)
            .unwrap()
            .iter()
            .filter(|l| l.entity == window)
            .collect();
        if edit == "deletion" {
            assert!(new_opening.is_empty());
            assert!(!e.scene.contains_key(&window));
            assert_ne!(
                lines(&e, view, host),
                before
                    .provider_lines(before_context)
                    .unwrap()
                    .iter()
                    .filter(|l| l.entity == host)
                    .cloned()
                    .collect::<Vec<_>>()
            );
            assert!(!lines(&e, view, door).is_empty());
        } else {
            assert!(!old_opening.is_empty());
            assert_eq!(new_opening.len(), old_opening.len());
            let (dx, dy) = match edit {
                "translation" => (0.5, 0.),
                "elevation" => (0., 0.4),
                _ => (0., 0.),
            };
            for old in old_opening {
                let a = Point2::new(old.start.x + dx, old.start.y + dy);
                let b = Point2::new(old.end.x + dx, old.end.y + dy);
                assert!(
                    new_opening.iter().any(|new| {
                        ((new.start.distance(a) < 1e-8 && new.end.distance(b) < 1e-8)
                            || (new.start.distance(b) < 1e-8 && new.end.distance(a) < 1e-8))
                            && changed.line_surface(window, new.feature)
                                == before.line_surface(window, old.feature)
                            && new.role == old.role
                    }),
                    "incorrect regenerated {edit} edge {old:?}"
                );
            }
        }
        e.undo().unwrap();
        assert_eq!(e.document.model(), &before_model);
        assert_eq!(e.scene, before_scene);
        let restored_context = e.native_section_context(view).unwrap();
        assert_frozen(changed_snapshot, &changed, restored_context);
        let restored = e.native_drawing(view).unwrap();
        assert_eq!(
            restored.provider_lines(restored_context).unwrap(),
            before.provider_lines(before_context).unwrap()
        );
        let restored_snapshot = e.section_snapshot(view).unwrap();
        e.redo().unwrap();
        assert_eq!(e.document.model(), &changed_model);
        assert_eq!(e.scene, changed_scene);
        let redone_context = e.native_section_context(view).unwrap();
        assert_frozen(restored_snapshot, &restored, redone_context);
        let redone = e.native_drawing(view).unwrap();
        assert_eq!(
            redone.provider_lines(redone_context).unwrap(),
            changed.provider_lines(current).unwrap()
        );
        for line in redone.provider_lines(redone_context).unwrap() {
            assert_eq!(
                redone.line_surface(line.entity, line.feature),
                changed.line_surface(line.entity, line.feature)
            );
        }
    }
}

#[test]
fn opening_sections_fail_atomically_on_invalid_late_source_and_budgets() {
    let (mut e, level, _, _, _, _) = fixture();
    let view = section(&mut e, level, Point2::new(0., 0.), Point2::new(8., 0.));
    let mut snapshot = e.section_snapshot(view).unwrap();
    snapshot.openings.last_mut().unwrap().elevation = f64::NAN;
    assert!(snapshot.derive().is_err());
    let mut snapshot = e.section_snapshot(view).unwrap();
    let source = snapshot.openings.pop().unwrap();
    snapshot.openings = (0..=MAX_PLAN_ELEMENTS)
        .map(|_| SectionOpening {
            entity: Id::new(),
            parameters: source.parameters.clone(),
            host: source.host.clone(),
            elevation: source.elevation,
        })
        .collect();
    // A miss still consumes the component budget; no partial empty drawing.
    snapshot.source.plane.origin.y = 100.;
    assert!(snapshot.derive().is_err());
}
