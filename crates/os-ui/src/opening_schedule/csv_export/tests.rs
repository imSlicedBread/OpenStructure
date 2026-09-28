use super::*;
use os_document::Command;
mod phases;

#[test]
fn configurable_csv_bounds_quotes_empty_and_stale_definition() {
    use os_model::ScheduleGroupField::Type;
    let (mut app, id) = saved(ScheduleCategory::All);
    let mut params = app.editor.document.model().schedules[&id]
        .parameters
        .clone();
    params.group_by = vec![Type];
    app.editor
        .command(
            "Group",
            Command::UpdateSchedule {
                id,
                parameters: params.clone(),
            },
        )
        .unwrap();
    let mut rows = defined_rows(app.editor.document.model(), Some(&params)).unwrap();
    rows[0].type_name = "=a,\"b\"\r\nc".into();
    let table = grouping::table(&rows, &params);
    let expected = table.rows.clone();
    assert_eq!(parse(&encode_grouped(table).unwrap().0)[1..], expected);
    let empty = encode_grouped(grouping::table(&[], &params)).unwrap();
    assert_eq!(parse(&empty.0)[1], ["Grand count", "", "0"]);
    rows[0].type_name = "x".repeat(MAX_BYTES);
    assert!(encode_grouped(grouping::table(&rows, &params)).is_err());
    let draft = ExportDraft::with_kind(&app, id, ReportKind::Quantities).unwrap();
    assert!(draft.current(&app));
    params.group_by.clear();
    app.editor
        .command(
            "Ungroup",
            Command::UpdateSchedule {
                id,
                parameters: params,
            },
        )
        .unwrap();
    assert!(!draft.current(&app));
}

#[test]
fn configurable_summary_ui_paper_csv_parity_and_instance_bytes_unchanged() {
    use os_model::ScheduleGroupField::{Kind, Width};
    let (mut app, id) = saved(ScheduleCategory::All);
    let before = report(app.editor.document.model(), id).unwrap().0;
    let mut params = app.editor.document.model().schedules[&id]
        .parameters
        .clone();
    params.group_by = vec![Kind, Width];
    app.editor
        .command(
            "Group",
            Command::UpdateSchedule {
                id,
                parameters: params,
            },
        )
        .unwrap();
    assert_eq!(report(app.editor.document.model(), id).unwrap().0, before);
    let paper = paper_table(app.editor.document.model(), id).unwrap();
    let csv = quantity_report(app.editor.document.model(), id).unwrap();
    let decoded = parse(&csv.0);
    assert_eq!(decoded[0], paper.columns);
    assert_eq!(decoded[1..], paper.rows);
    assert_eq!(csv.2, 5);
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let before_ui = app.editor.document.model().clone();
    let mut text = Vec::new();
    for frame in 0..12 {
        let events = if frame >= 3 {
            vec![
                egui::Event::PointerMoved(egui::pos2(400.0, 280.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -120.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        } else {
            vec![]
        };
        let output = ctx.run(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1800.0, 1400.0),
                )),
                ..Default::default()
            },
            |ctx| app.opening_schedule_window(ctx),
        );
        text.extend(output.shapes.iter().filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
            _ => None,
        }));
        if text.iter().any(|t| t == "Grand count") {
            for cell in paper.rows.iter().flatten().filter(|c| !c.is_empty()) {
                assert!(text.contains(cell), "UI missing {cell}");
            }
            assert_eq!(app.editor.document.model(), &before_ui);
            return;
        }
    }
    panic!("grouped summary not visible");
}

fn saved(category: ScheduleCategory) -> (DesktopApp, Id) {
    let (mut app, _, _) = super::super::tests::fixture();
    let schedule =
        os_model::Schedule::new("core.schedule", ScheduleParams::new("CSV report", category));
    let id = schedule.id();
    app.editor
        .command("Save schedule", Command::AddSchedule(schedule))
        .unwrap();
    app.opening_schedule.selected = Some(id);
    app.opening_schedule.session = Some(app.editor.document.session_id());
    (app, id)
}

// Independent small CSV reader for round-trip assertions (including embedded CRLF).
#[test]
fn quantity_exact_variants_identity_legacy_filter_order_and_csv_parity() {
    let (app, id) = saved(ScheduleCategory::All);
    let mut model = app.editor.document.model().clone();
    let door = model
        .openings
        .values()
        .find(|o| matches!(o.parameters.definition, OpeningDefinition::Typed { .. }))
        .unwrap()
        .parameters
        .clone();
    let window = model
        .openings
        .values()
        .find(|o| matches!(o.parameters.definition, OpeningDefinition::Legacy { .. }))
        .unwrap()
        .parameters
        .clone();
    let original_type = model.opening_types.values().next().unwrap().clone();
    let other_type =
        os_model::OpeningType::new("core.opening_type", original_type.parameters.clone());
    let other_id = other_type.id();
    model.opening_types.insert(other_id, other_type);
    for (definition, width, height, sill) in [
        (door.definition.clone(), None, None, None),
        (door.definition.clone(), Some(0.90001), None, None),
        (door.definition.clone(), Some(0.90002), None, None),
        (door.definition.clone(), None, Some(2.10001), None),
        (
            OpeningDefinition::Typed { type_id: other_id },
            None,
            None,
            None,
        ),
        (window.definition.clone(), None, None, None),
        (
            OpeningDefinition::Legacy {
                kind: OpeningKind::Window,
                width: 1.2,
                height: 1.1,
                sill: 0.90001,
            },
            None,
            None,
            None,
        ),
        (
            OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 1.2,
                height: 1.1,
                sill: 0.0,
            },
            None,
            None,
            None,
        ),
    ] {
        let mut params = door.clone();
        params.definition = definition;
        params.width_override = width;
        params.height_override = height;
        params.sill_override = sill;
        let opening = os_model::Opening::new("core.opening", params);
        model.openings.insert(opening.id(), opening);
    }
    let (baseline, _, count) = quantity_report(&model, id).unwrap();
    assert_eq!(count, 8);
    let parsed = parse(&baseline);
    assert_eq!(
        parsed
            .iter()
            .skip(1)
            .map(|r| r[6].parse::<usize>().unwrap())
            .sum::<usize>(),
        10
    );
    assert!(parsed.iter().any(|r| r[3] == "0.90001"));
    assert!(parsed.iter().any(|r| r[3] == "0.90002"));
    assert!(parsed.iter().any(|r| r[2] == "Legacy" && r[6] == "2"));
    for sort in ScheduleSort::ALL {
        model.schedules.get_mut(&id).unwrap().parameters.sort = sort;
        assert_eq!(quantity_report(&model, id).unwrap().0, baseline);
    }
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
    ] {
        for threshold in [0.0, 1.0, 99.0] {
            let def = &mut model.schedules.get_mut(&id).unwrap().parameters;
            def.category = category;
            def.filters = vec![ScheduleFilter::Numeric {
                field: ScheduleNumericField::Width,
                operator: os_model::ScheduleNumericOperator::Greater,
                value: threshold,
            }];
            let rows = defined_rows(&model, Some(&model.schedules[&id].parameters)).unwrap();
            let groups = quantities(&rows);
            let (bytes, columns, count) = quantity_report(&model, id).unwrap();
            let parsed = parse(&bytes);
            assert_eq!(parsed[0], columns);
            assert_eq!(
                parsed[1..],
                groups.iter().map(QuantityRow::cells).collect::<Vec<_>>()
            );
            assert_eq!(groups.iter().map(|g| g.count).sum::<usize>(), rows.len());
            assert_eq!(count, groups.len());
            if threshold == 99.0 {
                assert_eq!(parsed.len(), 1);
            }
        }
    }
    assert!(quantity_report(&model, Id::new()).is_err());
    let (rooms, id) = saved(ScheduleCategory::RoomFinish);
    assert!(quantity_report(rooms.editor.document.model(), id).is_err());
}

#[test]
fn quantity_precision_escaping_history_reopen_and_replace() {
    let (mut app, id) = saved(ScheduleCategory::Door);
    let ty = app
        .editor
        .document
        .model()
        .opening_types
        .values()
        .next()
        .unwrap()
        .clone();
    let before = quantity_report(app.editor.document.model(), id).unwrap().0;
    let mut parameters = ty.parameters.clone();
    parameters.width = 0.90001;
    parameters.name = "=café, \"東京\"".into();
    app.editor
        .command(
            "Type edit",
            Command::UpdateOpeningType {
                id: ty.id(),
                parameters,
            },
        )
        .unwrap();
    let after = quantity_report(app.editor.document.model(), id).unwrap().0;
    let parsed = parse(&after);
    assert_eq!(parsed[0], QUANTITY_COLUMNS);
    assert_eq!(parsed[1][1], "'=café, \"東京\"");
    assert_eq!(parsed[1][3], "0.90001");
    assert_eq!(parsed[1][4], "2.100");
    app.editor.document.undo();
    assert_eq!(
        quantity_report(app.editor.document.model(), id).unwrap().0,
        before
    );
    app.editor.document.redo();
    assert_eq!(
        quantity_report(app.editor.document.model(), id).unwrap().0,
        after
    );
    let dir = tempfile::tempdir().unwrap();
    use os_storage::StorageBackend;
    let archive = dir.path().join("quantity.osb");
    os_storage::ZipJsonStorage
        .save(&app.editor.document, &archive)
        .unwrap();
    let restored = os_storage::ZipJsonStorage.open(&archive).unwrap();
    assert_eq!(quantity_report(restored.model(), id).unwrap().0, after);
    let mut draft = ExportDraft::with_kind(&app, id, ReportKind::Quantities).unwrap();
    draft.path = dir.path().join("quantity.csv").to_string_lossy().into();
    std::fs::write(&draft.path, b"old").unwrap();
    assert!(draft.write(&app).is_err());
    assert_eq!(std::fs::read(&draft.path).unwrap(), b"old");
    draft.replace = true;
    draft.write(&app).unwrap();
    assert_eq!(std::fs::read(&draft.path).unwrap(), after);
    draft.bytes = vec![0; MAX_BYTES + 1];
    assert!(draft.write(&app).is_err());
    assert_eq!(std::fs::read(&draft.path).unwrap(), after);
}

fn parse(bytes: &[u8]) -> Vec<Vec<String>> {
    let text = std::str::from_utf8(bytes).unwrap();
    assert!(!text.starts_with('\u{feff}'));
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut value = String::new();
    let mut row = Vec::new();
    let mut rows = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                value.push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(std::mem::take(&mut value)),
            '\r' if !quoted => {
                assert_eq!(chars.next(), Some('\n'));
                row.push(std::mem::take(&mut value));
                rows.push(std::mem::take(&mut row));
            }
            '\n' if !quoted => panic!("bare LF record"),
            _ => value.push(c),
        }
    }
    assert!(!quoted && value.is_empty() && row.is_empty());
    rows
}

#[test]
fn csv_escaping_unicode_and_formula_safety_only_changes_text() {
    let values = [
        "café 東京",
        "a,b",
        "a\"b",
        "a\r\nb\nc",
        "",
        "  =SUM(1,2)",
        "\t+1",
        "\u{2003}-2",
        "@cmd",
    ];
    let csv = values
        .iter()
        .map(|v| cell(v, true))
        .collect::<Vec<_>>()
        .join(",")
        + "\r\n";
    let rows = parse(csv.as_bytes());
    for (i, value) in values.iter().enumerate() {
        assert_eq!(
            rows[0][i],
            if i >= 5 {
                format!("'{value}")
            } else {
                value.to_string()
            }
        );
    }
    assert_eq!(cell("-1.250", false), "-1.250");
    assert_eq!(cell("  safe=1", true), "  safe=1");
}

#[test]
fn csv_saved_categories_match_report_filters_sort_columns_and_effective_values() {
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
    ] {
        let (app, id) = saved(category);
        let mut model = app.editor.document.model().clone();
        let typed = model
            .openings
            .values_mut()
            .find(|o| matches!(o.parameters.definition, OpeningDefinition::Typed { .. }))
            .unwrap();
        typed.parameters.width_override = Some(1.123456);
        for sort in ScheduleSort::ALL {
            for filtered in [false, true] {
                let def = &mut model.schedules.get_mut(&id).unwrap().parameters;
                def.sort = sort;
                def.columns = vec![
                    ScheduleColumn::Id,
                    ScheduleColumn::Sill,
                    ScheduleColumn::Width,
                    ScheduleColumn::Name,
                    ScheduleColumn::Height,
                    ScheduleColumn::Type,
                ];
                def.filters = if filtered {
                    vec![ScheduleFilter::Numeric {
                        field: ScheduleNumericField::Width,
                        operator: os_model::ScheduleNumericOperator::Greater,
                        value: 1.15,
                    }]
                } else {
                    vec![]
                };
                let table = paper_table(&model, id).unwrap();
                let (bytes, columns, count) = report(&model, id).unwrap();
                let parsed = parse(&bytes);
                assert_eq!(parsed[0], columns);
                assert_eq!(&parsed[1..], table.rows);
                assert_eq!(count, table.rows.len());
                assert_eq!(&columns[1..3], ["Sill (m)", "Width (m)"]);
                assert_eq!(columns[4], "Height (m)");
                if !filtered && category != ScheduleCategory::Window {
                    assert!(parsed[1..].iter().any(|r| r[2] == "1.123"));
                }
                assert_eq!(
                    count,
                    if filtered {
                        usize::from(category != ScheduleCategory::Door)
                    } else if category == ScheduleCategory::All {
                        2
                    } else {
                        1
                    }
                );
            }
        }
    }
    let (app, id) = saved(ScheduleCategory::RoomFinish);
    assert!(report(app.editor.document.model(), id).is_err());
    assert!(report(app.editor.document.model(), Id::new()).is_err());
}

#[test]
fn csv_export_uses_committed_values_and_preserves_document_state() {
    for kind in [ReportKind::Instances, ReportKind::Quantities] {
        committed_values(kind);
    }
}

fn committed_values(kind: ReportKind) {
    let (mut app, id) = saved(ScheduleCategory::Door);
    let door = app
        .editor
        .document
        .model()
        .openings
        .values()
        .find(|o| matches!(o.parameters.definition, OpeningDefinition::Typed { .. }))
        .unwrap()
        .id();
    app.begin_schedule_cell(id, door, ScheduleNumericField::Width);
    app.opening_schedule.draft.as_mut().unwrap().value = "9.999".into();
    let before = app.editor.document.model().clone();
    let history = app.editor.document.history_stats();
    let dirty = app.editor.is_dirty();
    let temp = tempfile::tempdir().unwrap();
    let mut draft = ExportDraft::with_kind(&app, id, kind).unwrap();
    draft.path = temp.path().join("report.csv").to_string_lossy().into();
    draft.write(&app).unwrap();
    let rows = parse(&std::fs::read(&draft.path).unwrap());
    assert!(rows[1].iter().any(|c| c == "0.900"));
    assert!(!rows[1].iter().any(|c| c == "9.999"));
    assert_eq!(app.editor.document.model(), &before);
    assert_eq!(app.editor.document.history_stats(), history);
    assert_eq!(app.editor.is_dirty(), dirty);
}

#[test]
fn csv_stale_contexts_and_escape_or_close_never_write() {
    for kind in [ReportKind::Instances, ReportKind::Quantities] {
        stale_contexts(kind);
    }
}

fn stale_contexts(kind: ReportKind) {
    for change in 0..6 {
        let (mut app, id) = saved(ScheduleCategory::All);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("cancel.csv");
        let mut draft = ExportDraft::with_kind(&app, id, kind).unwrap();
        draft.path = path.to_string_lossy().into();
        match change {
            0 => draft.session = Id::new(),
            1 => draft.revision += 1,
            2 => draft.definition.name = "different".into(),
            3 => app.opening_schedule.selected = None,
            4 => app.opening_schedule.open = false,
            _ => {}
        }
        if change < 5 {
            assert!(draft.write(&app).is_err());
        }
        app.opening_schedule.csv_export = Some(draft);
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events: if change == 5 {
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ctx| app.schedule_csv_window(ctx),
        );
        assert!(app.opening_schedule.csv_export.is_none());
        assert!(!path.exists());
    }
}

#[test]
fn csv_atomic_no_clobber_replace_and_failure_preserve_destinations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.csv");
    write_report(&path, b"old\r\n", false).unwrap();
    assert!(write_report(&path, b"racing writer\r\n", false).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"old\r\n");
    write_report(&path, b"new\r\n", true).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new\r\n");
    assert!(write_report(&path, &vec![0; MAX_BYTES + 1], true).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"new\r\n");
    let folder = dir.path().join("folder.csv");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("keep"), b"keep").unwrap();
    assert!(write_report(&folder, b"replacement", true).is_err());
    assert_eq!(std::fs::read(folder.join("keep")).unwrap(), b"keep");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    assert!(write_report(&dir.path().join("missing/report.csv"), b"x", false).is_err());
    assert!(csv_path("report.txt").is_err());
    assert!(csv_path("report.CSV").is_ok());
}

#[test]
fn csv_dialog_cancel_and_path_edits_clear_consent() {
    for kind in [ReportKind::Instances, ReportKind::Quantities] {
        dialog_cancel(kind);
    }
}

fn dialog_cancel(kind: ReportKind) {
    let (mut app, id) = saved(ScheduleCategory::All);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dialog.csv");
    std::fs::write(&path, b"original").unwrap();
    let mut draft = ExportDraft::with_kind(&app, id, kind).unwrap();
    draft.path = path.to_string_lossy().into();
    draft.replace = true;
    app.opening_schedule.csv_export = Some(draft);
    let ctx = egui::Context::default();
    let frame = |app: &mut DesktopApp, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.schedule_csv_window(ctx),
        )
    };
    let _ = frame(&mut app, vec![]);
    let output = frame(&mut app, vec![]);
    let text_position = |output: &egui::FullOutput, label: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + egui::vec2(4.0, 5.0))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing {label}"))
    };
    let click = |app: &mut DesktopApp, pos| {
        for pressed in [true, false] {
            let _ = frame(
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    };
    click(&mut app, text_position(&output, &path.to_string_lossy()));
    let _ = frame(&mut app, vec![egui::Event::Text("x".into())]);
    assert!(!app.opening_schedule.csv_export.as_ref().unwrap().replace);
    let output = frame(&mut app, vec![]);
    click(&mut app, text_position(&output, "Cancel"));
    assert!(app.opening_schedule.csv_export.is_none());
    assert_eq!(std::fs::read(path).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn csv_action_is_only_visible_for_saved_opening_schedules() {
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
        ScheduleCategory::RoomFinish,
    ] {
        let (mut app, _) = saved(category);
        for live in [false, true] {
            if live {
                app.opening_schedule.selected = None;
            }
            let ctx = egui::Context::default();
            let output = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.schedule_csv_action(ui));
            });
            let visible = output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Export CSV"));
            assert_eq!(visible, !live && category != ScheduleCategory::RoomFinish);
            let quantities = output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Export quantity CSV"));
            assert_eq!(quantities, visible);
        }
    }
}

#[test]
fn quantity_summary_follows_instances_and_is_read_only_and_saved_only() {
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
        ScheduleCategory::RoomFinish,
    ] {
        for live in [false, true] {
            let (mut app, _) = saved(category);
            if live {
                app.opening_schedule.selected = None;
            }
            let before = app.editor.document.model().clone();
            let history = app.editor.document.history_stats();
            let dirty = app.editor.is_dirty();
            let ctx = egui::Context::default();
            let mut output = None;
            for _ in 0..3 {
                output = Some(ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1600.0, 1200.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| app.opening_schedule_window(ctx),
                ));
            }
            let output = output.unwrap();
            let position = |label: &str| {
                output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => Some(text.pos.y),
                    _ => None,
                })
            };
            let summary = position("Quantity summary");
            assert_eq!(
                summary.is_some(),
                !live && category != ScheduleCategory::RoomFinish
            );
            if let Some(summary) = summary {
                let instance = if category == ScheduleCategory::Window {
                    "Schedule window"
                } else {
                    "Schedule door"
                };
                assert!(position(instance).unwrap() < summary);
            }
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.document.history_stats(), history);
            assert_eq!(app.editor.is_dirty(), dirty);
        }
    }
}

#[test]
fn quantity_report_caps_encoded_output_and_escapes_line_breaks() {
    let (app, id) = saved(ScheduleCategory::Door);
    let model = app.editor.document.model();
    let mut rows = defined_rows(model, Some(&model.schedules[&id].parameters)).unwrap();
    // Exercise the encoder's bounds/quoting independently of model name validation.
    rows[0].type_name = "  @a,\"b\"\r\nc".into();
    let (bytes, _, _) = encode_quantities(&rows).unwrap();
    assert_eq!(parse(&bytes)[1][1], "'  @a,\"b\"\r\nc");
    rows[0].type_name = "x".repeat(MAX_BYTES);
    assert!(
        encode_quantities(&rows)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
}
