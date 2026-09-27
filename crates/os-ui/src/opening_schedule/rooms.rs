//! Live room intent and topology resolution shared by the schedule window and paper table.
use super::*;
use os_geometry::rooms::{FaceKey, SeedDiagnostic, derive_faces};

#[cfg(test)]
pub(crate) mod tests;

pub(super) struct RoomRow {
    pub id: Id,
    pub level: Id,
    level_name: String,
    parameters: os_model::RoomParams,
    finishes: [String; 3],
    pub area: Option<f64>,
    pub status: String,
}

pub(super) fn rows(model: &Model, definition: &ScheduleParams) -> Result<Vec<RoomRow>> {
    definition.validate()?;
    os_core::ensure(
        definition.category == ScheduleCategory::RoomFinish,
        "expected room finish schedule",
    )?;
    let mut levels = std::collections::BTreeMap::new();
    let mut result = Vec::new();
    for room in model.rooms.values() {
        let p = &room.parameters;
        if let std::collections::btree_map::Entry::Vacant(entry) = levels.entry(p.level) {
            entry.insert(derive_faces(&model.room_boundary_segments(p.level)?));
        }
        let resolved = match &levels[&p.level] {
            Err(error) => Err(format!("Room boundary derivation failed: {error:?}")),
            Ok(faces) => FaceKey::from_signature(&p.boundary_signature)
                .ok_or(SeedDiagnostic::FaceChanged)
                .and_then(|key| faces.resolve_seed(p.seed, &key))
                .map(|face| face.area_m2)
                .map_err(|error| {
                    match error {
                        SeedDiagnostic::InvalidSeed => "Room seed is invalid",
                        SeedDiagnostic::OnBoundary => "Room seed lies on a room boundary",
                        SeedDiagnostic::NotEnclosed => "Room is not enclosed",
                        SeedDiagnostic::AmbiguousBoundary => "Room lies in overlapping boundaries",
                        SeedDiagnostic::FaceChanged => "Room enclosure changed; reassign room",
                    }
                    .to_owned()
                }),
        };
        let (area, status) = match resolved {
            Ok(area) => (Some(area), "Enclosed".into()),
            Err(message) => (None, message),
        };
        let level = model
            .levels
            .get(&p.level)
            .ok_or_else(|| Error::Invalid("Room level is missing".into()))?;
        result.push(RoomRow {
            id: room.id(),
            level: p.level,
            level_name: level.parameters.name.clone(),
            parameters: p.clone(),
            finishes: [
                (p.floor_material, &p.floor_finish),
                (p.wall_material, &p.wall_finish),
                (p.ceiling_material, &p.ceiling_finish),
            ]
            .map(|(material, code)| {
                let name = material
                    .and_then(|id| model.materials.get(&id))
                    .map(|m| m.parameters.name.as_str());
                match (name, code.as_deref()) {
                    (Some(name), Some(code)) => format!("{name} · {code}"),
                    (Some(name), None) => name.to_owned(),
                    (None, Some(code)) => code.to_owned(),
                    (None, None) => String::new(),
                }
            }),
            area,
            status,
        });
    }
    result.sort_by(|a, b| {
        match definition.sort {
            ScheduleSort::LevelNumber => {
                (&a.level_name, &a.parameters.number).cmp(&(&b.level_name, &b.parameters.number))
            }
            ScheduleSort::Number => a.parameters.number.cmp(&b.parameters.number),
            ScheduleSort::Name => a.parameters.name.cmp(&b.parameters.name),
            ScheduleSort::Area => match (a.area, b.area) {
                (Some(a), Some(b)) => a.total_cmp(&b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            },
            _ => unreachable!("validated room sort"),
        }
        .then(a.id.cmp(&b.id))
    });
    Ok(result)
}

impl RoomRow {
    pub(super) fn cell(&self, column: ScheduleColumn) -> String {
        match column {
            ScheduleColumn::Level => self.level_name.clone(),
            ScheduleColumn::Number => self.parameters.number.clone(),
            ScheduleColumn::Name => self.parameters.name.clone(),
            ScheduleColumn::FloorFinish => self.finishes[0].clone(),
            ScheduleColumn::WallFinish => self.finishes[1].clone(),
            ScheduleColumn::CeilingFinish => self.finishes[2].clone(),
            ScheduleColumn::Area => self.area.map(|v| format!("{v:.2}")).unwrap_or_default(),
            ScheduleColumn::EnclosureStatus => self.status.clone(),
            _ => unreachable!("validated room column"),
        }
    }
}

impl DesktopApp {
    pub(super) fn select_schedule_room(&mut self, id: Id) {
        let Some(room) = self.editor.document.model().rooms.get(&id) else {
            return;
        };
        let view = floor_plan(self.editor.document.model(), room.parameters.level);
        if let Some(view) = view {
            self.plans.active_sheet = None;
            self.focus_plan(Some(view));
        }
        self.select(Some(id));
        self.status = if view.is_some() {
            "Room selected in its floor plan."
        } else {
            "Room selected. Create a floor plan on its level to navigate to it."
        }
        .into();
    }

    pub(super) fn room_schedule_rows(&mut self, ui: &mut egui::Ui, definition: &ScheduleParams) {
        ui.label("Materials and finish codes • Area in m² from current room boundaries • Click a row to edit room properties");
        match rows(self.editor.document.model(), definition) {
            Err(error) => {
                ui.colored_label(crate::theme::ERROR, error.to_string());
            }
            Ok(rows) => {
                if rows.is_empty() {
                    ui.label("No rooms.");
                }
                if rows.iter().any(|r| {
                    Some(r.id) == self.selected
                        && floor_plan(self.editor.document.model(), r.level).is_none()
                }) {
                    ui.label("Room selected. Create a floor plan on its level to navigate to it.");
                }
                let mut selected = None;
                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("room_finish_schedule")
                        .striped(true)
                        .show(ui, |ui| {
                            for column in &definition.columns {
                                ui.strong(if *column == ScheduleColumn::Name {
                                    "Name"
                                } else {
                                    column.label()
                                });
                            }
                            ui.end_row();
                            for row in rows {
                                ui.push_id(row.id, |ui| {
                                    for column in &definition.columns {
                                        if ui
                                            .selectable_label(
                                                self.selected == Some(row.id),
                                                row.cell(*column),
                                            )
                                            .clicked()
                                        {
                                            selected = Some(row.id);
                                        }
                                    }
                                    ui.end_row();
                                });
                            }
                        });
                });
                if let Some(id) = selected {
                    self.select_schedule_room(id);
                }
            }
        }
    }
}
