//! Shared read-only group counts for UI, paper and CSV.
use super::*;
use os_model::ScheduleGroupField;
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

fn compare(a: &Row, b: &Row, field: ScheduleGroupField) -> Ordering {
    match field {
        ScheduleGroupField::Level => a.level.cmp(&b.level),
        ScheduleGroupField::Kind => {
            (a.kind == OpeningKind::Window).cmp(&(b.kind == OpeningKind::Window))
        }
        ScheduleGroupField::Type => a.type_id.cmp(&b.type_id).then_with(|| {
            if a.type_id.is_none() {
                (a.kind == OpeningKind::Window).cmp(&(b.kind == OpeningKind::Window))
            } else {
                Ordering::Equal
            }
        }),
        ScheduleGroupField::Width => number_cmp(a.width, b.width),
        ScheduleGroupField::Height => number_cmp(a.height, b.height),
        ScheduleGroupField::Sill => number_cmp(a.sill, b.sill),
    }
}

fn number_cmp(a: f64, b: f64) -> Ordering {
    if a == b {
        Ordering::Equal
    } else {
        a.total_cmp(&b)
    }
}

fn label(row: &Row, field: ScheduleGroupField) -> String {
    match field {
        ScheduleGroupField::Level => format!("Level: {} [{}]", row.level_name, row.level),
        ScheduleGroupField::Kind => format!("Kind: {:?}", row.kind),
        ScheduleGroupField::Type => row.type_id.map_or_else(
            || format!("Type: Legacy {:?}", row.kind),
            |id| format!("Type: {} [{id}]", row.type_name),
        ),
        ScheduleGroupField::Width => format!("Width (m): {}", quantity_dimension(row.width)),
        ScheduleGroupField::Height => format!("Height (m): {}", quantity_dimension(row.height)),
        ScheduleGroupField::Sill => format!(
            "Sill (m): {}",
            quantity_dimension(if row.sill == 0.0 { 0.0 } else { row.sill })
        ),
    }
}

pub(super) fn ordered<'a>(rows: &'a [Row], keys: &[ScheduleGroupField]) -> Vec<&'a Row> {
    let mut rows: Vec<_> = rows.iter().collect();
    // Stable sort retains saved instance sort and UUID ties within groups.
    rows.sort_by(|a, b| {
        keys.iter()
            .map(|&key| compare(a, b, key))
            .find(|o| !o.is_eq())
            .unwrap_or(Ordering::Equal)
    });
    rows
}

pub(super) fn table(rows: &[Row], definition: &ScheduleParams) -> os_render::sheet::PaperTable {
    let keys = &definition.group_by;
    let sorted = ordered(rows, keys);
    let mut output = Vec::new();
    let mut start = 0;
    while start < sorted.len() {
        let mut first_end = start + 1;
        while first_end < sorted.len() && compare(sorted[start], sorted[first_end], keys[0]).is_eq()
        {
            first_end += 1;
        }
        let mut next = start;
        while next < first_end {
            let mut end = next + 1;
            while end < first_end
                && keys
                    .iter()
                    .all(|&key| compare(sorted[next], sorted[end], key).is_eq())
            {
                end += 1;
            }
            let mut cells = vec!["Group".into()];
            cells.extend(keys.iter().map(|&key| label(sorted[next], key)));
            cells.push((end - next).to_string());
            output.push(cells);
            next = end;
        }
        if keys.len() == 2 {
            output.push(vec![
                "Subtotal".into(),
                label(sorted[start], keys[0]),
                String::new(),
                (first_end - start).to_string(),
            ]);
        }
        start = first_end;
    }
    let mut grand = vec!["Grand count".into()];
    grand.extend(keys.iter().map(|_| String::new()));
    grand.push(rows.len().to_string());
    output.push(grand);
    let mut columns = vec!["Row".into()];
    columns.extend(keys.iter().map(|key| format!("{key:?}")));
    columns.push("Count".into());
    os_render::sheet::PaperTable {
        heading: definition.name.clone(),
        columns,
        rows: output,
    }
}
