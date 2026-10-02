//! Headless real-egui desktop frames; no native-window inspection implied.
use super::*;
#[path = "arc_tests.rs"]
mod arc_tests;
#[path = "crop_tests.rs"]
mod crop_tests;
#[path = "face_tests.rs"]
mod face_tests;
#[path = "junction_tests.rs"]
mod junction_tests;
#[path = "split_tests.rs"]
mod split_tests;
#[path = "wall_set_copy_tests.rs"]
mod wall_set_copy_tests;
#[path = "wall_set_move_tests.rs"]
mod wall_set_move_tests;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

fn dimension_repair_fixture(
    h: &mut Harness,
    layout: os_model::DimensionLayout,
) -> (Id, usize, DimensionReference, Point2) {
    use os_model::DimensionLayout::*;
    let mut model = h.app.editor.document.model().clone();
    let mut wall = model.walls[&h.wall].parameters.clone();
    *wall.path.straight_start_mut().unwrap() = if layout == Angular {
        Point2::new(0.0, -1.0)
    } else {
        Point2::new(2.0, 0.0)
    };
    *wall.path.straight_end_mut().unwrap() = if layout == Angular {
        Point2::new(0.0, 1.0)
    } else {
        Point2::new(3.0, 0.0)
    };
    let wall = os_model::Wall::new(os_walls::WALL_TYPE, wall);
    let other = wall.id();
    model.walls.insert(other, wall);
    let reference = DimensionReference::WallEndpoint {
        wall: h.wall,
        endpoint: DimensionEndpoint::Start,
    };
    let mut parameters = DimensionParams {
        layout,
        view: h.view,
        first: reference,
        second: DimensionReference::WallEndpoint {
            endpoint: DimensionEndpoint::End,
            wall: reference.entity(),
        },
        additional: Vec::new(),
        baseline_spacing_m: if layout == Angular { 0.25 } else { 0.37 },
        offset_m: if layout == Angular { 0.4 } else { -0.4 },
        orphan_hint: Point2::new(0.0, 0.6),
    };
    let (anchor, target, point) = match layout {
        Aligned => (1, parameters.second, Point2::new(1.0, 0.0)),
        Chain | Baseline => {
            let target = DimensionReference::WallEndpoint {
                wall: other,
                endpoint: DimensionEndpoint::Start,
            };
            parameters.additional.push(target);
            (2, target, Point2::new(2.0, 0.0))
        }
        Angular => {
            parameters.second = DimensionReference::WallEndpoint {
                wall: other,
                endpoint: DimensionEndpoint::Start,
            };
            (1, parameters.second, Point2::new(0.0, 0.7))
        }
    };
    parameters.validate_creation(&model).unwrap();
    match anchor {
        1 => {
            parameters.second = DimensionReference::WallEndpoint {
                wall: Id::new(),
                endpoint: parameters.second.wall_endpoint().unwrap().1,
            }
        }
        2 => {
            parameters.additional[0] = DimensionReference::WallEndpoint {
                wall: Id::new(),
                endpoint: parameters.additional[0].wall_endpoint().unwrap().1,
            }
        }
        _ => unreachable!(),
    }
    let dimension = os_model::Dimension::new("core.dimension", parameters);
    let id = dimension.id();
    model.dimensions.insert(id, dimension);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(id));
    h.settle();
    (id, anchor, target, point)
}

#[test]
fn dimension_repair_properties_preview_commit_and_history_at_both_dpis() {
    use os_model::DimensionLayout::*;
    for (size, scale) in PROFILES {
        for layout in [Aligned, Chain, Baseline, Angular] {
            let mut h = Harness::new(size, scale);
            let (id, anchor, target, point) = dimension_repair_fixture(&mut h, layout);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let revision = h.app.editor.document.revision();
            let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
            h.click_text_in_band(&format!("Replace anchor {}", anchor + 1), 0.0, h.size.y);
            h.frame(vec![]);
            assert!(
                h.app
                    .plans
                    .dimension_draft
                    .as_ref()
                    .unwrap()
                    .repair
                    .is_some()
            );
            let pointer = h.point(point);
            h.frame(vec![egui::Event::PointerMoved(pointer)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.revision(), revision);
            assert!(!h.app.editor.document.can_undo());
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
            assert_eq!(h.app.selected, Some(id));
            assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::LineSegment { stroke, .. } if stroke.width == 1.8 && stroke.color == theme::ACCENT)), "{layout:?}");
            assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::Text(t) if t.galley.job.text.contains(if layout == Angular { "90.00" } else { "m" }) && t.galley.job.sections.iter().any(|s| s.format.color == theme::ACCENT))));
            let mut expected = before.clone();
            let params = &mut expected.dimensions.get_mut(&id).unwrap().parameters;
            if anchor == 1 {
                params.second = target;
            } else {
                params.additional[anchor - 2] = target;
            }
            params.validate_creation(&before).unwrap();
            h.click(pointer);
            h.settle();
            assert!(!h.app.status_error, "{}", h.app.status);
            h.clean();
            assert_eq!(h.app.editor.document.model(), &expected);
            assert_eq!(h.app.editor.document.revision(), revision + 1);
            assert_eq!(h.app.selected, Some(id));
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
            h.settle();
            h.app.begin_dimension_repair(anchor);
            h.frame(vec![]);
            h.frame(vec![egui::Event::PointerMoved(h.point(point))]);
            assert!(h.app.editor.document.can_redo());
            assert_eq!(h.app.editor.document.model(), &before);
            h.frame(vec![escape()]);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &expected);
        }
    }
}

#[test]
fn dimension_repair_cancel_and_stale_context_at_both_dpis() {
    for (size, scale) in PROFILES {
        for cause in [
            "escape",
            "session",
            "revision",
            "selection",
            "selection set",
            "view",
            "provider",
            "drawing",
            "missing drawing",
            "pointer gone",
        ] {
            let mut h = Harness::new(size, scale);
            let (_, anchor, _, point) =
                dimension_repair_fixture(&mut h, os_model::DimensionLayout::Aligned);
            let before = h.app.editor.document.model().clone();
            h.app.begin_dimension_repair(anchor);
            h.frame(vec![]);
            let pointer = h.point(point);
            h.press(pointer);
            match cause {
                "escape" => h.frame(vec![escape()]),
                "pointer gone" => h.frame(vec![egui::Event::PointerGone]),
                "session" => h.app.editor.document = Document::from_model(before.clone()).unwrap(),
                "revision" => {
                    h.app
                        .editor
                        .command("Concurrent edit", Command::RenameProject("Changed".into()))
                        .unwrap();
                }
                "selection" => h.app.select(None),
                "selection set" => {
                    h.app.selected_ids.insert(h.wall);
                }
                "view" => h.app.focus_plan(None),
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                "missing drawing" => h.app.plans.drawing = None,
                _ => unreachable!(),
            }
            h.frame(vec![]);
            assert!(h.app.plans.dimension_draft.is_none(), "{cause}");
            h.release(pointer);
            h.frame(vec![]);
            assert!(!h.app.plans.dimension_repair_claimed, "{cause}");
            if cause == "revision" {
                h.app.history(false);
            }
            assert_eq!(h.app.editor.document.model(), &before, "{cause}");
            assert!(!h.app.editor.document.can_undo(), "{cause}");
        }
    }
}

#[test]
fn dimension_repair_first_anchor_rotated_plan_and_repeated_replacement() {
    use os_model::{DimensionLayout::*, PlanViewBasis};
    for (size, scale) in PROFILES {
        for layout in [Aligned, Chain, Baseline, Angular] {
            let mut h = Harness::new(size, scale);
            let (id, broken, target, _) = dimension_repair_fixture(&mut h, layout);
            let mut model = h.app.editor.document.model().clone();
            let p = &mut model.dimensions.get_mut(&id).unwrap().parameters;
            if broken == 1 {
                p.second = target;
            } else {
                p.additional[broken - 2] = target;
            }
            let first = p.first;
            p.first = DimensionReference::WallEndpoint {
                wall: Id::new(),
                endpoint: p.first.wall_endpoint().unwrap().1,
            };
            model
                .views
                .get_mut(&h.view)
                .unwrap()
                .parameters
                .plan
                .as_mut()
                .unwrap()
                .basis = PlanViewBasis {
                origin: Point2::new(0.2, 0.3),
                rotation: 0.63,
            };
            h.app.editor.document = Document::from_model(model).unwrap();
            h.app.editor.regenerate().unwrap();
            h.app.plans.poll(&h.app.editor);
            h.app.focus_plan(Some(h.view));
            h.app.select(Some(id));
            h.settle();
            let before = h.app.editor.document.model().clone();
            h.app.begin_dimension_repair(0);
            h.frame(vec![]);
            h.click(h.point(Point2::new(
                if layout == Angular { -0.7 } else { -1.0 },
                0.0,
            )));
            h.settle();
            assert!(!h.app.status_error, "{}", h.app.status);
            let mut expected = before.clone();
            expected.dimensions.get_mut(&id).unwrap().parameters.first = first;
            assert_eq!(h.app.editor.document.model(), &expected);
            assert_eq!(h.app.editor.document.revision(), 1);
            if layout == Angular {
                continue;
            }
            // Replacing an already resolved reference is deliberate too. Reuse a
            // geometrically equivalent wall, keeping every other parameter.
            let copy = os_model::Wall::new(
                os_walls::WALL_TYPE,
                expected.walls[&h.wall].parameters.clone(),
            );
            let copy_id = copy.id();
            h.app
                .editor
                .command("Fixture duplicate wall", Command::AddWall(copy))
                .unwrap();
            h.settle();
            // Identical visible endpoints have a stable UUID tie-break. Replace
            // whichever wall is not currently preferred by that picker.
            let preferred = h.wall.min(copy_id);
            let other = h.wall.max(copy_id);
            let mut p = h.app.editor.document.model().dimensions[&id]
                .parameters
                .clone();
            p.first = DimensionReference::WallEndpoint {
                wall: other,
                endpoint: p.first.wall_endpoint().unwrap().1,
            };
            h.app
                .editor
                .command(
                    "Fixture alternate reference",
                    Command::UpdateDimension { id, parameters: p },
                )
                .unwrap();
            h.settle();
            let before_second = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.app.begin_dimension_repair(0);
            h.frame(vec![]);
            h.click(h.point(Point2::new(-1.0, 0.0)));
            h.settle();
            assert!(!h.app.status_error, "{}", h.app.status);
            let mut expected_second = before_second.clone();
            expected_second
                .dimensions
                .get_mut(&id)
                .unwrap()
                .parameters
                .first = DimensionReference::WallEndpoint {
                wall: preferred,
                endpoint: DimensionEndpoint::Start,
            };
            assert_eq!(h.app.editor.document.model(), &expected_second);
            assert_eq!(h.app.editor.document.revision(), revision + 1);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before_second);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &expected_second);
        }
    }
}

#[test]
fn dimension_repair_invalid_targets_leave_model_and_history_unchanged() {
    use os_model::DimensionLayout::*;
    for (size, scale) in PROFILES {
        for (layout, case) in [
            (Aligned, "duplicate"),
            (Aligned, "empty"),
            (Aligned, "level"),
            (Aligned, "hidden"),
            (Aligned, "crop"),
            (Chain, "duplicate while missing"),
            (Chain, "order"),
            (Baseline, "off axis"),
            (Angular, "parallel"),
            (Angular, "duplicate"),
        ] {
            let mut h = Harness::new(size, scale);
            let (id, anchor, target, mut point) = dimension_repair_fixture(&mut h, layout);
            let mut model = h.app.editor.document.model().clone();
            match case {
                "duplicate" => point = Point2::new(-1.0, 0.0),
                "empty" => point = Point2::new(0.5, -0.7),
                "duplicate while missing" => {
                    model.dimensions.get_mut(&id).unwrap().parameters.first =
                        DimensionReference::WallEndpoint {
                            wall: Id::new(),
                            endpoint: model
                                .dimensions
                                .get_mut(&id)
                                .unwrap()
                                .parameters
                                .first
                                .wall_endpoint()
                                .unwrap()
                                .1,
                        };
                    point = Point2::new(1.0, 0.0);
                }
                "order" => {
                    model
                        .walls
                        .get_mut(&target.entity())
                        .unwrap()
                        .parameters
                        .path
                        .straight_start_mut()
                        .unwrap()
                        .x = 0.0;
                    point.x = 0.0;
                }
                "off axis" => {
                    model
                        .walls
                        .get_mut(&target.entity())
                        .unwrap()
                        .parameters
                        .path
                        .straight_start_mut()
                        .unwrap()
                        .y = 0.5;
                    point.y = 0.5;
                }
                "parallel" => {
                    let wall = &mut model.walls.get_mut(&target.entity()).unwrap().parameters;
                    *wall.path.straight_start_mut().unwrap() = Point2::new(-1.0, 0.7);
                    *wall.path.straight_end_mut().unwrap() = Point2::new(1.0, 0.7);
                    point = Point2::new(0.0, 0.7);
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
                        .walls = false
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
                        min: Point2::new(-2.0, -1.0),
                        max: Point2::new(0.5, 1.0),
                    })
                }
                "level" => {
                    let mut params = model.levels[&model.walls[&target.entity()].parameters.level]
                        .parameters
                        .clone();
                    params.name = "Other".into();
                    let level = os_model::Level::new("core.level", params);
                    model
                        .walls
                        .get_mut(&target.entity())
                        .unwrap()
                        .parameters
                        .level = level.id();
                    model.levels.insert(level.id(), level);
                }
                _ => unreachable!(),
            }
            h.app.editor.document = Document::from_model(model).unwrap();
            h.app.editor.regenerate().unwrap();
            h.app.plans.poll(&h.app.editor);
            h.app.focus_plan(Some(h.view));
            h.app.select(Some(id));
            h.settle();
            let before = h.app.editor.document.model().clone();
            h.app.begin_dimension_repair(anchor);
            h.frame(vec![]);
            let pointer = h.point(point);
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            assert!(
                h.app
                    .dimension_repair_candidate(
                        h.app.plans.drawing.as_ref().unwrap(),
                        context,
                        h.app.plans.cameras[&h.view],
                        h.app.plans.canvas_rect.unwrap(),
                        pointer
                    )
                    .is_err(),
                "{layout:?}: {case}"
            );
            h.click(pointer);
            assert_eq!(h.app.editor.document.model(), &before, "{case}");
            assert!(!h.app.editor.document.can_undo(), "{case}");
            assert!(h.app.plans.dimension_draft.is_some(), "{case}");
        }
    }
}

#[test]
fn dimension_repair_two_unavailable_anchors_commit_separately_at_both_dpis() {
    use os_model::DimensionLayout::*;
    for (size, scale) in PROFILES {
        for layout in [Aligned, Chain, Baseline, Angular] {
            for wrong_level in [false, true] {
                let mut h = Harness::new(size, scale);
                let (id, anchor, target, point) = dimension_repair_fixture(&mut h, layout);
                let mut model = h.app.editor.document.model().clone();
                let first = model.dimensions[&id].parameters.first;
                let unavailable_wall = if wrong_level {
                    let mut level = model.levels[&model.walls[&h.wall].parameters.level]
                        .parameters
                        .clone();
                    level.name = "Other repair level".into();
                    let level = os_model::Level::new("core.level", level);
                    let mut wall = model.walls[&h.wall].parameters.clone();
                    wall.level = level.id();
                    model.levels.insert(level.id(), level);
                    let wall = os_model::Wall::new(os_walls::WALL_TYPE, wall);
                    let id = wall.id();
                    model.walls.insert(id, wall);
                    id
                } else {
                    Id::new()
                };
                model.dimensions.get_mut(&id).unwrap().parameters.first =
                    DimensionReference::WallEndpoint {
                        wall: unavailable_wall,
                        endpoint: model
                            .dimensions
                            .get_mut(&id)
                            .unwrap()
                            .parameters
                            .first
                            .wall_endpoint()
                            .unwrap()
                            .1,
                    };
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.editor.regenerate().unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.app.select(Some(id));
                h.settle();
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let revision = h.app.editor.document.revision();
                let mut intermediate = before.clone();
                let params = &mut intermediate.dimensions.get_mut(&id).unwrap().parameters;
                if anchor == 1 {
                    params.second = target;
                } else {
                    params.additional[anchor - 2] = target;
                }
                assert!(params.validate_creation(&before).is_err());

                h.click_text_in_band(&format!("Replace anchor {}", anchor + 1), 0.0, h.size.y);
                h.frame(vec![]);
                h.frame(vec![egui::Event::PointerMoved(h.point(point))]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.revision(), revision);
                assert_eq!(h.app.editor.scene, scene);
                assert!(!h.app.editor.document.can_undo());
                assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Text(t) if t.galley.job.text.contains("still unresolved: 1"))));
                assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Circle(c) if c.radius == 7.0 && c.stroke.color == theme::ACCENT)));
                assert!(!h.output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::LineSegment { stroke, .. } if stroke.width == 1.8 && stroke.color == theme::ACCENT)), "no measured ghost before all anchors resolve");
                assert!(!h.output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Text(t) if (t.galley.job.text.ends_with(" m") || t.galley.job.text.ends_with('°'))
                        && t.galley.job.sections.iter().any(|s| s.format.color == theme::ACCENT))));
                h.click(h.point(point));
                h.settle();
                assert!(!h.app.status_error, "{}", h.app.status);
                h.clean();
                assert_eq!(h.app.editor.document.model(), &intermediate);
                assert_eq!(h.app.editor.document.revision(), revision + 1);
                let params = &intermediate.dimensions[&id].parameters;
                assert_eq!(
                    dimension_unavailable_anchors(params, &intermediate),
                    vec![1]
                );
                let reason = if layout == Angular {
                    params.resolve_angular(&intermediate).unwrap_err()
                } else {
                    params.resolve_points(&intermediate).unwrap_err()
                };
                assert!(matches!(
                    reason,
                    os_model::DimensionDiagnostic::MissingWall
                        | os_model::DimensionDiagnostic::MissingWallAt(1)
                        | os_model::DimensionDiagnostic::WrongLevel
                        | os_model::DimensionDiagnostic::WrongLevelAt(1)
                ));
                assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Text(t) if t.galley.job.text.contains("Broken reference:"))));

                // A fresh explicit action repairs the remaining original anchor.
                h.click_text_in_band("Replace anchor 1", 0.0, h.size.y);
                h.frame(vec![]);
                let first_point = Point2::new(if layout == Angular { -0.7 } else { -1.0 }, 0.0);
                h.frame(vec![egui::Event::PointerMoved(h.point(first_point))]);
                assert_eq!(h.app.editor.document.model(), &intermediate);
                assert_eq!(h.app.editor.document.revision(), revision + 1);
                assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::LineSegment { stroke, .. } if stroke.width == 1.8 && stroke.color == theme::ACCENT)));
                h.click(h.point(first_point));
                h.settle();
                assert!(!h.app.status_error, "{}", h.app.status);
                let mut resolved = intermediate.clone();
                resolved.dimensions.get_mut(&id).unwrap().parameters.first = first;
                resolved.dimensions[&id]
                    .parameters
                    .validate_creation(&resolved)
                    .unwrap();
                assert_eq!(h.app.editor.document.model(), &resolved);
                assert_eq!(h.app.editor.document.revision(), revision + 2);
                assert_eq!(h.app.selected, Some(id));
                h.clean();
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &intermediate);
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &before);
                assert!(!h.app.editor.document.can_undo());
                h.app.history(true);
                assert_eq!(h.app.editor.document.model(), &intermediate);
                h.app.history(true);
                assert_eq!(h.app.editor.document.model(), &resolved);
                assert!(!h.app.editor.document.can_redo());
            }
        }
    }
}

#[test]
fn angular_authoring_preview_cancel_and_single_transaction_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let mut params = h.app.editor.document.model().walls[&h.wall]
            .parameters
            .clone();
        *params.path.straight_start_mut().unwrap() = Point2::new(0.0, -1.0);
        *params.path.straight_end_mut().unwrap() = Point2::new(0.0, 1.0);
        let wall = os_model::Wall::new(
            &h.app.editor.document.model().walls[&h.wall].header.type_id,
            params,
        );
        let second = wall.id();
        h.app
            .editor
            .command("Fixture axis", Command::AddWall(wall))
            .unwrap();
        h.settle();
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        h.app
            .begin_dimension(h.view, os_model::DimensionLayout::Angular);
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.7, 0.0)));
        h.frame(vec![]);
        h.click(h.point(Point2::new(-0.7, 0.0)));
        assert!(
            h.app
                .plans
                .dimension_draft
                .as_ref()
                .unwrap()
                .second
                .is_none()
        );
        assert!(h.app.status_error);
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.0, 0.7)));
        assert!(h.app.plans.dimension_draft.as_ref().unwrap().placing);
        h.frame(vec![egui::Event::PointerMoved(
            h.point(Point2::new(0.5, 0.5)),
        )]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        h.frame(vec![key_pressed(egui::Key::Backspace)]);
        assert!(!h.app.plans.dimension_draft.as_ref().unwrap().placing);
        h.frame(vec![escape()]);
        h.frame(vec![]);
        h.clean();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app
            .begin_dimension(h.view, os_model::DimensionLayout::Angular);
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.7, 0.0)));
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.0, 0.7)));
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.5, 0.5)));
        h.settle();
        h.clean();
        assert!(!h.app.status_error, "{}", h.app.status);
        let id = h.app.selected.unwrap();
        let committed = h.app.editor.document.model().clone();
        let d = &committed.dimensions[&id];
        assert_eq!(d.parameters.layout, os_model::DimensionLayout::Angular);
        assert_eq!(d.parameters.first.entity(), h.wall);
        assert_eq!(d.parameters.second.entity(), second);
        assert!((d.parameters.resolve_angular(&committed).unwrap().degrees() - 90.0).abs() < 1e-8);
        assert_eq!(committed.dimensions.len(), before.dimensions.len() + 1);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        let mut without_dimension = committed.clone();
        without_dimension.dimensions.remove(&id);
        assert_eq!(without_dimension, before);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &committed);
    }
}

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    output: egui::FullOutput,
    size: egui::Vec2,
    scale: f32,
    time: f64,
    wall: Id,
    view: Id,
}
impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let material = os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: "Fixture concrete".into(),
                density_kg_m3: 2400.0,
                color: [180, 180, 180],
            },
        );
        app.draft.material = Some(material.id());
        let mut model = app.editor.document.model().clone();
        model.materials.insert(material.id(), material);
        app.editor.document = Document::from_model(model).unwrap();
        *app.draft.path.straight_start_mut().unwrap() = Point2::new(-1.0, 0.0);
        *app.draft.path.straight_end_mut().unwrap() = Point2::new(1.0, 0.0);
        app.draft.name = "Endpoint fixture".into();
        app.draft.height = 3.7;
        app.draft.thickness = 0.27;
        app.apply_wall();
        let wall = app
            .selected
            .unwrap_or_else(|| panic!("fixture wall failed: {}", app.status));
        let view = app
            .editor
            .create_floor_plan("Endpoint plan", app.active_level)
            .unwrap();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Self {
            app,
            ctx,
            output: Default::default(),
            size,
            scale,
            time: 0.0,
            wall,
            view,
        };
        h.settle();
        h
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1.0 / 60.0;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.scale);
        self.output = self.ctx.run(input, |ctx| self.app.show(ctx));
    }
    fn settle(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() && !self.app.editor.plugin_work_pending() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "plan/worker did not settle"
            );
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
    fn point(&self, world: Point2) -> egui::Pos2 {
        let rect = self.app.plans.canvas_rect.unwrap();
        let context = self.app.editor.native_plan_context(self.view).unwrap();
        let p = self.app.plans.cameras[&self.view]
            .project(
                context.basis.world_to_plane(world).unwrap(),
                [rect.width() as f64, rect.height() as f64],
            )
            .unwrap();
        rect.min + egui::vec2(p.x as f32, p.y as f32)
    }
    fn press(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
        self.frame(vec![button(p, true)]);
    }
    fn release(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p), button(p, false)]);
    }
    fn click(&mut self, p: egui::Pos2) {
        self.press(p);
        self.release(p);
    }
    fn click_text_in_band(&mut self, text: &str, y_min: f32, y_max: f32) {
        let position = self
            .output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(galley)
                    if galley.galley.job.text == text && (y_min..y_max).contains(&galley.pos.y) =>
                {
                    Some(egui::Rect::from_min_size(galley.pos, galley.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                let visible: Vec<_> = self
                    .output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                        _ => None,
                    })
                    .collect();
                panic!("text not rendered in band: {text}; visible text: {visible:?}")
            });
        self.click(position);
    }
    fn circles(&self) -> Vec<egui::Pos2> {
        self.output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c) if c.radius == 6.0 && c.fill == theme::ACCENT => {
                    Some(c.center)
                }
                _ => None,
            })
            .collect()
    }
    fn clean(&self) {
        assert!(self.app.plans.endpoint_drag.is_none());
        assert!(self.app.wall_gesture.is_none());
        assert!(self.app.plans.dimension_draft.is_none());
        assert!(!self.app.plans.endpoint_pointer_claimed);
    }
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn key_pressed(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn endpoint_handles_render_and_hit_in_logical_points() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        assert_eq!(h.ctx.pixels_per_point(), scale);
        let start = h.point(Point2::new(-1.0, 0.0));
        let end = h.point(Point2::new(1.0, 0.0));
        assert_eq!(h.circles(), vec![start, end]);
        for (delta, hit) in [
            (egui::vec2(10.0, 0.0), true),
            (egui::vec2(6.0, 8.0), true),
            (egui::vec2(10.1, 0.0), false),
            (egui::vec2(8.0, 8.0), false),
        ] {
            let p = h.point(Point2::new(-1.0, 0.0)) + delta;
            h.press(p);
            assert_eq!(
                h.app.plans.endpoint_drag.is_some(),
                hit,
                "{delta:?} at {scale}"
            );
            if hit {
                assert_eq!(
                    h.app.wall_gesture.as_ref().unwrap().edit_mode(),
                    Some(WallEdit::ResizeStart)
                );
            }
            h.release(p);
            h.clean();
            assert!(!h.app.editor.document.can_undo());
            h.app.select(Some(h.wall));
            h.frame(vec![]);
        }
        // Overlapping hit circles: Euclidean nearest, then start on exact tie.
        let handles = [
            (WallEdit::ResizeStart, start),
            (WallEdit::ResizeEnd, start + egui::vec2(12.0, 0.0)),
        ];
        assert_eq!(
            hit_endpoint(&handles, start + egui::vec2(6.0, 0.0)),
            Some(WallEdit::ResizeStart)
        );
        assert_eq!(
            hit_endpoint(&handles, start + egui::vec2(7.0, 0.0)),
            Some(WallEdit::ResizeEnd)
        );
        h.app
            .plans
            .cameras
            .get_mut(&h.view)
            .unwrap()
            .pixels_per_metre = 6.0;
        h.frame(vec![]);
        for (dx, expected) in [(0.0, WallEdit::ResizeStart), (1.0, WallEdit::ResizeEnd)] {
            let p = h.point(Point2::new(0.0, 0.0)) + egui::vec2(dx, 0.0);
            h.press(p);
            assert_eq!(
                h.app.wall_gesture.as_ref().unwrap().edit_mode(),
                Some(expected)
            );
            h.release(p);
            h.clean();
            assert!(!h.app.editor.document.can_undo());
            h.frame(vec![]);
        }
    }
}

#[test]
fn endpoint_drag_preview_commit_identity_properties_and_history() {
    for (size, scale) in PROFILES {
        for mode in [WallEdit::ResizeStart, WallEdit::ResizeEnd] {
            let mut h = Harness::new(size, scale);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let camera = h.app.plans.cameras[&h.view];
            let original = &before.walls[&h.wall];
            let moving = if mode == WallEdit::ResizeStart {
                original.parameters.start()
            } else {
                original.parameters.end()
            };
            h.press(h.point(moving));
            assert_eq!(
                h.app.wall_gesture.as_ref().unwrap().snap_exclusion(),
                Some(h.wall)
            );
            let destination = Point2::new(0.0, 1.0);
            let p = h.point(destination);
            h.frame(vec![egui::Event::PointerMoved(p)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.revision(), 0);
            assert!(!h.app.editor.document.can_undo());
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.plans.cameras[&h.view], camera);
            let expected = h
                .app
                .wall_gesture
                .as_ref()
                .unwrap()
                .parameters(destination)
                .unwrap();
            assert_eq!(expected.height, original.parameters.height);
            assert_eq!(expected.thickness, original.parameters.thickness);
            assert_eq!(expected.name, original.parameters.name);
            assert_eq!(expected.material, original.parameters.material);
            assert_eq!(expected.level, original.parameters.level);
            if mode == WallEdit::ResizeStart {
                assert_eq!(expected.end(), original.parameters.end());
            } else {
                assert_eq!(expected.start(), original.parameters.start());
            }
            let a = h.point(expected.start());
            let b = h.point(expected.end());
            let preview_lines: Vec<_> = h
                .output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.width == 2.0 && stroke.color == theme::ACCENT =>
                    {
                        Some(*points)
                    }
                    _ => None,
                })
                .collect();
            assert!(
                preview_lines.contains(&[a, b]),
                "scale={scale} mode={mode:?} expected={:?} got={preview_lines:?}",
                [a, b]
            );
            h.release(p);
            h.clean();
            assert!(!h.app.status_error, "{}", h.app.status);
            let edited = h.app.editor.document.model().clone();
            assert_eq!(edited.walls.len(), 1);
            assert_eq!(edited.walls[&h.wall].header, original.header);
            assert_eq!(edited.walls[&h.wall].parameters, expected);
            assert_eq!(h.app.editor.document.revision(), 1);
            assert_eq!(h.app.selected, Some(h.wall));
            assert_ne!(h.app.editor.scene, scene);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &edited);
        }
    }
}

#[test]
fn endpoint_cancel_invalid_outside_and_stale_leave_no_residue() {
    for (size, scale) in PROFILES {
        for reason in [
            "click",
            "long press",
            "outside",
            "escape",
            "collapse",
            "invalid exact",
            "revision",
            "settings",
            "view",
            "selection",
            "provider",
            "session",
        ] {
            let mut h = Harness::new(size, scale);
            let before = h.app.editor.document.model().clone();
            let camera = h.app.plans.cameras[&h.view];
            let start = h.point(Point2::new(-1.0, 0.0));
            h.press(start);
            let mut p = h.point(Point2::new(0.0, 1.0));
            if reason != "click" && reason != "long press" {
                h.frame(vec![egui::Event::PointerMoved(p)]);
            }
            match reason {
                "click" => p = start,
                "long press" => {
                    p = start;
                    h.time += 2.0;
                    h.frame(vec![]);
                }
                "outside" => {
                    p = h.app.plans.canvas_rect.unwrap().right_bottom() + egui::vec2(30.0, 30.0)
                }
                "escape" => h.frame(vec![escape()]),
                "collapse" => p = h.point(Point2::new(1.0, 0.0)),
                "invalid exact" => h.app.wall_gesture.as_mut().unwrap().length = "invalid".into(),
                "revision" => {
                    h.app
                        .editor
                        .command("revision", Command::RenameProject("New name".into()))
                        .unwrap();
                }
                "settings" => {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    settings.visibility.walls = false;
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Changed", h.app.active_level, settings)
                        .unwrap();
                }
                "view" => h.app.focus_plan(None),
                "selection" => h.app.select(None),
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "session" => h.app.editor.document = Document::from_model(before.clone()).unwrap(),
                _ => unreachable!(),
            }
            let expected = h.app.editor.document.model().clone();
            if reason == "escape" {
                h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(40.0, 20.0))]);
                assert_eq!(h.app.plans.cameras[&h.view], camera);
            }
            h.release(p);
            h.clean();
            assert_eq!(h.app.editor.document.model(), &expected, "{reason}");
            assert_eq!(
                h.app.editor.document.can_undo(),
                reason == "revision" || reason == "settings",
                "{reason}"
            );
            if let Some(current) = h.app.plans.cameras.get(&h.view) {
                assert_eq!(*current, camera, "{reason}");
            }
            // No delayed commit on a subsequent ordinary pointer release.
            h.frame(vec![egui::Event::PointerMoved(p)]);
            h.release(p);
            h.clean();
            assert_eq!(h.app.editor.document.model(), &expected);
        }
    }
}

#[test]
fn endpoint_handles_require_selected_checked_visible_native_wall() {
    for (size, scale) in PROFILES {
        for case in [
            "unselected",
            "other id",
            "off canvas",
            "hidden",
            "cropped",
            "partial crop",
            "absent drawing",
            "stale drawing",
        ] {
            let mut h = Harness::new(size, scale);
            match case {
                "unselected" => h.app.select(None),
                "other id" => h.app.selected = Some(Id::new()),
                "off canvas" => {
                    h.app.plans.cameras.get_mut(&h.view).unwrap().center = Point2::new(100.0, 100.0)
                }
                "hidden" | "cropped" | "partial crop" => {
                    let mut settings = h.app.editor.document.model().views[&h.view]
                        .parameters
                        .plan
                        .unwrap();
                    if case == "hidden" {
                        settings.visibility.walls = false;
                    } else {
                        settings.crop = Some(os_model::PlanViewCrop {
                            min: Point2::new(if case == "cropped" { 10.0 } else { 0.0 }, -1.0),
                            max: Point2::new(12.0, 1.0),
                        });
                    }
                    h.app
                        .editor
                        .update_floor_plan(h.view, "Visibility", h.app.active_level, settings)
                        .unwrap();
                    h.settle();
                }
                "absent drawing" => {
                    let context = h.app.plans.desired.unwrap();
                    h.app.plans.drawing =
                        Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap());
                }
                "stale drawing" => {
                    let mut context = h.app.plans.desired.unwrap();
                    context.model_revision += 1;
                    h.app.plans.drawing =
                        Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap());
                }
                _ => unreachable!(),
            }
            h.frame(vec![]);
            assert_eq!(
                h.circles().len(),
                usize::from(case == "partial crop"),
                "{case}"
            );
            if case == "partial crop" {
                assert_eq!(h.circles(), vec![h.point(Point2::new(1.0, 0.0))]);
            }
            let p = h.point(Point2::new(-1.0, 0.0));
            let model = h.app.editor.document.model().clone();
            h.press(p);
            assert!(h.app.plans.endpoint_drag.is_none(), "{case}");
            h.release(p);
            h.clean();
            assert_eq!(h.app.editor.document.model(), &model);
        }
    }
}

#[test]
fn endpoint_drag_reuses_snapping_and_exact_input_precedence() {
    for (size, scale) in PROFILES {
        for exact in [false, true] {
            let mut h = Harness::new(size, scale);
            let mut target = h.app.editor.document.model().walls[&h.wall]
                .parameters
                .clone();
            *target.path.straight_start_mut().unwrap() = Point2::new(0.0, 1.0);
            *target.path.straight_end_mut().unwrap() = Point2::new(1.0, 1.0);
            h.app
                .editor
                .wall_command("Snap target", Request::CreateWall(target))
                .unwrap();
            h.settle();
            let before = h.app.editor.document.model().clone();
            h.press(h.point(Point2::new(1.0, 0.0)));
            if exact {
                let gesture = h.app.wall_gesture.as_mut().unwrap();
                gesture.length = "3".into();
                gesture.angle_degrees = "90".into();
            }
            let near = h.point(Point2::new(0.0, 1.0)) + egui::vec2(3.0, 2.0);
            h.frame(vec![egui::Event::PointerMoved(near)]);
            assert_eq!(h.app.editor.document.model(), &before);
            h.release(near);
            h.clean();
            let wall = &h.app.editor.document.model().walls[&h.wall].parameters;
            assert_eq!(wall.start(), before.walls[&h.wall].parameters.start());
            let expected = if exact {
                Point2::new(-1.0, 3.0)
            } else {
                Point2::new(0.0, 1.0)
            };
            assert!(wall.end().distance(expected) < 1e-10, "{:?}", wall.end());
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
        }
    }
}

#[test]
fn aligned_dimensions_author_live_endpoint_references_with_one_history_step() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.select(Some(h.wall));
        let before = h.app.editor.document.model().clone();
        let before_revision = h.app.editor.document.revision();
        let view = h.view;
        h.frame(vec![]);
        h.click_text_in_band("Architecture", 28.0, 60.0);
        h.click_text_in_band("Aligned dimension", 60.0, 150.0);
        assert!(h.app.plans.dimension_draft.is_some());
        h.frame(vec![]);
        let canvas = h.app.plans.canvas_rect.unwrap();
        let pan_from = canvas.center() + egui::vec2(100.0, 90.0);
        let pan_to = pan_from + egui::vec2(35.0, 20.0);
        let camera = h.app.plans.cameras[&view];
        h.press(pan_from);
        h.frame(vec![egui::Event::PointerMoved(pan_to)]);
        h.release(pan_to);
        assert_eq!(h.app.plans.cameras[&view], camera);
        assert!(
            h.app
                .plans
                .dimension_draft
                .as_ref()
                .unwrap()
                .first
                .is_none()
        );
        let start = h.point(Point2::new(-1.0, 0.0));
        let end = h.point(Point2::new(1.0, 0.0));
        let placement = h.point(Point2::new(0.0, 0.5));

        h.click(start);
        assert!(h.app.plans.endpoint_drag.is_none());
        assert!(h.app.wall_gesture.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), before_revision);
        assert!(!h.app.editor.document.can_undo());

        h.click(end);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), before_revision);
        h.frame(vec![egui::Event::PointerMoved(placement)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.plans.cameras[&view], camera);
        assert!(h.output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "2.000 m"
        )));

        h.click(placement);
        h.clean();
        assert!(!h.app.status_error, "{}", h.app.status);
        let id = h.app.selected.expect("new dimension remains selected");
        let committed = h.app.editor.document.model();
        assert_eq!(committed.dimensions.len(), 1);
        let dimension = committed.dimensions[&id].clone();
        assert_eq!(dimension.parameters.view, view);
        assert_eq!(dimension.parameters.first.entity(), h.wall);
        assert_eq!(
            dimension.parameters.first.wall_endpoint().unwrap().1,
            DimensionEndpoint::Start
        );
        assert_eq!(dimension.parameters.second.entity(), h.wall);
        assert_eq!(
            dimension.parameters.second.wall_endpoint().unwrap().1,
            DimensionEndpoint::End
        );
        assert!((dimension.parameters.offset_m - 0.5).abs() < 1e-9);
        assert_eq!(committed.walls, before.walls);
        assert_eq!(h.app.editor.document.revision(), before_revision + 1);
        assert!(h.app.editor.document.can_undo());

        h.settle();
        assert!(h.output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "2.000 m"
        )));
        h.app.select(Some(h.wall));
        // Wall selection switches the Modify ribbon and changes canvas bounds.
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.0, 0.5)));
        assert_eq!(
            h.app.selected,
            Some(id),
            "dimension wins annotation picking"
        );

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dimension.osb");
        h.app.editor.save(&path).unwrap();
        let mut reopened = Editor::new().unwrap();
        reopened.open(&path).unwrap();
        assert_eq!(reopened.document.model().dimensions[&id], dimension);

        h.app.history(false);
        assert!(!h.app.editor.document.model().dimensions.contains_key(&id));
        h.app.history(true);
        assert_eq!(h.app.editor.document.model().dimensions[&id], dimension);
    }
}

#[test]
fn aligned_dimension_offset_is_editable_and_measurement_follows_wall_edits() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    h.click(h.point(Point2::new(1.0, 0.0)));
    h.click(h.point(Point2::new(0.0, 0.5)));
    let id = h.app.selected.unwrap();

    h.app.dimension_offset_draft = 0.75;
    h.app.apply_dimension_properties();
    assert!(!h.app.status_error, "{}", h.app.status);
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .offset_m
            - 0.75)
            .abs()
            < 1e-9
    );
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .orphan_hint
            .y
            - 0.75)
            .abs()
            < 1e-9
    );
    h.app.history(false);
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .offset_m
            - 0.5)
            .abs()
            < 1e-9
    );
    h.app.history(true);
    assert!(
        (h.app.editor.document.model().dimensions[&id]
            .parameters
            .offset_m
            - 0.75)
            .abs()
            < 1e-9
    );

    let mut wall = h.app.editor.document.model().walls[&h.wall]
        .parameters
        .clone();
    *wall.path.straight_end_mut().unwrap() = Point2::new(2.0, 0.0);
    h.app
        .editor
        .command(
            "Extend dimensioned wall",
            Command::UpdateWall {
                id: h.wall,
                parameters: wall,
            },
        )
        .unwrap();
    h.settle();
    let current = &h.app.editor.document.model().dimensions[&id];
    assert_eq!(
        current.parameters.first.wall_endpoint().unwrap().1,
        DimensionEndpoint::Start
    );
    assert_eq!(
        current.parameters.second.wall_endpoint().unwrap().1,
        DimensionEndpoint::End
    );
    assert_eq!(
        current
            .parameters
            .resolve(h.app.editor.document.model())
            .unwrap()
            .length_metres,
        3.0
    );
    assert!(h.output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "3.000 m"
    )));
    assert_eq!(h.app.selected, Some(id));
    h.app.history(false);
    h.settle();
    assert_eq!(
        h.app.editor.document.model().dimensions[&id]
            .parameters
            .resolve(h.app.editor.document.model())
            .unwrap()
            .length_metres,
        2.0
    );
}

fn add_dimension_test_opening(h: &mut Harness, kind: os_model::OpeningKind) -> (Id, Id) {
    use os_model::{
        Opening, OpeningDefinition, OpeningParams, OpeningType, OpeningTypeParams,
        WindowPanePosition,
    };
    let mut model = h.app.editor.document.model().clone();
    let opening_type = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            family: Default::default(),
            name: format!("Dimension {kind:?}"),
            kind,
            width: 0.6,
            height: 2.0,
            sill: if kind == os_model::OpeningKind::Window {
                0.8
            } else {
                0.0
            },
            pane_position: WindowPanePosition::Center,
        },
    );
    let type_id = opening_type.id();
    model.opening_types.insert(type_id, opening_type);
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: format!("Dimension {kind:?}"),
            host: h.wall,
            offset: 0.3,
            definition: OpeningDefinition::Typed { type_id },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let opening_id = opening.id();
    model.openings.insert(opening_id, opening);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.insert(opening_id);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.settle();
    (opening_id, type_id)
}

#[test]
fn opening_jamb_dimensions_create_follow_live_width_and_history_at_both_dpis() {
    use os_model::{DimensionJamb, DimensionLayout, DimensionReference, OpeningKind};

    for (size, scale) in PROFILES {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let mut h = Harness::new(size, scale);
            let (opening_id, type_id) = add_dimension_test_opening(&mut h, kind);

            let start = Point2::new(-0.7, 0.0);
            let end = Point2::new(-0.1, 0.0);
            let before = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.app.begin_dimension(h.view, DimensionLayout::Aligned);
            h.frame(vec![]);
            h.click(h.point(start));
            h.frame(vec![]);
            h.click(h.point(end));
            let draft = h.app.plans.dimension_draft.as_ref().unwrap();
            assert_eq!(
                draft.first.as_ref().unwrap().reference,
                DimensionReference::OpeningJamb {
                    opening: opening_id,
                    jamb: DimensionJamb::Start,
                }
            );
            assert_eq!(
                draft.second.as_ref().unwrap().reference,
                DimensionReference::OpeningJamb {
                    opening: opening_id,
                    jamb: DimensionJamb::End,
                }
            );
            let placement = h.point(Point2::new(-0.4, 0.5));
            h.frame(vec![egui::Event::PointerMoved(placement)]);
            assert_eq!(
                h.app.editor.document.model(),
                &before,
                "preview is immutable"
            );
            assert_eq!(h.app.editor.document.revision(), revision);
            assert!(
                h.output.shapes.iter().any(|shape| matches!(
                    &shape.shape,
                    egui::Shape::Text(text) if text.galley.job.text == "0.600 m"
                )),
                "preview labels: {:?}",
                h.output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            );
            h.click(placement);
            h.settle();
            h.clean();

            let id = h.app.selected.expect("created dimension selected");
            let committed = h.app.editor.document.model().clone();
            let dimension = &committed.dimensions[&id].parameters;
            assert_eq!(
                dimension.first,
                DimensionReference::OpeningJamb {
                    opening: opening_id,
                    jamb: DimensionJamb::Start,
                }
            );
            assert_eq!(
                dimension.second,
                DimensionReference::OpeningJamb {
                    opening: opening_id,
                    jamb: DimensionJamb::End,
                }
            );
            assert!((dimension.resolve(&committed).unwrap().length_metres - 0.6).abs() < 1e-9);
            assert_eq!(h.app.editor.document.revision(), revision + 1);

            let mut edited_type = committed.opening_types[&type_id].parameters.clone();
            edited_type.width = 1.0;
            h.app
                .editor
                .command(
                    "Resize dimensioned opening",
                    Command::UpdateOpeningType {
                        id: type_id,
                        parameters: edited_type,
                    },
                )
                .unwrap();
            h.settle();
            let live = &h.app.editor.document.model().dimensions[&id].parameters;
            assert!(
                (live
                    .resolve(h.app.editor.document.model())
                    .unwrap()
                    .length_metres
                    - 1.0)
                    .abs()
                    < 1e-9
            );
            assert!(h.output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "1.000 m"
            )));
            h.app.history(false);
            h.settle();
            assert!(
                (h.app.editor.document.model().dimensions[&id]
                    .parameters
                    .resolve(h.app.editor.document.model())
                    .unwrap()
                    .length_metres
                    - 0.6)
                    .abs()
                    < 1e-9
            );
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            h.app.history(true);
            h.settle();
            assert!(
                (h.app.editor.document.model().dimensions[&id]
                    .parameters
                    .resolve(h.app.editor.document.model())
                    .unwrap()
                    .length_metres
                    - 1.0)
                    .abs()
                    < 1e-9
            );
        }
    }
}

#[test]
fn dimension_repair_can_target_visible_opening_jambs_at_both_dpis() {
    use os_model::{
        DimensionJamb, DimensionLayout, DimensionParams, DimensionReference, OpeningKind,
    };

    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (opening, _) = add_dimension_test_opening(&mut h, OpeningKind::Door);
        let mut model = h.app.editor.document.model().clone();
        let missing = Id::new();
        let dimension = os_model::Dimension::new(
            "core.dimension",
            DimensionParams {
                layout: DimensionLayout::Aligned,
                view: h.view,
                first: DimensionReference::OpeningJamb {
                    opening: missing,
                    jamb: DimensionJamb::Start,
                },
                second: DimensionReference::WallEndpoint {
                    wall: h.wall,
                    endpoint: DimensionEndpoint::End,
                },
                additional: Vec::new(),
                baseline_spacing_m: 0.25,
                offset_m: 0.5,
                orphan_hint: Point2::new(0.0, 0.5),
            },
        );
        let id = dimension.id();
        model.dimensions.insert(id, dimension);
        h.app.editor.document = Document::from_model(model.clone()).unwrap();
        h.app.editor.regenerate().unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.app.select(Some(id));
        h.settle();
        let revision = h.app.editor.document.revision();

        h.click_text_in_band("Replace anchor 1", 0.0, h.size.y);
        h.frame(vec![]);
        let pointer = h.point(Point2::new(-0.7, 0.0));
        h.frame(vec![egui::Event::PointerMoved(pointer)]);
        assert_eq!(h.app.editor.document.model(), &model);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert!(h.output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "Opening Start jamb"
        )));
        h.click(pointer);
        h.settle();
        h.clean();
        let repaired = h.app.editor.document.model().clone();
        assert_eq!(
            repaired.dimensions[&id].parameters.first,
            DimensionReference::OpeningJamb {
                opening,
                jamb: DimensionJamb::Start,
            }
        );
        repaired.dimensions[&id]
            .parameters
            .validate_creation(&repaired)
            .unwrap();
        assert_eq!(h.app.editor.document.revision(), revision + 1);

        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &model);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &repaired);
    }
}

#[test]
fn opening_jamb_anchor_hit_radius_and_hidden_crop_precedence_are_logical_pixels() {
    use os_model::{DimensionJamb, DimensionReference, OpeningKind};

    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (opening, _) = add_dimension_test_opening(&mut h, OpeningKind::Door);
        let start = h.point(Point2::new(-0.7, 0.0));
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let rect = h.app.plans.canvas_rect.unwrap();
        let camera = h.app.plans.cameras[&h.view];
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        for (delta, expected) in [
            (egui::vec2(12.0, 0.0), true),
            (egui::vec2(12.01, 0.0), false),
        ] {
            let result = h
                .app
                .dimension_anchor_at(drawing, context, camera, rect, start + delta);
            if expected {
                assert_eq!(
                    result.unwrap().reference,
                    DimensionReference::OpeningJamb {
                        opening,
                        jamb: DimensionJamb::Start
                    },
                    "radius is in logical points at {scale}"
                );
            } else {
                assert!(result.is_err(), "12.01 pt must miss at {scale}");
            }
        }

        for visibility in ["hidden", "cropped"] {
            let mut model = h.app.editor.document.model().clone();
            let plan = model
                .views
                .get_mut(&h.view)
                .unwrap()
                .parameters
                .plan
                .as_mut()
                .unwrap();
            if visibility == "hidden" {
                plan.visibility.walls = false;
            } else {
                plan.crop = Some(os_model::PlanViewCrop {
                    min: Point2::new(-1.0, -0.5),
                    max: Point2::new(-0.8, 0.5),
                });
            }
            h.app.editor.document = Document::from_model(model).unwrap();
            h.app.editor.regenerate().unwrap();
            h.app.plans.poll(&h.app.editor);
            h.app.focus_plan(Some(h.view));
            h.settle();
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            let rect = h.app.plans.canvas_rect.unwrap();
            let camera = h.app.plans.cameras[&h.view];
            let drawing = h.app.plans.drawing.as_ref().unwrap();
            assert!(
                h.app
                    .dimension_anchor_at(drawing, context, camera, rect, start)
                    .is_err(),
                "opening jamb is not an anchor when {visibility}"
            );
        }
    }
}

#[test]
fn chain_and_baseline_dimensions_collect_ordered_anchors_and_commit_once() {
    for layout in [
        os_model::DimensionLayout::Chain,
        os_model::DimensionLayout::Baseline,
    ] {
        for (size, scale) in PROFILES {
            let mut h = Harness::new(size, scale);
            let source = &h.app.editor.document.model().walls[&h.wall];
            let wall_type = source.header.type_id.clone();
            let mut params = source.parameters.clone();
            params.name = "Dimension continuation".into();
            *params.path.straight_start_mut().unwrap() = Point2::new(2.0, 0.0);
            *params.path.straight_end_mut().unwrap() = Point2::new(3.0, 0.0);
            let continuation = os_model::Wall::new(&wall_type, params);
            let continuation_id = continuation.id();
            let mut off_axis_params = source.parameters.clone();
            off_axis_params.name = "Off-axis dimension test wall".into();
            *off_axis_params.path.straight_start_mut().unwrap() = Point2::new(2.0, 1.0);
            *off_axis_params.path.straight_end_mut().unwrap() = Point2::new(3.0, 1.0);
            let off_axis = os_model::Wall::new(&wall_type, off_axis_params);
            h.app
                .editor
                .document
                .execute(
                    "Add dimension test walls",
                    vec![Command::AddWall(continuation), Command::AddWall(off_axis)],
                )
                .unwrap();
            h.settle();

            let before = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.app.begin_dimension(h.view, layout);
            h.frame(vec![]);
            h.click(h.point(Point2::new(-1.0, 0.0)));
            h.frame(vec![]);
            h.click(h.point(Point2::new(1.0, 0.0)));
            h.frame(vec![]);
            h.click(h.point(Point2::new(2.0, 1.0)));
            assert_eq!(
                h.app
                    .plans
                    .dimension_draft
                    .as_ref()
                    .unwrap()
                    .additional
                    .len(),
                0,
                "invalid off-axis anchor must leave the draft unchanged"
            );
            assert!(h.app.status_error);
            h.frame(vec![]);
            h.click(h.point(Point2::new(2.0, 0.0)));
            assert_eq!(
                h.app.editor.document.model(),
                &before,
                "anchor selection is preview-only"
            );
            assert_eq!(h.app.editor.document.revision(), revision);
            h.frame(vec![]);
            h.frame(vec![key_pressed(egui::Key::Backspace)]);
            assert!(
                h.app
                    .plans
                    .dimension_draft
                    .as_ref()
                    .unwrap()
                    .additional
                    .is_empty()
            );
            assert_eq!(h.app.editor.document.model(), &before);
            h.frame(vec![]);
            h.click(h.point(Point2::new(2.0, 0.0)));
            h.frame(vec![]);
            h.click_text_in_band("Finish anchors", 0.0, h.size.y);
            h.frame(vec![]);
            assert!(h.app.plans.dimension_draft.as_ref().unwrap().placing);
            assert_eq!(
                h.app.editor.document.model(),
                &before,
                "finishing anchors is not a commit"
            );

            h.click(h.point(Point2::new(0.0, 0.5)));
            h.settle();
            h.clean();
            assert!(!h.app.status_error, "{}", h.app.status);
            let id = h.app.selected.expect("dimension remains selected");
            let committed = h.app.editor.document.model();
            assert_eq!(committed.dimensions.len(), 1);
            let dimension = committed.dimensions[&id].clone();
            assert_eq!(dimension.parameters.layout, layout);
            assert_eq!(dimension.parameters.first.entity(), h.wall);
            assert_eq!(
                dimension.parameters.first.wall_endpoint().unwrap().1,
                DimensionEndpoint::Start
            );
            assert_eq!(dimension.parameters.second.entity(), h.wall);
            assert_eq!(
                dimension.parameters.second.wall_endpoint().unwrap().1,
                DimensionEndpoint::End
            );
            assert_eq!(dimension.parameters.additional.len(), 1);
            assert_eq!(dimension.parameters.additional[0].entity(), continuation_id);
            assert_eq!(
                dimension.parameters.additional[0]
                    .wall_endpoint()
                    .unwrap()
                    .1,
                DimensionEndpoint::Start
            );
            assert_eq!(dimension.parameters.baseline_spacing_m, 0.25);
            assert_eq!(h.app.editor.document.revision(), revision + 1);

            let context = h.app.editor.native_plan_context(h.view).unwrap();
            let drawing = h.app.plans.drawing.as_ref().unwrap();
            let spans = drawing.dimensions(context).unwrap();
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].spans.len(), 1);
            assert_eq!(spans[0].entity, id);
            assert_eq!(spans[0].spans[0].entity, id);
            match layout {
                os_model::DimensionLayout::Chain => {
                    assert_eq!(spans[0].value_m, Some(2.0));
                    assert_eq!(spans[0].spans[0].value_m, Some(1.0));
                    assert!(spans[0].spans[0].shared_start_witness);
                }
                os_model::DimensionLayout::Baseline => {
                    assert_eq!(spans[0].value_m, Some(2.0));
                    assert_eq!(spans[0].spans[0].value_m, Some(3.0));
                    assert!(!spans[0].spans[0].shared_start_witness);
                    assert!(
                        (spans[0].spans[0].line_start.y - spans[0].line_start.y - 0.25).abs()
                            < 1e-9
                    );
                }
                os_model::DimensionLayout::Aligned => unreachable!(),
                os_model::DimensionLayout::Angular => unreachable!(),
            }

            h.app.history(false);
            assert!(!h.app.editor.document.model().dimensions.contains_key(&id));
            assert_eq!(h.app.editor.document.revision(), revision + 2);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model().dimensions[&id], dimension);
        }
    }
}

#[test]
fn aligned_dimension_cancel_and_context_change_leave_no_draft_or_partial_entity() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let before = h.app.editor.document.model().clone();
    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    assert_eq!(h.app.editor.document.model(), &before);
    h.frame(vec![escape()]);
    h.frame(vec![]);
    assert!(h.app.plans.dimension_draft.is_none());
    assert_eq!(h.app.editor.document.model(), &before);

    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    h.app
        .editor
        .command("Change document", Command::RenameProject("Changed".into()))
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.dimension_draft.is_none());
    assert!(h.app.editor.document.model().dimensions.is_empty());
    assert_eq!(
        h.app.editor.document.model().project.parameters.name,
        "Changed"
    );
    h.release(h.point(Point2::new(0.0, 0.5)));
    assert!(h.app.editor.document.model().dimensions.is_empty());
}

#[test]
fn dimension_draft_is_bound_to_its_native_plan_drawing_identity() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let before = h.app.editor.document.model().clone();
    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let drawing_identity = h.app.plans.drawing.as_ref().unwrap().identity();
    h.app.begin_aligned_dimension(h.view);
    assert_eq!(
        h.app
            .plans
            .dimension_draft
            .as_ref()
            .unwrap()
            .drawing_identity,
        Some(drawing_identity)
    );

    h.app.plans.drawing =
        Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), Vec::new()).unwrap());
    h.frame(vec![]);
    assert!(h.app.plans.dimension_draft.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
}

#[test]
fn aligned_dimension_shows_recoverable_orphan_without_blocking_wall_deletion() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_aligned_dimension(h.view);
    h.click(h.point(Point2::new(-1.0, 0.0)));
    h.click(h.point(Point2::new(1.0, 0.0)));
    h.click(h.point(Point2::new(0.0, 0.5)));
    let id = h.app.selected.unwrap();
    assert_eq!(h.app.editor.document.model().dimensions.len(), 1);

    h.app
        .editor
        .command("Delete referenced wall", Command::RemoveWall(h.wall))
        .unwrap();
    h.settle();
    assert!(h.app.editor.document.model().dimensions.contains_key(&id));
    assert_eq!(
        h.app.editor.document.model().dimensions[&id]
            .parameters
            .resolve(h.app.editor.document.model()),
        Err(os_model::DimensionDiagnostic::MissingWall)
    );
    assert!(h.output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "Dimension lost reference"
    )));
    assert!(!h.app.status_error);

    h.app.history(false);
    h.settle();
    assert!(h.app.editor.document.model().walls.contains_key(&h.wall));
    assert_eq!(
        h.app.editor.document.model().dimensions[&id]
            .parameters
            .resolve(h.app.editor.document.model())
            .unwrap()
            .length_metres,
        2.0
    );
}

#[cfg(feature = "external-plugins")]
#[test]
#[ignore = "requires explicitly installed independent Wall guest via OPENSTRUCTURE_WALL_TEST_PLUGIN"]
fn endpoint_installed_drag_submits_and_worker_owns_completion() {
    let directory =
        std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN").expect("installed Wall directory");
    for (size, scale) in PROFILES {
        for mode in [
            WallEdit::ResizeStart,
            WallEdit::ResizeEnd,
            WallEdit::TrimEnd,
        ] {
            for cancel in [false, true] {
                let mut h = Harness::new(size, scale);
                let boundary = (mode == WallEdit::TrimEnd).then(|| add_trim_boundary(&mut h, 1.5));
                h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                h.app
                    .editor
                    .host
                    .load_wasm_directory(
                        Path::new(&directory),
                        [
                            Permission::ModelRead,
                            Permission::ModelWrite,
                            Permission::UiTool,
                        ]
                        .into(),
                    )
                    .unwrap();
                h.settle();
                // Move the floating plugin window off the canvas with real input.
                let title = h
                    .output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.job.text == "Plugin tools" => {
                            Some(t.pos + egui::vec2(25.0, 5.0))
                        }
                        _ => None,
                    })
                    .unwrap();
                h.press(title);
                let to = egui::pos2(40.0, 90.0);
                h.frame(vec![egui::Event::PointerMoved(to)]);
                h.release(to);
                h.frame(vec![]);
                let before = h.app.editor.document.model().clone();
                if mode == WallEdit::TrimEnd {
                    h.app.begin_plan_wall_edit(h.view, h.wall, mode);
                    h.frame(vec![]);
                } else {
                    let moving = if mode == WallEdit::ResizeStart {
                        before.walls[&h.wall].parameters.start()
                    } else {
                        before.walls[&h.wall].parameters.end()
                    };
                    h.press(h.point(moving));
                }
                assert!(
                    h.app
                        .wall_gesture
                        .as_ref()
                        .expect("handle claimed")
                        .is_installed()
                );
                let destination = if mode == WallEdit::TrimEnd {
                    h.app
                        .wall_gesture
                        .as_ref()
                        .unwrap()
                        .trim_extend_point(&before.walls[&boundary.unwrap()].parameters)
                        .unwrap()
                } else {
                    Point2::new(0.0, 1.0)
                };
                let expected = h
                    .app
                    .wall_gesture
                    .as_ref()
                    .unwrap()
                    .parameters(destination)
                    .unwrap();
                let p = if mode == WallEdit::TrimEnd {
                    h.point(Point2::new(1.5, 0.0))
                } else {
                    h.point(destination)
                };
                h.frame(vec![egui::Event::PointerMoved(p)]);
                assert_eq!(h.app.editor.document.model(), &before);
                if mode == WallEdit::TrimEnd {
                    h.click(p);
                } else {
                    h.release(p);
                }
                h.clean();
                assert!(h.app.editor.plugin_work_pending(), "{}", h.app.status);
                assert_eq!(h.app.editor.document.model(), &before);
                assert!(!h.app.editor.document.can_undo());
                if cancel {
                    h.frame(vec![escape()]);
                }
                h.settle();
                if cancel {
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert!(!h.app.editor.document.can_undo());
                } else {
                    assert!(!h.app.status_error, "{}", h.app.status);
                    let edited = h.app.editor.document.model().clone();
                    assert_eq!(edited.walls[&h.wall].header, before.walls[&h.wall].header);
                    assert_eq!(edited.walls[&h.wall].parameters, expected);
                    h.app.history(false);
                    assert_eq!(h.app.editor.document.model(), &before);
                    assert!(!h.app.editor.document.can_undo());
                    h.app.history(true);
                    assert_eq!(h.app.editor.document.model(), &edited);
                }
                h.clean();
            }
        }
    }
}

#[test]
fn endpoint_body_and_empty_canvas_drag_keep_pan_precedence() {
    for (size, scale) in PROFILES {
        for body in [true, false] {
            let mut h = Harness::new(size, scale);
            let model = h.app.editor.document.model().clone();
            let camera = h.app.plans.cameras[&h.view];
            let p = h.point(Point2::new(0.0, if body { 0.0 } else { -1.0 }));
            h.press(p);
            assert!(h.app.plans.endpoint_drag.is_none());
            let target = p + egui::vec2(40.0, 30.0);
            h.frame(vec![egui::Event::PointerMoved(target)]);
            h.release(target);
            h.clean();
            assert_ne!(h.app.plans.cameras[&h.view], camera);
            assert_eq!(h.app.selected, Some(h.wall));
            assert_eq!(h.app.editor.document.model(), &model);
            assert!(!h.app.editor.document.can_undo());
            h.app.select(None);
            h.frame(vec![]);
            let body = h.point(Point2::new(0.0, 0.0));
            h.press(body);
            h.release(body);
            assert_eq!(h.app.selected, Some(h.wall));
        }
    }
}

#[test]
fn room_boundaries_and_placement_obey_view_crop_without_changing_area() {
    let crop = os_geometry::plan::PlanCrop {
        min: Point2::new(0.0, 0.0),
        max: Point2::new(1.0, 1.0),
    };
    assert_eq!(
        clip_plan_segment(Point2::new(-1.0, 0.5), Point2::new(2.0, 0.5), Some(crop),),
        Some((Point2::new(0.0, 0.5), Point2::new(1.0, 0.5)))
    );
    assert_eq!(
        clip_plan_segment(Point2::new(-1.0, 2.0), Point2::new(2.0, 2.0), Some(crop),),
        None
    );
    let context = PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: Default::default(),
        range: Default::default(),
        crop: Some(crop),
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    };
    assert!(point_in_plan_crop(context, Point2::new(0.5, 0.5)));
    assert!(!point_in_plan_crop(context, Point2::new(1.5, 0.5)));
    assert!(point_in_plan_crop(
        PlanContext {
            crop: None,
            ..context
        },
        Point2::new(1.5, 0.5)
    ));
}

#[test]
fn annotation_label_and_marker_bounds_stay_inside_the_plan_crop() {
    let crop = os_geometry::plan::PlanCrop {
        min: Point2::new(0.0, 0.0),
        max: Point2::new(1.0, 1.0),
    };
    let context = PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: Default::default(),
        range: Default::default(),
        crop: Some(crop),
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    };
    let camera = PlanCamera::default();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let size = [800.0, 600.0];
    let near_left_crop_edge = camera.project(Point2::new(0.05, 0.5), size).unwrap();
    let marker = rect.min + egui::vec2(near_left_crop_edge.x as f32, near_left_crop_edge.y as f32);
    assert!(
        !screen_rect_inside_plan_crop(
            context,
            camera,
            rect,
            size,
            egui::Rect::from_center_size(marker, egui::vec2(12.0, 12.0)),
        )
        .unwrap()
    );
    let comfortably_inside = camera.project(Point2::new(0.5, 0.5), size).unwrap();
    let label = rect.min + egui::vec2(comfortably_inside.x as f32, comfortably_inside.y as f32);
    assert!(
        screen_rect_inside_plan_crop(
            context,
            camera,
            rect,
            size,
            egui::Rect::from_center_size(label, egui::vec2(30.0, 18.0)),
        )
        .unwrap()
    );
}

fn install_transform_openings(
    h: &mut Harness,
    pane_position: os_model::WindowPanePosition,
    typed_wall: bool,
) -> (Id, Id, Option<Id>) {
    use os_model::{
        LayerFunction, Opening, OpeningDefinition, OpeningKind, OpeningParams, OpeningType,
        OpeningTypeParams, WallLayer, WallType, WallTypeAssignment, WallTypeParams,
    };

    let mut model = h.app.editor.document.model().clone();
    let door_type = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            family: Default::default(),
            name: "Transform door".into(),
            kind: OpeningKind::Door,
            width: 0.75,
            height: 2.1,
            sill: 0.0,
            pane_position: Default::default(),
        },
    );
    let door_type_id = door_type.id();
    model.opening_types.insert(door_type_id, door_type);
    let window_type = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            family: Default::default(),
            name: "Transform window".into(),
            kind: OpeningKind::Window,
            width: 0.45,
            height: 1.2,
            sill: 0.5,
            pane_position,
        },
    );
    let window_type_id = window_type.id();
    model.opening_types.insert(window_type_id, window_type);
    let door = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "Preserve door".into(),
            host: h.wall,
            offset: 0.2,
            definition: OpeningDefinition::Typed {
                type_id: door_type_id,
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: os_model::DoorHinge::End,
            swing: os_model::DoorSwing::Left,
        },
    );
    let door_id = door.id();
    model.openings.insert(door_id, door);
    let window = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "Preserve window".into(),
            host: h.wall,
            offset: 1.2,
            definition: OpeningDefinition::Typed {
                type_id: window_type_id,
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
    let window_id = window.id();
    model.openings.insert(window_id, window);

    let wall_type_id = if typed_wall {
        let thickness = model.walls[&h.wall].parameters.thickness;
        let wall_type = WallType::new(
            "core.wall_type",
            WallTypeParams {
                name: "Transform compound wall".into(),
                layers: vec![WallLayer {
                    id: Id::new(),
                    name: "Structure".into(),
                    thickness,
                    function: LayerFunction::Structure,
                    material: None,
                }],
            },
        );
        let id = wall_type.id();
        model.wall_types.insert(id, wall_type);
        model.wall_type_assignments.insert(
            h.wall,
            WallTypeAssignment {
                type_id: id,
                flipped: false,
            },
        );
        Some(id)
    } else {
        None
    };

    h.app.editor.document = Document::from_model(model).unwrap();
    h.app
        .editor
        .pending_geometry
        .extend([h.wall, door_id, window_id]);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();
    (door_id, window_id, wall_type_id)
}

fn add_trim_boundary(h: &mut Harness, x: f64) -> Id {
    let mut model = h.app.editor.document.model().clone();
    let mut parameters = model.walls[&h.wall].parameters.clone();
    parameters.name = "Trim boundary".into();
    *parameters.path.straight_start_mut().unwrap() = Point2::new(x, -1.0);
    *parameters.path.straight_end_mut().unwrap() = Point2::new(x, 1.0);
    let boundary = os_model::Wall::new(os_walls::WALL_TYPE, parameters);
    let id = boundary.id();
    model.walls.insert(id, boundary);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();
    id
}

#[test]
fn trim_extend_start_and_end_keep_hosted_openings_and_commit_once_at_both_dpis() {
    use crate::plan_gesture::WallEdit;
    use os_model::WindowPanePosition;

    for (size, scale) in PROFILES {
        for (mode, boundary_x, expected_start, expected_end) in [
            (
                WallEdit::TrimStart,
                -1.5,
                Point2::new(-1.5, 0.0),
                Point2::new(1.0, 0.0),
            ),
            (
                WallEdit::TrimEnd,
                1.5,
                Point2::new(-1.0, 0.0),
                Point2::new(1.5, 0.0),
            ),
        ] {
            let mut h = Harness::new(size, scale);
            let (door, window, _) =
                install_transform_openings(&mut h, WindowPanePosition::Center, true);
            let boundary = add_trim_boundary(&mut h, boundary_x);
            h.app.plans.split = true;
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let revision = h.app.editor.document.revision();
            let wall_before = before.walls[&h.wall].clone();
            let door_before = before.openings[&door].clone();
            let window_before = before.openings[&window].clone();
            let door_world_start = Point2::new(
                wall_before.parameters.start().x
                    + (wall_before.parameters.end().x - wall_before.parameters.start().x)
                        / wall_before.parameters.length()
                        * door_before.parameters.offset,
                wall_before.parameters.start().y
                    + (wall_before.parameters.end().y - wall_before.parameters.start().y)
                        / wall_before.parameters.length()
                        * door_before.parameters.offset,
            );
            let window_world_start = Point2::new(
                wall_before.parameters.start().x
                    + (wall_before.parameters.end().x - wall_before.parameters.start().x)
                        / wall_before.parameters.length()
                        * window_before.parameters.offset,
                wall_before.parameters.start().y
                    + (wall_before.parameters.end().y - wall_before.parameters.start().y)
                        / wall_before.parameters.length()
                        * window_before.parameters.offset,
            );

            let label = if mode == WallEdit::TrimStart {
                "Trim/Extend start"
            } else {
                "Trim/Extend end"
            };
            h.click_text_in_band(label, 0.0, h.size.y);
            h.frame(vec![]);
            assert!(h.app.wall_gesture.as_ref().unwrap().is_trim_extend());
            let boundary_hit = h.point(Point2::new(boundary_x, 0.0));
            assert!(h.app.plans.canvas_rect.unwrap().contains(boundary_hit));
            h.frame(vec![egui::Event::PointerMoved(boundary_hit)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.revision(), revision);
            assert_eq!(h.app.editor.scene, scene);
            assert!(h.output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::LineSegment { stroke, .. }
                    if stroke.width == 2.0 && stroke.color == theme::ACCENT
            )));

            h.click(boundary_hit);
            h.settle();
            h.clean();
            assert!(!h.app.status_error, "{}", h.app.status);
            let edited = h.app.editor.document.model().clone();
            let wall = &edited.walls[&h.wall];
            assert_eq!(wall.header, wall_before.header);
            assert_eq!(wall.parameters.level, wall_before.parameters.level);
            assert_eq!(wall.parameters.material, wall_before.parameters.material);
            assert_eq!(wall.parameters.thickness, wall_before.parameters.thickness);
            assert_eq!(wall.parameters.height, wall_before.parameters.height);
            assert!(wall.parameters.start().distance(expected_start) < 1e-9);
            assert!(wall.parameters.end().distance(expected_end) < 1e-9);
            assert!(edited.walls.contains_key(&boundary));
            assert_eq!(edited.openings[&door].id(), door);
            assert_eq!(edited.openings[&window].id(), window);
            if mode == WallEdit::TrimStart {
                assert!((edited.openings[&door].parameters.offset - 0.7).abs() < 1e-9);
                assert!((edited.openings[&window].parameters.offset - 1.7).abs() < 1e-9);
            } else {
                assert_eq!(
                    edited.openings[&door].parameters.offset,
                    door_before.parameters.offset
                );
                assert_eq!(
                    edited.openings[&window].parameters.offset,
                    window_before.parameters.offset
                );
            }
            let direction = Point2::new(
                (wall.parameters.end().x - wall.parameters.start().x) / wall.parameters.length(),
                (wall.parameters.end().y - wall.parameters.start().y) / wall.parameters.length(),
            );
            let door_world_after = Point2::new(
                wall.parameters.start().x + direction.x * edited.openings[&door].parameters.offset,
                wall.parameters.start().y + direction.y * edited.openings[&door].parameters.offset,
            );
            let window_world_after = Point2::new(
                wall.parameters.start().x
                    + direction.x * edited.openings[&window].parameters.offset,
                wall.parameters.start().y
                    + direction.y * edited.openings[&window].parameters.offset,
            );
            assert!(door_world_after.distance(door_world_start) < 1e-9);
            assert!(window_world_after.distance(window_world_start) < 1e-9);
            assert_eq!(h.app.editor.document.revision(), revision + 1);
            assert_ne!(h.app.editor.scene, scene);

            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &edited);
        }
    }
}

#[test]
fn trim_extend_rejects_invalid_opening_fit_without_partial_mutation() {
    use crate::plan_gesture::WallEdit;
    use os_model::WindowPanePosition;

    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    install_transform_openings(&mut h, WindowPanePosition::Center, true);
    let boundary = add_trim_boundary(&mut h, 0.0);
    let before = h.app.editor.document.model().clone();
    let revision = h.app.editor.document.revision();
    h.app
        .begin_plan_wall_edit(h.view, h.wall, WallEdit::TrimStart);
    h.frame(vec![]);
    let hit = h.point(Point2::new(0.0, 0.0));
    h.click(hit);
    assert!(h.app.status_error);
    assert!(h.app.status.contains("opening"), "{}", h.app.status);
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.document.revision(), revision);
    assert!(!h.app.editor.document.can_undo());
    assert!(h.app.editor.document.model().walls.contains_key(&boundary));
    h.frame(vec![escape()]);
    h.frame(vec![]);
    h.clean();
    assert_eq!(h.app.editor.document.model(), &before);
}

#[test]
fn wall_rotate_pointer_preview_commit_preserves_openings_and_history_at_both_dpis() {
    use crate::plan_workspace::transforms::Mode;
    use os_model::WindowPanePosition;

    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (door, window, _) =
            install_transform_openings(&mut h, WindowPanePosition::Center, true);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let revision = h.app.editor.document.revision();
        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
        let original_wall = before.walls[&h.wall].clone();
        let original_door = before.openings[&door].clone();
        let original_window = before.openings[&window].clone();
        h.app.plans.split = true;
        h.frame(vec![]);

        h.app.begin_wall_transform(Mode::Rotate);
        h.frame(vec![]);
        assert!(h.app.plans.transform.is_some());
        let exact = {
            let context = h.app.editor.native_plan_context(h.view).unwrap();
            let draft = h.app.plans.transform.as_mut().unwrap();
            draft.degrees = "90".into();
            draft
                .candidate(context.basis.world_to_plane(Point2::new(0.0, 1.0)).unwrap())
                .unwrap()
                .model
        };
        assert!((exact.walls[&h.wall].parameters.start().x).abs() < 1e-12);
        assert!((exact.walls[&h.wall].parameters.start().y + 1.0).abs() < 1e-12);
        assert!((exact.walls[&h.wall].parameters.end().x).abs() < 1e-12);
        assert!((exact.walls[&h.wall].parameters.end().y - 1.0).abs() < 1e-12);
        h.app.plans.transform.as_mut().unwrap().degrees.clear();
        let origin = h.point(original_wall.parameters.end());
        let destination = h.point(Point2::new(0.0, 1.0));
        h.press(origin);
        h.frame(vec![egui::Event::PointerMoved(destination)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert!(!h.app.editor.document.can_undo());
        assert_eq!(h.app.editor.scene, scene);
        assert!(h.output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::LineSegment { stroke, .. }
                if stroke.width == 2.0 && stroke.color == theme::ACCENT
        )));

        h.release(destination);
        h.frame(vec![]);
        h.settle();
        assert!(!h.app.status_error, "{}", h.app.status);
        assert!(h.app.plans.transform.is_none());
        assert_ne!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
        let edited = h.app.editor.document.model().clone();
        let edited_wall = &edited.walls[&h.wall];
        assert_eq!(edited_wall.header, original_wall.header);
        assert_eq!(edited_wall.parameters.name, original_wall.parameters.name);
        assert_eq!(edited_wall.parameters.level, original_wall.parameters.level);
        assert_eq!(
            edited_wall.parameters.height,
            original_wall.parameters.height
        );
        assert_eq!(
            edited_wall.parameters.thickness,
            original_wall.parameters.thickness
        );
        assert_eq!(
            edited_wall.parameters.material,
            original_wall.parameters.material
        );
        assert!((edited_wall.parameters.start().x).abs() < 1e-12);
        assert!((edited_wall.parameters.start().y + 1.0).abs() < 1e-12);
        assert!((edited_wall.parameters.end().x).abs() < 1e-12);
        assert!((edited_wall.parameters.end().y - 1.0).abs() < 1e-12);
        assert_eq!(edited.openings[&door], original_door);
        assert_eq!(edited.openings[&window], original_window);
        assert_eq!(edited.wall_type_assignments, before.wall_type_assignments);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_ne!(h.app.editor.scene, scene);

        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(!h.app.editor.document.can_undo());
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &edited);
    }
}

#[test]
fn wall_mirror_window_panes_preserve_reflected_geometry_and_siblings_both_dpis() {
    use crate::plan_workspace::transforms::{Draft, Mode};
    use os_model::WindowPanePosition::{Center, LeftFace, RightFace};
    for (size, scale) in PROFILES {
        for pane in [LeftFace, RightFace, Center] {
            for pinned in [false, true] {
                let mut h = Harness::new(size, scale);
                let (_, window, _) =
                    install_transform_openings(&mut h, if pinned { Center } else { pane }, false);
                let mut model = h.app.editor.document.model().clone();
                let window_type = model.openings[&window].parameters.type_id().unwrap();
                model
                    .opening_types
                    .get_mut(&window_type)
                    .unwrap()
                    .parameters
                    .family
                    .side_lite = Some(os_model::SideLite {
                    side: os_model::LiteSide::Start,
                    width_fraction: 0.25,
                    mullion_width: 0.04,
                    material: None,
                });
                model
                    .openings
                    .get_mut(&window)
                    .unwrap()
                    .parameters
                    .lite_side_override = Some(os_model::LiteSide::End);
                if pinned {
                    model
                        .openings
                        .get_mut(&window)
                        .unwrap()
                        .parameters
                        .pane_position_override = Some(pane);
                }
                let mut wall = model.walls[&h.wall].clone();
                wall.header.id = Id::new();
                wall.parameters.path.straight_start_mut().unwrap().y += 2.;
                wall.parameters.path.straight_end_mut().unwrap().y += 2.;
                let mut sibling = model.openings[&window].clone();
                sibling.header.id = Id::new();
                sibling.parameters.host = wall.id();
                let sibling_id = sibling.id();
                model.walls.insert(wall.id(), wall);
                model.openings.insert(sibling_id, sibling);
                h.app.editor.document = Document::from_model(model).unwrap();
                h.app.editor.regenerate().unwrap();
                h.app.plans.poll(&h.app.editor);
                h.app.focus_plan(Some(h.view));
                h.app.select(Some(h.wall));
                h.settle();
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let revision = h.app.editor.document.revision();
                let context = h.app.editor.native_plan_context(h.view).unwrap();
                let mut draft = Draft::begin(&h.app, Mode::Mirror).unwrap();
                draft.axis = Some(context.basis.world_to_plane(Point2::new(0., -0.5)).unwrap());
                let end = context.basis.world_to_plane(Point2::new(0., 0.5)).unwrap();
                let candidate = draft.candidate(end).unwrap();
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.scene, scene);
                assert_eq!(h.app.editor.document.revision(), revision);
                assert!(!h.app.editor.document.can_undo());
                let old = crate::opening_tools::panel_mesh(&before, window).unwrap();
                let new = crate::opening_tools::panel_mesh(&candidate.model, window).unwrap();
                for v in &old.vertices {
                    assert!(new.vertices.iter().any(|n| (n.x + v.x).abs() < 1e-10
                        && (n.y - v.y).abs() < 1e-10
                        && (n.z - v.z).abs() < 1e-10));
                }
                let expected = match pane {
                    LeftFace => Some(RightFace),
                    RightFace => Some(LeftFace),
                    Center => before.openings[&window].parameters.pane_position_override,
                };
                let mut opening = before.openings[&window].clone();
                opening.parameters.pane_position_override = expected;
                assert_eq!(
                    opening.parameters.lite_side_override,
                    Some(os_model::LiteSide::End),
                    "mirror preserves endpoint-relative lite handedness"
                );
                assert_eq!(candidate.model.openings[&window], opening);
                assert_eq!(
                    candidate.model.openings[&sibling_id],
                    before.openings[&sibling_id]
                );
                assert_eq!(candidate.model.opening_types, before.opening_types);
                draft.commit(&mut h.app, end).unwrap();
                assert_eq!(h.app.editor.document.model(), &candidate.model);
                assert_eq!(h.app.editor.document.revision(), revision + 1);
                assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &before);
                h.app.history(true);
                assert_eq!(h.app.editor.document.model(), &candidate.model);
            }
        }
    }
}

#[test]
fn wall_mirror_preview_commits_door_and_layer_handedness_once() {
    use crate::plan_workspace::transforms::Mode;
    use os_model::{DoorHinge, DoorSwing, WindowPanePosition};

    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (door, window, wall_type) =
            install_transform_openings(&mut h, WindowPanePosition::Center, true);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let revision = h.app.editor.document.revision();
        let door_before = before.openings[&door].clone();
        let window_before = before.openings[&window].clone();
        h.app.plans.split = true;
        h.frame(vec![]);

        h.app.begin_wall_transform(Mode::Mirror);
        h.frame(vec![]);
        let axis_start = h.point(Point2::new(0.0, -0.5));
        h.click(axis_start);
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(h.app.plans.transform.as_ref().unwrap().axis.is_some());
        let axis_end = h.point(Point2::new(0.0, 0.5));
        h.frame(vec![egui::Event::PointerMoved(axis_end)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(h.app.editor.scene, scene);
        h.click(axis_end);
        h.frame(vec![]);
        assert!(!h.app.status_error, "{}", h.app.status);
        let edited = h.app.editor.document.model().clone();
        let wall = &edited.walls[&h.wall];
        assert_eq!(wall.header, before.walls[&h.wall].header);
        assert!((wall.parameters.start().x - 1.0).abs() < 1e-12);
        assert!(wall.parameters.start().y.abs() < 1e-12);
        assert!((wall.parameters.end().x + 1.0).abs() < 1e-12);
        assert!(wall.parameters.end().y.abs() < 1e-12);
        assert_eq!(
            edited.wall_type_assignments[&h.wall].type_id,
            wall_type.unwrap()
        );
        assert!(edited.wall_type_assignments[&h.wall].flipped);
        assert_eq!(edited.openings[&door].id(), door_before.id());
        assert_eq!(
            edited.openings[&door].parameters.host,
            door_before.parameters.host
        );
        assert_eq!(
            edited.openings[&door].parameters.offset,
            door_before.parameters.offset
        );
        assert_eq!(edited.openings[&door].parameters.hinge, DoorHinge::End);
        assert_eq!(edited.openings[&door].parameters.swing, DoorSwing::Right);
        assert_eq!(edited.openings[&window], window_before);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_ne!(h.app.editor.scene, scene);

        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &edited);
    }
}

#[test]
fn wall_mirror_reflects_across_horizontal_and_diagonal_axes() {
    use crate::plan_workspace::transforms::{Draft, Mode};

    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    model
        .walls
        .get_mut(&h.wall)
        .unwrap()
        .parameters
        .path
        .straight_start_mut()
        .unwrap()
        .y = 0.5;
    model
        .walls
        .get_mut(&h.wall)
        .unwrap()
        .parameters
        .path
        .straight_end_mut()
        .unwrap()
        .y = 0.5;
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();

    let context = h.app.editor.native_plan_context(h.view).unwrap();
    for (axis_start, axis_end, expected_start, expected_end) in [
        (
            Point2::new(-0.8, 0.0),
            Point2::new(0.8, 0.0),
            Point2::new(-1.0, -0.5),
            Point2::new(1.0, -0.5),
        ),
        (
            Point2::new(-0.8, -0.8),
            Point2::new(0.8, 0.8),
            Point2::new(0.5, -1.0),
            Point2::new(0.5, 1.0),
        ),
    ] {
        let mut draft = Draft::begin(&h.app, Mode::Mirror).unwrap();
        draft.axis = Some(context.basis.world_to_plane(axis_start).unwrap());
        let candidate = draft
            .candidate(context.basis.world_to_plane(axis_end).unwrap())
            .unwrap();
        let wall = &candidate.model.walls[&h.wall].parameters;
        assert!(wall.start().distance(expected_start) < 1e-12);
        assert!(wall.end().distance(expected_end) < 1e-12);
        assert_eq!(
            h.app.editor.document.model().walls[&h.wall]
                .parameters
                .start()
                .y,
            0.5
        );
        assert_eq!(
            h.app.editor.document.model().walls[&h.wall]
                .parameters
                .end()
                .y,
            0.5
        );
    }
}

#[test]
fn wall_rotation_uses_rotated_plan_basis_at_large_coordinates() {
    use crate::plan_workspace::transforms::Mode;
    use os_model::{PlanSettings, PlanViewBasis};

    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let base = 100_000_000.0;
    let mut model = h.app.editor.document.model().clone();
    let wall = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
    *wall.path.straight_start_mut().unwrap() = Point2::new(base - 1.0, base);
    *wall.path.straight_end_mut().unwrap() = Point2::new(base + 1.0, base);
    let view = model.views.get_mut(&h.view).unwrap();
    let mut settings = view.parameters.plan.unwrap();
    settings.basis = PlanViewBasis {
        origin: Point2::new(base, base),
        rotation: 0.63,
    };
    view.parameters.plan = Some(PlanSettings { ..settings });
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();

    let before = h.app.editor.document.model().clone();
    h.app.begin_wall_transform(Mode::Rotate);
    h.frame(vec![]);
    let origin = h.point(before.walls[&h.wall].parameters.end());
    let destination = h.point(Point2::new(base, base + 1.0));
    h.press(origin);
    h.frame(vec![egui::Event::PointerMoved(destination)]);
    assert_eq!(h.app.editor.document.model(), &before);
    h.release(destination);
    h.frame(vec![]);
    assert!(!h.app.status_error, "{}", h.app.status);
    let edited = &h.app.editor.document.model().walls[&h.wall].parameters;
    assert!(edited.start().distance(Point2::new(base, base - 1.0)) < 1e-6);
    assert!(edited.end().distance(Point2::new(base, base + 1.0)) < 1e-6);
    assert!((edited.length() - 2.0).abs() < 1e-7);
}

#[test]
fn wall_transform_actions_open_from_existing_snaps_menu_without_resizing_canvas() {
    use crate::plan_workspace::transforms::Mode;

    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let canvas = h.app.plans.canvas_rect.unwrap();
        h.click_text_in_band("Snaps", 0.0, h.size.y);
        h.frame(vec![]);
        h.click_text_in_band("Wall transforms", 0.0, h.size.y);
        h.frame(vec![]);
        h.click_text_in_band("Rotate wall", 0.0, h.size.y);
        h.frame(vec![]);
        assert!(h.app.plans.transform.is_some());
        assert_eq!(h.app.plans.transform.as_ref().unwrap().mode, Mode::Rotate);
        assert_eq!(h.app.plans.canvas_rect.unwrap(), canvas);
        h.frame(vec![escape()]);
        h.frame(vec![]);
        assert!(h.app.plans.transform.is_none());
    }
}

#[test]
fn wall_transform_rejects_joined_walls_and_degenerate_axes() {
    use crate::plan_workspace::transforms::{Draft, Mode};
    use os_model::{WallAnchor, WallEndpoint, WallJoin, WallJoinParams, WindowPanePosition};

    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    let mut peer = model.walls[&h.wall].parameters.clone();
    *peer.path.straight_start_mut().unwrap() = Point2::new(1.0, 0.0);
    *peer.path.straight_end_mut().unwrap() = Point2::new(3.0, 0.0);
    let peer = os_model::Wall::new(os_walls::WALL_TYPE, peer);
    let peer_id = peer.id();
    model.walls.insert(peer_id, peer);
    let join = WallJoin::new(
        "core.wall_join",
        WallJoinParams::Butt {
            a: WallAnchor {
                wall: h.wall,
                endpoint: WallEndpoint::End,
            },
            b: WallAnchor {
                wall: peer_id,
                endpoint: WallEndpoint::Start,
            },
        },
    );
    model.wall_joins.insert(join.id(), join);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();
    assert!(Draft::begin(&h.app, Mode::Rotate).is_err());
    assert!(Draft::begin(&h.app, Mode::Mirror).is_err());
    assert!(Draft::begin(&h.app, Mode::Align).is_err());

    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    install_transform_openings(&mut h, WindowPanePosition::LeftFace, false);
    assert!(Draft::begin(&h.app, Mode::Mirror).is_ok());

    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    h.app.begin_wall_transform(Mode::Mirror);
    h.frame(vec![]);
    let draft = h.app.plans.transform.as_mut().unwrap();
    draft.axis = Some(Point2::new(0.0, 0.0));
    assert!(draft.candidate(Point2::new(0.0, 0.0)).is_err());
    assert_eq!(h.app.editor.document.revision(), 0);
    assert!(!h.app.editor.document.can_undo());

    for hidden in [true, false] {
        let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
        let view = h.app.editor.document.model().views[&h.view]
            .parameters
            .clone();
        let mut settings = view.plan.unwrap();
        if hidden {
            settings.visibility.walls = false;
        } else {
            settings.crop = Some(os_model::PlanViewCrop {
                min: Point2::new(10.0, 10.0),
                max: Point2::new(20.0, 20.0),
            });
        }
        h.app
            .editor
            .update_floor_plan(h.view, &view.name, view.level.unwrap(), settings)
            .unwrap();
        h.settle();
        assert!(Draft::begin(&h.app, Mode::Rotate).is_err());
        assert!(Draft::begin(&h.app, Mode::Align).is_err());
    }
}

#[test]
fn wall_transform_escape_and_stale_context_cancel_without_mutation() {
    use crate::plan_workspace::transforms::Mode;

    for mode in [Mode::Rotate, Mode::Align, Mode::Split] {
        for cause in [
            "escape",
            "revision",
            "session",
            "selection",
            "provider",
            "view",
            "drawing",
            "pointer gone",
        ] {
            let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
            let before = h.app.editor.document.model().clone();
            h.app.begin_wall_transform(mode);
            h.frame(vec![]);
            match cause {
                "escape" => h.frame(vec![escape()]),
                "revision" => {
                    h.app
                        .editor
                        .command("Concurrent edit", Command::RenameProject("Changed".into()))
                        .unwrap();
                    h.frame(vec![]);
                }
                "session" => {
                    h.app.editor.document = Document::from_model(before.clone()).unwrap();
                    h.frame(vec![]);
                }
                "selection" => {
                    h.app.select(None);
                    h.frame(vec![]);
                }
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                    h.frame(vec![]);
                }
                "view" => {
                    h.app.focus_plan(None);
                    h.frame(vec![]);
                }
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap());
                    h.frame(vec![]);
                }
                "pointer gone" => h.frame(vec![egui::Event::PointerGone]),
                _ => unreachable!(),
            }
            assert!(h.app.plans.transform.is_none(), "{cause}");
            assert!(!h.app.plans.transform_claimed, "{cause}");
            if cause != "revision" {
                assert_eq!(h.app.editor.document.model(), &before, "{cause}");
                assert!(!h.app.editor.document.can_undo(), "{cause}");
            } else {
                assert!(
                    h.app.editor.document.can_undo(),
                    "concurrent edit remains undoable"
                );
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &before, "{cause}");
            }
        }
    }
}

fn add_align_reference(h: &mut Harness) -> Id {
    let mut model = h.app.editor.document.model().clone();
    let mut parameters = model.walls[&h.wall].parameters.clone();
    // Short antiparallel target: its infinite centerline defines the alignment.
    *parameters.path.straight_start_mut().unwrap() = Point2::new(0.5, 0.8);
    *parameters.path.straight_end_mut().unwrap() = Point2::new(-0.5, 0.8);
    let wall = os_model::Wall::new(os_walls::WALL_TYPE, parameters);
    let id = wall.id();
    model.walls.insert(id, wall);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();
    id
}

#[test]
fn wall_align_menu_preview_commit_openings_and_pan_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        install_transform_openings(&mut h, os_model::WindowPanePosition::LeftFace, true);
        add_align_reference(&mut h);
        h.app.plans.split = true;
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let revision = h.app.editor.document.revision();
        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
        let camera = h.app.plans.cameras[&h.view];
        let canvas = h.app.plans.canvas_rect;
        h.click_text_in_band("Snaps", 0.0, h.size.y);
        h.frame(vec![]);
        h.click_text_in_band("Wall transforms", 0.0, h.size.y);
        h.frame(vec![]);
        h.click_text_in_band("Align wall", 0.0, h.size.y);
        h.frame(vec![]);
        assert_eq!(h.app.plans.canvas_rect, canvas);
        assert!(h.app.plans.transform.is_some());
        let target = h.point(Point2::new(0.0, 0.8));
        h.frame(vec![egui::Event::PointerMoved(target)]);
        // Start on a source endpoint: transform takes precedence over its grip.
        h.press(h.point(Point2::new(1.0, 0.0)));
        h.frame(vec![egui::Event::PointerMoved(target)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert!(!h.app.editor.document.can_undo());
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        assert_eq!(h.app.selected, Some(h.wall));
        assert!(h.app.plans.endpoint_drag.is_none());
        assert!(h.app.wall_gesture.is_none());
        assert!(h.output.shapes.iter().any(|s| matches!(
            &s.shape, egui::Shape::LineSegment { stroke, .. }
                if stroke.width == 2.0 && stroke.color == theme::ACCENT
        )));
        h.release(target);
        h.settle();
        assert!(!h.app.status_error, "{}", h.app.status);
        assert!(h.app.plans.transform.is_none());
        let mut expected = before.clone();
        let wall = &mut expected.walls.get_mut(&h.wall).unwrap().parameters;
        wall.path.straight_start_mut().unwrap().y = 0.8;
        wall.path.straight_end_mut().unwrap().y = 0.8;
        // Entire model equality includes reference wall, all wall properties,
        // type assignment, and every hosted door/window ID and parameter.
        assert_eq!(h.app.editor.document.model(), &expected);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_ne!(h.app.editor.scene, scene);
        assert_ne!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(!h.app.editor.document.can_undo());
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &expected);
        h.settle();
        let p = h.point(Point2::new(0.0, -0.7));
        let camera = h.app.plans.cameras[&h.view];
        h.press(p);
        let end = p + egui::vec2(30.0, 20.0);
        h.frame(vec![egui::Event::PointerMoved(end)]);
        h.release(end);
        assert_ne!(h.app.plans.cameras[&h.view], camera);
        assert_eq!(h.app.editor.document.model(), &expected);
    }
}

#[test]
fn wall_align_rejects_invalid_targets_without_history() {
    use crate::plan_workspace::transforms::{Draft, Mode};
    for case in ["self", "empty", "skew", "level", "crop", "zero"] {
        let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
        let target = add_align_reference(&mut h);
        let mut model = h.app.editor.document.model().clone();
        let mut point = Point2::new(0.0, 0.8);
        match case {
            "self" => point = Point2::new(0.8, 0.0),
            "empty" => point = Point2::new(0.0, -0.8),
            "skew" => {
                model
                    .walls
                    .get_mut(&target)
                    .unwrap()
                    .parameters
                    .path
                    .straight_end_mut()
                    .unwrap()
                    .y = 0.5;
                point = Point2::new(0.0, 0.65);
            }
            "level" => {
                let level = os_model::Level::new(
                    "core.level",
                    os_model::LevelParams {
                        name: "Other level".into(),
                        elevation: 0.0,
                        building: model.levels[&model.walls[&h.wall].parameters.level]
                            .parameters
                            .building,
                    },
                );
                model.walls.get_mut(&target).unwrap().parameters.level = level.id();
                model.levels.insert(level.id(), level);
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
                    min: Point2::new(-2.0, -1.0),
                    max: Point2::new(2.0, 0.5),
                });
            }
            "zero" => {
                let wall = &mut model.walls.get_mut(&target).unwrap().parameters;
                wall.path.straight_start_mut().unwrap().y = 0.0;
                wall.path.straight_end_mut().unwrap().y = 0.0;
                point = Point2::new(0.0, 0.0);
            }
            _ => unreachable!(),
        }
        h.app.editor.document = Document::from_model(model).unwrap();
        h.app.editor.regenerate().unwrap();
        h.app.plans.poll(&h.app.editor);
        h.app.focus_plan(Some(h.view));
        h.app.select(Some(h.wall));
        h.settle();
        let before = h.app.editor.document.model().clone();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let draft = Draft::begin(&h.app, Mode::Align).unwrap();
        assert!(
            draft
                .candidate(context.basis.world_to_plane(point).unwrap())
                .is_err(),
            "{case}"
        );
        h.app.begin_wall_transform(Mode::Align);
        h.frame(vec![]);
        h.click(h.point(point));
        assert_eq!(h.app.editor.document.model(), &before, "{case}");
        assert!(!h.app.editor.document.can_undo(), "{case}");
    }
}

#[test]
fn wall_align_rotated_basis_at_large_coordinates() {
    use crate::plan_workspace::transforms::{Draft, Mode};
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    add_align_reference(&mut h);
    let base = 100_000_000.0;
    let mut model = h.app.editor.document.model().clone();
    for wall in model.walls.values_mut() {
        wall.parameters.path.straight_start_mut().unwrap().x += base;
        wall.parameters.path.straight_start_mut().unwrap().y += base;
        wall.parameters.path.straight_end_mut().unwrap().x += base;
        wall.parameters.path.straight_end_mut().unwrap().y += base;
    }
    model
        .views
        .get_mut(&h.view)
        .unwrap()
        .parameters
        .plan
        .as_mut()
        .unwrap()
        .basis = os_model::PlanViewBasis {
        origin: Point2::new(base, base),
        rotation: 0.63,
    };
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();
    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let candidate = Draft::begin(&h.app, Mode::Align)
        .unwrap()
        .candidate(
            context
                .basis
                .world_to_plane(Point2::new(base, base + 0.8))
                .unwrap(),
        )
        .unwrap();
    let wall = &candidate.model.walls[&h.wall].parameters;
    assert!(wall.start().distance(Point2::new(base - 1.0, base + 0.8)) < 1e-6);
    assert!(wall.end().distance(Point2::new(base + 1.0, base + 0.8)) < 1e-6);
}

#[cfg(feature = "external-plugins")]
#[test]
#[ignore = "requires explicitly installed independent Wall guest via OPENSTRUCTURE_WALL_TEST_PLUGIN"]
fn wall_align_installed_worker_commits_or_cancels_atomically() {
    use crate::plan_workspace::transforms::{Draft, Mode};
    let directory =
        std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN").expect("Wall guest directory");
    for cancel in [false, true] {
        let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
        install_transform_openings(&mut h, os_model::WindowPanePosition::LeftFace, true);
        add_align_reference(&mut h);
        h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
        h.app
            .editor
            .host
            .load_wasm_directory(
                Path::new(&directory),
                [
                    Permission::ModelRead,
                    Permission::ModelWrite,
                    Permission::UiTool,
                ]
                .into(),
            )
            .unwrap();
        h.settle();
        let before = h.app.editor.document.model().clone();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let point = context.basis.world_to_plane(Point2::new(0.0, 0.8)).unwrap();
        let draft = Draft::begin(&h.app, Mode::Align).unwrap();
        let expected = draft.candidate(point).unwrap().model;
        assert!(!h.app.editor.plugin_work_pending());
        assert_eq!(h.app.editor.document.model(), &before);
        draft.commit(&mut h.app, point).unwrap();
        assert!(h.app.editor.plugin_work_pending());
        assert!(Draft::begin(&h.app, Mode::Align).is_err());
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(!h.app.editor.document.can_undo());
        if cancel {
            h.frame(vec![escape()]);
        }
        h.settle();
        if cancel {
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
        } else {
            assert!(!h.app.status_error, "{}", h.app.status);
            assert_eq!(h.app.editor.document.model(), &expected);
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &expected);
        }
    }
}
