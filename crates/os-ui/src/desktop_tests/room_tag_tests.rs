//! Room-tag acceptance through the existing desktop egui frame harness.
use super::*;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280., 800.), 1.),
    (egui::vec2(1000., 650.), 1.5),
];

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
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
fn setup(size: egui::Vec2, scale: f32) -> (Harness, Id, Id) {
    let mut h = Harness::at_size(size, scale);
    let level = h.app.active_level;
    for (start, end) in [
        (Point2::new(0., 0.), Point2::new(4., 0.)),
        (Point2::new(4., 0.), Point2::new(4., 3.)),
        (Point2::new(4., 3.), Point2::new(0., 3.)),
        (Point2::new(0., 3.), Point2::new(0., 0.)),
    ] {
        h.app
            .editor
            .command(
                "Boundary",
                Command::AddWall(os_model::Wall::new(
                    "org.openstructure.walls.wall",
                    WallParams {
                        name: "Boundary".into(),
                        start,
                        end,
                        height: 3.,
                        thickness: 0.2,
                        level,
                        material: None,
                    },
                )),
            )
            .unwrap();
    }
    let view = h.app.editor.create_floor_plan("Tag plan", level).unwrap();
    h.app.focus_plan(Some(view));
    h.settle_plan();
    h.click("Room");
    h.click_at(h.plan_point(view, Point2::new(1., 1.)));
    assert_eq!(
        h.app.editor.document.model().rooms.len(),
        1,
        "{}",
        h.app.status
    );
    let room = *h.app.editor.document.model().rooms.keys().next().unwrap();
    h.frame(vec![escape()]);
    h.settle_plan();
    h.frame(vec![]);
    assert_eq!(h.ctx.pixels_per_point(), scale);
    (h, view, room)
}
fn place(h: &mut Harness, view: Id, room: Id) -> Id {
    h.app.select(Some(room));
    h.frame(vec![]);
    h.click("Room Tag");
    h.click_at(h.plan_point(view, Point2::new(2., 1.5)));
    h.settle_plan();
    h.frame(vec![]);
    assert_eq!(
        h.app.editor.document.model().room_tags.len(),
        1,
        "{}",
        h.app.status
    );
    *h.app
        .editor
        .document
        .model()
        .room_tags
        .keys()
        .next()
        .unwrap()
}
fn start_drag(h: &mut Harness, view: Id, tag: Id) -> egui::Pos2 {
    let position = h.app.editor.document.model().room_tags[&tag]
        .parameters
        .position;
    let from = h.plan_point(view, position);
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    from
}
fn canvas_text(h: &Harness, text: &str) -> usize {
    let canvas = h.app.plans.canvas_rect.unwrap();
    h.output
        .shapes
        .iter()
        .filter(|shape| match &shape.shape {
            egui::Shape::Text(t) => t.galley.job.text == text && canvas.contains(t.pos),
            _ => false,
        })
        .count()
}

fn reveal_browser(h: &mut Harness, label: &str) -> egui::Rect {
    for _ in 0..20 {
        if let Some(rect) = h.visible_text_rect(label) {
            return rect;
        }
        let pointer = h.text_rect("Project Browser").center() + egui::vec2(0., 75.);
        h.frame(vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -90.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..8 {
            h.frame(vec![]);
        }
    }
    panic!("Browser entry is not reachable: {label}");
}

#[test]
fn room_tag_real_frames_place_move_live_label_select_and_pan_at_both_dpis() {
    for (size, scale) in PROFILES {
        let (mut h, view, room) = setup(size, scale);
        h.app.select(None);
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Room Tag");
        h.click_at(h.plan_point(view, Point2::new(1., 1.)));
        h.frame(vec![egui::Event::PointerMoved(
            h.plan_point(view, Point2::new(2., 1.5)),
        )]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        let room_params = &before.rooms[&room].parameters;
        let label = format!("{} · {}", room_params.number, room_params.name);
        assert!(canvas_text(&h, &label) > 0, "placement preview missing");
        h.click_at(h.plan_point(view, Point2::new(2., 1.5)));
        h.settle_plan();
        h.frame(vec![]);
        let tag = *h
            .app
            .editor
            .document
            .model()
            .room_tags
            .keys()
            .next()
            .unwrap();
        let placed = h.app.editor.document.model().clone();
        assert_eq!(placed.room_tags[&tag].parameters.room, room);
        assert_eq!(placed.room_tags[&tag].parameters.view, view);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_eq!(
            canvas_text(&h, &label),
            1,
            "explicit tag suppresses automatic name/number"
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &placed);
        h.settle_plan();
        h.frame(vec![]);
        h.app.select(None);
        h.frame(vec![]);
        h.click_at(h.plan_point(view, placed.room_tags[&tag].parameters.position));
        assert_eq!(h.app.selected, Some(tag));
        assert_eq!(h.app.editor.document.model(), &placed);
        h.frame(vec![]);
        let camera_probe = h.plan_point(view, Point2::new(0., 0.));
        let from = start_drag(&mut h, view, tag);
        let to = from + egui::vec2(25., -18.);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        assert_eq!(h.app.editor.document.model(), &placed);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_eq!(
            h.plan_point(view, Point2::new(0., 0.)),
            camera_probe,
            "tag preview must not pan"
        );
        h.frame(vec![button(to, false)]);
        h.settle_plan();
        h.frame(vec![]);
        let moved = h.app.editor.document.model().clone();
        assert_ne!(
            moved.room_tags[&tag].parameters.position,
            placed.room_tags[&tag].parameters.position
        );
        assert_eq!(moved.room_tags[&tag].header, placed.room_tags[&tag].header);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 2
        );
        assert_eq!(h.plan_point(view, Point2::new(0., 0.)), camera_probe);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &placed);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &moved);
        h.settle_plan();
        h.frame(vec![]);
        h.app.select(Some(room));
        h.frame(vec![]);
        h.app.room_number_draft = "A-102".into();
        h.app.room_name_draft = "Study".into();
        h.click("Apply room properties");
        h.settle_plan();
        h.frame(vec![]);
        let ctx = h.app.editor.native_plan_context(view).unwrap();
        let tags = h
            .app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .room_tags(ctx)
            .unwrap();
        assert_eq!(tags[0].label, "A-102 · Study");
        assert_eq!(tags[0].entity, tag);
        assert_eq!(canvas_text(&h, "A-102 · Study"), 1);
        assert_eq!(h.app.editor.document.model().room_tags, moved.room_tags);
        h.app.select(None);
        h.frame(vec![]);
        let probe = h.plan_point(view, Point2::new(0., 0.));
        let ordinary = h.plan_point(view, Point2::new(0.5, 0.5));
        let before_pan = h.app.editor.document.model().clone();
        h.drag(ordinary, ordinary + egui::vec2(35., 20.));
        assert_ne!(h.plan_point(view, Point2::new(0., 0.)), probe);
        assert_eq!(h.app.editor.document.model(), &before_pan);
    }
}

#[test]
fn room_tag_real_frames_invalid_release_escape_and_stale_cancel_through_release() {
    for (size, scale) in PROFILES {
        for case in [
            "outside", "escape", "revision", "session", "drawing", "view", "crop", "provider",
        ] {
            if case == "provider" && !cfg!(feature = "external-plugins") {
                continue;
            }
            let (mut h, view, room) = setup(size, scale);
            let tag = place(&mut h, view, room);
            let before = h.app.editor.document.model().room_tags.clone();
            let from = start_drag(&mut h, view, tag);
            let mut to = from + egui::vec2(30., 20.);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            match case {
                "outside" => {
                    to = h.app.plans.canvas_rect.unwrap().right_bottom() + egui::vec2(10., 10.);
                }
                "escape" => h.frame(vec![escape()]),
                "revision" => h
                    .app
                    .editor
                    .command("External edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(view).unwrap())
                }
                "view" => h.app.focus_plan(None),
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "crop" => {
                    let v = h.app.editor.document.model().views[&view]
                        .parameters
                        .clone();
                    let mut settings = v.plan.unwrap();
                    settings.crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(0., 0.),
                        max: Point2::new(4., 3.),
                    });
                    h.app
                        .editor
                        .update_floor_plan(view, "Tag plan", v.level.unwrap(), settings)
                        .unwrap();
                }
                _ => unreachable!(),
            }
            let history = h.app.editor.document.history_stats();
            let model = h.app.editor.document.model().clone();
            h.frame(vec![egui::Event::PointerMoved(to)]);
            let probe =
                (h.app.plans.active == Some(view)).then(|| h.plan_point(view, Point2::new(0., 0.)));
            h.frame(vec![egui::Event::PointerMoved(to + egui::vec2(5., 0.))]);
            h.frame(vec![button(to + egui::vec2(5., 0.), false)]);
            assert_eq!(h.app.editor.document.model(), &model, "{case} at {scale}");
            assert_eq!(h.app.editor.document.model().room_tags, before);
            assert_eq!(h.app.editor.document.history_stats(), history, "{case}");
            if let Some(probe) = probe {
                assert_eq!(
                    h.plan_point(view, Point2::new(0., 0.)),
                    probe,
                    "cancelled press became pan: {case}"
                );
            }
        }
    }
}

#[test]
fn room_tag_browser_focus_properties_position_delete_and_orphan_access() {
    for (size, scale) in PROFILES {
        let (mut h, view, room) = setup(size, scale);
        let tag = place(&mut h, view, room);
        h.app.properties_fraction = 0.45;
        h.app.select(None);
        h.app.focus_plan(None);
        h.frame(vec![]);
        h.frame(vec![]);
        let p = &h.app.editor.document.model().rooms[&room].parameters;
        let label = format!("Tag: {} · {}", p.number, p.name);
        // Collapse preceding groups to expose the tag in the existing browser scroll area.
        for label in ["Walls (4)", "Rooms (1)"] {
            if let Some(rect) = h.visible_text_rect(label) {
                h.click_at(rect.center());
            }
        }
        h.frame(vec![]);
        let rect = reveal_browser(&mut h, &label);
        h.click_at(rect.center());
        assert_eq!(h.app.selected, Some(tag));
        assert_eq!(h.app.plans.active, Some(view));
        h.settle_plan();
        h.frame(vec![]);
        assert!(
            h.visible_text_rect("Room tag · live room number and name")
                .is_some()
        );
        let before = h.app.editor.document.model().clone();
        h.dimension("X (m)", "2.5");
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Apply tag position");
        assert_eq!(
            h.app.editor.document.model().room_tags[&tag]
                .parameters
                .position
                .x,
            2.5
        );
        assert_eq!(
            h.app.editor.document.model().room_tags[&tag].header,
            before.room_tags[&tag].header
        );
        let moved = h.app.editor.document.model().clone();
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &moved);
        h.settle_plan();
        h.frame(vec![]);
        h.click("Delete room tag");
        assert!(h.app.editor.document.model().room_tags.is_empty());
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &moved);
        h.app
            .editor
            .command("Delete room", Command::RemoveRoom(room))
            .unwrap();
        h.settle_plan();
        h.frame(vec![]);
        h.app.select(None);
        h.frame(vec![]);
        let rect = reveal_browser(&mut h, "Room tag · Missing room");
        h.click_at(rect.center());
        h.frame(vec![]);
        assert_eq!(h.app.selected, Some(tag));
        assert!(h.visible_text_rect("Missing room").is_some());
        let ctx = h.app.editor.native_plan_context(view).unwrap();
        assert_eq!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .room_tags(ctx)
                .unwrap()[0]
                .diagnostic
                .as_deref(),
            Some("Missing room")
        );
        let orphan = h.app.editor.document.model().clone();
        let orphan_history = h.app.editor.document.history_stats();
        let from = start_drag(&mut h, view, tag);
        let to = from + egui::vec2(20., -10.);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        assert_eq!(h.app.editor.document.model(), &orphan);
        assert_eq!(h.app.editor.document.history_stats(), orphan_history);
        assert_eq!(canvas_text(&h, "Room tag · missing reference"), 1);
        h.frame(vec![button(to, false)]);
        let orphan_moved = h.app.editor.document.model().clone();
        assert_eq!(orphan_moved.rooms, orphan.rooms);
        assert_eq!(
            orphan_moved.room_tags[&tag].header,
            orphan.room_tags[&tag].header
        );
        assert_eq!(
            orphan_moved.room_tags[&tag].parameters.room,
            orphan.room_tags[&tag].parameters.room
        );
        assert_eq!(
            orphan_moved.room_tags[&tag].parameters.view,
            orphan.room_tags[&tag].parameters.view
        );
        assert_ne!(
            orphan_moved.room_tags[&tag].parameters.position,
            orphan.room_tags[&tag].parameters.position
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            orphan_history.undo_entries + 1
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &orphan);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &orphan_moved);
        h.settle_plan();
        h.frame(vec![]);
        let v = h.app.editor.document.model().views[&view]
            .parameters
            .clone();
        let mut settings = v.plan.unwrap();
        settings.crop = Some(os_model::PlanViewCrop {
            min: Point2::new(0., 0.),
            max: Point2::new(0.5, 0.5),
        });
        h.app
            .editor
            .update_floor_plan(view, "Tag plan", v.level.unwrap(), settings)
            .unwrap();
        h.settle_plan();
        h.frame(vec![]);
        assert_eq!(canvas_text(&h, "Room tag · missing reference"), 0);
        h.app.select(None);
        h.frame(vec![]);
        let rect = reveal_browser(&mut h, "Room tag · Missing room");
        h.click_at(rect.center());
        assert_eq!(
            h.app.selected,
            Some(tag),
            "cropped orphan remains selectable in Browser"
        );
    }
}

#[test]
fn room_tag_crop_rejects_drag_release_and_properties_outside_crop() {
    for (size, scale) in PROFILES {
        let (mut h, view, room) = setup(size, scale);
        let tag = place(&mut h, view, room);
        let v = h.app.editor.document.model().views[&view]
            .parameters
            .clone();
        let mut settings = v.plan.unwrap();
        settings.crop = Some(os_model::PlanViewCrop {
            min: Point2::new(1., 0.5),
            max: Point2::new(3., 2.5),
        });
        h.app
            .editor
            .update_floor_plan(view, "Tag plan", v.level.unwrap(), settings)
            .unwrap();
        h.settle_plan();
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        start_drag(&mut h, view, tag);
        let outside = h.plan_point(view, Point2::new(3.5, 1.5));
        assert!(h.app.plans.canvas_rect.unwrap().contains(outside));
        h.frame(vec![egui::Event::PointerMoved(outside)]);
        h.frame(vec![button(outside, false)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.frame(vec![]);
        h.dimension("X (m)", "3.5");
        h.click("Apply tag position");
        assert_eq!(
            h.app.editor.document.model(),
            &before,
            "Properties must reject positions outside owning view crop"
        );
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert!(h.app.status_error);
    }
}

#[test]
fn room_tag_placement_escape_stale_and_duplicate_leave_model_and_history_intact() {
    for (size, scale) in PROFILES {
        for case in ["escape", "revision", "drawing", "duplicate"] {
            let (mut h, view, room) = setup(size, scale);
            if case == "duplicate" {
                place(&mut h, view, room);
            }
            h.app.select(Some(room));
            h.frame(vec![]);
            h.click("Room Tag");
            let p = h.plan_point(view, Point2::new(2., 1.5));
            h.frame(vec![egui::Event::PointerMoved(p)]);
            match case {
                "escape" => h.frame(vec![escape()]),
                "revision" => h
                    .app
                    .editor
                    .command("Edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(view).unwrap())
                }
                _ => (),
            }
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.frame(vec![]);
            h.click_at(h.plan_point(view, Point2::new(2.5, 1.5)));
            assert_eq!(h.app.editor.document.model(), &before, "{case}");
            assert_eq!(h.app.editor.document.history_stats(), history, "{case}");
            if case == "duplicate" {
                assert!(h.app.status_error);
            }
        }
    }
}

#[test]
fn room_tag_drag_wins_over_overlapping_selected_wall_endpoint() {
    for (size, scale) in PROFILES {
        let (mut h, view, room) = setup(size, scale);
        let tag = place(&mut h, view, room);
        let wall = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .find(|w| w.parameters.start == Point2::new(0., 0.))
            .unwrap()
            .id();
        h.app.room_tag_position = Point2::new(0., 0.);
        h.click("Apply tag position");
        h.settle_plan();
        h.app.select(Some(wall));
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let from = start_drag(&mut h, view, tag);
        let to = from + egui::vec2(30., -25.);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        assert_eq!(h.app.editor.document.model(), &before);
        h.frame(vec![button(to, false)]);
        let after = h.app.editor.document.model();
        assert_eq!(after.walls, before.walls);
        assert_ne!(
            after.room_tags[&tag].parameters.position,
            before.room_tags[&tag].parameters.position
        );
        assert_eq!(h.app.selected, Some(tag));
    }
}
