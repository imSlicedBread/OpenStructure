use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{RoomSeparationLine, RoomSeparationLineParams, Schedule, Wall};

pub(crate) fn fixture() -> (DesktopApp, Id, Id, Id, Id) {
    let mut app = DesktopApp::new().unwrap();
    let level = app.active_level;
    let mut commands = Vec::new();
    for (start, end) in [
        ((0., 0.), (8., 0.)),
        ((8., 0.), (8., 3.)),
        ((8., 3.), (0., 3.)),
        ((0., 3.), (0., 0.)),
    ] {
        let mut p = crate::default_wall(level);
        *p.path.straight_start_mut().unwrap() = Point2::new(start.0, start.1);
        *p.path.straight_end_mut().unwrap() = Point2::new(end.0, end.1);
        commands.push(Command::AddWall(Wall::new(os_walls::WALL_TYPE, p)));
    }
    let separator = RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level,
            start: Point2::new(4., 0.),
            end: Point2::new(4., 3.),
        },
    );
    let partition = separator.id();
    commands.push(Command::AddRoomSeparationLine(separator));
    app.editor
        .document
        .execute("Room boundary", commands)
        .unwrap();
    let faces = derive_faces(
        &app.editor
            .document
            .model()
            .room_boundary_segments(level)
            .unwrap(),
    )
    .unwrap();
    let mut rooms = Vec::new();
    for seed in [Point2::new(1., 1.), Point2::new(6., 1.)] {
        rooms.push(
            crate::room_tools::create_at(
                &mut app.editor,
                level,
                seed,
                faces.assign_seed(seed).unwrap().key.as_signature().to_vec(),
            )
            .unwrap(),
        );
    }
    let view = app.editor.create_floor_plan("Room plan", level).unwrap();
    app.focus_plan(Some(view));
    let schedule = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Room Finishes", ScheduleCategory::RoomFinish),
    );
    let schedule_id = schedule.id();
    app.editor
        .command("Room schedule", Command::AddSchedule(schedule))
        .unwrap();
    app.opening_schedule.open = true;
    app.opening_schedule.session = Some(app.editor.document.session_id());
    app.opening_schedule.selected = Some(schedule_id);
    (app, rooms[0], schedule_id, view, partition)
}

pub(crate) fn move_partition(app: &mut DesktopApp, partition: Id) {
    let mut parameters = app.editor.document.model().room_separation_lines[&partition]
        .parameters
        .clone();
    parameters.start.x = 5.;
    parameters.end.x = 5.;
    app.editor
        .command(
            "Move partition",
            Command::UpdateRoomSeparationLine {
                id: partition,
                parameters,
            },
        )
        .unwrap();
}

pub(crate) fn add_material(app: &mut DesktopApp) -> Id {
    let material = os_model::Material::new(
        "core.material",
        os_model::MaterialParams {
            name: "Oak".into(),
            density_kg_m3: 650.,
            color: [160, 100, 60],
        },
    );
    let id = material.id();
    app.editor
        .command("Material", Command::AddMaterial(material))
        .unwrap();
    id
}

#[test]
fn room_material_schedule_live_name_code_color_and_unassigned_composition() {
    let (mut app, room, schedule, _, partition) = fixture();
    let material = add_material(&mut app);
    let definition = app.editor.document.model().schedules[&schedule]
        .parameters
        .clone();
    assert_eq!(definition.columns.len(), 8);
    app.select(Some(room));
    app.room_finish_draft = ["F-01".into(), "W-02".into(), "".into()];
    app.room_material_draft = [Some(material), None, Some(material)];
    app.apply_room_properties();
    let intent = app.editor.document.model().rooms[&room].clone();
    let row = rows(app.editor.document.model(), &definition)
        .unwrap()
        .remove(0);
    assert_eq!(row.cell(ScheduleColumn::FloorFinish), "Oak · F-01");
    assert_eq!(row.cell(ScheduleColumn::WallFinish), "W-02");
    assert_eq!(row.cell(ScheduleColumn::CeilingFinish), "Oak");
    let mut parameters = app.editor.document.model().materials[&material]
        .parameters
        .clone();
    parameters.color = [30, 90, 150];
    app.editor
        .command(
            "Color",
            Command::UpdateMaterial {
                id: material,
                parameters: parameters.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        rows(app.editor.document.model(), &definition).unwrap()[0]
            .cell(ScheduleColumn::FloorFinish),
        "Oak · F-01"
    );
    parameters.name = "Ash".into();
    app.editor
        .command(
            "Rename",
            Command::UpdateMaterial {
                id: material,
                parameters,
            },
        )
        .unwrap();
    assert_eq!(
        rows(app.editor.document.model(), &definition).unwrap()[0]
            .cell(ScheduleColumn::FloorFinish),
        "Ash · F-01"
    );
    assert_eq!(app.editor.document.model().rooms[&room], intent);
    app.editor
        .command(
            "Break enclosure",
            Command::RemoveRoomSeparationLine(partition),
        )
        .unwrap();
    let row = rows(app.editor.document.model(), &definition)
        .unwrap()
        .remove(0);
    assert_eq!(row.area, None);
    assert_eq!(row.cell(ScheduleColumn::FloorFinish), "Ash · F-01");
    assert_eq!(app.editor.document.model().rooms[&room], intent);
    app.select(Some(room));
    app.room_material_draft[0] = None;
    app.apply_room_properties();
    assert_eq!(
        rows(app.editor.document.model(), &definition).unwrap()[0]
            .cell(ScheduleColumn::FloorFinish),
        "F-01"
    );
    app.room_finish_draft[2] = "C-03".into();
    app.apply_room_properties();
    assert_eq!(
        app.editor.document.model().rooms[&room]
            .parameters
            .ceiling_material,
        Some(material)
    );
}

#[test]
fn room_material_properties_assignment_cancel_stale_and_history_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let (mut app, room, _, _, partition) = fixture();
        let material = add_material(&mut app);
        app.select(Some(room));
        app.properties_fraction = 0.95;
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        app.room_finish_draft = ["F-01".into(), "W-02".into(), "C-03".into()];
        frame(&mut app, &ctx, size, scale, vec![], true);
        let output = frame(&mut app, &ctx, size, scale, vec![], true);
        let click = |app: &mut DesktopApp, pos| {
            for pressed in [true, false] {
                frame(
                    app,
                    &ctx,
                    size,
                    scale,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    true,
                );
            }
        };
        click(&mut app, position(&output, "Unassigned"));
        let output = frame(&mut app, &ctx, size, scale, vec![], true);
        click(&mut app, position(&output, "Oak"));
        assert_eq!(app.room_material_draft[0], Some(material));
        app.room_material_draft[1] = Some(material);
        app.room_material_draft[2] = Some(material);
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        let output = frame(&mut app, &ctx, size, scale, vec![], true);
        click(&mut app, position(&output, "Apply room properties"));
        let assigned = app.editor.document.model().clone();
        let p = &assigned.rooms[&room].parameters;
        assert_eq!(
            [p.floor_material, p.wall_material, p.ceiling_material],
            [Some(material); 3]
        );
        assert_eq!(p.floor_finish.as_deref(), Some("F-01"));
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &before);
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model(), &assigned);
        app.select(Some(room));
        app.room_material_draft = [None; 3];
        app.room_finish_draft[0] = "cancel".into();
        frame(
            &mut app,
            &ctx,
            size,
            scale,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            true,
        );
        assert_eq!(app.room_material_draft, [Some(material); 3]);
        assert_eq!(app.room_finish_draft[0], "F-01");
        assert_eq!(app.editor.document.model(), &assigned);
        app.room_material_draft = [None; 3];
        move_partition(&mut app, partition);
        let moved = app.editor.document.model().clone();
        app.apply_room_properties();
        assert!(app.status.contains("cancelled"));
        assert_eq!(app.editor.document.model(), &moved);
        assert_eq!(app.room_material_draft, [Some(material); 3]);
        app.room_draft_context = Some((Id::new(), app.editor.document.revision()));
        app.room_material_draft = [None; 3];
        app.apply_room_properties();
        assert_eq!(app.editor.document.model(), &moved);
        app.room_material_draft[0] = Some(Id::new());
        app.apply_room_properties();
        assert!(app.status_error);
        assert_eq!(app.editor.document.model(), &moved);
    }
}

#[test]
fn room_finish_rows_live_area_order_unresolved_identity_and_history() {
    let (mut app, room, schedule, _, partition) = fixture();
    let definition = app.editor.document.model().schedules[&schedule]
        .parameters
        .clone();
    let before = rows(app.editor.document.model(), &definition).unwrap();
    assert_eq!(before[0].id, room);
    assert_eq!(before[0].area, Some(12.));
    app.select(Some(room));
    app.room_finish_draft = ["F1".into(), "W1".into(), "C1".into()];
    app.apply_room_properties();
    let original_room = app.editor.document.model().rooms[&room].clone();
    move_partition(&mut app, partition);
    let after = rows(app.editor.document.model(), &definition).unwrap();
    assert_eq!(after[0].area, Some(15.));
    assert_eq!(after[1].area, Some(9.));
    assert_eq!(app.editor.document.model().rooms[&room], original_room);
    let mut sorted = definition.clone();
    sorted.sort = ScheduleSort::Area;
    let by_area = rows(app.editor.document.model(), &sorted).unwrap();
    assert_eq!(by_area[0].area, Some(9.));
    assert_eq!(by_area[1].id, room);
    app.editor
        .command(
            "Break enclosure",
            Command::RemoveRoomSeparationLine(partition),
        )
        .unwrap();
    let broken = rows(app.editor.document.model(), &definition).unwrap();
    assert_eq!(broken[0].id, room);
    assert_eq!(broken[0].area, None);
    assert_eq!(broken[0].cell(ScheduleColumn::Area), "");
    assert!(broken[0].status.contains("enclosure changed"));
    assert_eq!(broken[0].cell(ScheduleColumn::FloorFinish), "F1");
    assert_eq!(broken[0].cell(ScheduleColumn::WallFinish), "W1");
    assert_eq!(broken[0].cell(ScheduleColumn::CeilingFinish), "C1");
    app.editor.undo().unwrap();
    assert_eq!(
        rows(app.editor.document.model(), &definition).unwrap()[0].area,
        Some(15.)
    );
    app.editor.undo().unwrap();
    assert_eq!(
        rows(app.editor.document.model(), &definition).unwrap()[0].area,
        Some(12.)
    );
    // Equal names/areas still have a stable UUID tie break.
    let mut model = app.editor.document.model().clone();
    for r in model.rooms.values_mut() {
        r.parameters.name = "Same".into();
    }
    sorted.sort = ScheduleSort::Name;
    let tied = rows(&model, &sorted).unwrap();
    assert!(tied[0].id < tied[1].id);
}

fn frame(
    app: &mut DesktopApp,
    ctx: &egui::Context,
    size: egui::Vec2,
    scale: f32,
    events: Vec<egui::Event>,
    properties: bool,
) -> egui::FullOutput {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(scale);
    ctx.run(input, |ctx| {
        if properties {
            app.palettes(ctx);
        } else {
            app.opening_schedule_window(ctx);
        }
    })
}

fn position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == label => {
                let rect = egui::Rect::from_min_size(t.pos, t.galley.size());
                s.clip_rect.intersects(rect).then_some(rect.center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing visible {label}"))
}

#[test]
fn room_finish_schedule_ui_selection_properties_history_and_stale_drafts_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let (mut app, room, _, view, partition) = fixture();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.plans.active = None;
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        frame(&mut app, &ctx, size, scale, vec![], false);
        let output = frame(&mut app, &ctx, size, scale, vec![], false);
        let pos = position(&output, "Room 1");
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                size,
                scale,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                false,
            );
        }
        assert_eq!(app.selected, Some(room));
        assert_eq!(app.plans.active, Some(view));
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        app.properties_fraction = 0.85;
        // Exercise keyboard entry in the visible finish text field, then apply via pointer.
        app.room_finish_draft = ["F1".into(), "W1".into(), "C1".into()];
        frame(&mut app, &ctx, size, scale, vec![], true);
        let output = frame(&mut app, &ctx, size, scale, vec![], true);
        let pos = position(&output, "F1");
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                size,
                scale,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                true,
            );
        }
        frame(
            &mut app,
            &ctx,
            size,
            scale,
            vec![egui::Event::Text("X".into())],
            true,
        );
        assert!(app.room_finish_draft[0].contains('X'));
        assert_eq!(app.editor.document.model(), &before);
        let output = frame(&mut app, &ctx, size, scale, vec![], true);
        let pos = position(&output, "Apply room properties");
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                size,
                scale,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                true,
            );
        }
        let edited = app.editor.document.model().clone();
        assert_eq!(
            edited.rooms[&room].parameters.floor_finish.as_deref(),
            Some(app.room_finish_draft[0].as_str())
        );
        assert_eq!(
            edited.rooms[&room].parameters.wall_finish.as_deref(),
            Some("W1")
        );
        assert_eq!(
            edited.rooms[&room].parameters.ceiling_finish.as_deref(),
            Some("C1")
        );
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &before);
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model(), &edited);
        app.select(Some(room));
        app.room_finish_draft[0] = "bad\ncode".into();
        app.apply_room_properties();
        assert_eq!(app.editor.document.model(), &edited);
        assert!(app.status_error);
        frame(
            &mut app,
            &ctx,
            size,
            scale,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            true,
        );
        assert_eq!(
            app.room_finish_draft[0],
            edited.rooms[&room].parameters.floor_finish.clone().unwrap()
        );
        app.room_finish_draft[0] = "stale".into();
        move_partition(&mut app, partition);
        let moved = app.editor.document.model().clone();
        app.apply_room_properties();
        assert_eq!(app.editor.document.model(), &moved);
        assert!(app.status.contains("cancelled"));
        app.room_draft_context = Some((Id::new(), app.editor.document.revision()));
        app.room_finish_draft[0] = "other session".into();
        app.apply_room_properties();
        assert_eq!(app.editor.document.model(), &moved);
    }
}
