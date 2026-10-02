//! Relative placement intent: applying it never changes the source or type.
use super::*;
use crate::opening_tools::DoorFlip;

const ACTIONS: [(DoorFlip, &str); 4] = [
    (DoorFlip::Hinge, "Flip hinge"),
    (DoorFlip::Swing, "Flip swing"),
    (DoorFlip::Pane, "Flip side"),
    (DoorFlip::Lite, "Flip lite"),
];

#[derive(Clone, Default)]
pub(super) struct Intent([bool; 4]);

impl Intent {
    pub fn apply(&self, model: &Model, mut parameters: OpeningParams) -> Result<OpeningParams> {
        for (active, (action, _)) in self.0.iter().zip(ACTIONS) {
            if *active {
                let resolved = model.resolve_opening(&parameters)?;
                if action.available(&resolved) {
                    parameters = action.parameters(&parameters, &resolved)?;
                }
            }
        }
        Ok(parameters)
    }

    fn retain_available(&mut self, resolved: &ResolvedOpening) {
        for (active, (action, _)) in self.0.iter_mut().zip(ACTIONS) {
            *active &= action.available(resolved);
        }
    }
}

impl OpeningPlacementDraft {
    pub fn base_parameters(&self, model: &Model, host: Id) -> Result<OpeningParams> {
        let mut parameters = if let Some(id) = self.source {
            let p = &model
                .openings
                .get(&id)
                .ok_or_else(|| Error::Invalid("Opening to copy no longer exists".into()))?
                .parameters;
            os_core::ensure(
                model.resolve_opening(p)?.kind == self.kind,
                "Opening copy kind changed",
            )?;
            p.clone()
        } else {
            OpeningParams {
                open_state: Default::default(),
                name: format!("{:?}", self.kind),
                host,
                offset: 0.,
                definition: OpeningDefinition::Legacy {
                    kind: self.kind,
                    width: if self.kind == OpeningKind::Door {
                        0.9
                    } else {
                        1.2
                    },
                    height: if self.kind == OpeningKind::Door {
                        2.1
                    } else {
                        1.2
                    },
                    sill: if self.kind == OpeningKind::Door {
                        0.
                    } else {
                        0.9
                    },
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            }
        };
        if let Some(type_id) = self.type_id {
            let ty = model
                .opening_types
                .get(&type_id)
                .ok_or_else(|| Error::Invalid("Selected opening type no longer exists".into()))?;
            os_core::ensure(
                ty.parameters.kind == self.kind,
                "Selected opening type has the wrong kind",
            )?;
            parameters.definition = OpeningDefinition::Typed { type_id };
        }
        parameters.host = host;
        Ok(parameters)
    }

    pub fn orientation_controls(&mut self, ui: &mut egui::Ui, model: &Model) {
        // Resolving orientation does not require a placed host.
        let Ok(base) = self.base_parameters(model, self.context.view_id) else {
            return;
        };
        let Ok(resolved) = model.resolve_opening(&base) else {
            return;
        };
        self.orientation.retain_available(&resolved);
        for (active, (action, label)) in self.orientation.0.iter_mut().zip(ACTIONS) {
            if action.available(&resolved) && ui.selectable_label(*active, label).clicked() {
                *active = !*active;
            }
        }
        if let Ok(parameters) = self.orientation.apply(model, base)
            && let Ok(effective) = model.resolve_opening(&parameters)
        {
            if effective.kind == OpeningKind::Door {
                ui.small(format!(
                    "Hinge: {:?} · Swing: {:?}",
                    effective.hinge, effective.swing
                ));
            } else if DoorFlip::Pane.available(&effective) {
                ui.small(format!("Side: {:?}", effective.pane_position));
            }
            if let Some(lite) = effective.family.side_lite {
                ui.small(format!("Lite: {:?}", lite.side));
            }
        }
    }
}

impl DesktopApp {
    pub(crate) fn change_placement_type(&mut self, kind: OpeningKind, id: Id) {
        if self.plans.opening_width_claimed {
            return;
        }
        if let Some(draft) = self.plans.opening_placement.as_mut()
            && draft.kind == kind
        {
            draft.type_id = Some(id);
            if let Ok(base) =
                draft.base_parameters(self.editor.document.model(), draft.context.view_id)
                && let Ok(resolved) = self.editor.document.model().resolve_opening(&base)
            {
                draft.orientation.retain_available(&resolved);
            }
        }
    }
}
