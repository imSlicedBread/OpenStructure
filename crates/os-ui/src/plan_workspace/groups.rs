//! Bounded wall-rooted groups in the plan workspace. Selection owns the instance.
use super::*;
use os_core::ensure;

pub(super) fn owner(model: &Model, id: Id) -> Id {
    model
        .group_instance_for_member(id)
        .map_or(id, |group| group.id())
}

pub(super) fn hits(model: &Model, ids: impl IntoIterator<Item = Id>) -> Vec<Id> {
    let mut seen = BTreeSet::new();
    ids.into_iter()
        .map(|id| owner(model, id))
        .filter(|id| seen.insert(*id))
        .collect()
}

pub(super) fn check_context(app: &DesktopApp) -> Result<Id> {
    ensure(app.plans.active_sheet.is_none(), "Open a floor plan")?;
    ensure(
        !app.editor.plugin_work_pending(),
        "Finish the pending plugin command",
    )?;
    ensure(
        !app.editor.host.worker_supported(os_walls::PLUGIN_ID),
        "Groups require the bundled native Wall provider",
    )?;
    ensure(
        app.editor
            .host
            .manifests()
            .any(|m| m.id == os_walls::PLUGIN_ID && m.entrypoint == "builtin:os-walls"),
        "Activate the bundled native Wall provider",
    )?;
    let view = app
        .plans
        .active
        .ok_or_else(|| Error::Invalid("Open a floor plan".into()))?;
    let model = app.editor.document.model();
    ensure(
        model
            .views
            .get(&view)
            .and_then(|v| v.parameters.plan)
            .is_some_and(|p| p.view_type == os_model::PlanViewType::FloorPlan),
        "Groups require a floor plan",
    )?;
    let context = app.editor.native_plan_context(view)?;
    app.plans
        .drawing
        .as_ref()
        .ok_or_else(|| Error::Invalid("Wait for the plan drawing".into()))?
        .items(context)?;
    Ok(view)
}

impl DesktopApp {
    pub(super) fn selected_group(&self) -> Option<Id> {
        let ids = hits(
            self.editor.document.model(),
            self.selected_ids.iter().copied(),
        );
        (ids.len() == 1).then(|| ids[0]).filter(|id| {
            self.editor
                .document
                .model()
                .group_instances
                .contains_key(id)
        })
    }

    fn create_group_command(&self) -> Result<Command> {
        let view = check_context(self)?;
        let model = self.editor.document.model();
        let context = self.editor.native_plan_context(view)?;
        let items = self.plans.drawing.as_ref().unwrap().items(context)?;
        ensure(
            !self.plans.group_name.trim().is_empty(),
            "Enter a group name",
        )?;
        ensure(
            context.show_walls
                && self
                    .selected_ids
                    .iter()
                    .all(|id| items.iter().any(|item| item.entity == *id)),
            "Select visible walls inside the plan crop",
        )?;
        ensure(
            self.selected_ids
                .iter()
                .all(|id| model.group_instance_for_member(*id).is_none()),
            "Selected members already belong to a group",
        )?;
        os_model::GroupDefinitionParams::capture_wall_assembly(
            model,
            &self.selected_ids,
            view,
            self.plans.group_name.trim(),
        )?;
        Ok(Command::CreateWallGroup {
            walls: self.selected_ids.clone(),
            view,
            name: self.plans.group_name.trim().into(),
        })
    }

    pub(super) fn create_plan_group(&mut self) {
        let result = self.create_group_command().and_then(|command| {
            let member = *self.selected_ids.first().unwrap();
            self.cancel_wall_set_interactions();
            self.editor
                .document
                .execute("Create wall group", vec![command])?;
            self.select(Some(member));
            Ok(())
        });
        self.report(
            result,
            "Wall group created, including hosted doors and windows.",
        );
    }

    pub(super) fn begin_group_duplicate(&mut self) {
        let result = self
            .selected_group()
            .ok_or_else(|| Error::Invalid("Select one group".into()))
            .and_then(|id| wall_set_move::Draft::begin_group_duplicate(self, id));
        match result {
            Ok(draft) => {
                self.cancel_wall_set_interactions();
                self.plans.wall_set_move = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn group_action(&mut self, unique: bool) {
        let result = check_context(self).and_then(|_| {
            let id = self
                .selected_group()
                .ok_or_else(|| Error::Invalid("Select one group".into()))?;
            let group = &self.editor.document.model().group_instances[&id];
            let definition =
                &self.editor.document.model().group_definitions[&group.parameters.definition];
            let wall_roots: BTreeSet<_> = definition
                .parameters
                .template
                .walls
                .keys()
                .map(|slot| group.parameters.members[slot])
                .collect();
            let name = format!(
                "{} unique",
                self.editor.document.model().group_definitions[&group.parameters.definition]
                    .parameters
                    .name
            );
            self.cancel_wall_set_interactions();
            self.editor.document.execute(
                if unique {
                    "Make group unique"
                } else {
                    "Ungroup"
                },
                vec![if unique {
                    Command::MakeGroupInstanceUnique { id, name }
                } else {
                    Command::UngroupInstance(id)
                }],
            )?;
            if unique {
                self.select(Some(id));
            } else {
                self.select_area(wall_roots);
            }
            Ok(())
        });
        self.report(
            result,
            if unique {
                "Group has an independent definition. Members remain locked until ungrouped."
            } else {
                "Ungrouped; all walls, doors and windows retained."
            },
        );
    }

    pub(super) fn group_toolbar(&mut self, ui: &mut egui::Ui) {
        if let Some(id) = self.selected_group() {
            let group = &self.editor.document.model().group_instances[&id];
            ui.label(format!(
                "Group: {}",
                self.editor.document.model().group_definitions[&group.parameters.definition]
                    .parameters
                    .name
            ));
            ui.add_enabled_ui(check_context(self).is_ok(), |ui| {
                if ui.button("Duplicate group").clicked() {
                    self.begin_group_duplicate();
                }
                if ui.button("Make unique").clicked() {
                    self.group_action(true);
                }
                if ui.button("Ungroup").clicked() {
                    self.group_action(false);
                }
            });
        } else if !self.selected_ids.is_empty()
            && self
                .selected_ids
                .iter()
                .all(|id| self.editor.document.model().walls.contains_key(id))
        {
            ui.label("Group name");
            ui.add(egui::TextEdit::singleline(&mut self.plans.group_name).desired_width(105.0));
            let valid = self.create_group_command();
            let reason = valid
                .as_ref()
                .err()
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    "Group selected walls with hosted doors, windows, joins and this plan's tags"
                        .into()
                });
            if ui
                .add_enabled(valid.is_ok(), egui::Button::new("Create group"))
                .on_hover_text(reason)
                .clicked()
            {
                self.create_plan_group();
            }
        }
    }
}
