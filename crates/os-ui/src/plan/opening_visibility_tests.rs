use super::*;
use os_document::Command;
use os_model::*;

#[test]
fn opening_visibility_independent_categories_views_cutouts_annotations_picks_snaps_and_paper() {
    let (mut editor, view, walls, phases) = phase_tests::fixture();
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let other = editor.create_floor_plan("Other plan", level).unwrap();
    let mut ids = Vec::new();
    for (kind, offset) in [(OpeningKind::Door, 1.), (OpeningKind::Window, 3.)] {
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: format!("{kind:?}"),
                host: walls[0],
                offset,
                definition: OpeningDefinition::Legacy {
                    kind,
                    width: 1.,
                    height: 2.1,
                    sill: 0.,
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let id = opening.id();
        ids.push(id);
        let tag = OpeningTag::new(
            "core.opening_tag",
            OpeningTagParams {
                view,
                opening: id,
                position: Point2::new(offset + 0.5, -1.),
                label_preset: Default::default(),
            },
        );
        let dimension = Dimension::new(
            "core.dimension",
            DimensionParams {
                view,
                first: DimensionReference::OpeningJamb {
                    opening: id,
                    jamb: DimensionJamb::Start,
                },
                second: DimensionReference::OpeningJamb {
                    opening: id,
                    jamb: DimensionJamb::End,
                },
                additional: vec![],
                layout: DimensionLayout::Aligned,
                baseline_spacing_m: 0.25,
                offset_m: 0.4,
                orphan_hint: Point2::new(offset + 0.5, -0.4),
            },
        );
        editor
            .document
            .execute(
                "Openings",
                vec![
                    Command::AddOpening(opening),
                    Command::SetElementLifecycle {
                        element: id,
                        lifecycle: ElementLifecycle {
                            created_in: phases[1],
                            demolished_in: None,
                        },
                    },
                    Command::AddOpeningTag(tag),
                    Command::AddDimension(dimension),
                ],
            )
            .unwrap();
    }
    let original = editor.document.model().clone();
    let initial = editor.native_drawing(view).unwrap();
    let initial_context = editor.native_plan_context(view).unwrap();
    let initial_items = initial.items(initial_context).unwrap().to_vec();
    let other_settings = original.views[&other].parameters.clone();
    let camera = os_render::plan::PlanCamera::default();
    let size = [1280., 800.];
    for (doors, windows) in [(false, true), (true, false), (false, false), (true, true)] {
        let mut parameters = editor.document.model().views[&view].parameters.clone();
        let visibility = &mut parameters.plan.as_mut().unwrap().visibility;
        visibility.doors = doors;
        visibility.windows = windows;
        editor
            .command(
                "Opening visibility",
                Command::UpdateView {
                    id: view,
                    parameters,
                },
            )
            .unwrap();
        let context = editor.native_plan_context(view).unwrap();
        let drawing = editor.native_drawing(view).unwrap();
        assert!(initial.items(context).is_err());
        assert_eq!(drawing.items(context).unwrap(), initial_items);
        assert_eq!(editor.document.model().openings, original.openings);
        assert_eq!(editor.document.model().walls, original.walls);
        assert_eq!(
            editor.document.model().views[&other].parameters,
            other_settings
        );
        assert!(
            ids.iter()
                .all(|id| editor.native_drawing(other).unwrap().is_native_line(*id))
        );
        assert_eq!(
            drawing.opening_tags(context).unwrap().len(),
            usize::from(doors) + usize::from(windows)
        );
        assert_eq!(
            drawing.dimensions(context).unwrap().len(),
            usize::from(doors) + usize::from(windows)
        );
        for ((id, visible), offset) in ids.iter().zip([doors, windows]).zip([1., 3.]) {
            assert_eq!(drawing.is_native_line(*id), visible);
            // The aperture remains empty even with no visible symbol.
            assert_ne!(
                drawing
                    .pick(context, Point2::new(offset + 0.5, 0.))
                    .unwrap(),
                Some(walls[0])
            );
            for line in initial
                .provider_lines(initial_context)
                .unwrap()
                .iter()
                .filter(|l| l.entity == *id)
            {
                let pointer = camera.project(line.start, size).unwrap();
                let hits = drawing
                    .hits_screen(context, camera, size, pointer, 1.)
                    .unwrap();
                if !visible {
                    assert!(!hits.contains(id));
                }
                let query = os_render::snapping::SnapQuery {
                    camera,
                    viewport: size,
                    pointer,
                    radius_pixels: 1.,
                    endpoints: true,
                    midpoints: true,
                    intersections: false,
                    perpendicular_from: None,
                    nearest: true,
                    axis_extensions: false,
                    exclude_entity: None,
                };
                let snap = drawing
                    .snap(context, query)
                    .unwrap()
                    .candidate(context, query)
                    .unwrap();
                if !visible {
                    assert!(snap.is_none_or(|s| s.entity != *id));
                }
            }
            if !visible {
                let injected = BTreeMap::from([(
                    *id,
                    initial
                        .provider_lines(initial_context)
                        .unwrap()
                        .iter()
                        .filter(|l| l.entity == *id)
                        .cloned()
                        .collect(),
                )]);
                let composed = editor
                    .plan_snapshot(view)
                    .unwrap()
                    .derive_with_provider_lines(injected)
                    .unwrap();
                assert!(
                    !composed
                        .provider_lines(context)
                        .unwrap()
                        .iter()
                        .any(|l| l.entity == *id)
                );
            }
        }
        let viewport = os_render::sheet::PaperViewport {
            center_mm: Point2::new(210., 140.),
            width_mm: 380.,
            height_mm: 230.,
            model_center_m: Point2::new(3., 3.),
            scale_denominator: 100.,
        };
        let paper = os_render::sheet::compose_view_sheet(
            os_render::sheet::PaperSheetInfo {
                width_mm: 420.,
                height_mm: 297.,
                number: "V1",
                name: "Visibility",
            },
            "Plan",
            viewport,
            context,
            &drawing,
        )
        .unwrap();
        for line in initial.provider_lines(initial_context).unwrap() {
            if !ids.contains(&line.entity) {
                continue;
            }
            // Symbol-only strokes away from wall edges uniquely identify these marks.
            if line.start.y.abs() < 0.01 || line.end.y.abs() < 0.01 || line.start.x == line.end.x {
                continue;
            }
            let points = vec![
                viewport.model_to_paper(line.start).unwrap(),
                viewport.model_to_paper(line.end).unwrap(),
            ];
            let present = paper.marks().iter().any(|m| {
                matches!(&m.kind,
                os_render::sheet::PaperMarkKind::Path { points_mm, .. } if *points_mm==points)
            });
            assert_eq!(
                present,
                if line.entity == ids[0] {
                    doors
                } else {
                    windows
                }
            );
        }
        assert!(paper.to_pdf().unwrap().starts_with(b"%PDF"));
    }
    // A phase-hidden aperture is physically absent in the view-only wall model.
    phase_tests::set_phase(&mut editor, view, phases[0], PhaseFilter::ShowAll);
    let c = editor.native_plan_context(view).unwrap();
    let d = editor.native_drawing(view).unwrap();
    for x in [1.5, 3.5] {
        assert_eq!(d.pick(c, Point2::new(x, 0.)).unwrap(), Some(walls[0]));
    }
    assert!(ids.iter().all(|id| !d.is_native_line(*id)));
}
