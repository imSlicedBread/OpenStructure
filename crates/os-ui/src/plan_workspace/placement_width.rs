//! Disposable width intent; shared opening types and copy sources are never edited.
use super::*;

#[derive(Clone, Default)]
pub(super) enum Intent {
    /// Preserve the base parameters, including a copied instance's nullable pin.
    #[default]
    Preserve,
    Exact(String),
}

impl Intent {
    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    pub fn apply(&self, parameters: &mut OpeningParams) -> Result<()> {
        match self {
            Self::Preserve => {}
            Self::Exact(text) => {
                os_core::ensure(
                    text.chars().count() <= 64,
                    "Width is limited to 64 characters",
                )?;
                let width = text.trim().parse::<f64>().map_err(|_| {
                    Error::Invalid("Width: enter a decimal number in metres".into())
                })?;
                os_core::ensure(
                    width.is_finite() && width >= 0.001,
                    "Width must be finite and at least 0.001 m",
                )?;
                match &mut parameters.definition {
                    OpeningDefinition::Typed { .. } => parameters.width_override = Some(width),
                    OpeningDefinition::Legacy { width: value, .. } => *value = width,
                }
            }
        }
        Ok(())
    }
}

impl OpeningPlacementDraft {
    pub fn width_controls(&mut self, ui: &mut egui::Ui, model: &Model) {
        if ui
            .checkbox(&mut self.draw_width, "Draw opening width")
            .changed()
        {
            self.exact_width = Intent::Preserve;
            self.width_drag = None;
        }
        let base = self
            .base_parameters(model, self.context.view_id)
            .and_then(|parameters| model.resolve_opening(&parameters));
        let mut exact = self.exact_width.is_exact();
        let base_hint = base.as_ref().map_or_else(
            |error| format!("Cannot resolve the current width: {error}"),
            |resolved| format!("Current effective width: {:.3} m", resolved.width),
        );
        if ui
            .checkbox(&mut exact, "Exact width")
            .on_hover_text(base_hint)
            .changed()
        {
            self.draw_width = false;
            self.width_drag = None;
            self.exact_width = if exact {
                Intent::Exact(
                    base.as_ref()
                        .map(|p| p.width.to_string())
                        .unwrap_or_default(),
                )
            } else {
                Intent::Preserve
            };
        }
        if let Intent::Exact(text) = &mut self.exact_width {
            ui.label("Width (m)");
            let response = ui.add(
                egui::TextEdit::singleline(text)
                    .id(egui::Id::new("opening_placement_width"))
                    .char_limit(64)
                    .desired_width(68.0),
            );
            if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                response.surrender_focus();
            }
            if ui.button("Reset width").clicked() {
                self.exact_width = Intent::Preserve;
            }
        } else if self.draw_width {
            ui.small("Width from drag");
        }
        let check = self
            .base_parameters(model, self.context.view_id)
            .and_then(|mut p| {
                self.exact_width.apply(&mut p)?;
                model.resolve_opening(&p)
            });
        if let Err(error) = check {
            ui.colored_label(theme::ERROR, format!("Cannot place: {error}"));
        }
    }
}
