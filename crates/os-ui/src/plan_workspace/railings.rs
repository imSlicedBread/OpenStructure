//! Native stair-hosted railing placement and staged shared-type editing.
use super::*;
use os_model::{
    StairRailing, StairRailingParams, StairRailingSide, StairRailingType, StairRailingTypeParams,
};

#[derive(Clone)]
pub(super) struct Edit {
    id: Id,
    type_id: Id,
    session: Id,
    revision: u64,
    view: Option<Id>,
    pub(super) parameters: StairRailingParams,
    pub(super) type_parameters: StairRailingTypeParams,
}

impl DesktopApp {
    pub(super) fn add_stair_railing(&mut self, stair_id: Id, side: StairRailingSide) {
        let result = self.add_stair_railing_inner(stair_id, side);
        self.report(result, "Stair railing added.");
    }

    fn add_stair_railing_inner(&mut self, stair_id: Id, side: StairRailingSide) -> Result<()> {
        let model = self.editor.document.model();
        let stair = model
            .stairs
            .get(&stair_id)
            .ok_or_else(|| os_core::Error::Invalid("Stair missing".into()))?;
        if let Some(existing) = model
            .railings
            .values()
            .find(|r| r.parameters.stair == stair_id && r.parameters.side == side)
        {
            let id = existing.id();
            self.select(Some(id));
            return Ok(());
        }
        let dimensions = stair.parameters.dimensions_in(model)?;
        os_core::ensure(
            stair.parameters.width >= 0.02 && dimensions.going >= 0.01 && dimensions.going <= 10.0,
            "This stair is too narrow or has unsupported tread spacing for the standard railing type",
        )?;

        let reusable_type = model.railing_types.values().find(|ty| {
            ty.parameters.validate_in(model).is_ok()
                && ty.parameters.top_rail_width <= stair.parameters.width
                && ty.parameters.post_width <= stair.parameters.width
                && ty.parameters.post_depth <= stair.parameters.width
                && ty.parameters.post_width < dimensions.going
                && ty.parameters.max_post_spacing >= dimensions.going
        });
        let mut commands = Vec::new();
        let type_id = if let Some(ty) = reusable_type {
            ty.id()
        } else {
            let number = model.railing_types.len() + 1;
            let rail_width = 0.05_f64.min(stair.parameters.width * 0.2);
            let post_depth = 0.05_f64.min(stair.parameters.width * 0.2);
            let post_width = 0.08_f64.min(dimensions.going * 0.5);
            os_core::ensure(
                [rail_width, post_depth, post_width]
                    .iter()
                    .all(|v| *v >= 0.005),
                "Stair is too narrow for a standard railing",
            )?;
            let parameters = StairRailingTypeParams {
                name: format!("Standard Stair Railing {number}"),
                top_rail_height: 0.95,
                top_rail_width: rail_width,
                top_rail_depth: 0.05,
                post_width,
                post_depth,
                max_post_spacing: dimensions.going.max(0.6),
                material: stair.parameters.material,
            };
            parameters.validate_in(model)?;
            let ty = StairRailingType::new("core.railing_type", parameters);
            let id = ty.id();
            commands.push(Command::AddRailingType(ty));
            id
        };
        let side_name = match side {
            StairRailingSide::Left => "Left",
            StairRailingSide::Right => "Right",
        };
        let railing = StairRailing::new(
            "core.railing",
            StairRailingParams {
                name: format!("{} {side_name} railing", stair.parameters.name),
                stair: stair_id,
                railing_type: type_id,
                side,
            },
        );
        let id = railing.id();
        commands.push(Command::AddRailing(railing));
        self.editor.commands("Place stair railing", commands)?;
        self.select(Some(id));
        Ok(())
    }

    pub(crate) fn railing_properties(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(railing) = self
            .selected
            .and_then(|id| self.editor.document.model().railings.get(&id))
            .cloned()
        else {
            self.plans.railing_edit = None;
            return false;
        };
        let Some(ty) = self
            .editor
            .document
            .model()
            .railing_types
            .get(&railing.parameters.railing_type)
            .cloned()
        else {
            self.plans.railing_edit = None;
            ui.label("The railing type is missing.");
            return true;
        };
        let id = railing.id();
        let type_id = ty.id();
        let session = self.editor.document.session_id();
        let revision = self.editor.document.revision();
        if self.plans.railing_edit.as_ref().is_none_or(|edit| {
            edit.id != id
                || edit.type_id != type_id
                || edit.session != session
                || edit.revision != revision
                || edit.view != self.plans.active
        }) {
            self.plans.railing_edit = Some(Edit {
                id,
                type_id,
                session,
                revision,
                view: self.plans.active,
                parameters: railing.parameters.clone(),
                type_parameters: ty.parameters.clone(),
            });
        }
        let model = self.editor.document.model();
        let shared_instances = model
            .railings
            .values()
            .filter(|r| r.parameters.railing_type == type_id)
            .count();
        let draft = self.plans.railing_edit.as_mut().unwrap();
        let (mut apply, mut cancel, mut delete) = (false, false, false);
        egui::ScrollArea::vertical()
            .id_salt("stair_railing_properties")
            .show(ui, |ui| {
                ui.label("Stair-hosted railing · metres");
                ui.text_edit_singleline(&mut draft.parameters.name);
                egui::ComboBox::from_id_salt("stair_railing_side")
                    .selected_text(match draft.parameters.side {
                        StairRailingSide::Left => "Left of ascent",
                        StairRailingSide::Right => "Right of ascent",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut draft.parameters.side,
                            StairRailingSide::Left,
                            "Left of ascent",
                        );
                        ui.selectable_value(
                            &mut draft.parameters.side,
                            StairRailingSide::Right,
                            "Right of ascent",
                        );
                    });
                ui.label(format!("Shared type · {shared_instances} instance(s)"));
                let p = &mut draft.type_parameters;
                ui.add(
                    egui::DragValue::new(&mut p.top_rail_height)
                        .speed(0.01)
                        .prefix("Top height ")
                        .suffix(" m"),
                );
                ui.add(
                    egui::DragValue::new(&mut p.top_rail_width)
                        .speed(0.005)
                        .prefix("Top width ")
                        .suffix(" m"),
                );
                ui.add(
                    egui::DragValue::new(&mut p.top_rail_depth)
                        .speed(0.005)
                        .prefix("Top depth ")
                        .suffix(" m"),
                );
                ui.add(
                    egui::DragValue::new(&mut p.post_width)
                        .speed(0.005)
                        .prefix("Post width ")
                        .suffix(" m"),
                );
                ui.add(
                    egui::DragValue::new(&mut p.post_depth)
                        .speed(0.005)
                        .prefix("Post depth ")
                        .suffix(" m"),
                );
                ui.add(
                    egui::DragValue::new(&mut p.max_post_spacing)
                        .speed(0.01)
                        .prefix("Max post spacing ")
                        .suffix(" m"),
                );
                egui::ComboBox::from_id_salt("stair_railing_material")
                    .selected_text(p.material.map_or("No material", |material| {
                        model
                            .materials
                            .get(&material)
                            .map_or("Missing material", |m| m.parameters.name.as_str())
                    }))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut p.material, None, "No material");
                        for (material_id, material) in &model.materials {
                            ui.selectable_value(
                                &mut p.material,
                                Some(*material_id),
                                &material.parameters.name,
                            );
                        }
                    });
                let valid = draft.type_parameters.validate_in(model).is_ok()
                    && draft
                        .parameters
                        .validate_with_type(&draft.type_parameters, model)
                        .is_ok();
                let changed = draft.parameters != railing.parameters
                    || draft.type_parameters != ty.parameters;
                ui.horizontal(|ui| {
                    apply = ui
                        .add_enabled(changed && valid, egui::Button::new("Apply railing"))
                        .clicked();
                    cancel = ui.button("Cancel edits").clicked();
                    delete = ui.button("Delete railing").clicked();
                });
                ui.label(format!("Host stair: {}", railing.parameters.stair));
                ui.label(format!("Stable ID: {id}"));
            });
        if cancel {
            self.plans.railing_edit = None;
        } else if apply {
            self.apply_railing_properties();
        }
        if delete {
            self.delete_railing();
        }
        true
    }

    pub(super) fn apply_railing_properties(&mut self) {
        let result = (|| {
            let draft = self
                .plans
                .railing_edit
                .as_ref()
                .cloned()
                .ok_or_else(|| os_core::Error::Invalid("No railing edit".into()))?;
            os_core::ensure(
                self.selected == Some(draft.id)
                    && self.plans.active == draft.view
                    && self.editor.document.session_id() == draft.session
                    && self.editor.document.revision() == draft.revision,
                "Railing edit is stale",
            )?;
            draft
                .type_parameters
                .validate_in(self.editor.document.model())?;
            draft
                .parameters
                .validate_with_type(&draft.type_parameters, self.editor.document.model())?;
            let commands = vec![
                Command::UpdateRailingType {
                    id: draft.type_id,
                    parameters: draft.type_parameters,
                },
                Command::UpdateRailing {
                    id: draft.id,
                    parameters: draft.parameters,
                },
            ];
            self.editor.document.preview_commands(commands.clone())?;
            self.editor.commands("Edit stair railing", commands)
        })();
        if result.is_ok() {
            self.plans.railing_edit = None;
        }
        self.report(result, "Stair railing updated.");
    }

    pub(super) fn delete_railing(&mut self) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().railings.contains_key(id))
        else {
            return;
        };
        let result = self
            .editor
            .command("Delete stair railing", Command::RemoveRailing(id));
        if result.is_ok() {
            self.select(None);
            self.plans.railing_edit = None;
        }
        self.report(result, "Stair railing deleted.");
    }
}
