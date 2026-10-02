//! Live native opening schedules. Rows are derived; instance edits use document transactions.
use crate::{DesktopApp, egui};
use os_core::{Error, Id, Result};
use os_model::{Model, OpeningDefinition, OpeningKind, ViewKind};
mod csv_export;
mod definitions;
mod grouping;
pub(crate) mod rooms;
use definitions::DefinitionDraft;
use os_model::{ScheduleCategory, ScheduleColumn, ScheduleParams, SchedulePhase, ScheduleSort};
use os_model::{ScheduleFilter, ScheduleNumericField, ScheduleTextField};

#[derive(Default)]
pub(super) struct OpeningSchedule {
    pub open: bool,
    pub(super) draft: Option<NumericDraft>,
    name_draft: Option<NameDraft>,
    pub(super) selected: Option<Id>,
    session: Option<Id>,
    definition_draft: Option<DefinitionDraft>,
    csv_export: Option<csv_export::ExportDraft>,
}

struct NameDraft {
    id: Id,
    session: Id,
    revision: u64,
    name: String,
    error: Option<String>,
}

pub(super) struct NumericDraft {
    id: Id,
    session: Id,
    revision: u64,
    schedule: Id,
    schedule_params: ScheduleParams,
    kind: OpeningKind,
    field: ScheduleNumericField,
    pub(super) value: String,
    effective: f64,
    inherit: bool,
    typed: bool,
    error: Option<String>,
}

impl NameDraft {
    fn current(&self, app: &DesktopApp) -> bool {
        self.session == app.editor.document.session_id()
            && self.revision == app.editor.document.revision()
            && app.editor.document.model().openings.contains_key(&self.id)
    }
}

impl NumericDraft {
    fn current(&self, app: &DesktopApp) -> bool {
        let model = app.editor.document.model();
        self.session == app.editor.document.session_id()
            && self.revision == app.editor.document.revision()
            && app.opening_schedule.selected == Some(self.schedule)
            && model
                .schedules
                .get(&self.schedule)
                .is_some_and(|s| s.parameters == self.schedule_params)
            && model.openings.get(&self.id).is_some_and(|o| {
                model
                    .resolve_opening(&o.parameters)
                    .is_ok_and(|r| r.kind == self.kind)
            })
    }

    fn editable(&self, model: &Model) -> Result<os_model::OpeningParams> {
        let opening = model
            .openings
            .get(&self.id)
            .ok_or_else(|| Error::Invalid("Schedule opening is missing".into()))?;
        let mut params = opening.parameters.clone();
        let value = if self.inherit {
            None
        } else {
            let value = self.value.trim().parse::<f64>().map_err(|_| {
                Error::Invalid("Schedule value must be a finite number of metres".into())
            })?;
            os_core::ensure(
                value.is_finite(),
                "Schedule value must be a finite number of metres",
            )?;
            Some(value)
        };
        match &mut params.definition {
            OpeningDefinition::Typed { .. } => match self.field {
                ScheduleNumericField::Width => params.width_override = value,
                ScheduleNumericField::Height => params.height_override = value,
                ScheduleNumericField::Sill => params.sill_override = value,
            },
            OpeningDefinition::Legacy {
                kind,
                width,
                height,
                sill,
            } => {
                os_core::ensure(
                    !self.inherit,
                    "Legacy openings do not inherit schedule values",
                )?;
                match self.field {
                    ScheduleNumericField::Width => *width = value.unwrap(),
                    ScheduleNumericField::Height => *height = value.unwrap(),
                    ScheduleNumericField::Sill => {
                        os_core::ensure(*kind == OpeningKind::Window, "Door sill is read-only")?;
                        *sill = value.unwrap();
                    }
                }
            }
        }
        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_document::Command;
    use os_model::{
        Opening, OpeningDefinition, OpeningParams, OpeningType, OpeningTypeParams, Wall,
        WindowPanePosition,
    };

    pub(super) fn fixture() -> (DesktopApp, Id, Id) {
        let mut app = DesktopApp::new().unwrap();
        let mut params = crate::default_wall(app.active_level);
        params.name = "Schedule host".into();
        *params.path.straight_end_mut().unwrap() = os_core::Point2::new(10.0, 0.0);
        let wall = Wall::new(os_walls::WALL_TYPE, params);
        let host = wall.id();
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                window_operation: Default::default(),
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
                open_state: Default::default(),
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
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
                open_state: Default::default(),
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
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

    fn save_schedule(app: &mut DesktopApp, category: ScheduleCategory) -> Id {
        let schedule =
            os_model::Schedule::new("core.schedule", ScheduleParams::new("Editable", category));
        let id = schedule.id();
        app.editor
            .command("Save schedule", Command::AddSchedule(schedule))
            .unwrap();
        app.opening_schedule.selected = Some(id);
        app.opening_schedule.session = Some(app.editor.document.session_id());
        id
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
    fn typed_schedule_dimensions_pin_equal_inherit_and_follow_type_edits() {
        let (mut app, door, ty) = fixture();
        let schedule = save_schedule(&mut app, ScheduleCategory::Door);
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        app.begin_schedule_cell(schedule, door, ScheduleNumericField::Width);
        app.opening_schedule.draft.as_mut().unwrap().value = "0.9".into();
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        app.apply_schedule_cell();
        assert!(app.opening_schedule.draft.is_none());
        assert_eq!(
            app.editor.document.model().openings[&door]
                .parameters
                .width_override,
            Some(0.9)
        );
        assert_eq!(
            app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let mut defaults = app.editor.document.model().opening_types[&ty]
            .parameters
            .clone();
        defaults.width = 1.1;
        app.editor
            .command(
                "Change type width",
                Command::UpdateOpeningType {
                    id: ty,
                    parameters: defaults,
                },
            )
            .unwrap();
        assert_eq!(
            app.editor
                .document
                .model()
                .resolve_opening(&app.editor.document.model().openings[&door].parameters)
                .unwrap()
                .width,
            0.9
        );
        app.begin_schedule_cell(schedule, door, ScheduleNumericField::Width);
        app.opening_schedule.draft.as_mut().unwrap().inherit = true;
        app.apply_schedule_cell();
        assert_eq!(
            app.editor.document.model().openings[&door]
                .parameters
                .width_override,
            None
        );
        let mut defaults = app.editor.document.model().opening_types[&ty]
            .parameters
            .clone();
        defaults.width = 1.2;
        app.editor
            .command(
                "Change inherited type width",
                Command::UpdateOpeningType {
                    id: ty,
                    parameters: defaults,
                },
            )
            .unwrap();
        assert_eq!(
            app.editor
                .document
                .model()
                .resolve_opening(&app.editor.document.model().openings[&door].parameters)
                .unwrap()
                .width,
            1.2
        );
        app.editor.undo().unwrap();
        assert_eq!(
            app.editor
                .document
                .model()
                .resolve_opening(&app.editor.document.model().openings[&door].parameters)
                .unwrap()
                .width,
            1.1
        );
    }

    #[test]
    fn numeric_schedule_cell_pointer_edit_opens_editor_and_enter_commits() {
        let (mut app, door, _) = fixture();
        save_schedule(&mut app, ScheduleCategory::Door);
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);

        click_text(&mut app, &ctx, "0.900");
        assert!(app.opening_schedule.draft.is_some());
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        click_text(&mut app, &ctx, "0.9");
        app.opening_schedule.draft.as_mut().unwrap().value = "1.05".into();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );

        assert!(app.opening_schedule.draft.is_none());
        assert_eq!(
            app.editor.document.model().openings[&door]
                .parameters
                .width_override,
            Some(1.05)
        );
        assert_eq!(
            app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
    }

    #[test]
    fn instance_name_editor_remains_draft_only_undoable_and_stale_safe() {
        let (mut app, door, _) = fixture();
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        click_text(&mut app, &ctx, "Edit name");
        assert_eq!(app.opening_schedule.name_draft.as_ref().unwrap().id, door);
        click_text(&mut app, &ctx, "Schedule door");
        frame(&mut app, &ctx, vec![egui::Event::Text(" renamed".into())]);
        let name = app
            .opening_schedule
            .name_draft
            .as_ref()
            .unwrap()
            .name
            .clone();
        assert_ne!(name, "Schedule door");
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        click_text(&mut app, &ctx, "Apply name");
        assert_eq!(
            app.editor.document.model().openings[&door].parameters.name,
            name
        );
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &before);
        app.editor.redo().unwrap();
        assert_eq!(
            app.editor.document.model().openings[&door].parameters.name,
            name
        );

        app.begin_schedule_name(door);
        app.opening_schedule.name_draft.as_mut().unwrap().name = "Uncommitted".into();
        app.editor
            .command(
                "Concurrent rename",
                Command::RenameProject("Changed".into()),
            )
            .unwrap();
        let after = app.editor.document.model().clone();
        let stats = app.editor.document.history_stats();
        frame(&mut app, &ctx, vec![]);
        assert!(app.opening_schedule.name_draft.is_none());
        assert_eq!(app.editor.document.model(), &after);
        assert_eq!(app.editor.document.history_stats(), stats);
    }

    #[test]
    fn invalid_name_draft_retains_value_and_model_until_correction() {
        let (mut app, door, _) = fixture();
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();

        for name in [" ".to_string(), "bad\nname".into(), "x".repeat(257)] {
            app.begin_schedule_name(door);
            app.opening_schedule.name_draft.as_mut().unwrap().name = name.clone();
            app.apply_schedule_name();
            let draft = app.opening_schedule.name_draft.as_ref().unwrap();
            assert_eq!(draft.name, name);
            assert!(draft.error.as_ref().unwrap().contains("name"));
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.document.history_stats(), history);
        }

        app.opening_schedule.name_draft.as_mut().unwrap().name = "Corrected".into();
        app.apply_schedule_name();
        assert_eq!(
            app.editor.document.model().openings[&door].parameters.name,
            "Corrected"
        );
        assert_eq!(
            app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
    }

    #[test]
    fn name_editor_cancel_and_escape_discard_without_history() {
        let (mut app, door, _) = fixture();
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);

        app.begin_schedule_name(door);
        app.opening_schedule.name_draft.as_mut().unwrap().name = "Uncommitted".into();
        click_text(&mut app, &ctx, "Cancel name");
        assert!(app.opening_schedule.name_draft.is_none());

        app.begin_schedule_name(door);
        app.opening_schedule.name_draft.as_mut().unwrap().name = "Uncommitted".into();
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
        assert!(app.opening_schedule.name_draft.is_none());
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
    }

    #[test]
    fn legacy_window_schedule_edits_fields_and_door_sill_is_suppressed() {
        let (mut app, door, _) = fixture();
        let model = app.editor.document.model().clone();
        let window = model
            .openings
            .values()
            .find(|o| {
                matches!(
                    o.parameters.definition,
                    OpeningDefinition::Legacy {
                        kind: OpeningKind::Window,
                        ..
                    }
                )
            })
            .unwrap()
            .id();
        let schedule = save_schedule(&mut app, ScheduleCategory::Window);
        app.begin_schedule_cell(schedule, window, ScheduleNumericField::Width);
        app.opening_schedule.draft.as_mut().unwrap().value = "1.3".into();
        app.apply_schedule_cell();
        assert_eq!(
            app.editor.document.model().openings[&window]
                .parameters
                .definition,
            OpeningDefinition::Legacy {
                kind: OpeningKind::Window,
                width: 1.3,
                height: 1.1,
                sill: 0.9
            }
        );
        assert!(!app.opening_schedule.draft.as_ref().is_some_and(|d| d.typed));
        let window_type = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                window_operation: Default::default(),
                family: Default::default(),
                name: "W-1200".into(),
                kind: OpeningKind::Window,
                width: 1.2,
                height: 1.1,
                sill: 0.9,
                pane_position: Default::default(),
            },
        );
        let type_id = window_type.id();
        let mut params = model.openings[&window].parameters.clone();
        params.definition = OpeningDefinition::Typed { type_id };
        params.pane_position_override = Some(WindowPanePosition::RightFace);
        app.editor
            .document
            .execute(
                "Type window",
                vec![
                    Command::AddOpeningType(window_type),
                    Command::UpdateOpening {
                        id: window,
                        parameters: params,
                    },
                ],
            )
            .unwrap();
        app.opening_schedule.selected = Some(schedule);
        app.begin_schedule_cell(schedule, window, ScheduleNumericField::Sill);
        let history = app.editor.document.history_stats();
        app.opening_schedule.draft.as_mut().unwrap().value = "0.900".into();
        app.apply_schedule_cell();
        assert_eq!(
            app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1,
            "equal to type default still pins"
        );
        assert_eq!(
            app.editor.document.model().openings[&window]
                .parameters
                .sill_override,
            Some(0.9)
        );
        let history = app.editor.document.history_stats();
        let pinned = app.editor.document.model().clone();
        app.begin_schedule_cell(schedule, window, ScheduleNumericField::Sill);
        app.opening_schedule.draft.as_mut().unwrap().value = "0.9".into();
        app.apply_schedule_cell();
        assert_eq!(
            app.editor.document.history_stats(),
            history,
            "no-op creates no history"
        );
        assert_eq!(app.editor.document.model(), &pinned);
        app.begin_schedule_cell(schedule, window, ScheduleNumericField::Height);
        app.opening_schedule.draft.as_mut().unwrap().value = "1.25".into();
        app.apply_schedule_cell();
        assert_eq!(
            app.editor.document.model().openings[&window]
                .parameters
                .height_override,
            Some(1.25)
        );
        assert_eq!(
            app.editor.document.model().openings[&window]
                .parameters
                .pane_position_override,
            Some(WindowPanePosition::RightFace)
        );
        assert!(editable_cell(
            ScheduleCategory::Door,
            OpeningKind::Door,
            ScheduleColumn::Width
        ));
        assert!(!editable_cell(
            ScheduleCategory::Door,
            OpeningKind::Door,
            ScheduleColumn::Sill
        ));
        assert!(!editable_cell(
            ScheduleCategory::All,
            OpeningKind::Door,
            ScheduleColumn::Width
        ));
        assert!(!editable_cell(
            ScheduleCategory::RoomFinish,
            OpeningKind::Window,
            ScheduleColumn::Height
        ));
        assert!(!editable_cell(
            ScheduleCategory::Window,
            OpeningKind::Door,
            ScheduleColumn::Sill
        ));
        assert_eq!(
            app.editor.document.model().openings[&door]
                .parameters
                .sill_override,
            None
        );
    }

    #[test]
    fn invalid_numeric_edit_rolls_back_retains_draft_and_allows_correction() {
        let (mut app, door, _) = fixture();
        let schedule = save_schedule(&mut app, ScheduleCategory::Door);
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        app.begin_schedule_cell(schedule, door, ScheduleNumericField::Width);
        app.opening_schedule.draft.as_mut().unwrap().value = "6.0".into();
        app.apply_schedule_cell();
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        assert!(app.opening_schedule.draft.as_ref().unwrap().error.is_some());
        app.opening_schedule.draft.as_mut().unwrap().value = "1.0".into();
        app.apply_schedule_cell();
        assert!(app.opening_schedule.draft.is_none());
        assert_eq!(
            app.editor
                .document
                .model()
                .resolve_opening(&app.editor.document.model().openings[&door].parameters)
                .unwrap()
                .width,
            1.0
        );
    }

    #[test]
    fn numeric_edit_cancel_escape_and_stale_schedule_or_session_never_commit() {
        let (mut app, door, _) = fixture();
        let ctx = egui::Context::default();
        let schedule = save_schedule(&mut app, ScheduleCategory::Door);
        let before = app.editor.document.model().clone();
        let history = app.editor.document.history_stats();
        app.opening_schedule.open = true;
        app.begin_schedule_cell(schedule, door, ScheduleNumericField::Width);
        app.opening_schedule.draft.as_mut().unwrap().value = "1.4".into();
        click_text(&mut app, &ctx, "Cancel");
        assert!(app.opening_schedule.draft.is_none());
        assert_eq!(app.editor.document.model(), &before);
        assert_eq!(app.editor.document.history_stats(), history);
        for stale in [false, true] {
            app.begin_schedule_cell(schedule, door, ScheduleNumericField::Width);
            if stale {
                app.opening_schedule.draft.as_mut().unwrap().session = Id::new();
            } else {
                app.opening_schedule.selected = None;
            }
            app.apply_schedule_cell();
            assert!(app.opening_schedule.draft.is_none());
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.document.history_stats(), history);
            app.opening_schedule.selected = Some(schedule);
            app.opening_schedule.session = Some(app.editor.document.session_id());
        }

        app.begin_schedule_cell(schedule, door, ScheduleNumericField::Width);
        let mut parameters = app.editor.document.model().openings[&door]
            .parameters
            .clone();
        parameters.name = "Concurrent edit".into();
        app.editor
            .command(
                "Concurrent opening edit",
                Command::UpdateOpening {
                    id: door,
                    parameters,
                },
            )
            .unwrap();
        let after = app.editor.document.model().clone();
        let history_after_edit = app.editor.document.history_stats();
        app.apply_schedule_cell();
        assert!(app.opening_schedule.draft.is_none());
        assert_eq!(app.editor.document.model(), &after);
        assert_eq!(app.editor.document.history_stats(), history_after_edit);

        app.begin_schedule_cell(schedule, door, ScheduleNumericField::Width);
        app.opening_schedule.open = true;
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
        assert!(app.opening_schedule.draft.is_none());
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: Id,
    name: String,
    kind: OpeningKind,
    type_name: String,
    type_id: Option<Id>,
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
                type_id: match opening.parameters.definition {
                    OpeningDefinition::Typed { type_id } => Some(type_id),
                    OpeningDefinition::Legacy { .. } => None,
                },
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

const QUANTITY_COLUMNS: [&str; 7] = [
    "Kind",
    "Type",
    "Type ID",
    "Width (m)",
    "Height (m)",
    "Sill (m)",
    "Count",
];

#[derive(Debug, PartialEq)]
struct QuantityRow<'a> {
    opening: &'a Row,
    count: usize,
}

impl QuantityRow<'_> {
    fn cells(&self) -> Vec<String> {
        let row = self.opening;
        vec![
            format!("{:?}", row.kind),
            row.type_name.clone(),
            row.type_id
                .map_or_else(|| "Legacy".into(), |id| id.to_string()),
            quantity_dimension(row.width),
            quantity_dimension(row.height),
            quantity_dimension(row.sill),
            self.count.to_string(),
        ]
    }
}

fn quantity_dimension(value: f64) -> String {
    let rounded = format!("{value:.3}");
    if rounded
        .parse::<f64>()
        .is_ok_and(|v| v.to_bits() == value.to_bits())
    {
        rounded
    } else {
        value.to_string()
    }
}

// Group exact effective dimensions, never rounded display strings or type names.
// Sorting references keeps the instance report's configured order untouched.
fn quantities(rows: &[Row]) -> Vec<QuantityRow<'_>> {
    let mut sorted: Vec<_> = rows.iter().collect();
    sorted.sort_by(|a, b| {
        (a.kind == OpeningKind::Window, a.type_id)
            .cmp(&(b.kind == OpeningKind::Window, b.type_id))
            .then(a.width.total_cmp(&b.width))
            .then(a.height.total_cmp(&b.height))
            .then(a.sill.total_cmp(&b.sill))
    });
    let mut groups: Vec<QuantityRow<'_>> = Vec::new();
    for row in sorted {
        if let Some(group) = groups.last_mut()
            && group.opening.kind == row.kind
            && group.opening.type_id == row.type_id
            && group.opening.width.total_cmp(&row.width).is_eq()
            && group.opening.height.total_cmp(&row.height).is_eq()
            && group.opening.sill.total_cmp(&row.sill).is_eq()
        {
            group.count += 1;
        } else {
            groups.push(QuantityRow {
                opening: row,
                count: 1,
            });
        }
    }
    groups
}

fn defined_rows(model: &Model, definition: Option<&ScheduleParams>) -> Result<Vec<Row>> {
    if let Some(definition) = definition {
        definition.validate()?;
        os_core::ensure(
            definition.category != ScheduleCategory::RoomFinish,
            "room schedule requires room rows",
        )?;
    }
    let mut result = rows(model)?;
    if let Some(definition) = definition {
        if let Some((target, filter)) = definition.phase.resolve(model)? {
            let mut eligible = Vec::with_capacity(result.len());
            for row in result {
                let opening = &model.openings[&row.id];
                if filter.includes(model.phase_status(row.id, target)?)
                    && filter.includes(model.phase_status(opening.parameters.host, target)?)
                {
                    eligible.push(row);
                }
            }
            result = eligible;
        }
        result.retain(|r| match definition.category {
            ScheduleCategory::All => true,
            ScheduleCategory::Door => r.kind == OpeningKind::Door,
            ScheduleCategory::Window => r.kind == OpeningKind::Window,
            ScheduleCategory::RoomFinish => false,
        });
        result.retain(|row| definition.filters.iter().all(|filter| row.matches(filter)));
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
                _ => unreachable!("validated opening sort"),
            }
            .then(a.id.cmp(&b.id))
        });
    }
    Ok(result)
}

impl Row {
    fn matches(&self, filter: &ScheduleFilter) -> bool {
        match filter {
            ScheduleFilter::Text {
                field,
                operator,
                value,
            } => operator.matches(
                match field {
                    ScheduleTextField::Name => &self.name,
                    ScheduleTextField::Type => &self.type_name,
                    ScheduleTextField::Level => &self.level_name,
                    ScheduleTextField::Host => &self.host_name,
                },
                value,
            ),
            ScheduleFilter::Numeric {
                field,
                operator,
                value,
            } => operator.matches(
                match field {
                    ScheduleNumericField::Width => self.width,
                    ScheduleNumericField::Height => self.height,
                    ScheduleNumericField::Sill => self.sill,
                },
                *value,
            ),
        }
    }

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
            _ => unreachable!("validated opening column"),
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
    if !definition.group_by.is_empty() {
        let mut table = grouping::table(&defined_rows(model, Some(definition))?, definition);
        table.heading = schedule_heading(model, definition)?;
        return Ok(table);
    }
    instance_table(model, id)
}

fn phase_context(model: &Model, definition: &ScheduleParams) -> Result<String> {
    let Some((target, filter)) = definition.phase.resolve(model)? else {
        return Ok(if definition.category == ScheduleCategory::RoomFinish {
            "Unphased".into()
        } else {
            "Legacy unphased".into()
        });
    };
    let mode = if matches!(
        definition.phase,
        SchedulePhase::PhaseAware { target: None, .. }
    ) {
        "Latest phase"
    } else {
        "Pinned phase"
    };
    Ok(format!(
        "{mode}: {} · {filter:?}",
        model.phases[&target].parameters.name
    ))
}

fn schedule_heading(model: &Model, definition: &ScheduleParams) -> Result<String> {
    if matches!(definition.phase, SchedulePhase::LegacyUnphased) {
        return Ok(definition.name.clone());
    }
    Ok(format!(
        "{} · {}",
        definition.name,
        phase_context(model, definition)?
    ))
}

fn instance_table(model: &Model, id: Id) -> Result<os_render::sheet::PaperTable> {
    let definition = &model
        .schedules
        .get(&id)
        .ok_or_else(|| Error::Invalid("Sheet schedule is missing".into()))?
        .parameters;
    definition.validate()?;
    Ok(os_render::sheet::PaperTable {
        heading: schedule_heading(model, definition)?,
        columns: definition
            .columns
            .iter()
            .map(|c| {
                if definition.category == ScheduleCategory::RoomFinish && *c == ScheduleColumn::Name
                {
                    "Name".into()
                } else {
                    c.label().to_owned()
                }
            })
            .collect(),
        rows: if definition.category == ScheduleCategory::RoomFinish {
            rooms::rows(model, definition)?
                .iter()
                .map(|row| definition.columns.iter().map(|c| row.cell(*c)).collect())
                .collect()
        } else {
            defined_rows(model, Some(definition))?
                .iter()
                .map(|row| definition.columns.iter().map(|c| row.cell(*c)).collect())
                .collect()
        },
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

fn numeric_field(column: ScheduleColumn) -> Option<ScheduleNumericField> {
    match column {
        ScheduleColumn::Width => Some(ScheduleNumericField::Width),
        ScheduleColumn::Height => Some(ScheduleNumericField::Height),
        ScheduleColumn::Sill => Some(ScheduleNumericField::Sill),
        _ => None,
    }
}

fn editable_cell(category: ScheduleCategory, kind: OpeningKind, column: ScheduleColumn) -> bool {
    match category {
        ScheduleCategory::Door => {
            kind == OpeningKind::Door
                && matches!(column, ScheduleColumn::Width | ScheduleColumn::Height)
        }
        ScheduleCategory::Window => {
            kind == OpeningKind::Window
                && matches!(
                    column,
                    ScheduleColumn::Width | ScheduleColumn::Height | ScheduleColumn::Sill
                )
        }
        ScheduleCategory::All | ScheduleCategory::RoomFinish => false,
    }
}

impl DesktopApp {
    fn begin_schedule_name(&mut self, id: Id) {
        self.opening_schedule.name_draft =
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
        let Some(mut draft) = self.opening_schedule.name_draft.take() else {
            return;
        };
        if !draft.current(self) {
            self.report(
                Err(Error::Invalid(
                    "Schedule name edit cancelled: document changed.".into(),
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
                self.opening_schedule.name_draft = Some(draft);
            }
        }
    }

    pub(super) fn begin_schedule_cell(
        &mut self,
        schedule: Id,
        id: Id,
        field: ScheduleNumericField,
    ) {
        let Some(definition) = self
            .editor
            .document
            .model()
            .schedules
            .get(&schedule)
            .map(|s| s.parameters.clone())
        else {
            return;
        };
        let Some(opening) = self.editor.document.model().openings.get(&id) else {
            return;
        };
        let Ok(resolved) = self
            .editor
            .document
            .model()
            .resolve_opening(&opening.parameters)
        else {
            return;
        };
        let column = match field {
            ScheduleNumericField::Width => ScheduleColumn::Width,
            ScheduleNumericField::Height => ScheduleColumn::Height,
            ScheduleNumericField::Sill => ScheduleColumn::Sill,
        };
        if !editable_cell(definition.category, resolved.kind, column) {
            return;
        }
        let value = match field {
            ScheduleNumericField::Width => resolved.width,
            ScheduleNumericField::Height => resolved.height,
            ScheduleNumericField::Sill => resolved.sill,
        };
        self.opening_schedule.draft = Some(NumericDraft {
            id,
            session: self.editor.document.session_id(),
            revision: self.editor.document.revision(),
            schedule,
            schedule_params: definition,
            kind: resolved.kind,
            field,
            value: value.to_string(),
            effective: value,
            inherit: false,
            typed: matches!(
                opening.parameters.definition,
                OpeningDefinition::Typed { .. }
            ),
            error: None,
        });
    }

    pub(super) fn apply_schedule_cell(&mut self) {
        let Some(mut draft) = self.opening_schedule.draft.take() else {
            return;
        };
        if !draft.current(self) {
            self.report(
                Err(Error::Invalid(
                    "Schedule edit cancelled: document, opening or schedule changed.".into(),
                )),
                "",
            );
            return;
        }
        let result = draft
            .editable(self.editor.document.model())
            .and_then(|parameters| {
                self.editor.command(
                    "Edit scheduled opening dimension",
                    os_document::Command::UpdateOpening {
                        id: draft.id,
                        parameters,
                    },
                )
            });
        match result {
            Ok(()) => self.report(Ok(()), "Opening dimension updated."),
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
            .name_draft
            .as_ref()
            .is_some_and(|draft| !draft.current(self))
        {
            self.opening_schedule.name_draft = None;
            self.report(
                Err(Error::Invalid(
                    "Schedule name edit cancelled: document changed.".into(),
                )),
                "",
            );
        }
        if self
            .opening_schedule
            .draft
            .as_ref()
            .is_some_and(|draft| !draft.current(self))
        {
            self.opening_schedule.draft = None;
            self.opening_schedule.name_draft = None;
            self.report(
                Err(Error::Invalid(
                    "Schedule edit cancelled: document, opening or schedule changed.".into(),
                )),
                "",
            );
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) || !self.opening_schedule.open {
            self.opening_schedule.draft = None;
            self.opening_schedule.name_draft = None;
        }
        if !self.opening_schedule.open {
            self.opening_schedule.csv_export = None;
            return;
        }
        let mut open = true;
        let mut clicked = None;
        let mut edit_window = None;
        let mut edit_name = None;
        let mut edit_cell = None;
        let mut apply_name = false;
        let mut apply_cell = false;
        let mut cancel = false;
        egui::Window::new("Schedules")
            .open(&mut open)
            .default_size(egui::vec2(820.0, 360.0))
            .show(ctx, |ui| {
                self.schedule_definition_controls(ui);
                self.schedule_csv_action(ui);
                let definition = self.opening_schedule.selected.and_then(|id| self.editor.document.model().schedules.get(&id)).map(|s| s.parameters.clone());
                if let Some(definition) = definition.as_ref().filter(|definition| {
                    matches!(definition.phase, SchedulePhase::PhaseAware { .. })
                }) {
                    match phase_context(self.editor.document.model(), definition) {
                        Ok(context) => { ui.label(context); }
                        Err(error) => { ui.colored_label(crate::theme::ERROR, error.to_string()); }
                    }
                }
                let columns = definition.as_ref().map(|d| d.columns.clone()).unwrap_or_else(|| ScheduleColumn::ALL.to_vec());
                if let Some(definition) = definition.as_ref().filter(|d| d.category == ScheduleCategory::RoomFinish) {
                    self.room_schedule_rows(ui, definition);
                    return;
                }
                ui.label("Dimensions in metres • Click a row to select • Edit name or click an eligible dimension");
                if let Some(id) = self.selected.filter(|id| self.editor.document.model().openings.get(id)
                    .and_then(|o| self.editor.document.model().resolve_opening(&o.parameters).ok())
                    .is_some_and(|o| o.kind == OpeningKind::Window))
                    && ui.add_enabled(self.opening_schedule.draft.is_none(), egui::Button::new("Edit window properties")).clicked() {
                    edit_window = Some(id);
                }
                if let Some(draft) = &mut self.opening_schedule.draft {
                    ui.label(format!("Edit {:?} (m) · effective {:.3}", draft.field, draft.effective));
                    if let Some(error) = &draft.error { ui.colored_label(crate::theme::ERROR, error); }
                    if draft.typed {
                        ui.checkbox(&mut draft.inherit, "Inherit type value / reset override");
                    }
                    ui.horizontal(|ui| {
                        apply_cell = ui.button("Apply").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                    ui.separator();
                }
                if let Some(draft) = &mut self.opening_schedule.name_draft {
                    ui.label(format!("Edit instance name · {}", draft.id));
                    ui.text_edit_singleline(&mut draft.name);
                    if let Some(error) = &draft.error { ui.colored_label(crate::theme::ERROR, error); }
                    ui.horizontal(|ui| {
                        apply_name = ui.button("Apply name").clicked();
                        cancel |= ui.button("Cancel name").clicked();
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
                                    for row in grouping::ordered(&rows, definition.as_ref().map_or(&[], |d| d.group_by.as_slice())).into_iter().filter(|row| kind.is_none_or(|kind| row.kind == kind)) {
                                        ui.push_id(row.id, |ui| {
                                            if ui.add_enabled(self.opening_schedule.name_draft.is_none() && self.opening_schedule.draft.is_none(),
                                                egui::Button::new("Edit name")).clicked() {
                                                edit_name = Some(row.id);
                                            }
                                            for column in &columns {
                                                let field = numeric_field(*column);
                                                let editable = definition.as_ref().is_some_and(|d| field.is_some_and(|_| editable_cell(d.category, row.kind, *column)));
                                                if editable && self.opening_schedule.draft.is_none() {
                                                    if ui.button(row.cell(*column)).clicked() {
                                                        edit_cell = Some((self.opening_schedule.selected.unwrap(), row.id, field.unwrap()));
                                                    }
                                                } else if self.opening_schedule.draft.as_ref().is_some_and(|d| d.id == row.id && Some(d.field) == field) {
                                                    let draft = self.opening_schedule.draft.as_mut().unwrap();
                                                    let response = ui.add(egui::TextEdit::singleline(&mut draft.value).desired_width(78.0));
                                                    apply_cell |= response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                                } else if ui.selectable_label(self.selected == Some(row.id), row.cell(*column)).clicked() {
                                                    clicked = Some(row.id);
                                                }
                                            }
                                            ui.end_row();
                                        });
                                    }
                                });
                                ui.separator();
                            }
                            if let Some(definition) = definition.as_ref().filter(|d| !d.group_by.is_empty()) {
                                ui.heading("Grouped quantity summary");
                                let table = grouping::table(&rows, definition);
                                egui::Grid::new("opening_group_summary").striped(true).show(ui, |ui| {
                                    for column in table.columns { ui.strong(column); }
                                    ui.end_row();
                                    for row in table.rows {
                                        for cell in row { ui.label(cell); }
                                        ui.end_row();
                                    }
                                });
                            } else if definition.is_some() {
                                ui.heading("Quantity summary");
                                ui.label("Filtered counts by kind, type ID and exact effective dimensions (m)");
                                egui::Grid::new("opening_quantity_summary").striped(true).show(ui, |ui| {
                                    for column in QUANTITY_COLUMNS { ui.strong(column); }
                                    ui.end_row();
                                    for group in quantities(&rows) {
                                        for cell in group.cells() { ui.label(cell); }
                                        ui.end_row();
                                    }
                                });
                            }
                        });
                    }
                }
            });
        self.opening_schedule.open = open;
        if cancel || !open {
            self.opening_schedule.draft = None;
            self.opening_schedule.name_draft = None;
            if !open {
                self.opening_schedule.definition_draft = None;
            }
        } else if apply_cell {
            self.apply_schedule_cell();
        }
        if apply_name {
            self.apply_schedule_name();
        }
        if let Some(id) = edit_name {
            self.begin_schedule_name(id);
        }
        if let Some((schedule, id, field)) = edit_cell {
            self.begin_schedule_cell(schedule, id, field);
        }
        if let Some(id) = clicked {
            self.select_schedule_opening(id);
        }
        if let Some(id) = edit_window {
            self.select_schedule_opening(id);
            self.begin_opening(None);
        }
        self.schedule_csv_window(ctx);
    }
}
