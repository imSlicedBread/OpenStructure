//! Phase snapshot acceptance, shared with real desktop input tests.
use super::*;
use os_document::{Command, Document};
use os_model::*;

pub(crate) fn fixture() -> (Editor, Id, [Id; 6], [Id; 3]) {
    let mut editor = Editor::new().unwrap();
    let mut model = editor.document.model().clone();
    let level = *model.levels.keys().next().unwrap();
    let future = new_phase("Future work", 2);
    let phases = [
        model.existing_phase().unwrap(),
        model.latest_phase().unwrap(),
        future.id(),
    ];
    model.phases.insert(future.id(), future);
    let lifetimes = [
        (0, None),
        (1, None),
        (0, Some(1)),
        (1, Some(1)),
        (2, None),
        (0, Some(0)),
    ];
    let walls = std::array::from_fn(|i| {
        let wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: format!("Phase wall {i}"),
                level,
                start: Point2::new(0., i as f64 * 2.),
                end: Point2::new(6., i as f64 * 2.),
                thickness: 0.2,
                height: 3.,
                material: None,
            },
        );
        let id = wall.id();
        model.walls.insert(id, wall);
        model.element_lifecycles.insert(
            id,
            ElementLifecycle {
                created_in: phases[lifetimes[i].0],
                demolished_in: lifetimes[i].1.map(|i| phases[i]),
            },
        );
        id
    });
    editor.document = Document::from_model(model).unwrap();
    let view = editor.create_floor_plan("Phase plan", level).unwrap();
    set_phase(&mut editor, view, phases[1], PhaseFilter::ShowAll);
    (editor, view, walls, phases)
}

pub(crate) fn set_phase(editor: &mut Editor, view: Id, phase: Id, filter: PhaseFilter) {
    let mut p = editor.document.model().views[&view].parameters.clone();
    let plan = p.plan.as_mut().unwrap();
    plan.target_phase = Some(phase);
    plan.phase_filter = filter;
    editor
        .command(
            "Plan phase",
            Command::UpdateView {
                id: view,
                parameters: p,
            },
        )
        .unwrap();
}

#[test]
fn phase_status_filter_matrix_excludes_geometry_and_picks_and_keeps_views_independent() {
    let (mut editor, view, walls, phases) = fixture();
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let other = editor.create_floor_plan("Existing survey", level).unwrap();
    set_phase(&mut editor, other, phases[0], PhaseFilter::ShowAll);
    let other_settings = editor.document.model().views[&other].parameters.clone();
    let expected = [vec![0, 1, 2, 3], vec![0], vec![1], vec![2], vec![3]];
    for (filter, indices) in [
        PhaseFilter::ShowAll,
        PhaseFilter::ShowExisting,
        PhaseFilter::ShowNew,
        PhaseFilter::ShowDemolished,
        PhaseFilter::ShowTemporary,
    ]
    .into_iter()
    .zip(expected)
    {
        let old = editor.plan_snapshot(view).unwrap();
        set_phase(&mut editor, view, phases[1], filter);
        let context = editor.native_plan_context(view).unwrap();
        let drawing = editor.native_drawing(view).unwrap();
        if old.context != context {
            assert!(old.derive().unwrap().items(context).is_err());
        }
        for (i, id) in walls.iter().enumerate() {
            let visible = indices.contains(&i);
            assert_eq!(
                drawing
                    .items(context)
                    .unwrap()
                    .iter()
                    .any(|item| item.entity == *id),
                visible,
                "{filter:?} / {i}"
            );
            assert_eq!(
                drawing
                    .pick(context, Point2::new(3., i as f64 * 2.))
                    .unwrap(),
                visible.then_some(*id)
            );
            let camera = os_render::plan::PlanCamera::default();
            let size = [800., 600.];
            let query = os_render::snapping::SnapQuery {
                camera,
                viewport: size,
                pointer: camera
                    .project(Point2::new(0., i as f64 * 2.), size)
                    .unwrap(),
                radius_pixels: 5.,
                endpoints: true,
                midpoints: false,
                intersections: false,
                perpendicular_from: None,
                nearest: false,
                axis_extensions: false,
                exclude_entity: None,
            };
            let hit = drawing
                .snap(context, query)
                .unwrap()
                .candidate(context, query)
                .unwrap();
            assert_eq!(hit.is_some_and(|hit| hit.entity == *id), visible);
        }
        assert_eq!(
            editor.document.model().views[&other].parameters,
            other_settings
        );
        let c = editor.native_plan_context(other).unwrap();
        let d = editor.native_drawing(other).unwrap();
        assert!(
            !d.items(c)
                .unwrap()
                .iter()
                .any(|item| item.entity == walls[1])
        );
    }
}

#[test]
fn phase_opening_cutout_tags_dimensions_host_and_provider_precedence() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut editor, view, walls, phases) = fixture();
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Future door".into(),
                host: walls[0],
                offset: 1.,
                definition: OpeningDefinition::Legacy {
                    kind,
                    width: 1.,
                    height: 2.1,
                    sill: 0.,
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let id = opening.id();
        let tag = OpeningTag::new(
            "core.opening_tag",
            OpeningTagParams {
                view,
                opening: id,
                position: Point2::new(1.5, -1.),
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
                orphan_hint: Point2::new(1.5, -0.4),
            },
        );
        editor
            .document
            .execute(
                "Door and annotations",
                vec![
                    Command::AddOpening(opening),
                    Command::SetElementLifecycle {
                        element: id,
                        lifecycle: ElementLifecycle {
                            created_in: phases[1],
                            demolished_in: Some(phases[1]),
                        },
                    },
                    Command::AddOpeningTag(tag),
                    Command::AddDimension(dimension),
                ],
            )
            .unwrap();
        for (phase, filter, aperture, host) in [
            (phases[0], PhaseFilter::ShowAll, false, true),
            (phases[1], PhaseFilter::ShowAll, true, true),
            (phases[1], PhaseFilter::ShowExisting, false, true),
            (phases[1], PhaseFilter::ShowTemporary, false, false),
            (phases[2], PhaseFilter::ShowAll, false, true),
        ] {
            set_phase(&mut editor, view, phase, filter);
            let snapshot = editor.plan_snapshot(view).unwrap();
            let context = snapshot.context;
            let lines = BTreeMap::from([(
                id,
                vec![os_render::plan::PlanLine {
                    entity: id,
                    feature: 0,
                    start: Point2::new(1., 0.),
                    end: Point2::new(2., 0.),
                    role: os_geometry::plan::PlanRole::Projected,
                }],
            )]);
            let drawing = if aperture {
                assert!(
                    snapshot.derive_with_provider_lines(lines).is_err(),
                    "provider cannot replace native opening graphics"
                );
                editor.native_drawing(view).unwrap()
            } else {
                snapshot.derive_with_provider_lines(lines).unwrap()
            };
            assert_eq!(
                drawing
                    .provider_lines(context)
                    .unwrap()
                    .iter()
                    .any(|l| l.entity == id),
                aperture
            );
            assert_eq!(
                drawing.opening_tags(context).unwrap().len(),
                usize::from(aperture)
            );
            assert_eq!(
                drawing.dimensions(context).unwrap().len(),
                usize::from(aperture)
            );
            assert_eq!(
                drawing.pick(context, Point2::new(1.5, 0.)).unwrap(),
                (host && !aperture).then_some(walls[0])
            );
            if aperture {
                assert!(
                    drawing.is_native_line(id),
                    "native opening symbol wins over provider lines"
                );
            }
        }
    }
}

#[test]
fn phase_reference_rejection_is_atomic_and_settings_are_undoable_and_persisted() {
    let (mut editor, view, _, phases) = fixture();
    let before = editor.document.model().clone();
    let history = editor.document.history_stats();
    let revision = editor.document.revision();
    let mut p = before.views[&view].parameters.clone();
    p.plan.as_mut().unwrap().target_phase = Some(Id::new());
    assert!(
        editor
            .document
            .execute(
                "Invalid phase",
                vec![Command::UpdateView {
                    id: view,
                    parameters: p
                }]
            )
            .is_err()
    );
    let mut pinned = before.clone();
    pinned.element_lifecycles.clear();
    let mut doc = Document::from_model(pinned).unwrap();
    assert!(
        doc.execute(
            "Delete referenced phase",
            vec![
                Command::RemovePhase(phases[1]),
                Command::UpdatePhase {
                    id: phases[2],
                    parameters: PhaseParams {
                        name: "Future work".into(),
                        order: 1
                    }
                }
            ]
        )
        .is_err()
    );
    assert_eq!(editor.document.model(), &before);
    assert_eq!(editor.document.history_stats(), history);
    assert_eq!(editor.document.revision(), revision);
    set_phase(&mut editor, view, phases[0], PhaseFilter::ShowExisting);
    let after = editor.document.model().clone();
    assert_eq!(
        editor.document.history_stats().undo_entries,
        history.undo_entries + 1
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.model(), &before);
    editor.redo().unwrap();
    assert_eq!(editor.document.model(), &after);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("phased.osb");
    editor.save(&path).unwrap();
    editor.open(&path).unwrap();
    assert_eq!(editor.document.model(), &after);
}

#[test]
fn phase_every_native_category_room_tags_and_stair_paper_strokes_follow_status() {
    let (mut editor, view, walls, phases) = fixture();
    let mut model = editor.document.model().clone();
    let level = model.views[&view].parameters.level.unwrap();
    let mut upper = model.levels[&level].clone();
    upper.header.id = Id::new();
    upper.parameters.elevation = 3.;
    upper.parameters.name = "Upper".into();
    let upper_id = upper.id();
    model.levels.insert(upper_id, upper);
    let boundary = vec![
        Point2::new(8., 0.),
        Point2::new(12., 0.),
        Point2::new(12., 4.),
        Point2::new(8., 4.),
    ];
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Slab".into(),
            level,
            material: None,
            boundary: boundary.clone(),
            holes: vec![],
            thickness: 0.2,
            top_offset: 0.,
        },
    );
    let roof = Roof::new(
        "core.roof",
        RoofParams {
            name: "Roof".into(),
            level,
            material: None,
            boundary: boundary.clone(),
            holes: vec![],
            thickness: 0.2,
            top_offset: 1.,
            slope_start: Point2::new(8., 0.),
            slope_end: Point2::new(12., 0.),
            rise_per_run: 0.1,
        },
    );
    let ceiling = Ceiling::new(
        "core.ceiling",
        CeilingParams {
            name: "Ceiling".into(),
            level,
            material: None,
            boundary_room: None,
            boundary: boundary.clone(),
            holes: vec![],
            thickness: 0.1,
            elevation_offset: 2.5,
        },
    );
    let column = Column::new(
        "core.column",
        ColumnParams {
            name: "Column".into(),
            level,
            center: Point2::new(15., 0.),
            width: 0.4,
            depth: 0.4,
            height: 3.,
            base_offset: 0.,
            material: None,
        },
    );
    let stair = Stair::new(
        "core.stair",
        StairParams {
            name: "Stair".into(),
            lower_level: level,
            upper_level: upper_id,
            start: Point2::new(16., 0.),
            end: Point2::new(20., 0.),
            width: 1.,
            riser_count: 12,
            structural_thickness: 0.15,
            material: None,
        },
    );
    let separator = RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level,
            start: Point2::new(8., 6.),
            end: Point2::new(12., 6.),
        },
    );
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "P1".into(),
            name: "Phase room".into(),
            level,
            seed: Point2::new(9., 1.),
            boundary_signature: vec![(walls[0], true), (walls[2], true), (walls[3], true)],
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
        },
    );
    let tag = RoomTag::new(
        "core.room_tag",
        RoomTagParams {
            view,
            room: room.id(),
            position: Point2::new(9., 1.),
        },
    );
    let ids = [
        floor.id(),
        roof.id(),
        ceiling.id(),
        column.id(),
        stair.id(),
        separator.id(),
        room.id(),
    ];
    model.floors.insert(floor.id(), floor);
    model.roofs.insert(roof.id(), roof);
    model.ceilings.insert(ceiling.id(), ceiling);
    model.columns.insert(column.id(), column);
    model.stairs.insert(stair.id(), stair);
    model
        .room_separation_lines
        .insert(separator.id(), separator);
    model.rooms.insert(room.id(), room);
    model.room_tags.insert(tag.id(), tag);
    // Six lifetimes exercise every native category against each filter. Geometry
    // remains the same; room enclosure intentionally uses the full-model boundary.
    let lifetimes = [
        (0, None),
        (1, None),
        (0, Some(1)),
        (1, Some(1)),
        (2, None),
        (0, Some(0)),
    ];
    for (index, (created, demolished)) in lifetimes.into_iter().enumerate() {
        for id in ids {
            model.element_lifecycles.insert(
                id,
                ElementLifecycle {
                    created_in: phases[created],
                    demolished_in: demolished.map(|i| phases[i]),
                },
            );
        }
        editor.document = Document::from_model(model.clone()).unwrap();
        for (filter_index, filter) in [
            PhaseFilter::ShowAll,
            PhaseFilter::ShowExisting,
            PhaseFilter::ShowNew,
            PhaseFilter::ShowDemolished,
            PhaseFilter::ShowTemporary,
        ]
        .into_iter()
        .enumerate()
        {
            set_phase(&mut editor, view, phases[1], filter);
            let visible = index < 4 && (filter_index == 0 || index + 1 == filter_index);
            let s = editor.plan_snapshot(view).unwrap();
            for count in [
                s.floors.len(),
                s.roofs.len(),
                s.columns.len(),
                s.stairs.len(),
                s.rooms.len(),
                s.room_separation_lines.len(),
                s.room_tags.len(),
            ] {
                assert_eq!(count, usize::from(visible), "status {index}, {filter:?}");
            }
            assert_eq!(
                s.room_segments,
                model.room_boundary_segments(level).unwrap(),
                "room topology is not rebuilt for phases"
            );
            let c = s.context;
            let d = s.derive().unwrap();
            assert_eq!(d.stairs(c).unwrap().len(), usize::from(visible));
            if visible {
                let stair = &d.stairs(c).unwrap()[0];
                assert!(!stair.footprints.is_empty() && !stair.lines.is_empty());
                for line in stair.visible_strokes() {
                    let appearance = d.appearance(c, stair.entity, line.role).unwrap().unwrap();
                    assert_eq!(appearance.dashed, index >= 2);
                }
                let paper = os_render::sheet::compose_view_sheet(
                    os_render::sheet::PaperSheetInfo {
                        width_mm: 420.,
                        height_mm: 297.,
                        number: "P1",
                        name: "Phases",
                    },
                    "Plan",
                    os_render::sheet::PaperViewport {
                        center_mm: Point2::new(210., 140.),
                        width_mm: 380.,
                        height_mm: 230.,
                        model_center_m: Point2::new(10., 3.),
                        scale_denominator: 100.,
                    },
                    c,
                    &d,
                )
                .unwrap();
                let projected = d
                    .appearance(c, stair.entity, os_geometry::plan::PlanRole::Projected)
                    .unwrap()
                    .unwrap();
                // Arrow endpoints prove these are stair marks, not nearby wall marks.
                let arrow = &stair.lines[0];
                let viewport = os_render::sheet::PaperViewport {
                    center_mm: Point2::new(210., 140.),
                    width_mm: 380.,
                    height_mm: 230.,
                    model_center_m: Point2::new(10., 3.),
                    scale_denominator: 100.,
                };
                let a = viewport.model_to_paper(arrow.start).unwrap();
                let b = viewport.model_to_paper(arrow.end).unwrap();
                assert!(paper.marks().iter().any(|m| matches!(&m.kind,os_render::sheet::PaperMarkKind::Path {points_mm,stroke:Some(s),..}
                    if points_mm==&vec![a,b] && [s.color.red,s.color.green,s.color.blue]==projected.color && s.dashed==projected.dashed)));
                paper.to_pdf().unwrap();
            }
            let mut p = editor.document.model().views[&view].parameters.clone();
            let old = p.plan.unwrap();
            let mut rcp = PlanSettings::reflected_ceiling();
            rcp.target_phase = old.target_phase;
            rcp.phase_filter = old.phase_filter;
            p.plan = Some(rcp);
            editor
                .command(
                    "RCP",
                    Command::UpdateView {
                        id: view,
                        parameters: p,
                    },
                )
                .unwrap();
            let s = editor.plan_snapshot(view).unwrap();
            assert_eq!(s.ceilings.len(), usize::from(visible));
            let c = s.context;
            let d = s.derive().unwrap();
            if visible {
                assert!(
                    d.appearance(c, ids[2], os_geometry::plan::PlanRole::Projected)
                        .unwrap()
                        .is_some()
                );
            }
            // Restore the floor range before the next filter.
            let mut p = editor.document.model().views[&view].parameters.clone();
            p.plan = Some(old);
            editor
                .command(
                    "Floor plan",
                    Command::UpdateView {
                        id: view,
                        parameters: p,
                    },
                )
                .unwrap();
        }
    }
}
