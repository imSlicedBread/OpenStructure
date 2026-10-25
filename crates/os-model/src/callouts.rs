//! Model-linked, independently configured enlarged floor plans.
use crate::*;
use os_core::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCalloutParams {
    pub parent: Id,
    pub target: Id,
    /// True only for the marker that created the enlarged view.
    pub owns_target: bool,
    /// Rectangle in the parent's persisted plan basis, independent of navigation.
    pub boundary: PlanViewCrop,
    /// Stable placement identity. None auto-resolves only a unique placement.
    pub destination: Option<Id>,
}
pub type PlanCallout = Entity<PlanCalloutParams>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalloutReference {
    Placed {
        sheet: Id,
        viewport: Id,
        label: String,
    },
    Unplaced,
    Ambiguous,
}
impl CalloutReference {
    pub fn label(&self) -> &str {
        match self {
            Self::Placed { label, .. } => label,
            Self::Unplaced => "Unplaced",
            Self::Ambiguous => "Choose destination",
        }
    }
}

impl Model {
    pub fn compatible_callout_plans(&self, parent: Id, target: Id) -> bool {
        let floor = |id| {
            self.views.get(&id).filter(|v| {
                v.parameters.kind == ViewKind::Plan
                    && v.parameters.level.is_some()
                    && v.parameters
                        .plan
                        .is_some_and(|p| p.view_type == PlanViewType::FloorPlan)
            })
        };
        parent != target
            && floor(parent)
                .zip(floor(target))
                .is_some_and(|(p, t)| p.parameters.level == t.parameters.level)
    }

    pub fn callout_reference(&self, p: &PlanCalloutParams) -> CalloutReference {
        self.drawing_reference(p.target, p.destination)
    }

    pub fn drawing_reference(&self, target: Id, destination: Option<Id>) -> CalloutReference {
        let mut placements = self
            .sheets
            .values()
            .flat_map(|s| {
                s.parameters
                    .viewports
                    .iter()
                    .filter(move |v| v.view == target)
                    .map(move |v| (s, v))
            })
            .filter(|(_, v)| destination.is_none_or(|id| v.id == id));
        let Some((sheet, viewport)) = placements.next() else {
            return CalloutReference::Unplaced;
        };
        if placements.next().is_some() {
            return CalloutReference::Ambiguous;
        }
        CalloutReference::Placed {
            sheet: sheet.id(),
            viewport: viewport.id,
            label: format!("{} / {}", viewport.detail_number, sheet.parameters.number),
        }
    }

    /// Every marker in an issued source is a required drawing reference.
    pub fn validate_callout_issue_references(&self, sheets: &[Id]) -> Result<()> {
        self.validate_section_callout_issue_references(sheets)?;
        for sheet in sheets.iter().filter_map(|id| self.sheets.get(id)) {
            for viewport in &sheet.parameters.viewports {
                for marker in self
                    .plan_callouts
                    .values()
                    .filter(|c| c.parameters.parent == viewport.view)
                {
                    ensure(
                        matches!(
                            self.callout_reference(&marker.parameters),
                            CalloutReference::Placed { .. }
                        ),
                        format!(
                            "Sheet {} has unresolved callout {}: {}",
                            sheet.parameters.number,
                            marker.id(),
                            self.callout_reference(&marker.parameters).label()
                        ),
                    )?;
                    if let CalloutReference::Placed {
                        sheet: destination, ..
                    } = self.callout_reference(&marker.parameters)
                    {
                        ensure(
                            sheets.contains(&destination),
                            format!(
                                "Sheet {} references a detail sheet omitted from this issue; include the destination sheet",
                                sheet.parameters.number
                            ),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_callouts(&self) -> Result<()> {
        ensure(self.plan_callouts.len() <= 10000, "callout limit exceeded")?;
        let mut owners = BTreeMap::new();
        for c in self.plan_callouts.values() {
            let p = &c.parameters;
            ensure(
                self.compatible_callout_plans(p.parent, p.target),
                "callout requires different floor plans on the same level; remove or repair references first",
            )?;
            PlanSettings {
                crop: Some(p.boundary),
                ..PlanSettings::default()
            }
            .validate()?;
            ensure(
                [p.boundary.min, p.boundary.max]
                    .iter()
                    .all(|p| p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
                "callout extent exceeds plan bounds",
            )?;
            if p.destination.is_some() {
                ensure(
                    matches!(self.callout_reference(p), CalloutReference::Placed { .. }),
                    "callout destination missing; explicitly clear, repair or remove the reference first",
                )?;
            }
            if p.owns_target {
                ensure(
                    owners.insert(p.target, p.parent).is_none(),
                    "enlarged plan has multiple owning markers",
                )?;
            }
        }
        for target in owners.keys() {
            let mut visited = BTreeSet::new();
            let mut next = Some(target);
            while let Some(id) = next {
                ensure(visited.insert(id), "cyclic enlarged plan ownership")?;
                next = owners.get(id);
            }
        }
        Ok(())
    }
}
