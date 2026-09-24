//! Fixed typography, single-rectangle tables. Every value must fit before marks are appended.
use super::*;

pub struct PaperTable {
    pub heading: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

const TEXT_MM: f64 = 2.5;
const TITLE_MM: f64 = 8.0;
const ROW_MM: f64 = 6.0;
const PAD_MM: f64 = 2.0;

// Conservative advance: every supported Helvetica/WinAnsi glyph fits within
// 1.05 em. Deliberately reserve this width rather than risk substituted-font clipping.
fn text_width(text: &str, context: &str) -> Result<f64> {
    ensure(
        text.len() <= 4096 && !text.chars().any(char::is_control),
        format!("schedule {context}: text exceeds 4096 bytes or contains controls"),
    )?;
    for ch in text.chars() {
        ensure(
            winansi_byte(ch).is_some(),
            format!(
                "schedule {context}: unsupported text U+{:04X} (Helvetica/WinAnsi)",
                u32::from(ch)
            ),
        )?;
    }
    Ok(text.chars().count() as f64 * TEXT_MM * 1.05)
}

pub fn append_schedule_table(
    mut page: SheetPage,
    rect: PaperRect,
    table: &PaperTable,
) -> Result<SheetPage> {
    rect.validate()?;
    ensure(
        rect.max_mm.x <= page.width_mm && rect.max_mm.y <= page.height_mm,
        "schedule table page overflow",
    )?;
    ensure(
        !table.columns.is_empty() && table.columns.len() <= 8,
        "schedule column overflow: requires 1 to 8 columns",
    )?;
    ensure(
        table.rows.len() <= 10_000,
        "schedule row budget exceeds 10000",
    )?;
    ensure(
        table.rows.iter().all(|r| r.len() == table.columns.len()),
        "schedule row has mismatched column count",
    )?;
    let required_height = TITLE_MM + ROW_MM * (table.rows.len() + 1) as f64;
    let height = rect.max_mm.y - rect.min_mm.y;
    ensure(
        required_height <= height,
        format!(
            "schedule row overflow: {} rows need {required_height:.2} mm height; available {height:.2} mm",
            table.rows.len()
        ),
    )?;
    ensure(
        !table.heading.trim().is_empty(),
        "schedule heading is empty",
    )?;
    let heading_width = text_width(&table.heading, "heading")? + 2.0 * PAD_MM;
    let mut widths = Vec::with_capacity(table.columns.len());
    for (index, column) in table.columns.iter().enumerate() {
        let mut width = text_width(column, &format!("column {} heading", index + 1))?;
        for (row, cells) in table.rows.iter().enumerate() {
            width = width.max(text_width(
                &cells[index],
                &format!("row {} column {}", row + 1, index + 1),
            )?);
        }
        widths.push(width + 2.0 * PAD_MM);
    }
    let required_width: f64 = widths.iter().sum();
    let width = rect.max_mm.x - rect.min_mm.x;
    ensure(
        required_width.max(heading_width) <= width,
        format!(
            "schedule column/heading overflow: need {:.2} mm width; available {width:.2} mm. Enlarge the table or reduce saved columns/text",
            required_width.max(heading_width)
        ),
    )?;
    let marks_needed = 1
        + table.columns.len() * (table.rows.len() + 1)
        + table.rows.len()
        + 3
        + table.columns.len()
        + 1;
    ensure(
        page.marks.len().saturating_add(marks_needed) <= MAX_SHEET_MARKS,
        "schedule mark budget overflow",
    )?;
    let extra = (width - required_width) / widths.len() as f64;
    for value in &mut widths {
        *value += extra;
    }
    let mut marks = Vec::with_capacity(marks_needed);
    let mut label = |x, y, text: &str| -> Result<()> {
        if !text.is_empty() {
            push_page_text(
                &mut marks,
                Point2::new(x, y),
                text,
                TEXT_MM,
                PaperColor::BLACK,
            )?;
        }
        Ok(())
    };
    label(rect.min_mm.x + PAD_MM, rect.min_mm.y + 5.0, &table.heading)?;
    for (row, cells) in std::iter::once(&table.columns)
        .chain(table.rows.iter())
        .enumerate()
    {
        let mut x = rect.min_mm.x;
        for (index, cell) in cells.iter().enumerate() {
            label(
                x + PAD_MM,
                rect.min_mm.y + TITLE_MM + row as f64 * ROW_MM + 4.0,
                cell,
            )?;
            x += widths[index];
        }
    }
    let stroke = Some(PaperStroke {
        color: PaperColor::BLACK,
        width_mm: 0.18,
        dashed: false,
    });
    let bottom = rect.min_mm.y + required_height;
    for y in std::iter::once(rect.min_mm.y)
        .chain((0..=table.rows.len() + 1).map(|row| rect.min_mm.y + TITLE_MM + row as f64 * ROW_MM))
    {
        marks.push(path_mark(
            vec![Point2::new(rect.min_mm.x, y), Point2::new(rect.max_mm.x, y)],
            false,
            stroke,
            None,
            None,
        ));
    }
    let mut x = rect.min_mm.x;
    for index in 0..=widths.len() {
        let top = if index == 0 || index == widths.len() {
            rect.min_mm.y
        } else {
            rect.min_mm.y + TITLE_MM
        };
        marks.push(path_mark(
            vec![Point2::new(x, top), Point2::new(x, bottom)],
            false,
            stroke,
            None,
            None,
        ));
        if index < widths.len() {
            x += widths[index];
        }
    }
    page.marks.extend(marks);
    page.validate()?;
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table() -> PaperTable {
        PaperTable {
            heading: "Doors".into(),
            columns: vec!["Name".into()],
            rows: vec![vec!["D01".into()]],
        }
    }
    fn rect() -> PaperRect {
        PaperRect {
            min_mm: Point2::new(10.0, 10.0),
            max_mm: Point2::new(80.0, 30.0),
        }
    }
    #[test]
    fn exact_height_searchable_text_and_one_row_overflow() {
        let page = SheetPage::new(420.0, 297.0, vec![]).unwrap();
        let mut data = table();
        let composed = append_schedule_table(page.clone(), rect(), &data).unwrap();
        let pdf = String::from_utf8(composed.to_pdf().unwrap()).unwrap();
        for text in ["Doors", "Name", "D01"] {
            assert!(composed.marks().iter().any(
                |m| matches!(&m.kind, PaperMarkKind::Text {text: value, ..} if value == text)
            ));
            let hex: String = text.bytes().map(|b| format!("{b:02X}")).collect();
            assert!(pdf.contains(&format!("<{hex}> Tj")));
        }
        data.rows.push(vec!["D02".into()]);
        assert!(
            append_schedule_table(page, rect(), &data)
                .unwrap_err()
                .to_string()
                .contains("row overflow")
        );
    }
    #[test]
    fn width_encoding_page_and_mark_limits_fail_explicitly() {
        let page = SheetPage::new(420.0, 297.0, vec![]).unwrap();
        let mut data = table();
        data.rows[0][0] = "W".repeat(50);
        assert!(
            append_schedule_table(page.clone(), rect(), &data)
                .unwrap_err()
                .to_string()
                .contains("column/heading overflow")
        );
        data.rows[0][0] = "Door 🚪".into();
        assert!(
            append_schedule_table(page.clone(), rect(), &data)
                .unwrap_err()
                .to_string()
                .contains("unsupported text U+1F6AA")
        );
        data = table();
        let mut outside = rect();
        outside.max_mm.x = 421.0;
        assert!(
            append_schedule_table(page.clone(), outside, &data)
                .unwrap_err()
                .to_string()
                .contains("page overflow")
        );
        let mark = PaperMark {
            clip: None,
            kind: PaperMarkKind::Text {
                baseline_mm: Point2::new(10.0, 10.0),
                text: "A".into(),
                size_mm: 2.5,
                color: PaperColor::BLACK,
            },
        };
        let full = SheetPage::new(420.0, 297.0, vec![mark; MAX_SHEET_MARKS]).unwrap();
        assert!(
            append_schedule_table(full, rect(), &data)
                .unwrap_err()
                .to_string()
                .contains("mark budget")
        );
    }
}
