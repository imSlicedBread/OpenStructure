//! Bounded office graphics standards. Links control all category strokes; local
//! styles are retained and become effective again when a view is unlinked.
use crate::{Entity, Model, ViewKind};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanLinePattern {
    #[default]
    Solid,
    /// Fixed 3 mm dash / 1.5 mm gap on paper.
    Dashed,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanStrokeStyle {
    pub color: [u8; 3],
    pub weight_mm: f64,
    pub pattern: PlanLinePattern,
}
impl PlanStrokeStyle {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.weight_mm.is_finite() && (0.05..=2.0).contains(&self.weight_mm),
            "plan line weight must be between 0.05 and 2 mm",
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCategoryStyle {
    pub cut: PlanStrokeStyle,
    pub projected: PlanStrokeStyle,
}
impl Default for PlanCategoryStyle {
    fn default() -> Self {
        Self {
            cut: PlanStrokeStyle {
                color: [0, 0, 0],
                weight_mm: 0.35,
                pattern: PlanLinePattern::Solid,
            },
            projected: PlanStrokeStyle {
                color: [0, 0, 0],
                weight_mm: 0.18,
                pattern: PlanLinePattern::Solid,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanGraphicsStyles {
    pub walls: PlanCategoryStyle,
    pub doors: PlanCategoryStyle,
    pub windows: PlanCategoryStyle,
    pub slabs: PlanCategoryStyle,
}
impl PlanGraphicsStyles {
    pub fn validate(self) -> Result<()> {
        for category in [self.walls, self.doors, self.windows, self.slabs] {
            category.cut.validate()?;
            category.projected.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanGraphicsTemplateParams {
    pub name: String,
    pub styles: PlanGraphicsStyles,
}
impl PlanGraphicsTemplateParams {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "graphics template name must be 1-256 bytes without control characters",
        )?;
        self.styles.validate()
    }
}
pub type PlanGraphicsTemplate = Entity<PlanGraphicsTemplateParams>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanGraphicsBinding {
    pub template: Option<Id>,
    pub local: PlanGraphicsStyles,
}

impl Model {
    /// None retains the legacy appearance for views without authored standards.
    pub fn plan_graphics_styles(&self, view: Id) -> Option<PlanGraphicsStyles> {
        self.plan_graphics.get(&view).map(|binding| {
            binding
                .template
                .and_then(|id| self.plan_graphics_templates.get(&id))
                .map_or(binding.local, |template| template.parameters.styles)
        })
    }
    pub(crate) fn validate_plan_graphics(&self) -> Result<()> {
        ensure(
            self.plan_graphics_templates.len() <= 256,
            "graphics template limit exceeded",
        )?;
        for template in self.plan_graphics_templates.values() {
            template.parameters.validate()?;
        }
        for (view, binding) in &self.plan_graphics {
            ensure(
                self.views
                    .get(view)
                    .is_some_and(|v| v.parameters.kind == ViewKind::Plan),
                "graphics settings require a native plan view",
            )?;
            binding.local.validate()?;
            ensure(
                binding
                    .template
                    .is_none_or(|id| self.plan_graphics_templates.contains_key(&id)),
                "graphics template missing or still in use",
            )?;
        }
        Ok(())
    }
}
