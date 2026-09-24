//! Live native opening schedules. Rows are derived; instance edits use document transactions.
use crate::{DesktopApp, egui};
use os_core::{Error, Id, Result};
use os_model::{Model, OpeningKind, ViewKind};
mod definitions;
use definitions::DefinitionDraft;
use os_model::{ScheduleCategory, ScheduleColumn, ScheduleParams, ScheduleSort};

#[derive(Default)]
pub(super) struct OpeningSchedule {
    pub open: bool,
    draft: Option<NameDraft>,
    selected: Option<Id>,
    session: Option<Id>,
    definition_draft: Option<DefinitionDraft>,
}

struct NameDraft {
    id: Id,
    session: Id,
    revision: u64,
    name: String,
    error: Option<String>,
}

impl NameDraft {
    fn current(&self, app: &DesktopApp) -> bool {
        self.session == app.editor.document.session_id()
            && self.revision == app.editor.document.revision()
            && app.editor.document.model().openings.contains_key(&self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_document::Command;
    use os_model::{
        Opening, OpeningDefinition, OpeningParams, OpeningType, OpeningTypeParams, Wall,
    };

    pub(super) fn fixture() -> (DesktopApp, Id, Id) {
        let mut app = DesktopApp::new().unwrap();
        let mut params = crate::default_wall(app.active_level);
        params.name = "Schedule host".into();
        params.end = os_core::Point2::new(10.0, 0.0);
        let wall = Wall::new(os_walls::WALL_TYPE, params);
        let host = wall.id();
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                family: Default::default(),
                name: "D-900".into(),
                pane_position: Default::default(),
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            },
        );
        let type_id = ty.id();
        let door = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Schedule door".into(),
                host,
                offset: 1.0,
                definition: OpeningDefinition::Typed { type_id },
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let door_id = door.id();
        let window = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Schedule window".into(),
                host,
                offset: 5.0,
                definition: OpeningDefinition::Legacy {
                    kind: OpeningKind::Window,
                    width: 1.2,
                    height: 1.1,
                    sill: 0.9,
                },
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        app.editor
            .document
            .execute(
                "Schedule fixture",
                vec![
                    Command::AddWall(wall),
                    Command::AddOpeningType(ty),
                    Command::AddOpening(door),
                    Command::AddOpening(window),
                ],
            )
            .unwrap();
        app.opening_schedule.open = true;
        (app, door_id, type_id)
    }

    #[test]
    fn resolved_rows_follow_type_history_and_archive_without_cached_data() {
        let (mut app, door, ty) = fixture();
        let before = rows(app.editor.document.model()).unwrap();
        assert_eq!(before.len(), 2);
        assert_eq!(before[0].id, door);
        assert_eq!(before[0].type_name, "D-900");
        assert_eq!(before[0].host_name, "Schedule host");
        assert_eq!(before[0].level, app.active_level);
        assert_eq!(before[0].width, 0.9);
        assert_eq!(
            (before[1].width, before[1].height, before[1].sill),
            (1.2, 1.1, 0.9)
        );
        assert_eq!(before[1].type_name, "Legacy (instance dimensions)");
        let mut parameters = app.editor.document.model().opening_types[&ty]
            .parameters
            .clone();
        parameters.width = 1.05;
        parameters.name = "D-1050".into();
        app.editor
            .document
            .execute(
                "Edit type",
                vec![Command::UpdateOpeningType { id: ty, parameters }],
            )
            .unwrap();
        let after = rows(app.editor.document.model()).unwrap();
        assert_eq!(after[0].width, 1.05);
        assert_eq!(after[0].type_name, "D-1050");
        app.editor.document.undo();
        assert_eq!(rows(app.editor.document.model()).unwrap(), before);
        app.editor.document.redo();
        assert_eq!(rows(app.editor.document.model()).unwrap(), after);
        let path = std::env::temp_dir().join(format!("opening-schedule-{}.osb", Id::new()));
        use os_storage::StorageBackend;
        os_storage::ZipJsonStorage
            .save(&app.editor.document, &path)
            .unwrap();
        let restored = os_storage::ZipJsonStorage.open(&path).unwrap();
        assert_eq!(rows(restored.model()).unwrap(), after);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn deterministic_order_and_invalid_references_are_explicit() {
        let (app, door, _) = fixture();
        let mut model = app.editor.document.model().clone();
        let mut duplicate = model.openings[&door].clone();
        duplicate = Opening::new("core.opening", duplicate.parameters);
        let other = duplicate.id();
        model.openings.insert(other, duplicate);
        let result = rows(&model).unwrap();
        assert_eq!(
            [result[0].id, result[1].id],
            [door.min(other), door.max(other)]
        );
        model.walls.clear();
        assert!(
            rows(&model)
                .unwrap_err()
                .to_string()
                .contains("host is missing")
        );
    }

    #[test]
    fn row_click_selects_and_navigates_without_model_or_history_changes() {
        for (size, scale) in [
            (egui::vec2(1280.0, 800.0), 1.0),
            (egui::vec2(1000.0, 650.0), 1.5),
        ] {
            for with_plan in [false, true] {
                let (mut app, door, _) = fixture();
                let plan = with_plan.then(|| {
                    app.editor
                        .create_floor_plan("Schedule plan", app.active_level)
                        .unwrap()
                });
                let before = app.editor.document.model().clone();
                let history = app.editor.document.history_stats();
                let ctx = egui::Context::default();
                crate::theme::apply(&ctx);
                let frame = |app: &mut DesktopApp, events| {
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
                    ctx.run(input, |ctx| app.opening_schedule_window(ctx))
                };
                frame(&mut app, vec![]);
                let output = frame(&mut app, vec![]);
                let position = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == "Schedule door" => {
                            Some(text.pos + egui::vec2(8.0, 5.0))
                        }
                        _ => None,
                    })
                    .expect("door row must be visible");
                for pressed in [true, false] {
                    frame(
                        &mut app,
                        vec![
                            egui::Event::PointerMoved(position),
                            egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                }
                assert_eq!(app.selected, Some(door));
                assert_eq!(app.plans.active, plan);
                assert_eq!(app.editor.document.model(), &before);
                assert_eq!(app.editor.document.history_stats(), history);
                if !with_plan {
                    assert!(app.status.contains("Create a floor plan"));
                    let output = frame(&mut app, vec![]);
                    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("Create a floor plan"))));
                }
            }
        }
    }

    #[test]
    fn view_ribbon_opens_schedule_window() {
        let (mut app, _, _) = fixture();
        app.opening_schedule.open = false;
        app.ribbon_tab = crate::RibbonTab::View;
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
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
                |ctx| app.show(ctx),
            )
        };
        let output = frame(&mut app, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Door/window schedule" => {
                    Some(text.pos + egui::vec2(8.0, 5.0))
                }
                _ => None,
            })
            .expect("View ribbon must expose the opening schedule");
        for pressed in [true, false] {
            frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(app.opening_schedule.open);
        let output = frame(&mut app, vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "Doors"
        )));
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "Windows"
        )));
    }

    fn frame(
        app: &mut DesktopApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.opening_schedule_window(ctx),
        )
    }

    fn click_text(app: &mut DesktopApp, ctx: &egui::Context, label: &str) {
        frame(app, ctx, vec![]);
        let output = frame(app, ctx, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + egui::vec2(5.0, 5.0))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing visible control: {label}"));
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }

    #[test]
    fn name_edit_is_draft_only_and_commits_one_undo_step() {
        let (mut app, door, _) = fixture();
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        click_text(&mut app, &ctx, "Edit name");
        assert_eq!(app.opening_schedule.draft.as_ref().unwrap().id, door);
        assert_eq!(app.selected, None);
        click_text(&mut app, &ctx, "Schedule door");
        frame(&mut app, &ctx, vec![egui::Event::Text(" renamed".into())]);
        let name = app.opening_schedule.draft.as_ref().unwrap().name.clone();
        assert_ne!(name, "Schedule door");
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        click_text(&mut app, &ctx, "Apply");
        assert!(app.opening_schedule.draft.is_none());
        let mut expected = before.clone();
        expected.openings.get_mut(&door).unwrap().parameters.name = name;
        assert_eq!(app.editor.document.model(), &expected);
        app.editor.document.undo();
        assert_eq!(app.editor.document.model(), &before);
        app.editor.document.redo();
        assert_eq!(app.editor.document.model(), &expected);
    }

    #[test]
    fn invalid_name_preserves_draft_model_and_history() {
        let (mut app, door, _) = fixture();
        let ctx = egui::Context::default();
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        for name in [" ".to_string(), "bad\nname".into(), "x".repeat(257)] {
            app.begin_schedule_name(door);
            app.opening_schedule.draft.as_mut().unwrap().name = name.clone();
            click_text(&mut app, &ctx, "Apply");
            let draft = app.opening_schedule.draft.as_ref().unwrap();
            assert_eq!(draft.name, name);
            assert!(draft.error.as_ref().unwrap().contains("name"));
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.document.history_stats(), history);
        }
        app.opening_schedule.draft.as_mut().unwrap().name = "Corrected".into();
        click_text(&mut app, &ctx, "Apply");
        assert_eq!(
            app.editor.document.model().openings[&door].parameters.name,
            "Corrected"
        );
    }

    #[test]
    fn stale_context_cancel_and_escape_never_commit() {
        let (mut app, door, _) = fixture();
        let ctx = egui::Context::default();
        for escape in [false, true] {
            app.begin_schedule_name(door);
            app.opening_schedule.draft.as_mut().unwrap().name = "Uncommitted".into();
            let before = app.editor.document.model().clone();
            let history = app.editor.document.history_stats();
            if escape {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            } else {
                click_text(&mut app, &ctx, "Cancel");
            }
            assert!(app.opening_schedule.draft.is_none());
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.document.history_stats(), history);
        }
        for session in [false, true] {
            app.begin_schedule_name(door);
            if session {
                // Same revision, different document session must also be rejected.
                app.opening_schedule.draft.as_mut().unwrap().session = Id::new();
            } else {
                let mut parameters = app.editor.document.model().openings[&door]
                    .parameters
                    .clone();
                parameters.name = "External edit".into();
                app.editor
                    .command(
                        "External edit",
                        Command::UpdateOpening {
                            id: door,
                            parameters,
                        },
                    )
                    .unwrap();
            }
            let before = app.editor.document.model().clone();
            let history = app.editor.document.history_stats();
            app.apply_schedule_name();
            assert!(app.opening_schedule.draft.is_none());
            assert!(app.status.contains("cancelled"));
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.document.history_stats(), history);
        }
        app.begin_schedule_name(door);
        app.editor.document.undo();
        frame(&mut app, &ctx, vec![]);
        assert!(app.opening_schedule.draft.is_none());
    }
}

#[derive(Debug, PartialEq)]
struct Row {
    id: Id,
    name: String,
    kind: OpeningKind,
    type_name: String,
    level: Id,
    level_name: String,
    host_name: String,
    width: f64,
    height: f64,
    sill: f64,
}

fn rows(model: &Model) -> Result<Vec<Row>> {
    let mut rows = model
        .openings
        .values()
        .map(|opening| {
            let resolved = model.resolve_opening(&opening.parameters)?;
            let host = model
                .walls
                .get(&resolved.host)
                .ok_or_else(|| Error::Invalid("Schedule opening host is missing".into()))?;
            let level = model
                .levels
                .get(&host.parameters.level)
                .ok_or_else(|| Error::Invalid("Schedule host level is missing".into()))?;
            Ok(Row {
                id: opening.id(),
                name: resolved.name,
                kind: resolved.kind,
                type_name: resolved
                    .type_name
                    .unwrap_or_else(|| "Legacy (instance dimensions)".into()),
                level: level.id(),
                level_name: level.parameters.name.clone(),
                host_name: host.parameters.name.clone(),
                width: resolved.width,
                height: resolved.height,
                sill: resolved.sill,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    rows.sort_by(|a, b| {
        (&a.level_name, a.kind == OpeningKind::Window, &a.name, a.id).cmp(&(
            &b.level_name,
            b.kind == OpeningKind::Window,
            &b.name,
            b.id,
        ))
    });
    Ok(rows)
}

fn defined_rows(model: &Model, definition: Option<&ScheduleParams>) -> Result<Vec<Row>> {
    let mut result = rows(model)?;
    if let Some(definition) = definition {
        result.retain(|r| match definition.category {
            ScheduleCategory::All => true,
            ScheduleCategory::Door => r.kind == OpeningKind::Door,
            ScheduleCategory::Window => r.kind == OpeningKind::Window,
        });
        result.sort_by(|a, b| {
            match definition.sort {
                ScheduleSort::LevelKindName => (
                    &a.level_name,
                    a.kind == OpeningKind::Window,
                    &a.name,
                )
                    .cmp(&(&b.level_name, b.kind == OpeningKind::Window, &b.name)),
                ScheduleSort::Name => a.name.cmp(&b.name),
                ScheduleSort::Type => a.type_name.cmp(&b.type_name),
                ScheduleSort::Width => a.width.total_cmp(&b.width),
                ScheduleSort::Height => a.height.total_cmp(&b.height),
                ScheduleSort::Sill => a.sill.total_cmp(&b.sill),
            }
            .then(a.id.cmp(&b.id))
        });
    }
    Ok(result)
}

impl Row {
    fn cell(&self, column: ScheduleColumn) -> String {
        match column {
            ScheduleColumn::Name => self.name.clone(),
            ScheduleColumn::Type => self.type_name.clone(),
            ScheduleColumn::Level => self.level_name.clone(),
            ScheduleColumn::Host => self.host_name.clone(),
            ScheduleColumn::Width => format!("{:.3}", self.width),
            ScheduleColumn::Height => format!("{:.3}", self.height),
            ScheduleColumn::Sill => format!("{:.3}", self.sill),
            ScheduleColumn::Id => self.id.to_string(),
        }
    }
}

pub(super) fn paper_table(model: &Model, id: Id) -> Result<os_render::sheet::PaperTable> {
    let definition = &model
        .schedules
        .get(&id)
        .ok_or_else(|| Error::Invalid("Sheet schedule is missing".into()))?
        .parameters;
    definition.validate()?;
    Ok(os_render::sheet::PaperTable {
        heading: definition.name.clone(),
        columns: definition
            .columns
            .iter()
            .map(|c| c.label().to_owned())
            .collect(),
        rows: defined_rows(model, Some(definition))?
            .iter()
            .map(|row| definition.columns.iter().map(|c| row.cell(*c)).collect())
            .collect(),
    })
}

fn floor_plan(model: &Model, level: Id) -> Option<Id> {
    model
        .views
        .values()
        .filter(|view| {
            view.parameters.kind == ViewKind::Plan
                && view.parameters.level == Some(level)
                && view.parameters.plan.is_some()
        })
        .map(|view| view.id())
        .min()
}

impl DesktopApp {
    fn begin_schedule_name(&mut self, id: Id) {
        self.opening_schedule.draft =
            self.editor
                .document
                .model()
                .openings
                .get(&id)
                .map(|o| NameDraft {
                    id,
                    session: self.editor.document.session_id(),
                    revision: self.editor.document.revision(),
                    name: o.parameters.name.clone(),
                    error: None,
                });
    }

    fn apply_schedule_name(&mut self) {
        let Some(mut draft) = self.opening_schedule.draft.take() else {
            return;
        };
        if !draft.current(self) {
            self.report(
                Err(Error::Invalid(
                    "Schedule edit cancelled: document changed; reopen the name editor.".into(),
                )),
                "",
            );
            return;
        }
        let mut parameters = self.editor.document.model().openings[&draft.id]
            .parameters
            .clone();
        parameters.name = draft.name.clone();
        match self.editor.command(
            "Rename opening",
            os_document::Command::UpdateOpening {
                id: draft.id,
                parameters,
            },
        ) {
            Ok(()) => self.report(Ok(()), "Opening name updated."),
            Err(error) => {
                draft.error = Some(error.to_string());
                self.opening_schedule.draft = Some(draft);
            }
        }
    }

    fn select_schedule_opening(&mut self, id: Id) {
        let level = self
            .editor
            .document
            .model()
            .openings
            .get(&id)
            .and_then(|opening| {
                self.editor
                    .document
                    .model()
                    .walls
                    .get(&opening.parameters.host)
            })
            .map(|wall| wall.parameters.level);
        let Some(level) = level else {
            self.report(
                Err(Error::Invalid("Opening is no longer available".into())),
                "",
            );
            return;
        };
        let view = floor_plan(self.editor.document.model(), level);
        if let Some(view) = view {
            self.plans.active_sheet = None;
            self.focus_plan(Some(view));
        }
        self.select(Some(id));
        self.report(
            Ok(()),
            if view.is_some() {
                "Opening selected in its floor plan."
            } else {
                "Opening selected. Create a floor plan on its level to navigate to it."
            },
        );
    }

    pub(super) fn opening_schedule_window(&mut self, ctx: &egui::Context) {
        self.refresh_schedule_definition(ctx);
        if self
            .opening_schedule
            .draft
            .as_ref()
            .is_some_and(|draft| !draft.current(self))
        {
            self.opening_schedule.draft = None;
            self.report(
                Err(Error::Invalid(
                    "Schedule edit cancelled: document changed; reopen the name editor.".into(),
                )),
                "",
            );
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) || !self.opening_schedule.open {
            self.opening_schedule.draft = None;
        }
        if !self.opening_schedule.open {
            return;
        }
        let mut open = true;
        let mut clicked = None;
        let mut edit = None;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Door/window schedule")
            .open(&mut open)
            .default_size(egui::vec2(820.0, 360.0))
            .show(ctx, |ui| {
                self.schedule_definition_controls(ui);
                let definition = self.opening_schedule.selected.and_then(|id| self.editor.document.model().schedules.get(&id)).map(|s| s.parameters.clone());
                let columns = definition.as_ref().map(|d| d.columns.clone()).unwrap_or_else(|| ScheduleColumn::ALL.to_vec());
                ui.label("Dimensions in metres • Click a row to select • Edit name to rename an instance");
                if let Some(draft) = &mut self.opening_schedule.draft {
                    ui.label(format!("Edit instance name · {}", draft.id));
                    ui.text_edit_singleline(&mut draft.name);
                    if let Some(error) = &draft.error {
                        ui.colored_label(crate::theme::ERROR, error);
                    }
                    ui.horizontal(|ui| {
                        apply = ui.button("Apply").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                    ui.separator();
                }
                match defined_rows(self.editor.document.model(), definition.as_ref()) {
                    Err(error) => { ui.colored_label(crate::theme::ERROR, error.to_string()); }
                    Ok(rows) => {
                        if let Some(row) = rows.iter().find(|row| Some(row.id) == self.selected)
                            && floor_plan(self.editor.document.model(), row.level).is_none()
                        {
                            ui.label("Opening selected. Create a floor plan on its level to navigate to it.");
                        }
                        egui::ScrollArea::both().show(ui, |ui| {
                            let groups = if definition.is_some() { vec![(None, "Openings")] } else { vec![(Some(OpeningKind::Door), "Doors"), (Some(OpeningKind::Window), "Windows")] };
                            for (kind, title) in groups {
                                ui.heading(title);
                                if !rows.iter().any(|row| kind.is_none_or(|kind| row.kind == kind)) {
                                    ui.label("No openings of this kind.");
                                }
                                egui::Grid::new(("opening_schedule", title)).striped(true).show(ui, |ui| {
                                    ui.strong("Action");
                                    for column in &columns { ui.strong(column.label()); }
                                    ui.end_row();
                                    for row in rows.iter().filter(|row| kind.is_none_or(|kind| row.kind == kind)) {
                                        ui.push_id(row.id, |ui| {
                                            if ui.add_enabled(self.opening_schedule.draft.is_none(), egui::Button::new("Edit name")).clicked() {
                                                edit = Some(row.id);
                                            }
                                            for value in columns.iter().map(|column| row.cell(*column)) {
                                                if ui.selectable_label(self.selected == Some(row.id), value).clicked() {
                                                    clicked = Some(row.id);
                                                }
                                            }
                                            ui.end_row();
                                        });
                                    }
                                });
                                ui.separator();
                            }
                        });
                    }
                }
            });
        self.opening_schedule.open = open;
        if cancel || !open {
            self.opening_schedule.draft = None;
            if !open {
                self.opening_schedule.definition_draft = None;
            }
        } else if apply {
            self.apply_schedule_name();
        } else if let Some(id) = edit {
            self.begin_schedule_name(id);
        }
        if let Some(id) = clicked {
            self.select_schedule_opening(id);
        }
    }
}
