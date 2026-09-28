use super::*;
use os_model::{ElementLifecycle, PhaseFilter, PhaseStatus, SchedulePhase};

const FILTERS: [PhaseFilter; 5] = [
    PhaseFilter::ShowAll,
    PhaseFilter::ShowExisting,
    PhaseFilter::ShowNew,
    PhaseFilter::ShowDemolished,
    PhaseFilter::ShowTemporary,
];

#[test]
fn phased_schedule_window_renders_only_openings_and_hosts_allowed_by_the_filter() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let (mut app, schedule_id) = saved(ScheduleCategory::All);
        let mut model = app.editor.document.model().clone();
        let existing = model.existing_phase().unwrap();
        let target = model.latest_phase().unwrap();
        let future = os_model::new_phase("Later", 2);
        let future_id = future.id();
        model.phases.insert(future_id, future);
        let door_id = *model
            .openings
            .values()
            .find(|opening| {
                model.resolve_opening(&opening.parameters).unwrap().kind == OpeningKind::Door
            })
            .map(|opening| &opening.header.id)
            .unwrap();
        let window_id = *model
            .openings
            .values()
            .find(|opening| {
                model.resolve_opening(&opening.parameters).unwrap().kind == OpeningKind::Window
            })
            .map(|opening| &opening.header.id)
            .unwrap();
        let host = model.openings[&door_id].parameters.host;
        model.element_lifecycles.insert(
            host,
            ElementLifecycle {
                created_in: existing,
                demolished_in: None,
            },
        );
        model.element_lifecycles.insert(
            door_id,
            ElementLifecycle {
                created_in: existing,
                demolished_in: None,
            },
        );
        model.element_lifecycles.insert(
            window_id,
            ElementLifecycle {
                created_in: future_id,
                demolished_in: None,
            },
        );
        let mut definition = model.schedules[&schedule_id].parameters.clone();
        definition.phase = SchedulePhase::PhaseAware {
            target: Some(target),
            filter: PhaseFilter::ShowAll,
        };
        app.editor.document = os_document::Document::from_model(model).unwrap();
        app.editor.regenerate().unwrap();
        app.editor
            .command(
                "Set schedule phase",
                os_document::Command::UpdateSchedule {
                    id: schedule_id,
                    parameters: definition,
                },
            )
            .unwrap();
        app.opening_schedule.selected = Some(schedule_id);
        app.opening_schedule.open = true;
        app.opening_schedule.session = Some(app.editor.document.session_id());
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut rendered = Vec::new();
        for _ in 0..3 {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .native_pixels_per_point = Some(scale);
            let output = ctx.run(input, |ctx| app.opening_schedule_window(ctx));
            rendered = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect();
        }
        assert!(rendered.iter().any(|text| text == "Schedule door"));
        assert!(!rendered.iter().any(|text| text == "Schedule window"));
        assert!(
            rendered
                .iter()
                .any(|text| text.starts_with("Pinned phase:"))
        );
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
    }
}

#[test]
fn schedule_phases_statuses_hosts_categories_and_report_parity() {
    use PhaseStatus::*;
    let (app, door, ty) = crate::opening_schedule::tests::fixture();
    let mut model = app.editor.document.model().clone();
    let existing = model.existing_phase().unwrap();
    let target = model.latest_phase().unwrap();
    let later = os_model::new_phase("Later", 2);
    let future = later.id();
    model.phases.insert(future, later);
    let template = model.openings[&door].clone();
    let wall_template = model.walls[&template.parameters.host].clone();
    let mut window_type = model.opening_types[&ty].clone();
    window_type.header.id = Id::new();
    window_type.parameters.name = "W-900".into();
    window_type.parameters.kind = OpeningKind::Window;
    let window_ty = window_type.id();
    model.opening_types.insert(window_ty, window_type);
    model.openings.clear();
    model.walls.clear();
    model.element_lifecycles.clear();
    let lifetime = |status| match status {
        Existing => ElementLifecycle {
            created_in: existing,
            demolished_in: None,
        },
        New => ElementLifecycle {
            created_in: target,
            demolished_in: None,
        },
        Demolished => ElementLifecycle {
            created_in: existing,
            demolished_in: Some(target),
        },
        Temporary => ElementLifecycle {
            created_in: target,
            demolished_in: Some(target),
        },
        Future => ElementLifecycle {
            created_in: future,
            demolished_in: None,
        },
        PreviouslyDemolished => ElementLifecycle {
            created_in: existing,
            demolished_in: Some(existing),
        },
    };
    let mut expected = Vec::new();
    for (host_status, opening_status, fallback) in [
        (Existing, Existing, false),
        (New, New, false),
        (Demolished, Demolished, false),
        (Temporary, Temporary, false),
        (Future, Future, false),
        (PreviouslyDemolished, PreviouslyDemolished, false),
        (Existing, New, false),
        (New, Existing, true),
        (Demolished, Existing, true),
        (Temporary, Existing, true),
        (Future, Existing, true),
        (PreviouslyDemolished, Existing, true),
    ] {
        for (kind, typed) in [
            (OpeningKind::Door, false),
            (OpeningKind::Door, true),
            (OpeningKind::Window, false),
            (OpeningKind::Window, true),
        ] {
            let mut wall = wall_template.clone();
            wall.header.id = Id::new();
            let host = wall.id();
            wall.parameters.path.straight_start_mut().unwrap().y = expected.len() as f64 * 5.;
            wall.parameters.path.straight_end_mut().unwrap().y = wall.parameters.start().y;
            model.walls.insert(host, wall);
            model.element_lifecycles.insert(host, lifetime(host_status));
            let mut opening = template.clone();
            opening.header.id = Id::new();
            opening.parameters.host = host;
            opening.parameters.name =
                format!("{host_status:?}-{opening_status:?}-{kind:?}-{typed}");
            opening.parameters.definition = if typed {
                OpeningDefinition::Typed {
                    type_id: if kind == OpeningKind::Door {
                        ty
                    } else {
                        window_ty
                    },
                }
            } else {
                OpeningDefinition::Legacy {
                    kind,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.0,
                }
            };
            let id = opening.id();
            model.openings.insert(id, opening);
            if !fallback {
                model
                    .element_lifecycles
                    .insert(id, lifetime(opening_status));
            }
            expected.push((id, kind, host_status, opening_status));
            assert_eq!(model.phase_status(id, target).unwrap(), opening_status);
            assert_eq!(model.phase_status(host, target).unwrap(), host_status);
        }
    }
    model.validate().unwrap();
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
    ] {
        for filter in FILTERS {
            let mut params = ScheduleParams::new("Phased", category);
            params.phase = SchedulePhase::PhaseAware {
                target: Some(target),
                filter,
            };
            params.columns = vec![ScheduleColumn::Id, ScheduleColumn::Name];
            let schedule = os_model::Schedule::new("core.schedule", params.clone());
            let schedule_id = schedule.id();
            model.schedules.insert(schedule_id, schedule);
            let expected_ids: std::collections::BTreeSet<_> = expected
                .iter()
                .filter(|(_, kind, host, opening)| {
                    (category == ScheduleCategory::All
                        || (category == ScheduleCategory::Door) == (*kind == OpeningKind::Door))
                        && filter.includes(*host)
                        && filter.includes(*opening)
                })
                .map(|(id, ..)| *id)
                .collect();
            let rows = defined_rows(&model, Some(&params)).unwrap();
            assert_eq!(
                rows.iter()
                    .map(|r| r.id)
                    .collect::<std::collections::BTreeSet<_>>(),
                expected_ids
            );
            let paper = paper_table(&model, schedule_id).unwrap();
            assert_eq!(
                paper
                    .rows
                    .iter()
                    .map(|r| r[0].clone())
                    .collect::<std::collections::BTreeSet<_>>(),
                expected_ids.iter().map(ToString::to_string).collect()
            );
            assert_eq!(
                parse(&report(&model, schedule_id).unwrap().0)[1..],
                paper.rows
            );
            assert_eq!(
                quantity_report(&model, schedule_id).unwrap(),
                encode_quantities(&rows).unwrap()
            );
            assert_eq!(
                quantities(&rows).iter().map(|q| q.count).sum::<usize>(),
                expected_ids.len()
            );
            use os_render::sheet::{PaperRect, SheetPage, append_schedule_table};
            let page = SheetPage::new(1000., 1000., vec![]).unwrap();
            let rect = PaperRect {
                min_mm: os_core::Point2::new(0., 0.),
                max_mm: os_core::Point2::new(950., 950.),
            };
            let pdf = String::from_utf8(
                append_schedule_table(page, rect, &paper)
                    .unwrap()
                    .to_pdf()
                    .unwrap(),
            )
            .unwrap();
            for (id, ..) in &expected {
                let hex: String = id.to_string().bytes().map(|b| format!("{b:02X}")).collect();
                assert_eq!(
                    pdf.contains(&format!("<{hex}> Tj")),
                    expected_ids.contains(id)
                );
            }
            assert!(paper.heading.contains(&format!("{filter:?}")));
            model
                .schedules
                .get_mut(&schedule_id)
                .unwrap()
                .parameters
                .group_by = vec![os_model::ScheduleGroupField::Kind];
            assert_eq!(
                parse(&quantity_report(&model, schedule_id).unwrap().0)[1..],
                paper_table(&model, schedule_id).unwrap().rows
            );
            model.schedules.remove(&schedule_id);
        }
    }
}

#[test]
fn frozen_47_legacy_csv_bytes_rows_and_reopen_remain_unphased() {
    use os_storage::{StorageBackend, ZipJsonStorage};
    let mut wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../os-storage/tests/fixtures/schema-47-schedule-phases.json"
    ))
    .unwrap();
    os_storage::migrate(&mut wire, 47).unwrap();
    let model: Model = serde_json::from_value(wire).unwrap();
    let quantity_header = "Kind,Type,Type ID,Width (m),Height (m),Sill (m),Count\r\n";
    let door_quantity = "Door,Legacy (instance dimensions),Legacy,0.900,2.100,0.000,1\r\n";
    let window_quantity = concat!(
        "Window,Legacy (instance dimensions),Legacy,0.500,1.000,1.100,1\r\n",
        "Window,Shared window,00000000-0000-4000-8000-000000000011,1.000,1.200,0.800,1\r\n"
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy-phases.osb");
    let document = os_document::Document::from_model(model.clone()).unwrap();
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    for model in [&model, reopened.model()] {
        for schedule in model.schedules.values() {
            let params = &schedule.parameters;
            assert_eq!(params.phase, SchedulePhase::LegacyUnphased);
            let (instances, quantities) = match params.category {
                ScheduleCategory::Door => (
                    "Removed door,0.900\r\n".to_owned(),
                    door_quantity.to_owned(),
                ),
                ScheduleCategory::Window => (
                    "Legacy sill,0.500\r\nLegacy window,1.000\r\n".to_owned(),
                    window_quantity.to_owned(),
                ),
                ScheduleCategory::All => (
                    "Legacy sill,0.500\r\nLegacy window,1.000\r\nRemoved door,0.900\r\n".to_owned(),
                    format!("{door_quantity}{window_quantity}"),
                ),
                ScheduleCategory::RoomFinish => {
                    assert!(paper_table(model, schedule.id()).unwrap().rows.is_empty());
                    continue;
                }
            };
            assert_eq!(
                report(model, schedule.id()).unwrap().0,
                format!("Instance / name,Width (m)\r\n{instances}").as_bytes()
            );
            assert_eq!(
                quantity_report(model, schedule.id()).unwrap().0,
                format!("{quantity_header}{quantities}").as_bytes()
            );
            let baseline = defined_rows(model, Some(params)).unwrap();
            assert!(!baseline.is_empty());
            let mut aware = params.clone();
            aware.phase = SchedulePhase::PhaseAware {
                target: model.latest_phase(),
                filter: PhaseFilter::ShowAll,
            };
            assert!(defined_rows(model, Some(&aware)).unwrap().len() < baseline.len());
        }
    }
}

#[test]
fn schedule_phase_empty_reports_and_missing_pin_errors() {
    let (app, id) = saved(ScheduleCategory::All);
    let mut model = app.editor.document.model().clone();
    model.schedules.get_mut(&id).unwrap().parameters.phase = SchedulePhase::PhaseAware {
        target: None,
        filter: PhaseFilter::ShowDemolished,
    };
    assert!(
        defined_rows(&model, Some(&model.schedules[&id].parameters))
            .unwrap()
            .is_empty()
    );
    assert!(paper_table(&model, id).unwrap().rows.is_empty());
    assert_eq!(parse(&report(&model, id).unwrap().0).len(), 1);
    assert_eq!(parse(&quantity_report(&model, id).unwrap().0).len(), 1);
    model.schedules.get_mut(&id).unwrap().parameters.phase = SchedulePhase::PhaseAware {
        target: Some(Id::new()),
        filter: PhaseFilter::ShowAll,
    };
    assert!(defined_rows(&model, Some(&model.schedules[&id].parameters)).is_err());
    assert!(paper_table(&model, id).is_err());
    assert!(report(&model, id).is_err());
    assert!(quantity_report(&model, id).is_err());
}
