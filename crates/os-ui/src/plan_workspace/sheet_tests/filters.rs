use super::*;
use os_model::{
    Schedule, ScheduleCategory, ScheduleColumn, ScheduleFilter, ScheduleNumericField,
    ScheduleNumericOperator, ScheduleParams, ScheduleTextField, ScheduleTextOperator,
};

fn settle(app: &mut DesktopApp) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.plans.poll(&app.editor);
        if app.plans.ready() {
            return;
        }
        assert!(Instant::now() < deadline, "drawing did not settle");
        std::thread::yield_now();
    }
}

#[test]
fn opening_schedule_filters_propagate_to_paper_table_sheet_preview_and_vector_pdf() {
    for (size, scale) in PROFILES {
        let (mut app, _) = app_with_plan();
        let view = app.plans.active.unwrap();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, crate::default_wall(app.active_level));
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                open_state: Default::default(),
                name: "D01".into(),
                host: wall.id(),
                offset: 1.0,
                definition: OpeningDefinition::Legacy {
                    kind: OpeningKind::Door,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.0,
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
        let door = opening.id();
        let mut params = ScheduleParams::new("Filtered doors", ScheduleCategory::Door);
        params.columns = vec![ScheduleColumn::Name, ScheduleColumn::Width];
        params.filters = vec![
            ScheduleFilter::Text {
                field: ScheduleTextField::Name,
                operator: ScheduleTextOperator::StartsWith,
                value: "d".into(),
            },
            ScheduleFilter::Numeric {
                field: ScheduleNumericField::Width,
                operator: ScheduleNumericOperator::Greater,
                value: 0.8999999,
            },
        ];
        let schedule = Schedule::new("core.schedule", params);
        let schedule_id = schedule.id();
        app.editor
            .document
            .execute(
                "Fixture",
                vec![
                    Command::AddWall(wall),
                    Command::AddOpening(opening),
                    Command::AddSchedule(schedule),
                ],
            )
            .unwrap();
        app.create_sheet_from_active_view();
        settle(&mut app);
        let sheet = app.plans.active_sheet.unwrap();
        app.place_sheet_schedule(sheet, Some(schedule_id));
        assert!(!app.status_error, "{}", app.status);
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let verify = |app: &mut DesktopApp, expected: bool| {
            settle(app);
            let table =
                crate::opening_schedule::paper_table(app.editor.document.model(), schedule_id)
                    .unwrap();
            assert_eq!(table.rows.len(), usize::from(expected));
            let pdf = String::from_utf8(app.sheet_page().unwrap().to_pdf().unwrap()).unwrap();
            assert_eq!(pdf.contains("<443031> Tj"), expected, "PDF row membership");
            assert_eq!(
                pdf.contains("<302E393030> Tj"),
                expected,
                "unrounded filter / rounded display"
            );
            let _ = ctx.run(raw_input(size, scale, 0., vec![]), |ctx| {
                app.plan_workspace(ctx)
            });
            let output = ctx.run(raw_input(size, scale, 1., vec![]), |ctx| {
                app.plan_workspace(ctx)
            });
            assert_eq!(
                output
                    .shapes
                    .iter()
                    .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="D01")),
                expected,
                "visible sheet row"
            );
        };
        verify(&mut app, true);
        let mut parameters = app.editor.document.model().schedules[&schedule_id]
            .parameters
            .clone();
        parameters.filters[1] = ScheduleFilter::Numeric {
            field: ScheduleNumericField::Width,
            operator: ScheduleNumericOperator::Greater,
            value: 0.9,
        };
        app.editor
            .command(
                "Exclude at exact boundary",
                Command::UpdateSchedule {
                    id: schedule_id,
                    parameters,
                },
            )
            .unwrap();
        verify(&mut app, false);
        app.editor.undo().unwrap();
        verify(&mut app, true);
        app.editor.redo().unwrap();
        verify(&mut app, false);
        // Live source change crosses the threshold but still displays 0.900.
        let mut parameters = app.editor.document.model().openings[&door]
            .parameters
            .clone();
        if let OpeningDefinition::Legacy { width, .. } = &mut parameters.definition {
            *width = 0.9000001;
        }
        app.editor
            .command(
                "Source dimension",
                Command::UpdateOpening {
                    id: door,
                    parameters,
                },
            )
            .unwrap();
        verify(&mut app, true);
        app.editor.undo().unwrap();
        verify(&mut app, false);
        app.editor.redo().unwrap();
        verify(&mut app, true);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("filtered-sheet.osb");
        let model = app.editor.document.model().clone();
        app.editor.save(&path).unwrap();
        app.editor.open(&path).unwrap();
        assert_eq!(app.editor.document.model(), &model);
        app.plans.poll(&app.editor);
        app.plans.active = Some(view);
        app.plans.active_sheet = Some(sheet);
        verify(&mut app, true);
    }
}
