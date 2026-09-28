use super::*;

#[test]
fn nearby_heights_sills_and_equal_zero_are_exact_and_distinguishable() {
    let (app, _, _) = super::super::tests::fixture();
    let mut rows = super::super::rows(app.editor.document.model()).unwrap();
    rows[1].height = rows[0].height + 0.00001;
    rows[0].sill = 0.90001;
    rows[1].sill = 0.90002;
    let mut definition = ScheduleParams::new("Precise", ScheduleCategory::All);
    for key in [ScheduleGroupField::Height, ScheduleGroupField::Sill] {
        definition.group_by = vec![key];
        let result = table(&rows, &definition);
        assert_eq!(result.rows.len(), 3);
        assert_ne!(result.rows[0][1], result.rows[1][1]);
    }
    rows[0].sill = -0.0;
    rows[1].sill = 0.0;
    assert_eq!(
        table(&rows, &definition).rows[0],
        ["Group", "Sill (m): 0.000", "2"]
    );
}

#[test]
fn selected_keys_exact_values_identity_legacy_and_stable_order() {
    let (app, _, _) = super::super::tests::fixture();
    let mut rows = super::super::rows(app.editor.document.model()).unwrap();
    let mut variant = rows[0].clone();
    variant.id = Id::new();
    variant.width = 0.90001;
    let mut same_name = variant.clone();
    same_name.id = Id::new();
    same_name.type_id = Some(Id::new());
    let mut legacy = rows[0].clone();
    legacy.id = Id::new();
    legacy.type_id = None;
    let mut legacy_variant = legacy.clone();
    legacy_variant.id = Id::new();
    legacy_variant.width = 0.90002;
    rows.extend([variant, same_name, legacy, legacy_variant]);
    let mut definition = ScheduleParams::new("Counts", ScheduleCategory::All);
    definition.group_by = vec![ScheduleGroupField::Type];
    let result = table(&rows, &definition);
    assert_eq!(result.rows.len(), 5);
    assert_eq!(result.rows[0], ["Group", "Type: Legacy Door", "2"]);
    assert_eq!(result.rows[1], ["Group", "Type: Legacy Window", "1"]);
    assert_eq!(result.rows.last().unwrap(), &["Grand count", "", "6"]);
    definition.group_by = vec![ScheduleGroupField::Type, ScheduleGroupField::Width];
    let result = table(&rows, &definition);
    assert_eq!(result.rows.iter().filter(|r| r[0] == "Group").count(), 6);
    assert_eq!(result.rows.iter().filter(|r| r[0] == "Subtotal").count(), 4);
    assert!(result.rows.iter().any(|r| r[2] == "Width (m): 0.90001"));
    assert!(result.rows.iter().any(|r| r[2] == "Width (m): 0.90002"));
    let subtotal: usize = result
        .rows
        .iter()
        .filter(|r| r[0] == "Subtotal")
        .map(|r| r[3].parse::<usize>().unwrap())
        .sum();
    assert_eq!(subtotal, rows.len());
    let mut reversed = rows.clone();
    reversed.reverse();
    assert_eq!(table(&reversed, &definition).rows, result.rows);
    definition.group_by = vec![ScheduleGroupField::Kind];
    let order = ordered(&rows, &definition.group_by);
    assert_eq!(
        order
            .iter()
            .filter(|r| r.kind == OpeningKind::Door)
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        rows.iter()
            .filter(|r| r.kind == OpeningKind::Door)
            .map(|r| r.id)
            .collect::<Vec<_>>()
    );
    definition.group_by = vec![ScheduleGroupField::Width];
    assert_eq!(
        table(&rows, &definition)
            .rows
            .iter()
            .filter(|r| r[0] == "Group")
            .count(),
        4
    );
    definition.group_by = vec![ScheduleGroupField::Level, ScheduleGroupField::Kind];
    assert_eq!(table(&rows, &definition).rows.len(), 4);
    assert_eq!(
        table(&[], &definition).rows,
        vec![vec!["Grand count", "", "", "0"]]
    );
}

#[test]
fn filtered_committed_rows_and_paper_pdf_share_counts_and_report_overflow() {
    let (app, _, _) = super::super::tests::fixture();
    let mut model = app.editor.document.model().clone();
    let mut definition = ScheduleParams::new("Grouped", ScheduleCategory::All);
    definition.group_by = vec![ScheduleGroupField::Kind, ScheduleGroupField::Width];
    definition.filters.push(ScheduleFilter::Numeric {
        field: ScheduleNumericField::Width,
        operator: os_model::ScheduleNumericOperator::Less,
        value: 1.0,
    });
    let schedule = os_model::Schedule::new("core.schedule", definition.clone());
    let id = schedule.id();
    model.schedules.insert(id, schedule);
    let rows = defined_rows(&model, Some(&definition)).unwrap();
    assert_eq!(rows.len(), 1);
    let summary = table(&rows, &definition);
    let paper = paper_table(&model, id).unwrap();
    assert_eq!(summary.columns, paper.columns);
    assert_eq!(summary.rows, paper.rows);
    use os_render::sheet::{PaperRect, SheetPage, append_schedule_table};
    let page = SheetPage::new(1000.0, 1000.0, vec![]).unwrap();
    let rect = PaperRect {
        min_mm: os_core::Point2::new(0.0, 0.0),
        max_mm: os_core::Point2::new(900.0, 900.0),
    };
    let pdf = String::from_utf8(
        append_schedule_table(page.clone(), rect, &paper)
            .unwrap()
            .to_pdf()
            .unwrap(),
    )
    .unwrap();
    for cell in paper.rows.iter().flatten().filter(|c| !c.is_empty()) {
        let hex = cell
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<String>();
        assert!(pdf.contains(&format!("<{hex}> Tj")), "missing PDF {cell}");
    }
    let short = PaperRect {
        max_mm: os_core::Point2::new(900.0, 20.0),
        ..rect
    };
    assert!(
        append_schedule_table(page.clone(), short, &paper)
            .unwrap_err()
            .to_string()
            .contains("row overflow")
    );
    let narrow = PaperRect {
        max_mm: os_core::Point2::new(20.0, 900.0),
        ..rect
    };
    assert!(
        append_schedule_table(page, narrow, &paper)
            .unwrap_err()
            .to_string()
            .contains("overflow")
    );
}
