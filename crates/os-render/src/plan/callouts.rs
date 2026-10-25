use super::*;

/// Frozen drawing reference: every output adapter consumes this resolved text.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanCalloutGraphic {
    pub entity: Id,
    pub boundary: PlanCrop,
    pub label: String,
}
impl PlanCalloutGraphic {
    pub fn corners(&self) -> [Point2; 4] {
        let c = self.boundary;
        [
            c.min,
            Point2::new(c.max.x, c.min.y),
            c.max,
            Point2::new(c.min.x, c.max.y),
        ]
    }
    pub fn anchor(&self) -> Point2 {
        Point2::new(self.boundary.min.x, self.boundary.max.y)
    }
}
impl PlanDrawing {
    pub fn with_callouts(mut self, callouts: Vec<PlanCalloutGraphic>) -> Result<Self> {
        ensure(
            callouts.len() <= MAX_PLAN_ELEMENTS,
            "callout drawing budget exceeded",
        )?;
        for c in &callouts {
            ensure(
                !c.entity.0.is_nil()
                    && c.boundary.min.is_finite()
                    && c.boundary.max.is_finite()
                    && c.boundary.min.x < c.boundary.max.x
                    && c.boundary.min.y < c.boundary.max.y
                    && !c.label.is_empty()
                    && c.label.len() <= 128
                    && !c.label.chars().any(char::is_control),
                "invalid callout graphic",
            )?;
        }
        self.callouts = callouts;
        Ok(self)
    }
    pub fn callouts(&self, context: PlanContext) -> Result<&[PlanCalloutGraphic]> {
        ensure(self.context == context, "stale callout drawing")?;
        Ok(&self.callouts)
    }
}
