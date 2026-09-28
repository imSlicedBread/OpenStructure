//! Bounded reports from committed saved opening schedules.
use super::*;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy)]
enum ReportKind {
    Instances,
    Quantities,
}

pub(super) struct ExportDraft {
    kind: ReportKind,
    session: Id,
    revision: u64,
    schedule: Id,
    definition: ScheduleParams,
    bytes: Vec<u8>,
    columns: Vec<String>,
    rows: usize,
    path: String,
    replace: bool,
    error: Option<String>,
}

fn cell(value: &str, text: bool) -> String {
    let dangerous = text && value.trim_start().starts_with(['=', '+', '-', '@']);
    let value = if dangerous {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

fn report(model: &Model, id: Id) -> Result<(Vec<u8>, Vec<String>, usize)> {
    let definition = &model
        .schedules
        .get(&id)
        .ok_or_else(|| Error::Invalid("Saved schedule is missing".into()))?
        .parameters;
    os_core::ensure(
        definition.category != ScheduleCategory::RoomFinish,
        "CSV export requires a saved Door, Window or All opening schedule",
    )?;
    let table = instance_table(model, id)?;
    // Schedule labels already carry the dimension units.
    let columns = table.columns;
    let mut bytes = Vec::new();
    let mut record = |values: Vec<String>| -> Result<()> {
        let line = values.join(",") + "\r\n";
        os_core::ensure(
            bytes.len().saturating_add(line.len()) <= MAX_BYTES,
            "Schedule CSV report exceeds the 16 MiB limit",
        )?;
        bytes.extend_from_slice(line.as_bytes());
        Ok(())
    };
    record(columns.iter().map(|v| cell(v, true)).collect())?;
    for row in &table.rows {
        record(
            row.iter()
                .zip(&definition.columns)
                .map(|(value, column)| cell(value, numeric_field(*column).is_none()))
                .collect(),
        )?;
    }
    Ok((bytes, columns, table.rows.len()))
}

fn quantity_report(model: &Model, id: Id) -> Result<(Vec<u8>, Vec<String>, usize)> {
    let definition = &model
        .schedules
        .get(&id)
        .ok_or_else(|| Error::Invalid("Saved schedule is missing".into()))?
        .parameters;
    let rows = defined_rows(model, Some(definition))?;
    if !definition.group_by.is_empty() {
        return encode_grouped(grouping::table(&rows, definition));
    }
    encode_quantities(&rows)
}

fn encode_grouped(table: os_render::sheet::PaperTable) -> Result<(Vec<u8>, Vec<String>, usize)> {
    let mut bytes = (table.columns.join(",") + "\r\n").into_bytes();
    for row in &table.rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, value)| cell(value, i + 1 < table.columns.len()))
            .collect::<Vec<_>>()
            .join(",")
            + "\r\n";
        os_core::ensure(
            bytes.len().saturating_add(line.len()) <= MAX_BYTES,
            "Schedule CSV report exceeds the 16 MiB limit",
        )?;
        bytes.extend_from_slice(line.as_bytes());
    }
    Ok((bytes, table.columns, table.rows.len()))
}

fn encode_quantities(rows: &[Row]) -> Result<(Vec<u8>, Vec<String>, usize)> {
    let groups = quantities(rows);
    let columns: Vec<String> = QUANTITY_COLUMNS.iter().map(|s| (*s).into()).collect();
    let mut bytes = (columns.join(",") + "\r\n").into_bytes();
    for group in &groups {
        let line = group
            .cells()
            .iter()
            .enumerate()
            .map(|(index, value)| cell(value, index < 3))
            .collect::<Vec<_>>()
            .join(",")
            + "\r\n";
        os_core::ensure(
            bytes.len().saturating_add(line.len()) <= MAX_BYTES,
            "Schedule CSV report exceeds the 16 MiB limit",
        )?;
        bytes.extend_from_slice(line.as_bytes());
    }
    Ok((bytes, columns, groups.len()))
}

fn csv_path(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value.trim());
    os_core::ensure(
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("csv")),
        "Choose a file path ending in .csv",
    )?;
    Ok(path)
}

fn write_report(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    os_core::ensure(
        bytes.len() <= MAX_BYTES,
        "Schedule CSV report exceeds the 16 MiB limit",
    )?;
    let error = |e: std::io::Error| Error::Storage(e.to_string());
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    temporary.write_all(bytes).map_err(error)?;
    temporary.as_file_mut().sync_all().map_err(error)?;
    if replace {
        temporary
            .persist(path)
            .map_err(|e| Error::Storage(e.to_string()))?;
    } else {
        temporary
            .persist_noclobber(path)
            .map_err(|e| Error::Storage(e.to_string()))?;
    }
    Ok(())
}

impl ExportDraft {
    fn new(app: &DesktopApp, schedule: Id) -> Result<Self> {
        Self::with_kind(app, schedule, ReportKind::Instances)
    }

    fn with_kind(app: &DesktopApp, schedule: Id, kind: ReportKind) -> Result<Self> {
        let (bytes, columns, rows) = match kind {
            ReportKind::Instances => report(app.editor.document.model(), schedule)?,
            ReportKind::Quantities => quantity_report(app.editor.document.model(), schedule)?,
        };
        Ok(Self {
            kind,
            session: app.editor.document.session_id(),
            revision: app.editor.document.revision(),
            schedule,
            definition: app.editor.document.model().schedules[&schedule]
                .parameters
                .clone(),
            bytes,
            columns,
            rows,
            path: match kind {
                ReportKind::Instances => "opening-schedule.csv",
                ReportKind::Quantities => "opening-quantities.csv",
            }
            .into(),
            replace: false,
            error: None,
        })
    }

    fn current(&self, app: &DesktopApp) -> bool {
        self.session == app.editor.document.session_id()
            && self.revision == app.editor.document.revision()
            && app.opening_schedule.open
            && app.opening_schedule.selected == Some(self.schedule)
            && app
                .editor
                .document
                .model()
                .schedules
                .get(&self.schedule)
                .is_some_and(|s| s.parameters == self.definition)
    }

    fn write(&self, app: &DesktopApp) -> Result<()> {
        os_core::ensure(
            self.current(app),
            "Schedule CSV export cancelled: context changed",
        )?;
        write_report(&csv_path(&self.path)?, &self.bytes, self.replace)
    }
}

impl DesktopApp {
    pub(super) fn schedule_csv_action(&mut self, ui: &mut egui::Ui) {
        if let Some(id) = self.opening_schedule.selected.filter(|id| {
            self.editor
                .document
                .model()
                .schedules
                .get(id)
                .is_some_and(|s| s.parameters.category != ScheduleCategory::RoomFinish)
        }) {
            let result = if ui.button("Export CSV").clicked() {
                Some(ExportDraft::new(self, id))
            } else if ui.button("Export quantity CSV").clicked() {
                Some(ExportDraft::with_kind(self, id, ReportKind::Quantities))
            } else {
                None
            };
            match result {
                Some(Ok(draft)) => self.opening_schedule.csv_export = Some(draft),
                Some(Err(error)) => self.report(Err(error), ""),
                None => {}
            }
        }
    }

    pub(super) fn schedule_csv_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.opening_schedule.csv_export.take() else {
            return;
        };
        if !draft.current(self) {
            self.status = "Schedule CSV export cancelled: document or schedule changed.".into();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return;
        }
        let mut open = true;
        let mut cancel = false;
        let mut export = false;
        let title = match draft.kind {
            ReportKind::Instances => "Export opening schedule CSV",
            ReportKind::Quantities => "Export opening quantity CSV",
        };
        egui::Window::new(title).open(&mut open).show(ctx, |ui| {
            ui.label(format!("Schedule: {}", draft.definition.name));
            ui.label(format!("Columns: {}", draft.columns.join(", ")));
            ui.label(match draft.kind {
                ReportKind::Instances => format!("{} rows", draft.rows),
                ReportKind::Quantities if !draft.definition.group_by.is_empty() => format!("{} summary rows (groups, subtotals and grand count)", draft.rows),
                ReportKind::Quantities => format!("{} quantity groups", draft.rows),
            });
            ui.label(match draft.kind {
                ReportKind::Instances => "Report only: metres rounded to three decimals; not lossless model interchange.",
                ReportKind::Quantities => "Quantity report: metres use three decimals when exact, otherwise shortest round-trip decimals to distinguish exact variants; not lossless model interchange.",
            });
            ui.label("Text beginning with =, +, - or @ after whitespace is prefixed with an apostrophe for spreadsheet safety.");
            ui.label("UTF-8 without BOM · comma separated · CRLF records · maximum 16 MiB");
            if ui.text_edit_singleline(&mut draft.path).changed() {
                draft.replace = false;
                draft.error = None;
            }
            let path = csv_path(&draft.path);
            let exists = path.as_ref().is_ok_and(|p| p.symlink_metadata().is_ok());
            if exists { ui.checkbox(&mut draft.replace, "Replace existing CSV file"); }
            if let Err(error) = &path { ui.colored_label(crate::theme::ERROR, error.to_string()); }
            if let Some(error) = &draft.error { ui.colored_label(crate::theme::ERROR, error); }
            ui.horizontal(|ui| {
                export = ui.add_enabled(path.is_ok() && (!exists || draft.replace), egui::Button::new("Export")).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if !open || cancel {
            return;
        }
        if export {
            match draft.write(self) {
                Ok(()) => {
                    self.status = match draft.kind {
                        ReportKind::Instances => {
                            format!("Exported {} opening schedule rows to CSV.", draft.rows)
                        }
                        ReportKind::Quantities if !draft.definition.group_by.is_empty() => {
                            format!("Exported {} opening summary rows to CSV.", draft.rows)
                        }
                        ReportKind::Quantities => {
                            format!("Exported {} opening quantity groups to CSV.", draft.rows)
                        }
                    };
                    return;
                }
                Err(error) => {
                    draft.error = Some(error.to_string());
                    draft.replace = false;
                }
            }
        }
        self.opening_schedule.csv_export = Some(draft);
    }
}

#[cfg(test)]
mod tests;
