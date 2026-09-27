//! View-owned labels resolved from live opening parameters.
use crate::{Entity, Model, ResolvedOpening, ViewKind};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_OPENING_TAGS: usize = 10_000;

/// Closed label choices; the actual text always comes from the live opening.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningTagLabelPreset {
    #[default]
    Full,
    InstanceName,
    TypeAndDimensions,
    DimensionsOnly,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningTagParams {
    pub view: Id,
    pub opening: Id,
    /// World XY in metres.
    pub position: Point2,
    pub label_preset: OpeningTagLabelPreset,
}
pub type OpeningTag = Entity<OpeningTagParams>;

impl OpeningTagParams {
    pub fn validate(&self, model: &Model) -> Result<()> {
        ensure(
            !self.opening.0.is_nil(),
            "opening tag target must not be nil",
        )?;
        ensure(
            self.position.is_finite()
                && self.position.x.abs() <= 1e6
                && self.position.y.abs() <= 1e6,
            "invalid opening tag position",
        )?;
        ensure(
            model.views.get(&self.view).is_some_and(|v| {
                v.parameters.kind == ViewKind::Plan && v.parameters.level.is_some()
            }),
            "opening tag requires a plan view",
        )
    }

    pub fn resolve(&self, model: &Model) -> std::result::Result<ResolvedOpening, &'static str> {
        let level = model
            .views
            .get(&self.view)
            .filter(|v| v.parameters.kind == ViewKind::Plan)
            .and_then(|v| v.parameters.level)
            .ok_or("Missing plan view")?;
        let opening = model.openings.get(&self.opening).ok_or("Missing opening")?;
        let host = model
            .walls
            .get(&opening.parameters.host)
            .ok_or("Missing host wall")?;
        if host.parameters.level != level {
            return Err("Opening is on another level");
        }
        model
            .resolve_opening(&opening.parameters)
            .map_err(|_| "Invalid opening")
    }

    /// Text is derived, never persisted or used as identity.
    pub fn label(&self, model: &Model) -> (String, Option<String>) {
        match self.resolve(model) {
            Ok(o) => {
                let dimensions = format!("{:.3} × {:.3} m", o.width, o.height);
                let type_name = o.type_name.as_deref().unwrap_or("Legacy");
                let label = match self.label_preset {
                    OpeningTagLabelPreset::Full => {
                        format!("{} · {type_name} · {dimensions}", o.name)
                    }
                    OpeningTagLabelPreset::InstanceName => o.name,
                    OpeningTagLabelPreset::TypeAndDimensions => {
                        format!("{type_name} · {dimensions}")
                    }
                    OpeningTagLabelPreset::DimensionsOnly => dimensions,
                };
                (label, None)
            }
            Err(reason) => (format!("Opening tag · {reason}"), Some(reason.into())),
        }
    }

    pub fn validate_creation(&self, model: &Model) -> Result<()> {
        self.validate(model)?;
        self.resolve(model)
            .map_err(|reason| os_core::Error::Invalid(reason.into()))?;
        Ok(())
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    ensure(
        model.opening_tags.len() <= MAX_OPENING_TAGS,
        "opening tags exceed 10000 elements",
    )?;
    let mut pairs = std::collections::BTreeSet::new();
    for tag in model.opening_tags.values() {
        tag.parameters.validate(model)?;
        ensure(
            pairs.insert((tag.parameters.view, tag.parameters.opening)),
            "only one opening tag per opening per view",
        )?;
    }
    Ok(())
}
