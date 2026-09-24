use super::*;

/// Column footprints use the same checked crop, range and basis as wall prisms.
pub type PlanColumnItem = PlanItem;

impl PlanDrawing {
    pub fn with_columns(
        mut self,
        columns: &[(Id, Solid, os_geometry::SurfaceIdentity)],
    ) -> Result<Self> {
        ensure(self.columns.is_empty(), "columns already attached")?;
        ensure(
            self.source_ids.len().saturating_add(columns.len()) <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 source elements",
        )?;
        ensure(
            self.items
                .len()
                .saturating_add(self.lines.len())
                .saturating_add(self.grids.len())
                .saturating_add(self.floors.len())
                .saturating_add(self.rooms.len())
                .saturating_add(self.room_faces.len())
                .saturating_add(
                    self.dimensions
                        .iter()
                        .map(PlanDimensionItem::graphic_count)
                        .sum::<usize>(),
                )
                .saturating_add(columns.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        for (id, solid, surface) in columns {
            ensure(
                !id.0.is_nil() && self.source_ids.insert(*id),
                "invalid or duplicate column identity",
            )?;
            if let Some(footprint) = rectangular_plan(
                solid,
                self.context.range,
                self.context.basis,
                self.context.crop,
            )? {
                self.columns.push(PlanItem {
                    entity: *id,
                    footprint,
                    surface: *surface,
                    hidden_edges: BTreeSet::new(),
                    split_edges: Vec::new(),
                });
            }
        }
        self.columns.sort_by_key(|item| {
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
    pub fn columns(&self, current: PlanContext) -> Result<&[PlanColumnItem]> {
        self.items(current)?;
        Ok(&self.columns)
    }
    /// Call after model/wall picking and before room/floor interior picking.
    pub fn pick_column(&self, current: PlanContext, point: Point2) -> Result<Option<Id>> {
        ensure(point.is_finite(), "invalid column pick point")?;
        Ok(self
            .columns(current)?
            .iter()
            .rev()
            .find(|item| item.footprint.contains(point))
            .map(|item| item.entity))
    }
}
