//! Independent, view-owned straight drafting geometry in world XY metres.
use crate::{Entity, Model, ViewKind};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_DETAIL_LINES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailLineParams {
    pub view: Id,
    pub start: Point2,
    pub end: Point2,
}
pub type DetailLine = Entity<DetailLineParams>;

impl DetailLineParams {
    pub fn validate(&self, model: &Model) -> Result<()> {
        ensure(
            model.views.get(&self.view).is_some_and(|view| {
                view.parameters.kind == ViewKind::Plan
                    && view.parameters.plan.is_some()
                    && view
                        .parameters
                        .level
                        .is_some_and(|id| model.levels.contains_key(&id))
            }),
            "detail line requires a configured plan view and valid level",
        )?;
        ensure(
            [self.start, self.end]
                .iter()
                .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
            "detail line coordinates must be finite and within +/- 1000000 metres",
        )?;
        ensure(
            self.start.distance(self.end) > 1e-9,
            "detail line endpoints must be distinct",
        )
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    ensure(
        model.detail_lines.len() <= MAX_DETAIL_LINES,
        "detail lines exceed 10000 elements",
    )?;
    for line in model.detail_lines.values() {
        line.parameters.validate(model)?;
    }
    Ok(())
}
