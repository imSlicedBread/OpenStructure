//! Real egui frames at both requested DPI profiles; no native-window claim.
use super::wall_set_copy_tests::{Bay, fixture, load};
use super::*;
use crate::plan_workspace::{groups, wall_set_move::Draft};

fn setup(size: egui::Vec2, scale: f32) -> (Harness, Bay) {
    let mut h = Harness::new(size, scale);
    let (mut model, bay) = fixture(&mut h);
    model.rooms.clear();
    model.room_tags.clear();
    model.dimensions.clear();
    load(&mut h, model, &bay.walls);
    h.app.plans.cameras.insert(
        h.view,
        PlanCamera {
            center: Point2::new(4.0, 1.0),
            pixels_per_metre: 28.0,
        },
    );
    h.app.plans.group_name = "Door and window bay".into();
    h.frame(vec![]);
    (h, bay)
}

fn toolbar(h: &mut Harness, label: &str) {
    h.frame(vec![]);
    h.click_text_in_band(label, 100.0, h.size.y);
    assert!(!h.app.status_error, "{}: {}", label, h.app.status);
}

#[test]
fn groups_toolbar_prepared_preview_commit_history_unique_and_ungroup_both_dpis() {
    for (size, scale) in PROFILES {
        let (mut h, bay) = setup(size, scale);
        let ungrouped = h.app.editor.document.model().clone();
        toolbar(&mut h, "Create group");
        h.settle();
        let source = h.app.selected_group().unwrap();
        assert_eq!(h.app.selected, Some(source));
        assert_eq!(h.app.selected_ids, [source].into());
        let before = h.app.editor.document.model().clone();
        assert_eq!(before.walls, ungrouped.walls);
        assert_eq!(before.openings, ungrouped.openings);
        assert_eq!(before.opening_tags, ungrouped.opening_tags);
        assert!(
            h.circles().is_empty(),
            "group selection must not expose wall endpoint grips"
        );
        let history = h.app.editor.document.history_stats();
        let scene = h.app.editor.scene.clone();
        toolbar(&mut h, "Duplicate group");
        h.frame(vec![]);
        h.click(h.point(Point2::new(2.0, 3.0)));
        {
            let draft = h.app.plans.wall_set_move.as_mut().unwrap();
            draft.gesture.length = "6".into();
            draft.gesture.angle_degrees = "0".into();
        }
        let destination = Point2::new(8.0, 3.0);
        h.frame(vec![egui::Event::PointerMoved(h.point(destination))]);
        let draft = h.app.plans.wall_set_move.as_ref().unwrap();
        let expected = draft.candidate(destination).unwrap().model;
        assert_eq!(expected, draft.candidate(destination).unwrap().model);
        let duplicate = draft.copied_group().unwrap();
        h.app.plans.wall_set_move.as_mut().unwrap().gesture.length = "0.2".into();
        assert!(
            h.app
                .plans
                .wall_set_move
                .as_ref()
                .unwrap()
                .candidate(Point2::new(2.2, 3.0))
                .is_err()
        );
        h.app.plans.wall_set_move.as_mut().unwrap().gesture.length = "6".into();
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.editor.scene, scene);
        // Check actual rendered accent segments for both destination symbols.
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        for opening in expected
            .openings
            .values()
            .filter(|o| !before.openings.contains_key(&o.id()))
        {
            let native =
                os_geometry::walls::NativeWall::from_model(&expected, opening.parameters.host)
                    .unwrap();
            let resolved = expected.resolve_opening(&opening.parameters).unwrap();
            let lines = crate::opening_tools::plan_symbol(
                opening.id(),
                &resolved,
                &native.parameters,
                native.elevation,
                context,
            )
            .unwrap();
            assert!(!lines.is_empty());
            assert!(lines.iter().any(|line| {
                let a = h.point(context.basis.plane_to_world(line.start).unwrap());
                let b = h.point(context.basis.plane_to_world(line.end).unwrap());
                h.output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment { points, stroke } if stroke.color == theme::ACCENT && points[0].distance(a) < 0.1 && points[1].distance(b) < 0.1))
            }), "destination door/window symbol missing");
        }
        for tag in expected
            .opening_tags
            .values()
            .filter(|t| !before.opening_tags.contains_key(&t.id()))
        {
            let position = h.point(tag.parameters.position);
            let label = tag.parameters.label(&expected).0;
            assert!(h.output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == label && text.pos.distance(position) < 15.0)), "destination tag missing");
        }
        h.click(h.point(destination));
        h.settle();
        assert_eq!(h.app.editor.document.model(), &expected, "{}", h.app.status);
        assert_eq!(h.app.selected, Some(duplicate));
        assert_eq!(h.app.selected_ids, [duplicate].into());
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        for source_opening in bay.openings {
            let old = &before.openings[&source_opening];
            assert_eq!(expected.openings[&source_opening], *old);
            let slot = before.group_instances[&source]
                .parameters
                .members
                .iter()
                .find(|(_, id)| **id == source_opening)
                .unwrap()
                .0;
            let copy_id = expected.group_instances[&duplicate].parameters.members[slot];
            let copy = &expected.openings[&copy_id];
            assert_ne!(copy_id, source_opening);
            let mut parameters = old.parameters.clone();
            parameters.host = copy.parameters.host;
            assert_eq!(copy.parameters, parameters);
            assert!(h.app.editor.scene.contains_key(&copy_id));
        }
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &expected);
        h.app.select(Some(duplicate));
        toolbar(&mut h, "Make unique");
        h.settle();
        let unique = h.app.editor.document.model().clone();
        assert_ne!(
            unique.group_instances[&source].parameters.definition,
            unique.group_instances[&duplicate].parameters.definition
        );
        assert_eq!(unique.openings, expected.openings);
        toolbar(&mut h, "Ungroup");
        assert!(
            !h.app
                .editor
                .document
                .model()
                .group_instances
                .contains_key(&duplicate)
        );
        assert_eq!(h.app.editor.document.model().openings, expected.openings);
        let definition =
            &unique.group_definitions[&unique.group_instances[&duplicate].parameters.definition];
        let roots: BTreeSet<_> = definition
            .parameters
            .template
            .walls
            .keys()
            .map(|slot| unique.group_instances[&duplicate].parameters.members[slot])
            .collect();
        assert_eq!(h.app.selected_ids, roots);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &unique);
    }
}

fn hit_point(h: &Harness, id: Id) -> egui::Pos2 {
    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let drawing = h.app.plans.drawing.as_ref().unwrap();
    if let Some(tag) = drawing
        .opening_tags(context)
        .unwrap()
        .iter()
        .find(|tag| tag.entity == id)
    {
        let rect = h.app.plans.canvas_rect.unwrap();
        let bounds = tag
            .bounds(
                context,
                h.app.plans.cameras[&h.view],
                [rect.width() as f64, rect.height() as f64],
            )
            .unwrap()
            .unwrap();
        return rect.min
            + egui::vec2(
                ((bounds[0].x + bounds[1].x) / 2.0) as f32,
                ((bounds[0].y + bounds[1].y) / 2.0) as f32,
            );
    }
    let line = drawing
        .provider_lines(context)
        .unwrap()
        .iter()
        .find(|line| line.entity == id)
        .unwrap();
    h.point(
        context
            .basis
            .plane_to_world(Point2::new(
                (line.start.x + line.end.x) / 2.0,
                (line.start.y + line.end.y) / 2.0,
            ))
            .unwrap(),
    )
}

#[test]
fn groups_click_tab_marquee_member_locks_and_pan_both_dpis() {
    for (size, scale) in PROFILES {
        let (mut h, bay) = setup(size, scale);
        toolbar(&mut h, "Create group");
        h.settle();
        let group = h.app.selected_group().unwrap();
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        for member in bay.openings.into_iter().chain(bay.tags) {
            h.app.select(None);
            h.frame(vec![]);
            let point = hit_point(&h, member);
            h.click(point);
            h.frame(vec![]);
            assert_eq!(h.app.selected, Some(group), "member {member}");
            assert_eq!(h.app.selected_ids, [group].into());
            assert!(h.app.plans.opening_tag_draft.is_none());
            assert!(h.app.plans.opening_move.is_none());
            assert!(h.circles().is_empty());
            h.frame(vec![key_pressed(egui::Key::Tab)]);
            h.click(point);
            assert_eq!(h.app.selected, Some(group));
        }
        assert_eq!(
            groups::hits(
                &before,
                bay.walls
                    .iter()
                    .copied()
                    .chain(bay.openings)
                    .chain(bay.tags)
            ),
            vec![group]
        );
        // Marquee hits a member, normalizes to its owner, and deduplicates members.
        h.app.select(None);
        h.frame(vec![]);
        let a = h.point(Point2::new(-1.5, 3.0));
        let b = h.point(Point2::new(1.5, -1.0));
        let shift = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
        };
        h.frame(vec![egui::Event::PointerMoved(a), shift(a, true)]);
        h.frame(vec![egui::Event::PointerMoved(b)]);
        h.frame(vec![shift(b, false)]);
        assert_eq!(h.app.selected_ids, [group].into());
        // Direct entry points also reject individual member edits and wall-set copies.
        h.app.selected_ids = bay.walls.clone();
        assert!(Draft::begin(&h.app).is_err());
        assert!(Draft::begin_copy(&h.app).is_err());
        assert!(
            crate::plan_gesture::WallGesture::begin_edit(
                &h.app.editor,
                h.view,
                h.wall,
                WallEdit::ResizeEnd
            )
            .is_err()
        );
        h.app.select(Some(group));
        h.frame(vec![]);
        let camera = h.app.plans.cameras[&h.view];
        let from = h.point(Point2::new(4.0, -1.0));
        let to = from + egui::vec2(25.0, 18.0);
        h.press(from);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        h.release(to);
        assert_ne!(h.app.plans.cameras[&h.view], camera);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.app.select(Some(bay.snap_wall));
        assert_eq!(h.app.selected_ids, [bay.snap_wall].into());
    }
}

#[test]
fn groups_cancel_stale_and_snap_safety_both_dpis() {
    for (size, scale) in PROFILES {
        for cause in 0..11 {
            let (mut h, _) = setup(size, scale);
            toolbar(&mut h, "Create group");
            h.settle();
            h.app.begin_group_duplicate();
            h.frame(vec![]);
            let draft = h.app.plans.wall_set_move.as_ref().unwrap();
            let rect = h.app.plans.canvas_rect.unwrap();
            let camera = h.app.plans.cameras[&h.view];
            let snapped = draft
                .target(
                    h.app.plans.drawing.as_ref().unwrap(),
                    camera,
                    [rect.width() as f64, rect.height() as f64],
                    camera
                        .project(
                            Point2::new(10.05, 3.0),
                            [rect.width() as f64, rect.height() as f64],
                        )
                        .unwrap(),
                    SnapOptions {
                        enabled: true,
                        endpoints: true,
                        intersections: false,
                        perpendicular: false,
                        midpoints: false,
                        nearest: false,
                        axis_extensions: false,
                    },
                )
                .unwrap();
            assert!(snapped.distance(Point2::new(10.0, 3.0)) < 1e-8);
            let base = h.point(Point2::new(2.0, 3.0));
            h.click(base);
            let target = h.point(Point2::new(7.0, 3.0));
            h.press(target);
            match cause {
                0 => h.frame(vec![escape()]),
                1 => h.release(egui::pos2(0.0, 0.0)),
                2 => {
                    h.app
                        .editor
                        .document
                        .execute(
                            "Concurrent edit",
                            vec![Command::RenameProject("Changed".into())],
                        )
                        .unwrap();
                }
                3 => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap();
                }
                4 => {
                    h.app.plans.drawing = None;
                }
                5 => {
                    h.app.plans.active = None;
                }
                6 => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                7 => h.frame(vec![egui::Event::PointerGone]),
                8 => {
                    let camera = h.app.plans.cameras[&h.view];
                    h.app.plans.cameras.insert(
                        h.view,
                        PlanCamera {
                            center: Point2::new(camera.center.x + 1.0, camera.center.y),
                            ..camera
                        },
                    );
                }
                9 => {
                    h.app.plans.canvas_rect = Some(
                        h.app
                            .plans
                            .canvas_rect
                            .unwrap()
                            .translate(egui::vec2(1.0, 0.0)),
                    );
                }
                _ => {
                    h.app.plans.selection_filters.set_enabled(
                        crate::plan_workspace::selection_filters::Category::Wall,
                        false,
                    );
                }
            }
            let model = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.frame(vec![]);
            h.release(target);
            assert!(h.app.plans.wall_set_move.is_none(), "cause {cause}");
            assert_eq!(h.app.editor.document.model(), &model, "cause {cause}");
            assert_eq!(
                h.app.editor.document.history_stats(),
                history,
                "cause {cause}"
            );
            assert_eq!(model.group_instances.len(), 1);
        }
    }
}

#[test]
fn grouped_walls_are_not_eligible_for_new_opening_hosts() {
    let (mut h, bay) = setup(PROFILES[0].0, PROFILES[0].1);
    toolbar(&mut h, "Create group");
    h.settle();
    let model = h.app.editor.document.model();
    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let drawing = h.app.plans.drawing.as_ref().unwrap();
    let camera = h.app.plans.cameras[&h.view];
    let rect = h.app.plans.canvas_rect.unwrap();
    let point = h.point(Point2::new(0.0, 0.0));
    assert!(
        crate::plan_workspace::opening_host_hit(
            model,
            drawing,
            context,
            camera,
            [f64::from(rect.width()), f64::from(rect.height())],
            Point2::new(
                f64::from(point.x - rect.left()),
                f64::from(point.y - rect.top()),
            ),
        )
        .unwrap()
        .is_none()
    );
    assert!(
        bay.walls
            .iter()
            .all(|wall| model.group_instance_for_member(*wall).is_some())
    );
}
