//! Native section detail markers, in station/elevation metres.
use crate::*;
use os_core::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionCalloutBounds {
    #[serde(deserialize_with = "crate::sections::strict_point")]
    pub min: os_core::Point2,
    #[serde(deserialize_with = "crate::sections::strict_point")]
    pub max: os_core::Point2,
}
impl SectionCalloutBounds {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.min.is_finite()
                && self.max.is_finite()
                && self.min.x < self.max.x
                && self.min.y < self.max.y
                && [self.min, self.max]
                    .iter()
                    .all(|p| p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
            "section callout requires bounded positive station/elevation extents",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionCalloutParams {
    pub parent: Id,
    pub target: Id,
    pub owns_target: bool,
    pub boundary: SectionCalloutBounds,
    pub destination: Option<Id>,
}
pub type SectionCallout = Entity<SectionCalloutParams>;
impl Model {
    /// Creation/retarget compatibility only. Later independent view edits remain valid.
    /// Exact authored axes are required: reversed axes have different stations.
    pub fn compatible_callout_sections(&self, parent: Id, target: Id) -> bool {
        if parent == target {
            return false;
        }
        let Some((p, t)) = self.views.get(&parent).zip(self.views.get(&target)) else {
            return false;
        };
        let Some((a, b)) = p.parameters.section.zip(t.parameters.section) else {
            return false;
        };
        p.parameters.kind == ViewKind::Section
            && t.parameters.kind == ViewKind::Section
            && p.parameters.level == t.parameters.level
            && p.parameters.level.is_some()
            && a.start == b.start
            && a.end == b.end
            && a.side == b.side
            && a.mode == b.mode
            && a.far_depth == b.far_depth
    }
    pub fn section_callout_reference(&self, p: &SectionCalloutParams) -> CalloutReference {
        self.drawing_reference(p.target, p.destination)
    }
    /// Every marker in an issued source is a required drawing reference.
    pub fn validate_section_callout_issue_references(&self, sheets: &[Id]) -> Result<()> {
        for sheet in sheets.iter().filter_map(|id| self.sheets.get(id)) {
            for viewport in &sheet.parameters.viewports {
                for marker in self
                    .section_callouts
                    .values()
                    .filter(|c| c.parameters.parent == viewport.view)
                {
                    ensure(
                        matches!(
                            self.section_callout_reference(&marker.parameters),
                            CalloutReference::Placed { .. }
                        ),
                        format!(
                            "Sheet {} has unresolved callout {}: {}",
                            sheet.parameters.number,
                            marker.id(),
                            self.section_callout_reference(&marker.parameters).label()
                        ),
                    )?;
                    if let CalloutReference::Placed {
                        sheet: destination, ..
                    } = self.section_callout_reference(&marker.parameters)
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

    pub(crate) fn validate_section_callouts(&self) -> Result<()> {
        ensure(
            self.section_callouts.len() <= 10000,
            "callout limit exceeded",
        )?;
        let mut owners = BTreeMap::new();
        for c in self.section_callouts.values() {
            let p = &c.parameters;
            ensure(
                p.parent != p.target
                    && self
                        .views
                        .get(&p.parent)
                        .zip(self.views.get(&p.target))
                        .is_some_and(|(a, b)| {
                            a.parameters.kind == ViewKind::Section
                                && b.parameters.kind == ViewKind::Section
                                && a.parameters.section.is_some()
                                && b.parameters.section.is_some()
                                && a.parameters.level.is_some()
                                && a.parameters.level == b.parameters.level
                        }),
                "callout requires distinct native sections on the same marker level; remove or repair references first",
            )?;
            p.boundary.validate()?;
            if p.destination.is_some() {
                ensure(
                    matches!(
                        self.section_callout_reference(p),
                        CalloutReference::Placed { .. }
                    ),
                    "callout destination missing; explicitly clear, repair or remove the reference first",
                )?;
            }
            if p.owns_target {
                ensure(
                    owners.insert(p.target, p.parent).is_none(),
                    "enlarged section has multiple owning markers",
                )?;
            }
        }
        for target in owners.keys() {
            let mut visited = BTreeSet::new();
            let mut next = Some(target);
            while let Some(id) = next {
                ensure(visited.insert(id), "cyclic enlarged section ownership")?;
                next = owners.get(id);
            }
        }
        Ok(())
    }
}
