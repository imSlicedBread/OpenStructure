//! Project-owned reusable native door/window types.
use super::*;
use os_core::ensure;
use os_model::{
    OpeningDefinition, OpeningKind, OpeningType, OpeningTypeParams, WindowPanePosition,
};

const TYPE_FIELDS: [&str; 4] = [
    "Width (m)",
    "Height (m)",
    "Sill above floor (m)",
    "Type name",
];

pub(super) struct OpeningTypeDraft {
    session: Id,
    revision: u64,
    id: Id,
    kind: OpeningKind,
    pane_position: WindowPanePosition,
    family: os_model::OpeningFamily,
    selected_vertex: usize,
    editing_cut: bool,
    editing: bool,
    duplicate: bool,
    source_opening: Option<Id>,
    values: [String; 4],
    error: Option<String>,
}

impl OpeningTypeDraft {
    fn new(
        editor: &Editor,
        kind: OpeningKind,
        dimensions: Option<os_model::ResolvedOpening>,
        source_opening: Option<Id>,
    ) -> Self {
        let pane_position = dimensions
            .as_ref()
            .map_or(WindowPanePosition::Center, |p| p.pane_position);
        let family = dimensions
            .as_ref()
            .map(|p| p.family.clone())
            .unwrap_or_default();
        let (width, height, sill, name) = dimensions.map_or_else(
            || {
                (
                    if kind == OpeningKind::Door { 0.9 } else { 1.2 },
                    if kind == OpeningKind::Door { 2.1 } else { 1.2 },
                    if kind == OpeningKind::Door { 0.0 } else { 0.9 },
                    match kind {
                        OpeningKind::Door => "Basic Door 900 × 2100".into(),
                        OpeningKind::Window => "Basic Window 1200 × 1200".into(),
                    },
                )
            },
            |resolved| {
                (
                    resolved.width,
                    resolved.height,
                    resolved.sill,
                    resolved.type_name.unwrap_or_else(|| match kind {
                        OpeningKind::Door => "Basic Door 900 × 2100".into(),
                        OpeningKind::Window => "Basic Window 1200 × 1200".into(),
                    }),
                )
            },
        );
        Self {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            id: Id::new(),
            kind,
            pane_position,
            family,
            selected_vertex: 0,
            editing_cut: false,
            editing: false,
            duplicate: false,
            source_opening,
            values: [
                width.to_string(),
                height.to_string(),
                sill.to_string(),
                name,
            ],
            error: None,
        }
    }

    fn edit(editor: &Editor, id: Id) -> Result<Self> {
        let entity = editor
            .document
            .model()
            .opening_types
            .get(&id)
            .ok_or_else(|| Error::Invalid("opening type missing".into()))?;
        let p = &entity.parameters;
        Ok(Self {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            id,
            kind: p.kind,
            pane_position: p.pane_position,
            family: p.family.clone(),
            selected_vertex: 0,
            editing_cut: false,
            editing: true,
            duplicate: false,
            source_opening: None,
            values: [
                p.width.to_string(),
                p.height.to_string(),
                p.sill.to_string(),
                p.name.clone(),
            ],
            error: None,
        })
    }

    fn current(&self, editor: &Editor) -> bool {
        self.session == editor.document.session_id()
            && self.revision == editor.document.revision()
            && (!self.editing || editor.document.model().opening_types.contains_key(&self.id))
            && self
                .source_opening
                .is_none_or(|id| editor.document.model().openings.contains_key(&id))
    }

    fn parameters(&self) -> Result<OpeningTypeParams> {
        let parse = |index: usize| {
            self.values[index]
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::Invalid(format!("{} needs a number", TYPE_FIELDS[index])))
        };
        Ok(OpeningTypeParams {
            family: self.family.clone(),
            name: self.values[3].trim().to_owned(),
            kind: self.kind,
            pane_position: self.pane_position,
            width: parse(0)?,
            height: parse(1)?,
            sill: if self.kind == OpeningKind::Door {
                0.0
            } else {
                parse(2)?
            },
        })
    }

    #[cfg(test)]
    pub(super) fn profile_for_test(&self) -> &[os_core::Point2] {
        &self.family.profile
    }

    #[cfg(test)]
    pub(super) fn cut_profile_for_test(&self) -> &[os_core::Point2] {
        &self.family.cut_profile
    }

    #[cfg(test)]
    pub(super) fn frame_width_for_test(&self) -> f64 {
        self.family.frame_width
    }

    fn apply(&self, editor: &mut Editor) -> Result<Id> {
        ensure(self.current(editor), "Opening type draft is stale")?;
        self.preview_model(editor)?;
        let parameters = self.parameters()?;
        if self.editing && !self.duplicate {
            editor.command(
                "Edit opening type",
                Command::UpdateOpeningType {
                    id: self.id,
                    parameters,
                },
            )?;
        } else {
            let mut opening_type = OpeningType::new("core.opening_type", parameters);
            opening_type.header.id = self.id;
            let mut commands = vec![Command::AddOpeningType(opening_type)];
            if let Some(opening_id) = self.source_opening {
                let opening = editor
                    .document
                    .model()
                    .openings
                    .get(&opening_id)
                    .ok_or_else(|| Error::Invalid("source opening missing".into()))?;
                let mut instance = opening.parameters.clone();
                instance.definition = OpeningDefinition::Typed { type_id: self.id };
                commands.push(Command::UpdateOpening {
                    id: opening_id,
                    parameters: instance,
                });
            }
            editor.document.execute(
                if self.source_opening.is_some() {
                    "Create type from opening"
                } else if self.duplicate {
                    "Duplicate opening type"
                } else {
                    "Create opening type"
                },
                commands,
            )?;
            editor.regenerate()?;
        }
        Ok(self.source_opening.unwrap_or(self.id))
    }

    fn delete(&self, editor: &mut Editor) -> Result<()> {
        ensure(
            self.editing && !self.duplicate,
            "only a saved type can be deleted",
        )?;
        ensure(self.current(editor), "Opening type draft is stale")?;
        editor.command("Delete opening type", Command::RemoveOpeningType(self.id))
    }

    fn duplicate(&mut self) {
        if !self.editing || self.duplicate {
            return;
        }
        self.id = Id::new();
        self.editing = false;
        self.duplicate = true;
        self.source_opening = None;
        if !self.values[3].ends_with(" Copy") {
            self.values[3].push_str(" Copy");
        }
    }
}

#[path = "opening_family_editor.rs"]
mod family_editor;

#[cfg(test)]
#[path = "opening_profile_tests.rs"]
mod profile_tests;

impl DesktopApp {
    pub(super) fn preferred_type(&self, kind: OpeningKind) -> Option<Id> {
        match kind {
            OpeningKind::Door => self.preferred_door_type,
            OpeningKind::Window => self.preferred_window_type,
        }
    }

    pub(super) fn remember_type(&mut self, kind: OpeningKind, id: Option<Id>) {
        match kind {
            OpeningKind::Door => self.preferred_door_type = id,
            OpeningKind::Window => self.preferred_window_type = id,
        }
    }

    pub(super) fn begin_new_opening_type(&mut self, kind: OpeningKind) {
        self.cancel_plan_wall();
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.room_placement_active = false;
        self.opening_draft = None;
        self.opening_type_draft = Some(OpeningTypeDraft::new(&self.editor, kind, None, None));
    }

    pub(super) fn begin_edit_opening_type(&mut self, id: Id) {
        match OpeningTypeDraft::edit(&self.editor, id) {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_aligned_dimension();
                self.plans.room_placement_active = false;
                self.opening_draft = None;
                self.cancel_opening_placement();
                self.opening_type_draft = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn begin_type_from_opening(&mut self, id: Id) {
        let result = self
            .editor
            .document
            .model()
            .openings
            .get(&id)
            .ok_or_else(|| Error::Invalid("opening missing".into()))
            .and_then(|opening| {
                let resolved = self
                    .editor
                    .document
                    .model()
                    .resolve_opening(&opening.parameters)?;
                let mut draft =
                    OpeningTypeDraft::new(&self.editor, resolved.kind, Some(resolved), Some(id));
                draft.values[3] = format!("{} Type", opening.parameters.name);
                Ok(draft)
            });
        match result {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_aligned_dimension();
                self.plans.room_placement_active = false;
                self.opening_draft = None;
                self.cancel_opening_placement();
                self.opening_type_draft = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn opening_type_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.opening_type_draft.take() else {
            return;
        };
        if !draft.current(&self.editor) {
            self.report(
                Err(Error::Invalid(
                    "Opening type editor canceled because its document changed".into(),
                )),
                "",
            );
            return;
        }
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let instances = self
            .editor
            .document
            .model()
            .openings
            .values()
            .filter(|opening| {
                opening.parameters.definition == (OpeningDefinition::Typed { type_id: draft.id })
            })
            .count();
        egui::Modal::new(egui::Id::new("opening_type_dialog")).show(ctx, |ui| {
            ui.set_width(610.0);
            ui.heading(format!(
                "{} {:?} type",
                if draft.editing && !draft.duplicate {
                    "Edit"
                } else {
                    "New"
                },
                draft.kind
            ));
            if draft.editing && !draft.duplicate {
                ui.label(format!("{instances} placed instance(s) use this type."));
                ui.small(
                    "Type edits update every assigned opening in one undoable transaction.",
                );
            }
            egui::ScrollArea::vertical()
                .scroll_source(egui::scroll_area::ScrollSource {
                    drag: false,
                    ..Default::default()
                })
                .max_height((ctx.content_rect().height()-170.0).max(150.0))
                .show(ui, |ui| {
            egui::Grid::new("opening_type_fields")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.values[3])
                            .desired_width(230.0)
                            .char_limit(256),
                    );
                    ui.end_row();
                    for (index, label) in TYPE_FIELDS.iter().take(3).enumerate() {
                        ui.label(*label);
                        ui.add_enabled(
                            index != 2 || draft.kind == OpeningKind::Window,
                            egui::TextEdit::singleline(&mut draft.values[index])
                                .desired_width(230.0)
                                .char_limit(64),
                        );
                        ui.end_row();
                    }
                });
            if draft.kind == OpeningKind::Window {
                ui.label("Pane position");
                ui.horizontal(|ui| {
                    for (position, label) in [
                        (WindowPanePosition::Center, "Center"),
                        (WindowPanePosition::LeftFace, "Left face"),
                        (WindowPanePosition::RightFace, "Right face"),
                    ] {
                        ui.selectable_value(&mut draft.pane_position, position, label);
                    }
                });
                ui.small("Left/right follow the wall's start → end direction. Face positions align the pane's outside surface with the wall face.");
            }
            draft.profile_editor(ui, &self.editor, self.plans.active);
            });
            if let Some(error) = &draft.error {
                ui.colored_label(theme::ERROR, error);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Cancel type").clicked() {
                    close = true;
                }
                if draft.editing && !draft.duplicate && ui.button("Duplicate type").clicked() {
                    draft.duplicate();
                }
                let apply = ui.button("Apply type").clicked();
                let delete =
                    draft.editing && !draft.duplicate && ui.button("Delete type").clicked();
                if (apply || delete) && !close {
                    let result = if delete {
                        draft.delete(&mut self.editor).map(|()| None)
                    } else {
                        draft.apply(&mut self.editor).map(Some)
                    };
                    match result {
                        Ok(selection) => {
                            if self
                                .editor
                                .document
                                .model()
                                .opening_types
                                .contains_key(&draft.id)
                            {
                                self.remember_type(draft.kind, Some(draft.id));
                            } else if delete && self.preferred_type(draft.kind) == Some(draft.id) {
                                self.remember_type(draft.kind, None);
                            }
                            close = true;
                            self.select(selection);
                            self.report(
                                Ok(()),
                                if delete {
                                    "Opening type deleted."
                                } else {
                                    "Opening type applied."
                                },
                            );
                            ctx.request_repaint();
                        }
                        Err(error) => draft.error = Some(error.to_string()),
                    }
                }
            });
        });
        if !close {
            self.opening_type_draft = Some(draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_core::Point2;
    use os_model::{Opening, OpeningDefinition, Wall, WallParams};

    #[test]
    fn shared_type_edit_updates_instances_once_and_rolls_back_invalid_dimensions() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: "Type host".into(),
                start: Point2::new(0.0, 0.0),
                end: Point2::new(8.0, 0.0),
                thickness: 0.2,
                height: 3.0,
                level,
                material: None,
            },
        );
        let host = wall.id();
        editor.command("Add host", Command::AddWall(wall)).unwrap();

        let mut create = OpeningTypeDraft::new(&editor, OpeningKind::Door, None, None);
        create.values[3] = "D-900".into();
        let type_id = create.id;
        create.apply(&mut editor).unwrap();
        let openings = [1.0, 5.0].map(|offset| {
            Opening::new(
                "core.opening",
                os_model::OpeningParams {
                    hinge: Default::default(),
                    swing: Default::default(),
                    name: "Door instance".into(),
                    host,
                    offset,
                    definition: OpeningDefinition::Typed { type_id },
                },
            )
        });
        let ids = [openings[0].id(), openings[1].id()];
        editor
            .document
            .execute(
                "Place instances",
                openings.into_iter().map(Command::AddOpening).collect(),
            )
            .unwrap();

        let before_edit = editor.document.model().clone();
        let history = editor.document.history_stats().undo_entries;
        let mut edit = OpeningTypeDraft::edit(&editor, type_id).unwrap();
        edit.values[0] = "1.05".into();
        edit.apply(&mut editor).unwrap();
        assert_eq!(editor.document.history_stats().undo_entries, history + 1);
        for id in ids {
            let instance = &editor.document.model().openings[&id];
            assert_eq!(instance.id(), id);
            assert_eq!(
                editor
                    .document
                    .model()
                    .resolve_opening(&instance.parameters)
                    .unwrap()
                    .width,
                1.05,
            );
        }
        let edited = editor.document.model().clone();
        editor.undo().unwrap();
        assert_eq!(editor.document.model(), &before_edit);
        editor.redo().unwrap();
        assert_eq!(editor.document.model(), &edited);

        let mut invalid = OpeningTypeDraft::edit(&editor, type_id).unwrap();
        invalid.values[0] = "20".into();
        let before_invalid = editor.document.model().clone();
        let history = editor.document.history_stats();
        assert!(invalid.apply(&mut editor).is_err());
        assert_eq!(editor.document.model(), &before_invalid);
        assert_eq!(editor.document.history_stats(), history);
    }
}
