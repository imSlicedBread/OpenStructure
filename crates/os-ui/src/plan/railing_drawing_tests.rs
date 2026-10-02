use super::*;
use os_document::Document;
use os_geometry::section::{VerticalSectionPlane, vertical_section};
use os_model::{
    ElementLifecycle, Level, LevelParams, Material, MaterialParams, PhaseFilter, PlanViewRange,
    SectionViewSettings, Stair, StairParams, StairRailing, StairRailingParams, StairRailingSide,
    StairRailingType, StairRailingTypeParams,
};
use os_render::plan::PlanCamera;
use os_render::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};

fn fixture(side: StairRailingSide, start: Point2, end: Point2) -> (Editor, Id, Id, Id) {
    let mut editor = Editor::new().unwrap();
    let mut model = editor.document.model().clone();
    let lower = model.levels.values().next().unwrap().clone();
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Railing upper".into(),
            elevation: lower.parameters.elevation + 2.0,
            building: lower.parameters.building,
        },
    );
    let upper_id = upper.id();
    model.levels.insert(upper_id, upper);
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Railing finish".into(),
            density_kg_m3: 7800.,
            color: [80, 100, 120],
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    let stair = Stair::new(
        "core.stair",
        StairParams {
            name: "Flight".into(),
            lower_level: lower.id(),
            upper_level: upper_id,
            start,
            end,
            width: 1.2,
            riser_count: 8,
            structural_thickness: 0.2,
            material: Some(material_id),
        },
    );
    let stair_id = stair.id();
    model.stairs.insert(stair_id, stair);
    let ty = StairRailingType::new(
        "core.railing_type",
        StairRailingTypeParams {
            name: "Guardrail".into(),
            top_rail_height: 0.95,
            top_rail_width: 0.05,
            top_rail_depth: 0.05,
            post_width: 0.08,
            post_depth: 0.05,
            max_post_spacing: 0.7,
            material: Some(material_id),
        },
    );
    let type_id = ty.id();
    model.railing_types.insert(type_id, ty);
    let railing = StairRailing::new(
        "core.railing",
        StairRailingParams {
            name: "Guardrail".into(),
            stair: stair_id,
            railing_type: type_id,
            side,
        },
    );
    let railing_id = railing.id();
    model.railings.insert(railing_id, railing);
    editor.document = Document::from_model(model).unwrap();
    let view = editor
        .create_floor_plan("Railing plan", lower.id())
        .unwrap();
    let mut parameters = editor.document.model().views[&view].parameters.clone();
    parameters.plan.as_mut().unwrap().range = PlanViewRange {
        top: 6.,
        cut: 5.,
        bottom: -1.,
        depth: -2.,
    };
    editor
        .command(
            "Railing plan range",
            Command::UpdateView {
                id: view,
                parameters,
            },
        )
        .unwrap();
    (editor, view, stair_id, railing_id)
}

fn horizontal() -> (Editor, Id, Id, Id) {
    fixture(
        StairRailingSide::Left,
        Point2::new(0., 0.),
        Point2::new(4., 0.),
    )
}

fn section(editor: &mut Editor, start: Point2, end: Point2) -> Id {
    let level = *editor.document.model().levels.keys().next().unwrap();
    let view = View::new(
        "core.view",
        ViewParams {
            name: format!("Section {}", editor.document.model().views.len()),
            kind: ViewKind::Section,
            level: Some(level),
            settings_revision: 0,
            plan: None,
            section: Some(SectionViewSettings::new(start, end, -1., 6.)),
        },
    );
    let id = view.id();
    editor
        .command("Railing section", Command::AddView(view))
        .unwrap();
    id
}

#[test]
fn plan_side_reversal_and_diagonal_projection_follow_host_and_pick_own_id() {
    for (start, end) in [
        (Point2::new(0., 0.), Point2::new(4., 0.)),
        (Point2::new(4., 0.), Point2::new(0., 0.)),
        (Point2::new(1., 2.), Point2::new(4., 5.)),
    ] {
        for side in [StairRailingSide::Left, StairRailingSide::Right] {
            let (editor, view, stair, railing) = fixture(side, start, end);
            let before = serde_json::to_value(editor.document.model()).unwrap();
            let context = editor.native_plan_context(view).unwrap();
            let drawing = editor.native_drawing(view).unwrap();
            let lines: Vec<_> = drawing
                .provider_lines(context)
                .unwrap()
                .iter()
                .filter(|l| l.entity == railing)
                .collect();
            assert_eq!(lines.len(), 37); // One centerline and nine four-edge posts.
            assert!(drawing.is_native_line(railing));
            assert!(
                drawing
                    .provider_lines(context)
                    .unwrap()
                    .iter()
                    .filter(|l| l.entity == stair)
                    .all(|l| drawing.line_surface(stair, l.feature).material
                        == editor.document.model().stairs[&stair].parameters.material)
            );
            let rail = lines.iter().find(|l| l.feature == 0).unwrap();
            let run = start.distance(end);
            let sign = if side == StairRailingSide::Left {
                1.
            } else {
                -1.
            };
            let offset = 0.575 * sign;
            let normal = Point2::new(-(end.y - start.y) / run, (end.x - start.x) / run);
            assert!(
                rail.start.distance(Point2::new(
                    start.x + normal.x * offset,
                    start.y + normal.y * offset
                )) < 1e-10
            );
            assert!(
                rail.end.distance(Point2::new(
                    end.x + normal.x * offset,
                    end.y + normal.y * offset
                )) < 1e-10
            );
            let world = Point2::new(
                (rail.start.x + rail.end.x) * 0.5,
                (rail.start.y + rail.end.y) * 0.5,
            );
            for viewport in [[1280., 800.], [1000. / 1.5, 650. / 1.5]] {
                let camera = PlanCamera::default();
                let pointer = camera.project(world, viewport).unwrap();
                assert_eq!(
                    drawing
                        .pick_screen(context, camera, viewport, pointer, 3.)
                        .unwrap(),
                    Some(railing)
                );
                assert_ne!(railing, stair);
            }
            assert_eq!(
                serde_json::to_value(editor.document.model()).unwrap(),
                before
            );
        }
    }
}

#[test]
fn range_and_crop_hide_marks_and_keep_post_feature_identity() {
    let (editor, view, _, railing) = horizontal();
    let model = editor.document.model();
    let members =
        os_geometry::railings::railing_geometry(&model.railings[&railing].parameters, model)
            .unwrap()
            .members;
    let context = editor.native_plan_context(view).unwrap();
    let full = railing_plan_lines(railing, &members, context).unwrap();
    let mut cropped = context;
    cropped.crop = Some(PlanCrop {
        min: Point2::new(1.2, 0.5),
        max: Point2::new(2.2, 0.7),
    });
    let drawing = PlanDrawing::from_prisms(cropped, &BTreeMap::new(), vec![railing])
        .unwrap()
        .with_native_lines(BTreeMap::from([(railing, full.clone())]))
        .unwrap();
    let retained = drawing.provider_lines(cropped).unwrap();
    assert!(!retained.is_empty());
    assert!(retained.len() < full.len());
    for line in retained {
        assert!(full.iter().any(|original| original.feature == line.feature));
        for p in [line.start, line.end] {
            assert!((1.2 - 1e-9..=2.2 + 1e-9).contains(&p.x));
            assert!((0.5 - 1e-9..=0.7 + 1e-9).contains(&p.y));
        }
    }
    let mut cut = context;
    cut.range = PlanRange {
        top: 6.,
        cut: 1.5,
        bottom: 0.,
        depth: -1.,
    };
    let lines = railing_plan_lines(railing, &members, cut).unwrap();
    assert!(lines.iter().any(|l| l.role == PlanRole::Cut));
    assert!(lines.iter().any(|l| l.role == PlanRole::Projected));
    assert!(
        lines
            .iter()
            .filter(|l| l.feature >= 8)
            .all(|l| full.iter().any(|original| original.feature == l.feature
                && original.start == l.start
                && original.end == l.end))
    );
    let mut below = context;
    below.range = PlanRange {
        top: -0.5,
        cut: -1.,
        bottom: -2.,
        depth: -3.,
    };
    assert!(
        railing_plan_lines(railing, &members, below)
            .unwrap()
            .is_empty()
    );
    let mut miss = context;
    miss.crop = Some(PlanCrop {
        min: Point2::new(10., 10.),
        max: Point2::new(11., 11.),
    });
    let drawing = PlanDrawing::from_prisms(miss, &BTreeMap::new(), vec![railing])
        .unwrap()
        .with_native_lines(BTreeMap::from([(railing, full)]))
        .unwrap();
    assert!(drawing.provider_lines(miss).unwrap().is_empty());
    let camera = PlanCamera::default();
    let viewport = [1280., 800.];
    assert_eq!(
        drawing
            .pick_screen(
                miss,
                camera,
                viewport,
                camera.project(Point2::new(2., 0.575), viewport).unwrap(),
                3.
            )
            .unwrap(),
        None
    );
}

#[test]
fn phase_filter_hides_railing_and_hidden_host_dependents() {
    let (mut editor, view, stair, railing) = horizontal();
    for hidden in [railing, stair] {
        let mut model = editor.document.model().clone();
        model.element_lifecycles.clear();
        model.element_lifecycles.insert(
            hidden,
            ElementLifecycle {
                created_in: model.latest_phase().unwrap(),
                demolished_in: None,
            },
        );
        model
            .views
            .get_mut(&view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .phase_filter = PhaseFilter::ShowExisting;
        editor.document = Document::from_model(model).unwrap();
        let ctx = editor.native_plan_context(view).unwrap();
        let drawing = editor.native_drawing(view).unwrap();
        assert!(
            !drawing
                .provider_lines(ctx)
                .unwrap()
                .iter()
                .any(|l| l.entity == railing)
        );
    }
}

#[test]
fn editor_basis_and_section_crop_apply_to_railing_lines() {
    let (mut editor, plan, _, railing) = horizontal();
    let mut parameters = editor.document.model().views[&plan].parameters.clone();
    let settings = parameters.plan.as_mut().unwrap();
    settings.basis.origin = Point2::new(1., 0.);
    settings.basis.rotation = std::f64::consts::FRAC_PI_2;
    editor
        .command(
            "Rotate plan",
            Command::UpdateView {
                id: plan,
                parameters,
            },
        )
        .unwrap();
    let context = editor.native_plan_context(plan).unwrap();
    let drawing = editor.native_drawing(plan).unwrap();
    let rail = drawing
        .provider_lines(context)
        .unwrap()
        .iter()
        .find(|l| l.entity == railing && l.feature == 0)
        .unwrap();
    assert!(
        rail.start.distance(
            context
                .basis
                .world_to_plane(Point2::new(0., 0.575))
                .unwrap()
        ) < 1e-10
    );
    assert!(
        rail.end.distance(
            context
                .basis
                .world_to_plane(Point2::new(4., 0.575))
                .unwrap()
        ) < 1e-10
    );

    let view = section(&mut editor, Point2::new(1., 0.575), Point2::new(3., 0.575));
    let mut parameters = editor.document.model().views[&view].parameters.clone();
    parameters.section.as_mut().unwrap().top_elevation = 1.5;
    editor
        .command(
            "Crop section height",
            Command::UpdateView {
                id: view,
                parameters,
            },
        )
        .unwrap();
    let context = editor.native_section_context(view).unwrap();
    let drawing = editor.native_drawing(view).unwrap();
    let lines: Vec<_> = drawing
        .provider_lines(context)
        .unwrap()
        .iter()
        .filter(|l| l.entity == railing)
        .collect();
    assert!(!lines.is_empty());
    assert!(lines.len() < 40);
    for line in lines {
        for p in [line.start, line.end] {
            assert!((-1e-9..=2. + 1e-9).contains(&p.x));
            assert!((-1. - 1e-9..=1.5 + 1e-9).contains(&p.y));
        }
        assert!(
            drawing
                .line_surface(railing, line.feature)
                .material
                .is_some()
        );
    }
}

#[test]
fn section_uses_independent_member_contours_and_material_with_stable_owner() {
    let (mut editor, _, _, railing) = horizontal();
    let view = section(&mut editor, Point2::new(-1., 0.575), Point2::new(5., 0.575));
    let ctx = editor.native_section_context(view).unwrap();
    let snapshot = editor.section_snapshot(view).unwrap();
    let plane = snapshot.plane;
    let mut expected = Vec::new();
    for (index, member) in snapshot.railing_members[&railing].iter().enumerate() {
        let mut edges = Vec::new();
        let mut count = 0;
        add_section_contours(
            &mut edges,
            vertical_section(member, plane).unwrap(),
            &mut count,
        )
        .unwrap();
        for (edge, line) in reduce_section_edges(edges).unwrap().into_iter().enumerate() {
            expected.push(((index * 4 + edge) as u32, line));
        }
    }
    assert_eq!(expected.len(), 40); // Closed rail contour and nine closed post contours.
    let before = serde_json::to_value(editor.document.model()).unwrap();
    let drawing = snapshot.derive().unwrap();
    let lines: Vec<_> = drawing
        .provider_lines(ctx)
        .unwrap()
        .iter()
        .filter(|l| l.entity == railing)
        .collect();
    assert_eq!(lines.len(), expected.len());
    let model = editor.document.model();
    let material = model.railing_types[&model.railings[&railing].parameters.railing_type]
        .parameters
        .material;
    for (line, (feature, segment)) in lines.iter().zip(expected) {
        assert_eq!(
            (line.feature, line.start, line.end, line.role),
            (feature, segment.start, segment.end, PlanRole::Cut)
        );
        assert_eq!(drawing.line_surface(railing, feature).material, material);
    }
    assert!(drawing.is_native_line(railing));
    assert_eq!(
        serde_json::to_value(editor.document.model()).unwrap(),
        before
    );
    // Avoid a coincident tread/mesh-diagonal plane; the section kernel rejects
    // those exact coplanar cuts instead of returning ambiguous contours.
    let transverse = section(&mut editor, Point2::new(2.013, -1.), Point2::new(2.013, 1.));
    let ctx = editor.native_section_context(transverse).unwrap();
    assert_eq!(
        editor
            .native_drawing(transverse)
            .unwrap()
            .provider_lines(ctx)
            .unwrap()
            .iter()
            .filter(|l| l.entity == railing)
            .count(),
        8
    );
    let miss = section(&mut editor, Point2::new(-1., -1.), Point2::new(5., -1.));
    let ctx = editor.native_section_context(miss).unwrap();
    assert!(
        !editor
            .native_drawing(miss)
            .unwrap()
            .provider_lines(ctx)
            .unwrap()
            .iter()
            .any(|l| l.entity == railing)
    );
}

fn assert_sheet_consumes_railing_lines(drawing: &PlanDrawing, ctx: PlanContext, railing: Id) {
    let viewport = PaperViewport {
        center_mm: Point2::new(148.5, 105.),
        width_mm: 200.,
        height_mm: 140.,
        model_center_m: Point2::new(2., 1.),
        scale_denominator: 100.,
    };
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 297.,
            height_mm: 210.,
            number: "A1",
            name: "Railing",
        },
        "View",
        viewport,
        ctx,
        drawing,
    )
    .unwrap();
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    for line in drawing
        .provider_lines(ctx)
        .unwrap()
        .iter()
        .filter(|l| l.entity == railing)
    {
        let expected = [
            viewport.model_to_paper(line.start).unwrap(),
            viewport.model_to_paper(line.end).unwrap(),
        ];
        assert!(page.marks().iter().any(|mark| matches!(&mark.kind, PaperMarkKind::Path { points_mm, closed: false, .. } if points_mm.as_slice() == expected)));
        let pt = 72. / 25.4;
        let path = format!(
            "{:.6} {:.6} m\n{:.6} {:.6} l",
            expected[0].x * pt,
            (210. - expected[0].y) * pt,
            expected[1].x * pt,
            (210. - expected[1].y) * pt
        );
        assert!(pdf.contains(&path), "missing railing vector {path}");
    }
    assert!(pdf.starts_with("%PDF-"));
}

#[test]
fn native_plan_and_section_sheet_pdf_consume_railing_lines_and_reject_stale_context() {
    let (mut editor, view, stair, railing) = horizontal();
    let ctx = editor.native_plan_context(view).unwrap();
    let drawing = editor.native_drawing(view).unwrap();
    assert_sheet_consumes_railing_lines(&drawing, ctx, railing);
    let section_view = section(&mut editor, Point2::new(-1., 0.575), Point2::new(5., 0.575));
    let ctx = editor.native_section_context(section_view).unwrap();
    let section_drawing = editor.native_drawing(section_view).unwrap();
    assert_sheet_consumes_railing_lines(&section_drawing, ctx, railing);
    let mut parameters = editor.document.model().stairs[&stair].parameters.clone();
    parameters.end = Point2::new(5., 0.);
    editor
        .command(
            "Lengthen host",
            Command::UpdateStair {
                id: stair,
                parameters,
            },
        )
        .unwrap();
    assert!(
        section_drawing
            .provider_lines(editor.native_section_context(section_view).unwrap())
            .is_err()
    );
    let ctx = editor.native_plan_context(view).unwrap();
    let drawing = editor.native_drawing(view).unwrap();
    assert!(
        drawing
            .provider_lines(ctx)
            .unwrap()
            .iter()
            .any(|l| l.entity == railing && (l.end.x - 5.).abs() < 1e-10)
    );
    assert!(drawing.is_native_line(railing));
}

#[test]
fn section_budget_rejects_excess_members_and_plan_checks_coordinates() {
    let (editor, view, _, railing) = horizontal();
    let model = editor.document.model();
    let geometry =
        os_geometry::railings::railing_geometry(&model.railings[&railing].parameters, model)
            .unwrap();
    let ctx = editor.native_plan_context(view).unwrap();
    let mut invalid = geometry.members.clone();
    invalid[0].vertices[0].x = f64::NAN;
    assert!(railing_plan_lines(railing, &invalid, ctx).is_err());
    let source = SectionSnapshot {
        context: ctx,
        plane: VerticalSectionPlane {
            origin: Point2::new(0., 0.575),
            direction: Point2::new(1., 0.),
        },
        interfaces: Vec::new(),
        walls: Vec::new(),
        floors: Vec::new(),
        ceilings: Vec::new(),
        roofs: Vec::new(),
        stairs: Vec::new(),
        ramps: Vec::new(),
        curtains: Vec::new(),
    };
    assert!(
        source
            .derive_with_railings(BTreeMap::from([(
                railing,
                vec![geometry.members[0].clone(); 514]
            )]))
            .is_err()
    );
}
