//! Real egui dialog, canvas and history frames at both acceptance profiles.
use super::*;

fn fixture(
    profile: usize,
    kind: OpeningKind,
    legacy: bool,
    arc: f64,
    batch: bool,
) -> (Harness, Id, Id, Vec<Id>) {
    let (size, scale) = PROFILES[profile];
    let (mut h, id) = Harness::movable(kind, false, legacy, size, scale);
    let mut model = h.app.editor.document.model().clone();
    model
        .openings
        .get_mut(&id)
        .unwrap()
        .header
        .properties
        .insert("catalog-note".into(), serde_json::json!("Keep me"));
    if arc != 0. {
        model.walls.get_mut(&h.wall).unwrap().parameters.path = os_model::WallPath::CircularArc {
            center: Point2::default(),
            radius: 4.,
            start_angle_rad: arc * 5.8,
            signed_sweep_rad: arc * 4.7,
        };
    }
    let source_type = *model.opening_types.keys().next().unwrap();
    let mut parameters = model.opening_types[&source_type].parameters.clone();
    parameters.name = "Destination type".into();
    parameters.width = 1.45;
    parameters.height = 1.7;
    parameters.sill = if kind == OpeningKind::Window { 0.6 } else { 0. };
    parameters.family.frame_width = 0.04;
    let ty = OpeningType::new("core.opening_type", parameters);
    let destination = ty.id();
    model.opening_types.insert(destination, ty);
    let mut ids = vec![id];
    if batch {
        let mut p = model.openings[&id].parameters.clone();
        p.offset = 4.5;
        // Typed pins must survive while the first instance inherits new dimensions.
        if !legacy {
            p.width_override = Some(1.1);
            p.height_override = Some(1.6);
            p.sill_override = (kind == OpeningKind::Window).then_some(0.7);
            p.pane_position_override =
                (kind == OpeningKind::Window).then_some(os_model::WindowPanePosition::RightFace);
        }
        let opening = Opening::new("core.opening", p);
        ids.push(opening.id());
        model.openings.insert(opening.id(), opening);
    }
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app
        .editor
        .pending_geometry
        .extend(ids.iter().copied().chain([h.wall]));
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select_area(ids.iter().copied().collect());
    h.settle();
    h.app.plans.split = true;
    h.app.plans.cameras.insert(
        h.view,
        PlanCamera {
            center: Point2::default(),
            pixels_per_metre: 22.,
        },
    );
    h.frame(vec![]);
    (h, source_type, destination, ids)
}

fn choose(h: &mut Harness, destination: Id) {
    let name = opening_type_assignment::type_label(
        &h.app.editor.document.model().opening_types[&destination],
    );
    let current = h
        .app
        .plans
        .opening_type_assignment
        .as_ref()
        .unwrap()
        .destination;
    if current != destination {
        let current_name = opening_type_assignment::type_label(
            &h.app.editor.document.model().opening_types[&current],
        );
        h.text_click(&current_name);
        h.text_click(&name);
    }
    h.frame(vec![]);
    assert_eq!(
        h.app
            .plans
            .opening_type_assignment
            .as_ref()
            .unwrap()
            .destination,
        destination
    );
}

fn begin(h: &mut Harness) {
    h.ribbon_click("Architecture", 30.0..58.0);
    h.text_click("Change opening type…");
    assert!(
        h.app.plans.opening_type_assignment.is_some(),
        "{}",
        h.app.status
    );
}

#[test]
fn paired_doors_assignment_explicit_reset_atomic_history_both_dpis() {
    for profile in 0..2 {
        let (mut h, source, destination, ids) =
            fixture(profile, OpeningKind::Door, false, 0., true);
        let mut model = h.app.editor.document.model().clone();
        model
            .opening_types
            .get_mut(&source)
            .unwrap()
            .parameters
            .family
            .door_leaves = os_model::DoorLeaves::Paired {
            active_fraction: 0.6,
        };
        for id in &ids {
            model.openings.get_mut(id).unwrap().parameters.open_state =
                os_model::OpeningState::DoorPairAngles {
                    active_degrees: 30.,
                    inactive_degrees: 60.,
                };
        }
        h.app.editor.document = Document::from_model(model).unwrap();
        h.app.editor.pending_geometry.extend(ids.iter().copied());
        h.app.editor.regenerate().unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.settle();
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        begin(&mut h);
        choose(&mut h, destination);
        assert!(
            h.app
                .plans
                .opening_type_assignment
                .as_ref()
                .unwrap()
                .preview
                .is_err()
        );
        h.text_click("Reset paired poses to Default");
        assert!(
            h.app
                .plans
                .opening_type_assignment
                .as_ref()
                .unwrap()
                .preview
                .is_ok()
        );
        assert_eq!(h.app.editor.document.model(), &before);
        h.text_click("Apply type");
        h.settle();
        let after = h.app.editor.document.model().clone();
        for id in &ids {
            assert_eq!(after.openings[id].parameters.type_id(), Some(destination));
            assert_eq!(
                after.openings[id].parameters.open_state,
                os_model::OpeningState::Default
            );
            assert_eq!(
                after.openings[id].parameters.width_override,
                before.openings[id].parameters.width_override
            );
        }
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &after);
    }
}

#[test]
fn opening_type_assignment_egui_preview_commit_history_straight_and_signed_arcs() {
    for profile in 0..2 {
        for kind in KINDS {
            for (legacy, arc, batch) in [(false, 0., false), (false, 1., true), (true, -1., true)] {
                let (mut h, _, destination, ids) = fixture(profile, kind, legacy, arc, batch);
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let history = h.app.editor.document.history_stats();
                let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
                let selection = h.app.selected_ids.clone();
                begin(&mut h);
                choose(&mut h, destination);
                assert!(h.has_text(
                    "Legacy openings keep their width, height and window sill as pinned overrides."
                ));
                let preview = h
                    .app
                    .plans
                    .opening_type_assignment
                    .as_ref()
                    .unwrap()
                    .preview
                    .as_ref()
                    .unwrap();
                assert_eq!(preview.commands.len(), ids.len());
                let context = h.app.editor.native_plan_context(h.view).unwrap();
                let preview_items = preview.drawing.items(context).unwrap().to_vec();
                let preview_lines = preview.drawing.provider_lines(context).unwrap().to_vec();
                assert!(!preview_items.is_empty());
                let rect = h.app.plans.canvas_rect.unwrap();
                let camera = h.app.plans.cameras[&h.view];
                let points: Vec<_> = preview_items[0]
                    .footprint
                    .vertices()
                    .iter()
                    .map(|p| {
                        let p = camera
                            .project(*p, [rect.width() as f64, rect.height() as f64])
                            .unwrap();
                        rect.min + egui::vec2(p.x as f32, p.y as f32)
                    })
                    .collect();
                assert!(h.output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Path(path) if path.closed && path.points == points)), "candidate aperture must be painted");
                assert!(
                    ids.iter()
                        .all(|id| preview_lines.iter().any(|l| l.entity == *id))
                );
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
                h.text_click("Apply type");
                h.settle();
                assert!(h.app.plans.opening_type_assignment.is_none());
                let after = h.app.editor.document.model().clone();
                let mut expected = before.clone();
                for &id in &ids {
                    let original = &before.openings[&id].parameters;
                    let resolved = before.resolve_opening(original).unwrap();
                    let p = &mut expected.openings.get_mut(&id).unwrap().parameters;
                    p.definition = OpeningDefinition::Typed {
                        type_id: destination,
                    };
                    if legacy {
                        p.width_override = Some(resolved.width);
                        p.height_override = Some(resolved.height);
                        p.sill_override = (kind == OpeningKind::Window).then_some(resolved.sill);
                    }
                    let now = after
                        .resolve_opening(&after.openings[&id].parameters)
                        .unwrap();
                    assert_eq!(
                        now.width,
                        if legacy {
                            resolved.width
                        } else {
                            original.width_override.unwrap_or(1.45)
                        }
                    );
                    assert_eq!(
                        now.height,
                        if legacy {
                            resolved.height
                        } else {
                            original.height_override.unwrap_or(1.7)
                        }
                    );
                    assert_eq!(
                        h.app.editor.scene[&id],
                        crate::opening_tools::panel_mesh(&after, id).unwrap()
                    );
                }
                assert_eq!(after, expected);
                assert_eq!(h.app.selected_ids, selection);
                assert_eq!(
                    h.app.editor.document.history_stats().undo_entries,
                    history.undo_entries + 1
                );
                assert_eq!(
                    h.app.editor.scene[&h.wall],
                    crate::opening_tools::host_mesh(&after, h.wall).unwrap()
                );
                assert_ne!(h.app.editor.scene, scene);
                let committed = h.app.editor.native_wall_plan(h.view).unwrap();
                let context = h.app.editor.native_plan_context(h.view).unwrap();
                assert_eq!(committed.items(context).unwrap(), preview_items);
                for line in preview_lines {
                    assert!(committed.provider_lines(context).unwrap().contains(&line));
                }
                let after_scene = h.app.editor.scene.clone();
                h.app.history(false);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.selected_ids, selection);
                h.app.history(true);
                h.settle();
                assert_eq!(h.app.editor.document.model(), &after);
                assert_eq!(h.app.editor.scene, after_scene);
                assert_eq!(h.app.selected_ids, selection);
                begin(&mut h);
                choose(&mut h, destination);
                let history = h.app.editor.document.history_stats();
                h.text_click("Apply type");
                h.settle();
                assert_eq!(h.app.editor.document.history_stats(), history);
                assert_eq!(h.app.editor.document.model(), &after);
            }
        }
    }
}

#[test]
fn opening_type_assignment_batch_rejection_and_stale_cancellation() {
    for profile in 0..2 {
        for reason in [
            "overlap",
            "head",
            "kind",
            "cancel",
            "escape",
            "selection",
            "revision",
            "session",
            "view",
            "drawing",
            "provider",
        ] {
            let (mut h, _, destination, ids) =
                fixture(profile, OpeningKind::Window, false, 0., true);
            if matches!(reason, "overlap" | "head" | "kind") {
                let mut model = h.app.editor.document.model().clone();
                let ty = &mut model
                    .opening_types
                    .get_mut(&destination)
                    .unwrap()
                    .parameters;
                match reason {
                    "overlap" => ty.width = 3.1,
                    "head" => ty.height = 2.9,
                    "kind" => {
                        ty.kind = OpeningKind::Door;
                        ty.sill = 0.;
                    }
                    _ => unreachable!(),
                }
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.settle();
            }
            begin(&mut h);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            if matches!(reason, "overlap" | "head" | "kind") {
                if reason == "kind" {
                    h.app
                        .plans
                        .opening_type_assignment
                        .as_mut()
                        .unwrap()
                        .destination = destination;
                } else {
                    choose(&mut h, destination);
                }
                assert!(h.app.apply_opening_type_assignment().is_err());
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.editor.document.history_stats(), history);
                continue;
            }
            match reason {
                "cancel" => h.text_click("Cancel type change"),
                "escape" => h.frame(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }]),
                "selection" => h.app.select(Some(ids[0])),
                "revision" => {
                    let mut p = before.openings[&ids[0]].parameters.clone();
                    p.name = "Intervening name".into();
                    h.app
                        .editor
                        .document
                        .execute(
                            "Intervening edit",
                            vec![Command::UpdateOpening {
                                id: ids[0],
                                parameters: p,
                            }],
                        )
                        .unwrap();
                }
                "session" => h.app.editor.document = Document::from_model(before.clone()).unwrap(),
                "view" => h.app.plans.active = None,
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                _ => unreachable!(),
            }
            h.frame(vec![]);
            assert!(h.app.plans.opening_type_assignment.is_none(), "{reason}");
            let mut expected = before.clone();
            if reason == "revision" {
                expected.openings.get_mut(&ids[0]).unwrap().parameters.name =
                    "Intervening name".into();
            }
            assert_eq!(h.app.editor.document.model(), &expected, "{reason}");
            assert_eq!(h.app.editor.scene, scene, "{reason}");
        }
    }
}

#[test]
fn opening_type_assignment_rejects_non_opening_mixed_hidden_and_unbounded_selection() {
    for profile in 0..2 {
        for reason in ["empty", "large", "wall", "mixed", "hidden", "crop", "level"] {
            let (mut h, _, _, ids) = fixture(profile, OpeningKind::Window, true, 0., true);
            let mut model = h.app.editor.document.model().clone();
            match reason {
                "empty" => h.app.select(None),
                "large" => h.app.selected_ids = (0..257).map(|_| Id::new()).collect(),
                "wall" => {
                    h.app.selected_ids.insert(h.wall);
                }
                "mixed" => {
                    model
                        .openings
                        .get_mut(&ids[1])
                        .unwrap()
                        .parameters
                        .definition = OpeningDefinition::Legacy {
                        kind: OpeningKind::Door,
                        width: 1.,
                        height: 2.,
                        sill: 0.,
                    };
                }
                "hidden" => {
                    model
                        .views
                        .get_mut(&h.view)
                        .unwrap()
                        .parameters
                        .plan
                        .as_mut()
                        .unwrap()
                        .visibility
                        .windows = false
                }
                "crop" => {
                    model
                        .views
                        .get_mut(&h.view)
                        .unwrap()
                        .parameters
                        .plan
                        .as_mut()
                        .unwrap()
                        .crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(-4., -3.),
                        max: Point2::new(-0.5, 0.),
                    });
                }
                "level" => {
                    let level = os_model::Level::new(
                        "core.level",
                        os_model::LevelParams {
                            building: model.levels[&h.app.active_level].parameters.building,
                            name: "Other".into(),
                            elevation: 0.,
                        },
                    );
                    model.walls.get_mut(&h.wall).unwrap().parameters.level = level.id();
                    model.levels.insert(level.id(), level);
                }
                _ => unreachable!(),
            }
            h.app.editor.document = Document::from_model(model).unwrap();
            h.app.plans.poll(&h.app.editor);
            h.app.focus_plan(Some(h.view));
            h.settle();
            if reason == "crop" {
                // One member remains visible: rejecting the whole selection must
                // not silently turn this into a single-opening assignment.
                h.app.select_area(ids.iter().copied().collect());
                let context = h.app.editor.native_plan_context(h.view).unwrap();
                let lines = h
                    .app
                    .plans
                    .drawing
                    .as_ref()
                    .unwrap()
                    .provider_lines(context)
                    .unwrap();
                assert!(lines.iter().any(|line| line.entity == ids[0]));
                assert!(!lines.iter().any(|line| line.entity == ids[1]));
                assert_eq!(h.app.selected_ids.len(), 2);
            }
            assert!(
                opening_type_assignment::Draft::begin(&h.app).is_err(),
                "{reason}"
            );
            assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        }
    }
}

#[test]
fn opening_type_assignment_marquee_duplicate_names_annotations_and_storage() {
    for profile in 0..2 {
        let (mut h, source, destination, ids) =
            fixture(profile, OpeningKind::Window, false, 0., true);
        let mut model = h.app.editor.document.model().clone();
        // Both labels intentionally have the same name; UUID chooses the assignment.
        model
            .opening_types
            .get_mut(&source)
            .unwrap()
            .parameters
            .name = "Same name".into();
        model
            .opening_types
            .get_mut(&destination)
            .unwrap()
            .parameters
            .name = "Same name".into();
        let tag = os_model::OpeningTag::new(
            "core.opening_tag",
            os_model::OpeningTagParams {
                opening: ids[0],
                view: h.view,
                position: Point2::new(0., 1.),
                label_preset: os_model::OpeningTagLabelPreset::TypeAndDimensions,
            },
        );
        let tag_id = tag.id();
        model.opening_tags.insert(tag_id, tag);
        let dimension = os_model::Dimension::new(
            "core.dimension",
            os_model::DimensionParams {
                view: h.view,
                layout: os_model::DimensionLayout::Aligned,
                first: os_model::DimensionReference::OpeningJamb {
                    opening: ids[0],
                    jamb: os_model::DimensionJamb::Start,
                },
                second: os_model::DimensionReference::OpeningJamb {
                    opening: ids[0],
                    jamb: os_model::DimensionJamb::End,
                },
                additional: vec![],
                baseline_spacing_m: 0.25,
                offset_m: 1.,
                orphan_hint: Point2::default(),
            },
        );
        let dimension_id = dimension.id();
        model.dimensions.insert(dimension_id, dimension);
        h.app.editor.document = Document::from_model(model.clone()).unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.settle();
        h.app.plans.cameras.insert(
            h.view,
            PlanCamera {
                center: Point2::default(),
                pixels_per_metre: 20.,
            },
        );
        h.app.select(None);
        for category in selection_filters::Category::ALL {
            h.app
                .plans
                .selection_filters
                .set_enabled(category, category == selection_filters::Category::Window);
        }
        h.frame(vec![]);
        let a = h.point(Point2::new(-3., 2.));
        let b = h.point(Point2::new(3., -3.));
        h.hover(a);
        h.frame(vec![egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
        }]);
        h.hover(b);
        h.frame(vec![button(b, false)]);
        h.frame(vec![]);
        assert_eq!(h.app.selected_ids, ids.iter().copied().collect());
        begin(&mut h);
        choose(&mut h, destination);
        let camera = h.app.plans.cameras[&h.view];
        // The dialog owns the canvas while its candidate is visible.
        let rect = h.app.plans.canvas_rect.unwrap();
        h.drag(
            rect.right_bottom() - egui::vec2(35., 40.),
            rect.right_bottom() - egui::vec2(60., 75.),
        );
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        assert_eq!(h.app.editor.document.model(), &model);
        h.text_click("Apply type");
        h.settle();
        let after = h.app.editor.document.model();
        assert!(
            ids.iter()
                .all(|id| after.openings[id].parameters.type_id() == Some(destination))
        );
        assert_eq!(after.opening_tags, model.opening_tags);
        assert_eq!(after.dimensions, model.dimensions);
        assert_ne!(
            after.opening_tags[&tag_id].parameters.label(after),
            model.opening_tags[&tag_id].parameters.label(&model)
        );
        assert!(
            (after.dimensions[&dimension_id]
                .parameters
                .resolve(after)
                .unwrap()
                .length_metres
                - 1.45)
                .abs()
                < 1e-9
        );
        let path = std::env::temp_dir().join(format!("opening-type-assignment-{}.os", Id::new()));
        ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
        let loaded = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(loaded.model(), after);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn opening_type_assignment_accepts_256_across_hosts_and_commits_only_changes() {
    let (mut h, _, destination, ids) = fixture(0, OpeningKind::Window, false, 0., false);
    let mut model = h.app.editor.document.model().clone();
    model
        .opening_types
        .get_mut(&destination)
        .unwrap()
        .parameters
        .family
        .frame_width = 0.;
    let original = model.openings[&ids[0]].parameters.clone();
    model.openings.clear();
    model.walls.get_mut(&h.wall).unwrap().parameters.path = os_model::WallPath::Straight {
        start: Point2::new(0., 0.),
        end: Point2::new(20., 0.),
    };
    // Distribute the batch within the existing per-plan geometry budget.
    let mut hosts = vec![h.wall];
    for index in 1..32 {
        let mut other = model.walls[&h.wall].parameters.clone();
        other.path = os_model::WallPath::Straight {
            start: Point2::new(0., index as f64 * 3.),
            end: Point2::new(20., index as f64 * 3.),
        };
        let other = os_model::Wall::new(os_walls::WALL_TYPE, other);
        hosts.push(other.id());
        model.walls.insert(other.id(), other);
    }
    for &host in &hosts {
        for index in 0..8 {
            let mut p = original.clone();
            p.host = host;
            p.offset = 1. + index as f64 * 2.;
            if index == 0 {
                p.definition = OpeningDefinition::Typed {
                    type_id: destination,
                };
            }
            let opening = Opening::new("core.opening", p);
            model.openings.insert(opening.id(), opening);
        }
    }
    h.app.editor.document = Document::from_model(model.clone()).unwrap();
    h.app
        .editor
        .pending_geometry
        .extend(model.openings.keys().copied().chain(hosts.iter().copied()));
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select_area(model.openings.keys().copied().collect());
    h.app
        .editor
        .native_wall_plan(h.view)
        .expect("256-opening native plan");
    h.settle();
    let mut draft = opening_type_assignment::Draft::begin(&h.app).unwrap();
    draft.destination = destination;
    let candidate = draft.candidate(&model).unwrap();
    assert_eq!(candidate.commands.len(), 224);
    assert_eq!(candidate.hosts, hosts.iter().copied().collect());
    h.app.plans.opening_type_assignment = Some(draft);
    h.app.apply_opening_type_assignment().unwrap();
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
    assert_eq!(h.app.selected_ids.len(), 256);
    for host in hosts {
        assert_eq!(
            h.app.editor.scene[&host],
            crate::opening_tools::host_mesh(h.app.editor.document.model(), host).unwrap()
        );
    }
    h.app.history(false);
    assert_eq!(h.app.editor.document.model(), &model);
    assert_eq!(h.app.selected_ids.len(), 256);
}
