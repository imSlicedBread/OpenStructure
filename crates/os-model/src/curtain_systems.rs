//! Rectangular curtain topology. Coordinates are station and height in metres.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CURTAIN_SYSTEMS: usize = 256;
pub const MAX_CURTAIN_GRIDS: usize = 18;
const MIN_CLEAR: f64 = 0.001;
type PanelBoundary = [Id; 4];
type MemberSpan = (Id, [Id; 2]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurtainPanelKind {
    Opaque,
    Glazing,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurtainPanelTypeParams {
    pub name: String,
    pub kind: CurtainPanelKind,
    pub thickness: f64,
    pub material: Option<Id>,
}
pub type CurtainPanelType = Entity<CurtainPanelTypeParams>;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurtainMullionTypeParams {
    pub name: String,
    pub width: f64,
    pub depth: f64,
    pub material: Option<Id>,
}
pub type CurtainMullionType = Entity<CurtainMullionTypeParams>;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurtainGrid {
    pub id: Id,
    pub position: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurtainPanel {
    pub id: Id,
    /// Left, right, bottom, top boundary IDs.
    pub boundaries: [Id; 4],
    pub panel_type: Id,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurtainMullion {
    pub id: Id,
    pub grid: Id,
    pub span: [Id; 2],
    pub mullion_type: Id,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurtainSystemParams {
    pub name: String,
    pub level: Id,
    pub start: Point2,
    pub end: Point2,
    pub base_offset: f64,
    pub height: f64,
    pub normal_flip: bool,
    pub panel_type: Id,
    pub mullion_type: Id,
    pub vertical: Vec<CurtainGrid>,
    pub horizontal: Vec<CurtainGrid>,
    pub panels: Vec<CurtainPanel>,
    pub mullions: Vec<CurtainMullion>,
}
pub type CurtainSystem = Entity<CurtainSystemParams>;

/// One authoritative local box with explicit ownership and analytic quantities.
#[derive(Clone, Debug, PartialEq)]
pub struct CurtainComponent {
    pub id: Id,
    pub material: Option<Id>,
    /// Station, normal depth, height. Interiors of boxes do not intersect.
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub panel: bool,
    pub area: f64,
    pub length: f64,
    pub volume: f64,
    pub mass_kg: Option<f64>,
}
fn dimension(v: f64) -> bool {
    v.is_finite() && v > MIN_CLEAR && v <= 1000.0
}
fn name(v: &str) -> bool {
    !v.trim().is_empty() && v.len() <= 256 && !v.chars().any(char::is_control)
}
fn material(model: &Model, id: Option<Id>) -> bool {
    id.is_none_or(|id| model.materials.contains_key(&id))
}

impl CurtainSystemParams {
    /// Map station, signed normal depth and height to world metres. Callers
    /// validate parameters through `resolve` before deriving geometry.
    pub fn world_point(&self, local: [f64; 3], level_elevation: f64) -> [f64; 3] {
        let angle = (self.end.y - self.start.y).atan2(self.end.x - self.start.x);
        let (sin, cos) = angle.sin_cos();
        let depth = if self.normal_flip {
            -local[1]
        } else {
            local[1]
        };
        [
            self.start.x + cos * local[0] - sin * depth,
            self.start.y + sin * local[0] + cos * depth,
            level_elevation + self.base_offset + local[2],
        ]
    }
    /// Validate an edit against the committed topology, even when the caller
    /// supplies a complete replacement payload instead of using `reconcile`.
    pub fn validate_transition(&self, previous: &Self) -> Result<()> {
        self.grid_keys()?;
        let old_ids: BTreeSet<_> = previous.child_ids().collect();
        for (old, new) in [
            (&previous.vertical, &self.vertical),
            (&previous.horizontal, &self.horizontal),
        ] {
            ensure(
                old.first().map(|g| g.id) == new.first().map(|g| g.id)
                    && old.last().map(|g| g.id) == new.last().map(|g| g.id),
                "curtain perimeter identities must be preserved",
            )?;
            for grid in new {
                ensure(
                    !old_ids.contains(&grid.id) || old.iter().any(|g| g.id == grid.id),
                    "curtain grid identity cannot change roles",
                )?;
                ensure(
                    !old.iter()
                        .any(|g| g.position == grid.position && g.id != grid.id),
                    "unchanged curtain grid identity must be preserved",
                )?;
            }
        }
        for old in &previous.panels {
            if let Some(new) = self.panels.iter().find(|p| p.boundaries == old.boundaries) {
                ensure(
                    new.id == old.id,
                    "unchanged curtain panel identity must be preserved",
                )?;
            } else {
                ensure(
                    old.panel_type == previous.panel_type,
                    "grid edit would discard customized curtain panel",
                )?;
            }
        }
        for new in &self.panels {
            ensure(
                !old_ids.contains(&new.id)
                    || previous
                        .panels
                        .iter()
                        .any(|p| p.id == new.id && p.boundaries == new.boundaries),
                "curtain panel identity cannot change topology",
            )?;
        }
        for old in &previous.mullions {
            if let Some(new) = self
                .mullions
                .iter()
                .find(|m| (m.grid, m.span) == (old.grid, old.span))
            {
                ensure(
                    new.id == old.id,
                    "unchanged curtain mullion identity must be preserved",
                )?;
            } else {
                ensure(
                    old.mullion_type == previous.mullion_type,
                    "grid edit would discard customized curtain mullion",
                )?;
            }
        }
        for new in &self.mullions {
            ensure(
                !old_ids.contains(&new.id)
                    || previous
                        .mullions
                        .iter()
                        .any(|m| m.id == new.id && (m.grid, m.span) == (new.grid, new.span)),
                "curtain mullion identity cannot change topology",
            )?;
        }
        Ok(())
    }
    pub fn child_ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.vertical
            .iter()
            .chain(&self.horizontal)
            .map(|g| g.id)
            .chain(self.panels.iter().map(|p| p.id))
            .chain(self.mullions.iter().map(|m| m.id))
    }
    fn grid_keys(&self) -> Result<(Vec<PanelBoundary>, Vec<MemberSpan>)> {
        ensure(name(&self.name), "invalid curtain name")?;
        ensure(
            [self.start, self.end]
                .iter()
                .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6)
                && dimension(self.start.distance(self.end))
                && dimension(self.height)
                && self.base_offset.is_finite()
                && self.base_offset.abs() <= 1000.0,
            "invalid curtain dimensions",
        )?;
        for (grids, extent) in [
            (&self.vertical, self.start.distance(self.end)),
            (&self.horizontal, self.height),
        ] {
            ensure(
                (2..=MAX_CURTAIN_GRIDS).contains(&grids.len()),
                "curtain grid count exceeds bounds",
            )?;
            ensure(
                grids.first().unwrap().position == 0.0
                    && (grids.last().unwrap().position - extent).abs() < 1e-8,
                "curtain perimeter must match extent",
            )?;
            ensure(
                grids.iter().all(|g| g.position.is_finite())
                    && grids
                        .windows(2)
                        .all(|g| g[1].position - g[0].position > MIN_CLEAR),
                "curtain grids must be ordered and distinct",
            )?;
        }
        let mut ids = BTreeSet::new();
        ensure(
            self.vertical
                .iter()
                .chain(&self.horizontal)
                .all(|g| !g.id.0.is_nil() && ids.insert(g.id)),
            "duplicate curtain grid identity",
        )?;
        let mut panels = Vec::new();
        let mut members = Vec::new();
        for x in &self.vertical {
            members.push((
                x.id,
                [self.horizontal[0].id, self.horizontal.last().unwrap().id],
            ));
        }
        for x in self.vertical.windows(2) {
            for z in &self.horizontal {
                members.push((z.id, [x[0].id, x[1].id]));
            }
            for z in self.horizontal.windows(2) {
                panels.push([x[0].id, x[1].id, z[0].id, z[1].id]);
            }
        }
        Ok((panels, members))
    }
    /// Reconcile a staged grid edit atomically. Customized disappearing components
    /// must first be reset explicitly; unchanged topology retains IDs and types.
    pub fn reconcile(&mut self) -> Result<()> {
        let (panels, members) = self.grid_keys()?;
        let mut ids = BTreeSet::new();
        ensure(
            self.child_ids().all(|id| !id.0.is_nil() && ids.insert(id)),
            "duplicate curtain component identity",
        )?;
        ensure(
            self.panels
                .iter()
                .map(|p| p.boundaries)
                .collect::<BTreeSet<_>>()
                .len()
                == self.panels.len()
                && self
                    .mullions
                    .iter()
                    .map(|m| (m.grid, m.span))
                    .collect::<BTreeSet<_>>()
                    .len()
                    == self.mullions.len(),
            "duplicate curtain topology",
        )?;
        ensure(
            self.panels
                .iter()
                .all(|p| panels.contains(&p.boundaries) || p.panel_type == self.panel_type)
                && self.mullions.iter().all(|m| {
                    members.contains(&(m.grid, m.span)) || m.mullion_type == self.mullion_type
                }),
            "grid edit would discard customized curtain assignments",
        )?;
        let old_panels: BTreeMap<_, _> = self.panels.iter().map(|p| (p.boundaries, p)).collect();
        let old_members: BTreeMap<_, _> = self
            .mullions
            .iter()
            .map(|m| ((m.grid, m.span), m))
            .collect();
        let new_panels = panels
            .into_iter()
            .map(|boundaries| {
                old_panels
                    .get(&boundaries)
                    .map(|p| (*p).clone())
                    .unwrap_or(CurtainPanel {
                        id: Id::new(),
                        boundaries,
                        panel_type: self.panel_type,
                    })
            })
            .collect();
        let new_members = members
            .into_iter()
            .map(|(grid, span)| {
                old_members
                    .get(&(grid, span))
                    .map(|m| (*m).clone())
                    .unwrap_or(CurtainMullion {
                        id: Id::new(),
                        grid,
                        span,
                        mullion_type: self.mullion_type,
                    })
            })
            .collect();
        self.panels = new_panels;
        self.mullions = new_members;
        Ok(())
    }
    pub fn resolve(&self, model: &Model) -> Result<Vec<CurtainComponent>> {
        let (panels, members) = self.grid_keys()?;
        let level = model
            .levels
            .get(&self.level)
            .ok_or_else(|| os_core::Error::Invalid("curtain level missing".into()))?;
        ensure(
            [
                level.parameters.elevation + self.base_offset,
                level.parameters.elevation + self.base_offset + self.height,
            ]
            .iter()
            .all(|z| z.is_finite() && z.abs() <= 1e6),
            "curtain elevation out of bounds",
        )?;
        ensure(
            model.curtain_panel_types.contains_key(&self.panel_type)
                && model.curtain_mullion_types.contains_key(&self.mullion_type),
            "curtain default type missing",
        )?;
        ensure(
            panels.len() == self.panels.len() && members.len() == self.mullions.len(),
            "incomplete curtain topology",
        )?;
        ensure(
            self.panels
                .iter()
                .map(|p| p.boundaries)
                .collect::<BTreeSet<_>>()
                == panels.into_iter().collect()
                && self
                    .mullions
                    .iter()
                    .map(|m| (m.grid, m.span))
                    .collect::<BTreeSet<_>>()
                    == members.into_iter().collect(),
            "invalid curtain topology",
        )?;
        let mut ids = BTreeSet::new();
        ensure(
            self.child_ids().all(|id| !id.0.is_nil() && ids.insert(id)),
            "duplicate curtain component identity",
        )?;
        let positions: BTreeMap<_, _> = self
            .vertical
            .iter()
            .chain(&self.horizontal)
            .map(|g| (g.id, g.position))
            .collect();
        let member_types: BTreeMap<_, _> = self
            .mullions
            .iter()
            .map(|m| {
                model
                    .curtain_mullion_types
                    .get(&m.mullion_type)
                    .map(|t| ((m.grid, m.span), &t.parameters))
                    .ok_or_else(|| os_core::Error::Invalid("curtain mullion type missing".into()))
            })
            .collect::<Result<_>>()?;
        let full_height = [self.horizontal[0].id, self.horizontal.last().unwrap().id];
        let vertical_width = |id| member_types[&(id, full_height)].width;
        let mut result = Vec::new();
        let mut add =
            |id, mat: Option<Id>, min: [f64; 3], max: [f64; 3], panel, length: f64| -> Result<()> {
                ensure(
                    (0..3).all(|i| dimension(max[i] - min[i])),
                    "curtain clear bay or member collapsed",
                )?;
                for x in [min[0], max[0]] {
                    for y in [min[1], max[1]] {
                        for z in [min[2], max[2]] {
                            ensure(
                                self.world_point([x, y, z], level.parameters.elevation)
                                    .into_iter()
                                    .all(|v| v.is_finite() && v.abs() <= 1e6),
                                "curtain component outside finite coordinate envelope",
                            )?;
                        }
                    }
                }
                let area = (max[0] - min[0]) * (max[2] - min[2]);
                let volume = area * (max[1] - min[1]);
                let mass_kg = mat
                    .and_then(|id| model.materials.get(&id))
                    .map(|m| volume * m.parameters.density_kg_m3);
                ensure(
                    area.is_finite()
                        && volume.is_finite()
                        && length.is_finite()
                        && mass_kg.is_none_or(f64::is_finite),
                    "nonfinite curtain quantity",
                )?;
                result.push(CurtainComponent {
                    id,
                    material: mat,
                    min,
                    max,
                    panel,
                    area,
                    length,
                    volume,
                    mass_kg,
                });
                Ok(())
            };
        for m in &self.mullions {
            let t = member_types[&(m.grid, m.span)];
            let v = self.vertical.iter().any(|g| g.id == m.grid);
            let (x0, x1, z0, z1) = if v {
                let x = positions[&m.grid];
                (
                    (x - t.width / 2.0).max(0.0),
                    (x + t.width / 2.0).min(self.start.distance(self.end)),
                    0.0,
                    self.height,
                )
            } else {
                let z = positions[&m.grid];
                (
                    positions[&m.span[0]] + vertical_width(m.span[0]) / 2.0,
                    positions[&m.span[1]] - vertical_width(m.span[1]) / 2.0,
                    (z - t.width / 2.0).max(0.0),
                    (z + t.width / 2.0).min(self.height),
                )
            };
            add(
                m.id,
                t.material,
                [x0, -t.depth / 2.0, z0],
                [x1, t.depth / 2.0, z1],
                false,
                if v { z1 - z0 } else { x1 - x0 },
            )?;
        }
        for p in &self.panels {
            let t = model
                .curtain_panel_types
                .get(&p.panel_type)
                .ok_or_else(|| os_core::Error::Invalid("curtain panel type missing".into()))?;
            let [l, r, b, u] = p.boundaries;
            let x0 = positions[&l] + vertical_width(l) / 2.0;
            let x1 = positions[&r] - vertical_width(r) / 2.0;
            let z0 = positions[&b] + member_types[&(b, [l, r])].width / 2.0;
            let z1 = positions[&u] - member_types[&(u, [l, r])].width / 2.0;
            add(
                p.id,
                t.parameters.material,
                [x0, -t.parameters.thickness / 2.0, z0],
                [x1, t.parameters.thickness / 2.0, z1],
                true,
                0.0,
            )?;
        }
        Ok(result)
    }
}

pub(crate) fn validate(model: &Model, ids: &mut BTreeSet<Id>) -> Result<()> {
    ensure(
        model.curtain_systems.len() <= MAX_CURTAIN_SYSTEMS
            && model.curtain_panel_types.len() <= 256
            && model.curtain_mullion_types.len() <= 256,
        "curtain collection limit exceeded",
    )?;
    for t in model.curtain_panel_types.values() {
        ensure(
            name(&t.parameters.name)
                && dimension(t.parameters.thickness)
                && material(model, t.parameters.material),
            "invalid curtain panel type",
        )?;
    }
    for t in model.curtain_mullion_types.values() {
        ensure(
            name(&t.parameters.name)
                && dimension(t.parameters.width)
                && dimension(t.parameters.depth)
                && material(model, t.parameters.material),
            "invalid curtain mullion type",
        )?;
    }
    for assembly in model.curtain_systems.values() {
        assembly.parameters.resolve(model)?;
        for id in assembly.parameters.child_ids() {
            ensure(ids.insert(id), "curtain child identity already in use")?;
        }
    }
    Ok(())
}
