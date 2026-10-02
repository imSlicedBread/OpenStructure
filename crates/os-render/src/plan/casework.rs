use super::*;

/// Casework envelopes follow the same checked crop/range/basis policy as prisms.
pub type PlanCaseworkItem = PlanItem;

impl PlanDrawing {
    pub fn with_casework(
        mut self,
        casework: &[(Id, Solid, os_geometry::SurfaceIdentity)],
    ) -> Result<Self> {
        ensure(self.casework.is_empty(), "casework already attached")?;
        ensure(
            self.source_ids.len().saturating_add(casework.len()) <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 source elements",
        )?;
        ensure(
            self.items
                .len()
                .saturating_add(self.columns.len())
                .saturating_add(self.casework.len())
                .saturating_add(self.lines.len())
                .saturating_add(self.grids.len())
                .saturating_add(self.floors.len())
                .saturating_add(self.ceilings.len())
                .saturating_add(self.rooms.len())
                .saturating_add(self.room_faces.len())
                .saturating_add(
                    self.dimensions
                        .iter()
                        .map(PlanDimensionItem::graphic_count)
                        .sum::<usize>(),
                )
                .saturating_add(casework.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        for (id, solid, surface) in casework {
            ensure(
                !id.0.is_nil() && self.source_ids.insert(*id),
                "invalid or duplicate casework identity",
            )?;
            if let Some(footprint) = rectangular_plan(
                solid,
                self.context.range,
                self.context.basis,
                self.context.crop,
            )? {
                self.casework.push(PlanItem {
                    entity: *id,
                    footprint,
                    surface: *surface,
                    hidden_edges: BTreeSet::new(),
                    split_edges: Vec::new(),
                });
            }
        }
        self.casework.sort_by_key(|item| {
            (
                match item.footprint.role {
                    PlanRole::Depth => 0,
                    PlanRole::Projected => 1,
                    PlanRole::Cut => 2,
                },
                item.entity,
            )
        });
        Ok(self)
    }

    pub fn casework(&self, current: PlanContext) -> Result<&[PlanCaseworkItem]> {
        self.items(current)?;
        Ok(&self.casework)
    }

    pub fn pick_casework(&self, current: PlanContext, point: Point2) -> Result<Option<Id>> {
        ensure(point.is_finite(), "invalid casework pick point")?;
        Ok(self
            .casework(current)?
            .iter()
            .rev()
            .find(|item| item.footprint.contains(point))
            .map(|item| item.entity))
    }
}
